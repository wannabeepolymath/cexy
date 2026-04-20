//! Canonical commands and explicit outcomes for the matching engine.
//!
//! The order book no longer uses empty `Trades` as a silent rejection signal.

use crate::orderbook::order::Order;
use crate::orderbook::order_modify::OrderModify;
use crate::orderbook::trade::Trades;
use crate::orderbook::types::{OrderId, OrderIds};
use thiserror::Error;

/// A single inbound action for [`crate::engine::Engine::execute`](crate::engine::Engine::execute).
#[derive(Debug, Clone)]
pub enum Command {
    PlaceOrder { order: Order },
    CancelOrder { order_id: OrderId },
    CancelOrders { order_ids: OrderIds },
    ModifyOrder { modify: OrderModify },
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
    #[error(transparent)]
    PlaceRejected(#[from] PlaceOrderReject),
}

pub type ModifyOrderResult = Result<ModifyOrderSuccess, ModifyOrderReject>;
