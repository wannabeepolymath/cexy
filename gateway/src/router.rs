//! Router: dispatches every request to the shard that owns the instrument.
//!
//! The router is the first [`EngineHandle`] implementation that uses real
//! shard threads. It owns a fixed [`Vec<ShardThread>`] plus a [`ShardMap`]
//! that decides which shard handles each [`InstrumentId`]. For any inbound
//! operation it:
//!
//! 1. resolves the `InstrumentId` to a `ShardId` via [`ShardMap`],
//! 2. sends exactly one [`ShardRequest`] to that shard's thread,
//! 3. awaits the matching [`ShardReply`] and lifts it into the
//!    [`EngineHandle`] contract.
//!
//! ### Error policy
//!
//! Business-level outcomes (unknown instrument, rejects, etc.) flow back
//! through [`EngineError`] / `ShardReply` variants as normal. Infrastructure
//! failures - a shard thread having died - currently panic with a loud
//! message. Graceful degradation for this class of failure is explicitly
//! deferred to Phase 9 (panic isolation + backpressure).
//!
//! ### Cross-instrument atomicity
//!
//! The router never reaches into more than one shard per call, so
//! per-instrument atomicity is identical to that of [`ShardThread::submit`].
//! There is deliberately no cross-shard ordering guarantee; cross-shard
//! features (portfolio margin, multi-leg orders) are deferred.
//!
//! [`EngineHandle`]: crate::engine_handle::EngineHandle
//! [`ShardThread`]: engine::shard::ShardThread
//! [`ShardMap`]: engine::shard_map::ShardMap

use engine::commands::{Command, EngineError, InstrumentId};
use engine::event_bus::EventSender;
use engine::orderbook::level_info::OrderbookLevelInfo;
use engine::orderbook::types::Price;
use engine::shard::{ExecuteReply, ShardReply, ShardRequest, ShardThread};
use engine::shard_map::{ShardMap, ShardMapError};
use thiserror::Error;

use crate::engine_handle::{EngineHandle, EngineReply};

/// Failure modes for [`Router::new`] / [`Router::with_map`].
#[derive(Debug, Error)]
pub enum RouterError {
    #[error(transparent)]
    Map(#[from] ShardMapError),
}

/// Ordered collection of shard threads plus the static routing table that
/// decides which one owns each instrument.
pub struct Router {
    // Indexed by `ShardId`. Never resized; a router has a fixed number
    // of shards for its lifetime.
    shards: Vec<ShardThread>,
    map: ShardMap,
}

impl Router {
    /// Build a router with `shard_count` freshly-spawned shard threads and
    /// the default modulo routing map. Shards do not publish events
    /// anywhere; use [`Router::new_with_events`] to attach an event bus.
    ///
    /// Kept primarily for tests that don't need an event pipeline. The
    /// production binary uses [`Router::new_with_events`].
    #[allow(dead_code)]
    pub fn new(shard_count: u16) -> Result<Self, RouterError> {
        let map = ShardMap::new(shard_count)?;
        Ok(Self::with_map(map))
    }

    /// Like [`Router::new`] but every shard publishes its events to
    /// `event_tx`. The router itself does not read events; the bus owns
    /// the consumer thread.
    pub fn new_with_events(
        shard_count: u16,
        event_tx: EventSender,
    ) -> Result<Self, RouterError> {
        let map = ShardMap::new(shard_count)?;
        Ok(Self::with_map_and_events(map, event_tx))
    }

    /// Build a router around an explicit [`ShardMap`]. The number of
    /// shards spawned equals [`ShardMap::shard_count`]. No event
    /// publication; see [`Router::with_map_and_events`].
    #[allow(dead_code)]
    pub fn with_map(map: ShardMap) -> Self {
        let shards = (0..map.shard_count()).map(ShardThread::spawn).collect();
        Self { shards, map }
    }

    /// Like [`Router::with_map`] but each spawned shard receives its own
    /// clone of `event_tx` and publishes events through it.
    pub fn with_map_and_events(map: ShardMap, event_tx: EventSender) -> Self {
        let shards = (0..map.shard_count())
            .map(|shard_id| ShardThread::spawn_with_events(shard_id, event_tx.clone()))
            .collect();
        Self { shards, map }
    }

