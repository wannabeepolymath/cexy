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
    /// the default modulo routing map.
    pub fn new(shard_count: u16) -> Result<Self, RouterError> {
        let map = ShardMap::new(shard_count)?;
        Ok(Self::with_map(map))
    }

    /// Build a router around an explicit [`ShardMap`]. The number of
    /// shards spawned equals [`ShardMap::shard_count`].
    pub fn with_map(map: ShardMap) -> Self {
        let shards = (0..map.shard_count()).map(ShardThread::spawn).collect();
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
}
