//! Single-writer partition of the matching engine.
//!
//! A [`Shard`] owns a disjoint subset of instruments (their [`Orderbook`]s)
//! and is driven by exactly one writer. This module introduces only the
//! vocabulary and the synchronous processing loop; the thread/channel plumbing
//! that realises the "one-writer-per-shard" model is layered on top in a
//! later phase.
//!
//! ### Design
//!
//! [`Shard`] wraps an [`Engine`] internally rather than re-implementing the
//! instrument registry. This keeps the matching/registration logic
//! single-sourced in [`Engine`] while letting the shard focus on its
//! message-driven interface: consume a [`ShardRequest`], produce a
//! [`ShardReply`].
//!
//! ### Atomicity
//!
//! [`ShardRequest::Execute`] returns the command's output _together with_
//! the post-command top-of-book for the affected instrument. This matches
//! the [`crate::engine::Engine::execute`] + `best_*` pattern used by the
//! gateway today, and keeps the contract the shard thread-loop will
//! eventually uphold (no interleaving between execute and snapshot).
//!
//! [`Orderbook`]: crate::orderbook::orderbook::Orderbook
//! [`Engine`]: crate::engine::Engine

use crate::commands::{Command, CommandOutput, EngineError, InstrumentId};
use crate::engine::Engine;
use crate::events::Events;
use crate::orderbook::level_info::OrderbookLevelInfo;
use crate::orderbook::types::Price;

/// Identifier for a shard. Flat namespace; the router is responsible for
/// mapping instruments to shard ids.
pub type ShardId = u16;

/// Messages accepted by [`Shard::process`].
#[derive(Debug, Clone)]
pub enum ShardRequest {
    /// Run a matching-engine command on an instrument owned by this shard.
    Execute(Command),
    /// Create a book for a new instrument on this shard. Idempotent.
    RegisterInstrument(InstrumentId),
    /// Read-only orderbook snapshot for an instrument owned by this shard.
    OrderbookSnapshot(InstrumentId),
    /// Best bid and best ask for an instrument owned by this shard.
    TopOfBook(InstrumentId),
}

/// Reply bundle for a successful [`ShardRequest::Execute`].
#[derive(Debug)]
pub struct ExecuteReply {
    pub output: CommandOutput,
    pub events: Events,
    pub best_bid: Option<Price>,
    pub best_ask: Option<Price>,
}

/// Reply for any [`ShardRequest`]. Variants line up 1:1 with requests.
#[derive(Debug)]
pub enum ShardReply {
    Execute(Result<ExecuteReply, EngineError>),
    /// `created` is `true` when the call created a new book, `false` on
    /// idempotent re-registration.
    RegisterInstrument {
        created: bool,
    },
    /// `None` when the instrument is not registered on this shard.
    OrderbookSnapshot(Option<OrderbookLevelInfo>),
    /// `None` outer = instrument not registered. Inner `Option`s are `None`
    /// when the corresponding side is empty.
    TopOfBook(Option<(Option<Price>, Option<Price>)>),
}

/// A matching-engine shard.
///
/// This commit only exposes the synchronous [`Shard::process`] loop. The
/// next commit will drive it from a dedicated OS thread over a channel.
pub struct Shard {
    shard_id: ShardId,
    engine: Engine,
}

impl Shard {
    /// Create an empty shard with no books.
    pub fn new(shard_id: ShardId) -> Self {
        Self {
            shard_id,
            engine: Engine::new(),
        }
    }

    /// Shard identifier assigned at construction.
    pub fn shard_id(&self) -> ShardId {
        self.shard_id
    }

    /// Number of instruments currently owned by this shard.
    pub fn instrument_count(&self) -> usize {
        self.engine.registered_instruments().count()
    }