    /// Number of shard threads owned by this router.
    pub fn shard_count(&self) -> u16 {
        self.map.shard_count()
    }

    /// Snapshot of the routing table. Callers only need it for
    /// introspection / admin tooling; the router itself does lookups
    /// internally.
    #[cfg(test)]
    pub fn shard_map(&self) -> &ShardMap {
        &self.map
    }

    fn dispatch(&self, instrument_id: InstrumentId, request: ShardRequest) -> ShardReply {
        let shard_id = self.map.shard_for(instrument_id);
        let shard = &self.shards[usize::from(shard_id)];
        shard
            .submit(request)
            .unwrap_or_else(|e| panic!("router: shard {shard_id} unreachable: {e}"))
    }
}

impl EngineHandle for Router {
    fn submit(&self, cmd: Command) -> Result<EngineReply, EngineError> {
        let instrument_id = cmd.instrument_id();
        match self.dispatch(instrument_id, ShardRequest::Execute(cmd)) {
            ShardReply::Execute(Ok(ExecuteReply {
                output,
                events,
                best_bid,
                best_ask,
            })) => Ok(EngineReply {
                output,
                events,
                best_bid,
                best_ask,
            }),
            ShardReply::Execute(Err(e)) => Err(e),
            other => panic!("router: expected Execute reply, got {other:?}"),
        }
    }

    fn register_instrument(&self, instrument_id: InstrumentId) -> bool {
        match self.dispatch(
            instrument_id,
            ShardRequest::RegisterInstrument(instrument_id),
        ) {
            ShardReply::RegisterInstrument { created } => created,
            other => panic!("router: expected RegisterInstrument reply, got {other:?}"),
        }
    }

    fn orderbook_snapshot(&self, instrument_id: InstrumentId) -> Option<OrderbookLevelInfo> {
        match self.dispatch(
            instrument_id,
            ShardRequest::OrderbookSnapshot(instrument_id),
        ) {
            ShardReply::OrderbookSnapshot(snap) => snap,
            other => panic!("router: expected OrderbookSnapshot reply, got {other:?}"),
        }
    }

