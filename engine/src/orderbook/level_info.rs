use crate::orderbook::types::{Price, Quantity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelInfo {
    pub price: Price,
    pub quantity: Quantity,
}

impl LevelInfo {
    pub fn new(price: Price, quantity: Quantity) -> Self {
        Self { price, quantity }
    }
}

pub type LevelInfos = Vec<LevelInfo>;


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderbookLevelInfo {
    bids: LevelInfos,
    asks: LevelInfos,
}

impl OrderbookLevelInfo {
    pub fn new(bids: LevelInfos, asks: LevelInfos) -> Self {
        Self { bids, asks }
    }

    pub fn get_bids(&self) -> &LevelInfos {
        &self.bids
    }

    pub fn get_asks(&self) -> &LevelInfos {
        &self.asks
    }
}