    /// Handle a single request and return the corresponding reply.
    ///
    /// The shard must only be driven by one writer at a time; this type
    /// enforces that by taking `&mut self`.
    pub fn process(&mut self, request: ShardRequest) -> ShardReply {
        match request {
            ShardRequest::Execute(cmd) => {
                let instrument_id = cmd.instrument_id();
                match self.engine.execute(cmd) {
                    Ok(result) => {
                        let best_bid = self.engine.best_bid(instrument_id);
                        let best_ask = self.engine.best_ask(instrument_id);
                        ShardReply::Execute(Ok(ExecuteReply {
                            output: result.output,
                            events: result.events,
                            best_bid,
                            best_ask,
                        }))
                    }
                    Err(e) => ShardReply::Execute(Err(e)),
                }
            }
            ShardRequest::RegisterInstrument(instrument_id) => {
                let created = self.engine.register_instrument(instrument_id);
                ShardReply::RegisterInstrument { created }
            }
            ShardRequest::OrderbookSnapshot(instrument_id) => {
                ShardReply::OrderbookSnapshot(self.engine.get_orderbook_state(instrument_id))
            }
            ShardRequest::TopOfBook(instrument_id) => {
                if !self.engine.is_registered(instrument_id) {
                    ShardReply::TopOfBook(None)
                } else {
                    ShardReply::TopOfBook(Some((
                        self.engine.best_bid(instrument_id),
                        self.engine.best_ask(instrument_id),
                    )))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orderbook::order::Order;
    use crate::orderbook::order_type::OrderType;
    use crate::orderbook::side::Side;
    use crate::orderbook::types::{OrderId, Price, Quantity};

    fn limit_order(order_id: OrderId, side: Side, price: Price, quantity: Quantity) -> Order {
        Order::new(order_id, side, OrderType::GoodTillCancel, price, quantity)
    }

    fn place_cmd(instrument_id: InstrumentId, order_id: OrderId, side: Side) -> Command {
        Command::PlaceOrder {
            instrument_id,
            account_id: 1,
            request_id: order_id,
            order: limit_order(order_id, side, 100, 10),
        }
    }

    #[test]
    fn new_shard_has_no_instruments() {
        let shard = Shard::new(0);
        assert_eq!(shard.shard_id(), 0);
        assert_eq!(shard.instrument_count(), 0);
    }

    #[test]
    fn register_instrument_is_idempotent() {
        let mut shard = Shard::new(1);

        match shard.process(ShardRequest::RegisterInstrument(42)) {
            ShardReply::RegisterInstrument { created } => assert!(created),
            other => panic!("unexpected reply: {other:?}"),
        }
        match shard.process(ShardRequest::RegisterInstrument(42)) {
            ShardReply::RegisterInstrument { created } => assert!(!created),
            other => panic!("unexpected reply: {other:?}"),
        }
        assert_eq!(shard.instrument_count(), 1);
    }

    #[test]
    fn execute_on_unknown_instrument_returns_engine_error() {
        let mut shard = Shard::new(0);
        let reply = shard.process(ShardRequest::Execute(place_cmd(99, 1, Side::Buy)));
        match reply {
            ShardReply::Execute(Err(EngineError::UnknownInstrument(99))) => {}
            other => panic!("unexpected reply: {other:?}"),
        }
    }

    #[test]
    fn execute_returns_bundled_top_of_book() {
        let mut shard = Shard::new(0);
        shard.process(ShardRequest::RegisterInstrument(1));

        let reply = shard.process(ShardRequest::Execute(place_cmd(1, 1, Side::Buy)));
        match reply {
            ShardReply::Execute(Ok(r)) => {
                assert!(matches!(r.output, CommandOutput::PlaceOrder(Ok(_))));
                assert_eq!(r.best_bid, Some(100));
                assert_eq!(r.best_ask, None);
                assert!(!r.events.is_empty(), "expected events to be emitted");
            }
            other => panic!("unexpected reply: {other:?}"),
        }
    }

    #[test]
    fn top_of_book_distinguishes_unregistered_from_empty() {
        let mut shard = Shard::new(0);
        match shard.process(ShardRequest::TopOfBook(1)) {
            ShardReply::TopOfBook(None) => {}
            other => panic!("unexpected reply: {other:?}"),
        }

        shard.process(ShardRequest::RegisterInstrument(1));
        match shard.process(ShardRequest::TopOfBook(1)) {
            ShardReply::TopOfBook(Some((None, None))) => {}
            other => panic!("unexpected reply: {other:?}"),
        }

        shard.process(ShardRequest::Execute(place_cmd(1, 1, Side::Buy)));
        match shard.process(ShardRequest::TopOfBook(1)) {
            ShardReply::TopOfBook(Some((Some(100), None))) => {}
            other => panic!("unexpected reply: {other:?}"),
        }
    }

    #[test]
    fn orderbook_snapshot_returns_none_for_unregistered() {
        let mut shard = Shard::new(0);
        match shard.process(ShardRequest::OrderbookSnapshot(7)) {
            ShardReply::OrderbookSnapshot(None) => {}
            other => panic!("unexpected reply: {other:?}"),
        }
    }

    #[test]
    fn shard_isolates_separate_instruments() {
        let mut shard = Shard::new(0);
        shard.process(ShardRequest::RegisterInstrument(1));
        shard.process(ShardRequest::RegisterInstrument(2));

        shard.process(ShardRequest::Execute(place_cmd(1, 10, Side::Buy)));
        shard.process(ShardRequest::Execute(place_cmd(2, 20, Side::Sell)));

        match shard.process(ShardRequest::TopOfBook(1)) {
            ShardReply::TopOfBook(Some((Some(100), None))) => {}
            other => panic!("instrument 1 unexpected reply: {other:?}"),
        }
        match shard.process(ShardRequest::TopOfBook(2)) {
            ShardReply::TopOfBook(Some((None, Some(100)))) => {}
            other => panic!("instrument 2 unexpected reply: {other:?}"),
        }
    }
}
