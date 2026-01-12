use crate::Orderbook::orderbook;

pub struct Engine {
    pub orderbook: Orderbook,
}

impl Engine {
    pub fn new() -> Self {
        Self { orderbook: Orderbook::new() }
    }
}