use std::cmp::min;
use std::collections::{BTreeMap, VecDeque, HashMap};
use crate::orderbook::types::{Price, Quantity, OrderId};
use crate::orderbook::side::Side;
use crate::orderbook::order_type::OrderType;
use crate::orderbook::order::{Order};
use crate::orderbook::trade::Trades;

#[derive(Debug, Clone, Default)]
struct LevelData {
    quantity: Quantity,
    count: u64,
}

struct OrderEntry {
    order: Order,
}

type OrderIdList = VecDeque<OrderId>;

pub struct Orderbook {
    orders: HashMap<OrderId, Order>,
    bids: BTreeMap<Price, OrderIdList>,
    asks: BTreeMap<Price, OrderIdList>
}

impl Orderbook {
    pub fn new() -> Self {
        Self {
            orders: HashMap::new(),
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        }
    }

    pub fn best_bid(&self) -> Option<Price> {
        self.bids.keys().next_back().copied()
    }

    pub fn best_ask(&self) -> Option<Price> {
        self.asks.keys().next().copied()
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


    pub fn add_order(&mut self, order: Order) {
        let order_id = order.order_id;
        if self.orders.contains_key(&order_id) {
            return;
        }
        if order.order_type() == OrderType::FillAndKill && !self.can_match(order.side, order.price) {
            return;
        }

        self.orders.insert(order_id, order);

        match order.side {
            Side::Buy => {
                self.bids.entry(order.price).or_default().push_back(order_id);
            }
            Side::Sell => {
                self.asks.entry(order.price).or_default().push_back(order_id);
            }
        }

        // self.match_order();
    }

    pub fn cancel_order(&mut self, order_id: OrderId) {
        let Some(order) = self.orders.remove(&order_id) else {return};

        let price_levels = match order.side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        if let Some(order_ids) = price_levels.get_mut(&order.price) {
            order_ids.retain(|&id| id != order_id);
            if order_ids.is_empty() {
                price_levels.remove(&order.price);
            }
        }
    }
}