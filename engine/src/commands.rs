//! Canonical commands and explicit outcomes for the matching engine.
//!
//! The order book no longer uses empty `Trades` as a silent rejection signal.

use crate::orderbook::order::Order;
use crate::orderbook::order_modify::OrderModify;
use crate::orderbook::trade::Trades;
use crate::orderbook::types::{OrderId, OrderIds};
use thiserror::Error;

pub type InstrumentId = u32;
pub type AccountId = u64;
pub type RequestId = u64;

/// A single inbound action for [`crate::engine::Engine::execute`](crate::engine::Engine::execute).
#[derive(Debug, Clone)]
pub enum Command {
    PlaceOrder {
        instrument_id: InstrumentId,
        account_id: AccountId,
        request_id: RequestId,
        order: Order,
    },
    CancelOrder {
        instrument_id: InstrumentId,
        account_id: AccountId,
        request_id: RequestId,
        order_id: OrderId,
    },
    CancelOrders {
        instrument_id: InstrumentId,
        account_id: AccountId,
        request_id: RequestId,
        order_ids: OrderIds,
    },
    ModifyOrder {
        instrument_id: InstrumentId,
        account_id: AccountId,
        request_id: RequestId,
        modify: OrderModify,
    },
}

impl Command {
    pub fn instrument_id(&self) -> InstrumentId {
        match self {
            Command::PlaceOrder { instrument_id, .. }
            | Command::CancelOrder { instrument_id, .. }
            | Command::CancelOrders { instrument_id, .. }
            | Command::ModifyOrder { instrument_id, .. } => *instrument_id,
        }
    }
}

/// Infrastructure-level errors surfaced by [`crate::engine::Engine::execute`].
///
/// Distinct from per-command business rejects (e.g. [`PlaceOrderReject`]):
/// these represent routing/plumbing failures rather than matching outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EngineError {
    #[error("unknown instrument: {0}")]
    UnknownInstrument(InstrumentId),
}

/// Result of [`Command`] dispatch; discriminant matches [`Command`].
#[derive(Debug)]
pub enum CommandOutput {
    PlaceOrder(PlaceOrderResult),
    CancelOrder(CancelOrderResult),
    CancelOrders(CancelOrdersSummary),
    ModifyOrder(ModifyOrderResult),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaceOrderSuccess {
    pub trades: Trades,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlaceOrderReject {
    #[error("duplicate order id")]
    DuplicateOrderId,
    #[error("post-only order would take liquidity")]
    PostOnlyWouldTakeLiquidity,
    #[error("no liquidity for market order")]
    NoLiquidityForMarket,
    #[error("fill-and-kill: no immediate match")]
    FillAndKillNoMatch,
    #[error("fill-or-kill: insufficient liquidity at limit price")]
    FillOrKillInsufficientLiquidity,
}

pub type PlaceOrderResult = Result<PlaceOrderSuccess, PlaceOrderReject>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOrderResult {
    Cancelled,
    NotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CancelOrdersSummary {
    pub cancelled: usize,
    pub not_found: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModifyOrderSuccess {
    pub trades: Trades,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModifyOrderReject {
    #[error("order not found")]
    OrderNotFound,
    #[error("side change not allowed on modify")]
    SideChangeNotAllowed,
    #[error(transparent)]
    PlaceRejected(#[from] PlaceOrderReject),
}

pub type ModifyOrderResult = Result<ModifyOrderSuccess, ModifyOrderReject>;
