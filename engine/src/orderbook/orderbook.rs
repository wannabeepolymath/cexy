use std::collections::{BTreeMap, VecDeque, HashMap};
use crate::orderbook::types::{Price, Quantity, OrderId};
use crate::orderbook::side::Side;
use crate::orderbook::order::Order;

#[derive(Debug, Clone, Default)]
struct LevelData {
    quantity: Quantity,
    count: u64,
}

struct OrderEntry {
    order: Order,
}

type OrderList = VecDeque<Order>;

pub struct Orderbook {
    orders: HashMap<OrderId, OrderEntry>,
    bids: BTreeMap<Price, OrderList>,
    asks: BTreeMap<Price, OrderList>,
}

impl Orderbook {
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn can_match(&self, side: Side, price: Price) -> bool {
        match side {
            Side::Buy => {
                if self.asks.is_empty() {
                    return false;
                }
                let best_ask = *self.asks.keys().next().unwrap();
                price >= best_ask
            }
            Side::Sell => {
                if self.bids.is_empty() {
                    return false;
                }
                let best_bid = *self.bids.keys().next_back().unwrap();
                price <= best_bid
            }
        }
    }
}