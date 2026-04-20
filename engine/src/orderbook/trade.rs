use crate::orderbook::types::{OrderId, Price, Quantity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeInfo {
    order_id: OrderId,
    price: Price,
    quantity: Quantity,
}

impl TradeInfo{
    pub fn new(order_id: OrderId, price: Price, quantity: Quantity) -> Self {
        Self {
            order_id, 
            price, 
            quantity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trade {
    bids_trades: TradeInfo,
    asks_trades: TradeInfo,
}

impl Trade {
    pub fn new(bids_trades: TradeInfo, asks_trades: TradeInfo) -> Self {
        Self {
            bids_trades,
            asks_trades,
        }
    }

    pub fn bids_trades(&self) -> &TradeInfo {
        &self.bids_trades
    }

    pub fn asks_trades(&self) -> &TradeInfo {
        &self.asks_trades
    }
}


pub type Trades = Vec<Trade>;