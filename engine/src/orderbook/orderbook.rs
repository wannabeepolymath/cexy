use std::collections::{BTreeMap, VecDeque, HashMap};
use crate::orderbook::types::{Price, Quantity, OrderId};
use crate::orderbook::side::Side;
use crate::orderbook::order_type::OrderType;
use crate::orderbook::order::{Order};
use crate::orderbook::trade::{Trade, Trades, TradeInfo};
use crate::orderbook::order_modify::OrderModify;
use crate::orderbook::level_info::{OrderbookLevelInfo, LevelInfo, LevelInfos};

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

    fn match_orders(&mut self) -> Trades {
        let mut trades = Vec::new();

        loop {
            let Some(bid_price) = self.best_bid() else { break };
            let Some(ask_price) = self.best_ask() else { break };

            if bid_price < ask_price {
                break;
            }

            loop {
                let bid_id = self.bids.get(&bid_price).and_then(|ids| ids.front().copied());
                let ask_id = self.asks.get(&ask_price).and_then(|ids| ids.front().copied());

                let (Some(bid_id), Some(ask_id)) = (bid_id, ask_id) else {
                    break;
                };

                let quantity = {
                    let bid = self.orders.get(&bid_id).unwrap();
                    let ask = self.orders.get(&ask_id).unwrap();
                    std::cmp::min(bid.remaining_quantity(), ask.remaining_quantity())
                };

                self.orders.get_mut(&bid_id).unwrap().fill(quantity).unwrap();
                self.orders.get_mut(&ask_id).unwrap().fill(quantity).unwrap();

                // Check if filled
                let bid_filled = self.orders.get(&bid_id).unwrap().is_filled();
                let ask_filled = self.orders.get(&ask_id).unwrap().is_filled();
                let bid_price_val = self.orders.get(&bid_id).unwrap().price();
                let ask_price_val = self.orders.get(&ask_id).unwrap().price();

                // Remove filled orders from queue
                if bid_filled {
                    if let Some(ids) = self.bids.get_mut(&bid_price) {
                        ids.pop_front();
                    }
                    self.orders.remove(&bid_id);
                }

                if ask_filled {
                    if let Some(ids) = self.asks.get_mut(&ask_price) {
                        ids.pop_front();
                    }
                    self.orders.remove(&ask_id);
                }

                trades.push(Trade::new(
                    TradeInfo::new(bid_id, bid_price_val, quantity),
                    TradeInfo::new(ask_id, ask_price_val, quantity),
                ));
            }

            // Clean up empty levels
            if self.bids.get(&bid_price).map_or(true, |ids| ids.is_empty()) {
                self.bids.remove(&bid_price);
            }

            if self.asks.get(&ask_price).map_or(true, |ids| ids.is_empty()) {
                self.asks.remove(&ask_price);
            }
        }
        trades
    }

    pub fn add_order(&mut self, order: Order) -> Trades {
        let order_id = order.order_id;
        if self.orders.contains_key(&order_id) {
            return Trades::new();
        }
        if order.order_type() == OrderType::FillAndKill && !self.can_match(order.side, order.price) {
            return Trades::new();
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

        self.match_orders()
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

    pub fn modify_order(&mut self, order_modify: OrderModify) -> Trades {
        let Some(order) = self.orders.get(&order_modify.order_id()) else {
            return Trades::new();
        };

        let order_type = order.order_type();
        
        self.cancel_order(order_modify.order_id());
        self.add_order(order_modify.modify(order_type))
    }

    pub fn size(&self) -> usize {
        self.orders.len()
    }

    /// Returns the current state of the orderbook.
    pub fn get_order_infos(&self) -> OrderbookLevelInfo {
        let mut bid_infos: LevelInfos = Vec::with_capacity(self.bids.len());
        let mut ask_infos: LevelInfos = Vec::with_capacity(self.asks.len());

        // Bids - highest price first (iterate in reverse)
        for (&price, order_ids) in self.bids.iter().rev() {
            let quantity: Quantity = order_ids
                .iter()
                .filter_map(|id| self.orders.get(id))
                .map(|o| o.remaining_quantity())
                .sum();
            bid_infos.push(LevelInfo::new(price, quantity));
        }

        // Asks - lowest price first (iterate forward)
        for (&price, order_ids) in self.asks.iter() {
            let quantity: Quantity = order_ids
                .iter()
                .filter_map(|id| self.orders.get(id))
                .map(|o| o.remaining_quantity())
                .sum();
            ask_infos.push(LevelInfo::new(price, quantity));
        }

        OrderbookLevelInfo::new(bid_infos, ask_infos)
    }
}


impl Default for Orderbook {
    fn default() -> Self {
        Self::new()
    }
}