//! Engine event vocabulary.
//!
//! Every mutating operation on an [`crate::orderbook::orderbook::Orderbook`]
//! produces a sequence of [`Event`]s in addition to the usual
//! [`crate::commands::CommandOutput`]. Events are per-book ordered via
//! [`Event::seq`] and are the single source of truth for downstream consumers
//! (market data publishers, journal/WAL, metrics).
//!
//! This module only defines the vocabulary. Emission is wired in a later step.

use crate::commands::{InstrumentId, PlaceOrderReject};
use crate::orderbook::side::Side;
use crate::orderbook::trade::Trade;
use crate::orderbook::types::{OrderId, Price, Quantity};

/// Monotonically increasing sequence number, scoped to a single orderbook.
pub type EventSeq = u64;

/// Reason an inbound place/modify was rejected.
///
/// Flat superset of [`PlaceOrderReject`] plus modify-only failure modes,
/// so a single event variant can carry either source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectReason {
    // Place-path rejects.
    DuplicateOrderId,
    PostOnlyWouldTakeLiquidity,
    NoLiquidityForMarket,
    FillAndKillNoMatch,
    FillOrKillInsufficientLiquidity,
    // Modify-path rejects.
    OrderNotFound,
    SideChangeNotAllowed,
}

impl From<PlaceOrderReject> for RejectReason {
    fn from(r: PlaceOrderReject) -> Self {
        match r {
            PlaceOrderReject::DuplicateOrderId => RejectReason::DuplicateOrderId,
            PlaceOrderReject::PostOnlyWouldTakeLiquidity => {
                RejectReason::PostOnlyWouldTakeLiquidity
            }
            PlaceOrderReject::NoLiquidityForMarket => RejectReason::NoLiquidityForMarket,
            PlaceOrderReject::FillAndKillNoMatch => RejectReason::FillAndKillNoMatch,
            PlaceOrderReject::FillOrKillInsufficientLiquidity => {
                RejectReason::FillOrKillInsufficientLiquidity
            }
        }
    }
}

/// A single fact emitted by the engine after processing a command.
///
/// Events are ordered per-instrument via their `seq` field. Across
/// instruments there is no global ordering guarantee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A new order was accepted into the book (post-matching; may have
    /// partially filled before resting).
    OrderAccepted {
        seq: EventSeq,
        instrument_id: InstrumentId,
        order_id: OrderId,
        side: Side,
        price: Price,
        quantity: Quantity,
    },
    /// An inbound place/modify was rejected before or during matching.
    OrderRejected {
        seq: EventSeq,
        instrument_id: InstrumentId,
        order_id: OrderId,
        reason: RejectReason,
    },
    /// An existing resting order was cancelled (explicitly or as part of
    /// a modify).
    OrderCanceled {
        seq: EventSeq,
        instrument_id: InstrumentId,
        order_id: OrderId,
        remaining_quantity: Quantity,
    },
    /// A match occurred between a maker and a taker.
    TradeExecuted {
        seq: EventSeq,
        instrument_id: InstrumentId,
        trade: Trade,
    },
    /// Best bid and/or best ask changed as a result of the command.
    TopOfBookUpdated {
        seq: EventSeq,
        instrument_id: InstrumentId,
        best_bid: Option<Price>,
        best_ask: Option<Price>,
    },
}

impl Event {
    /// Sequence number assigned by the originating orderbook.
    pub fn seq(&self) -> EventSeq {
        match self {
            Event::OrderAccepted { seq, .. }
            | Event::OrderRejected { seq, .. }
            | Event::OrderCanceled { seq, .. }
            | Event::TradeExecuted { seq, .. }
            | Event::TopOfBookUpdated { seq, .. } => *seq,
        }
    }

    /// Instrument this event belongs to.
    pub fn instrument_id(&self) -> InstrumentId {
        match self {
            Event::OrderAccepted { instrument_id, .. }
            | Event::OrderRejected { instrument_id, .. }
            | Event::OrderCanceled { instrument_id, .. }
            | Event::TradeExecuted { instrument_id, .. }
            | Event::TopOfBookUpdated { instrument_id, .. } => *instrument_id,
        }
    }
}

/// Convenience alias for a batch of events emitted by a single command.
pub type Events = Vec<Event>;
