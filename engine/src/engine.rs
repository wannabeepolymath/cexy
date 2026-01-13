use crate::orderbook::orderbook::Orderbook;

pub struct Engine {
    pub orderbook: Orderbook,
}

impl Engine {
    pub fn new() -> Self {
        Self { orderbook: Orderbook::new() }
    }
}