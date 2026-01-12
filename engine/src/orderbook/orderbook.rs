use std::collections::BTreeMap;
use crate::orderbook::types::{Price};
use crate::orderbook::order::Order;

pub struct Orderbook {
    pub bids: BTreeMap<Price, Vec<Order>>,
    pub asks: BTreeMap<Price, Vec<Order>>
}

impl Orderbook {
    pub fn new() -> Self {
        Self { bids: BTreeMap::new(), asks: BTreeMap::new() }
    }
}