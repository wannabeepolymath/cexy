use std::collections::{BTreeMap, VecDeque, HashMap};
use crate::orderbook::types::{Price, Quantity, OrderId};
use crate::orderbook::side::Side;
use crate::orderbook::order::OrderPointer;
use crate::orderbook::trade::Trades;

#[derive(Debug, Clone, Default)]
struct LevelData {
    quantity: Quantity,
    count: u64,
}

struct OrderEntry {
    order: OrderPointer,
}

type OrderList = VecDeque<OrderPointer>;

pub struct Orderbook {
    data: HashMap<Price, LevelData>,
    orders: HashMap<OrderId, OrderEntry>,
    bids: BTreeMap<Price, OrderList>,
    asks: BTreeMap<Price, OrderList>,
}

impl Orderbook {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
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
    
    pub fn match_order(&self) -> Trades {
        let mut trades = Vec::with_capacity(self.orders.len());

        loop {
            if self.bids.is_empty() || self.asks.is_empty() {
                break;
            }
            
            let ask_price = *self.asks.keys().next().unwrap();
            let bid_price = *self.bids.keys().next_back().unwrap();

            if bid_price < ask_price {
                break;
            }

            loop {
                let quantity = min(self.bids.get(&bid_price).unwrap().quantity, self.asks.get(&ask_price).unwrap().quantity);


        }
    }
}