    fn top_of_book(
        &self,
        instrument_id: InstrumentId,
    ) -> Option<(Option<Price>, Option<Price>)> {
        match self.dispatch(instrument_id, ShardRequest::TopOfBook(instrument_id)) {
            ShardReply::TopOfBook(tob) => tob,
            other => panic!("router: expected TopOfBook reply, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::commands::{Command, CommandOutput};
    use engine::orderbook::order::Order;
    use engine::orderbook::order_type::OrderType;
    use engine::orderbook::side::Side;

    fn place_cmd(instrument_id: InstrumentId, order_id: u64, side: Side, price: i32) -> Command {
        Command::PlaceOrder {
            instrument_id,
            account_id: 1,
            request_id: order_id,
            order: Order::new(order_id, side, OrderType::GoodTillCancel, price, 10),
        }
    }

    #[test]
    fn rejects_zero_shard_count() {
        assert!(matches!(
            Router::new(0),
            Err(RouterError::Map(ShardMapError::ZeroShardCount))
        ));
    }

    #[test]
    fn register_then_submit_routes_to_same_shard() {
        let router = Router::new(2).unwrap();
        // Modulo mapping: instrument 1 -> shard 1, instrument 2 -> shard 0.
        assert_eq!(router.shard_map().shard_for(1), 1);
        assert_eq!(router.shard_map().shard_for(2), 0);

        assert!(router.register_instrument(1));
        assert!(router.register_instrument(2));

        // Both shards now handle their own instrument and reject the other.
        let reply = router.submit(place_cmd(1, 1, Side::Buy, 100)).unwrap();
        assert!(matches!(reply.output, CommandOutput::PlaceOrder(Ok(_))));
        assert_eq!(reply.best_bid, Some(100));
    }

    #[test]
    fn unknown_instrument_surfaces_engine_error() {
        let router = Router::new(2).unwrap();
        let err = router
            .submit(place_cmd(42, 1, Side::Buy, 100))
            .expect_err("expected UnknownInstrument");
        assert!(matches!(err, EngineError::UnknownInstrument(42)));
    }

    #[test]
    fn instruments_on_different_shards_are_isolated() {
        let router = Router::new(2).unwrap();
        assert!(router.register_instrument(1));
        assert!(router.register_instrument(2));

        router.submit(place_cmd(1, 10, Side::Buy, 100)).unwrap();
        router.submit(place_cmd(2, 20, Side::Sell, 200)).unwrap();

        // Each instrument sees only its own activity.
        assert_eq!(router.top_of_book(1), Some((Some(100), None)));
        assert_eq!(router.top_of_book(2), Some((None, Some(200))));
    }

    #[test]
    fn top_of_book_for_unregistered_returns_none() {
        let router = Router::new(2).unwrap();
        assert_eq!(router.top_of_book(99), None);
    }

    #[test]
    fn override_pins_instrument_to_specific_shard() {
        // Default: instrument 1 -> shard 1. Override: pin it to shard 0.
        let map = ShardMap::with_overrides(2, [(1u32, 0u16)]).unwrap();
        let router = Router::with_map(map);
        assert_eq!(router.shard_map().shard_for(1), 0);

        router.register_instrument(1);
        let reply = router.submit(place_cmd(1, 1, Side::Buy, 100)).unwrap();
        assert!(matches!(reply.output, CommandOutput::PlaceOrder(Ok(_))));
    }

    #[test]
    fn events_from_both_shards_reach_single_consumer_in_per_instrument_order() {
        use engine::event_bus::{EventBus, EventConsumer};
        use engine::events::Event;
        use std::sync::{Arc, Mutex};

        struct VecConsumer(Arc<Mutex<Vec<Event>>>);
        impl EventConsumer for VecConsumer {
            fn consume(&mut self, event: Event) {
                self.0.lock().unwrap().push(event);
            }
        }

        let collected = Arc::new(Mutex::new(Vec::<Event>::new()));
        let bus = EventBus::new(VecConsumer(Arc::clone(&collected)));
        let router = Router::new_with_events(2, bus.sender()).unwrap();

        // One instrument per shard under modulo mapping.
        assert_eq!(router.shard_map().shard_for(1), 1);
        assert_eq!(router.shard_map().shard_for(2), 0);
        assert!(router.register_instrument(1));
        assert!(router.register_instrument(2));

        // A few place commands on each instrument so both shards produce
        // multiple events, enough to catch an ordering regression.
        for i in 0..5 {
            router
                .submit(place_cmd(1, 100 + i, Side::Buy, 100))
                .unwrap();
            router
                .submit(place_cmd(2, 200 + i, Side::Sell, 200))
                .unwrap();
        }

        // Shut down in order: router first (closes shard senders, which
        // flushes any in-flight events to the bus), then the bus (closes
        // the event channel, joins the consumer thread).
        drop(router);
        drop(bus);

        let got = collected.lock().unwrap();
        assert!(got.len() >= 10, "too few events collected: {}", got.len());

        // Per-instrument ordering: seqs for each instrument must be
        // strictly monotonic in arrival order.
        let mut seqs_1: Vec<_> = got
            .iter()
            .filter(|e| e.instrument_id() == 1)
            .map(|e| e.seq())
            .collect();
        let mut seqs_2: Vec<_> = got
            .iter()
            .filter(|e| e.instrument_id() == 2)
            .map(|e| e.seq())
            .collect();

        // Every event should belong to either instrument 1 or 2.
        assert_eq!(seqs_1.len() + seqs_2.len(), got.len());
        assert!(!seqs_1.is_empty());
        assert!(!seqs_2.is_empty());

        // Monotonic within each instrument.
        let check_monotonic = |seqs: &mut Vec<u64>| {
            for pair in seqs.windows(2) {
                assert!(pair[0] < pair[1], "seq regressed: {seqs:?}");
            }
        };
        check_monotonic(&mut seqs_1);
        check_monotonic(&mut seqs_2);
    }
}
