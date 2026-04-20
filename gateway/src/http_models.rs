use engine::orderbook::types::{OrderId, Price, Quantity};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Deserialize)]
pub struct CreateOrderRequest {
    pub instrument_id: u32,
    pub account_id: u64,
    pub request_id: u64,
    pub order_id: OrderId,
    pub side: String,
    pub order_type: String,
    pub price: Option<Price>,
    pub quantity: Quantity,
}

#[derive(Deserialize)]
pub struct ModifyOrderRequest {
    pub instrument_id: u32,
    pub account_id: u64,
    pub request_id: u64,
    pub order_id: OrderId,
    pub side: String,
    pub price: Price,
    pub quantity: Quantity,
}

#[derive(Deserialize)]
pub struct CancelOrderQuery {
    pub instrument_id: u32,
    pub account_id: u64,
    pub request_id: u64,
}

#[derive(Deserialize)]
pub struct InstrumentQuery {
    pub instrument_id: u32,
}

#[derive(Serialize)]
pub struct OrderResult {
    pub trades: usize,
    pub best_bid: Option<Price>,
    pub best_ask: Option<Price>,
}

#[derive(Serialize)]
pub struct TopOfBookResponse {
    pub best_bid: Option<Price>,
    pub best_ask: Option<Price>,
}

#[derive(Serialize)]
pub struct Level {
    pub price: Price,
    pub quantity: Quantity,
}

#[derive(Serialize)]
pub struct OrderbookResponse {
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
}

#[derive(Deserialize)]
pub struct RegisterInstrumentRequest {
    pub instrument_id: u32,
}

#[derive(Serialize)]
pub struct RegisterInstrumentResponse {
    pub instrument_id: u32,
    /// `true` when this call actually created a book, `false` on idempotent re-registration.
    pub created: bool,
}
