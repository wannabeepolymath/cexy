

pub struct Engine {
    pub orderbook: Vec<Orderbook>,
}

impl Engine {
    pub fn new() -> Self {
        Self { orderbook: Vec::new() }
    }
}