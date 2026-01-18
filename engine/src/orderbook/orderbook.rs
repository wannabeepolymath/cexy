use std::collections::{BTreeMap, VecDeque, HashMap};
use crate::orderbook::types::{Price, Quantity, OrderId, OrderIds};
use crate::orderbook::side::Side;
use crate::orderbook::order_type::OrderType;
use crate::orderbook::order::{Order};
use crate::orderbook::trade::{Trade, Trades, TradeInfo};
use crate::orderbook::order_modify::OrderModify;
use crate::orderbook::level_info::{OrderbookLevelInfo, LevelInfo, LevelInfos};

#[derive(Debug, Clone, PartialEq)]
enum LevelAction{
    Add,
    Remove,
    Match
}
#[derive(Debug, Clone, Default)]
struct LevelData {
    quantity: Quantity,
    count: u64,
}

type OrderIdList = VecDeque<OrderId>;

pub struct Orderbook {
    data: HashMap<Price, LevelData>,
    orders: HashMap<OrderId, Order>,
    bids: BTreeMap<Price, OrderIdList>,
    asks: BTreeMap<Price, OrderIdList>
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

    pub fn best_bid(&self) -> Option<Price> {
        self.bids.keys().next_back().copied()
    }

    pub fn best_ask(&self) -> Option<Price> {
        self.asks.keys().next().copied()
    }

    pub fn worst_bid(&self) -> Option<Price> {
        self.bids.keys().next().copied()
    }
    pub fn worst_ask(&self) -> Option<Price> {
        self.asks.keys().next_back().copied()
    }

    pub fn can_fully_fill(&self, side: Side, price: Price, mut quantity: Quantity) -> bool {
        if self.can_match(side, price) { return false }

        let levels = match side {
            Side::Buy => &self.asks,
            Side::Sell => &self.bids,
        };

        match side {
            Side::Buy => {
                for(&ask_price, order_ids) in levels.iter(){
                    if ask_price > price { break; }

                    let level_qty:Quantity = order_ids.iter()
                        .filter_map(|id|{self.orders.get(id)})
                        .map(|order| order.remaining_quantity())
                        .sum();

                    if quantity <= level_qty { return true }

                    quantity -= quantity.saturating_sub(level_qty);
                }
            },
            Side::Sell => {
                for (&bid_price, order_ids) in levels.iter().rev() {
                    if bid_price < price { break; }
                    let level_qty: Quantity = order_ids.iter()
                        .filter_map(|id| self.orders.get(id))
                        .map(|o| o.remaining_quantity())
                        .sum();

                    if quantity <= level_qty { return true }
                    quantity -= quantity.saturating_sub(level_qty);
                }
            }
        }
        false
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
                self.on_order_matched(bid_price_val, quantity, bid_filled);
                self.on_order_matched(ask_price_val, quantity, ask_filled);
            }

            if self.bids.get(&bid_price).map_or(true, |ids| ids.is_empty()) {
                self.bids.remove(&bid_price);
                self.data.remove(&bid_price);
            }

            if self.asks.get(&ask_price).map_or(true, |ids| ids.is_empty()) {
                self.asks.remove(&ask_price);
                self.data.remove(&ask_price);
            }
        }
        trades
    }

    pub fn add_order(&mut self, mut order: Order) -> Trades {
        let order_id = order.order_id;
        let side = order.side();
        let price = order.price();
        let quantity = order.initial_quantity();
        let order_type = order.order_type();

        if self.orders.contains_key(&order_id) {
            return Trades::new();
        }

        if order_type == OrderType::Market {
            let converted = match side {
                Side::Buy => {
                    if let Some(&worst_ask) = self.asks.keys().next_back() {
                        order.to_good_till_cancel(worst_ask).ok();
                        true
                    } else {
                        false
                    }
                }
                Side::Sell => {
                    if let Some(&worst_bid) = self.bids.keys().next() {
                        order.to_good_till_cancel(worst_bid).ok();
                        true
                    } else {
                        false
                    }
                }
            };
            if !converted {
                return Trades::new();
            }
        }

        if order_type == OrderType::FillAndKill && !self.can_match(side, price) {
            return Trades::new();
        }

        if order_type == OrderType::FillOrKill && !self.can_fully_fill(side, price, quantity) {
            return Trades::new();
        }

        self.orders.insert(order_id, order);

        match side {
            Side::Buy => {
                self.bids.entry(price).or_insert_with(VecDeque::new).push_back(order_id);
            }
            Side::Sell => {
                self.asks.entry(price).or_insert_with(VecDeque::new).push_back(order_id);
            }
        }
        self.on_order_added(price, quantity);

        self.match_orders()
    }

    pub fn cancel_order(&mut self, order_id: OrderId) {
        self.cancel_order_internal(order_id);
    }

    pub fn cancel_orders(&mut self, order_ids: OrderIds) {
        for order_id in order_ids {
            self.cancel_order_internal(order_id);
        }
    }
    fn cancel_order_internal(&mut self, order_id: OrderId) {
        let Some(order) = self.orders.remove(&order_id) else { return };

        let price = order.price();
        let side = order.side();
        let remaining = order.remaining_quantity();

        let price_levels = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        if let Some(order_ids) = price_levels.get_mut(&price) {
            order_ids.retain(|&id| id != order_id);
            if order_ids.is_empty() {
                price_levels.remove(&price);
            }
        }
        self.on_order_cancelled(price, remaining);
    }

    pub fn modify_order(&mut self, order_modify: OrderModify) -> Trades {
        let Some(order) = self.orders.get(&order_modify.order_id()) else {
            return Trades::new();
        };

        let order_type = order.order_type();
        
        self.cancel_order(order_modify.order_id());
        self.add_order(order_modify.modify(order_type))
    }


    fn on_order_added(&mut self, price: Price, quantity: Quantity) {
        self.update_level_data(price, quantity, LevelAction::Add);
    }
    fn on_order_cancelled(&mut self, price: Price, quantity: Quantity) {
        self.update_level_data(price, quantity, LevelAction::Remove);
    }
    fn on_order_matched(&mut self, price: Price, quantity: Quantity, is_filled: bool) {
        self.update_level_data(
            price,
            quantity,
            if is_filled {
                LevelAction::Remove
            } else {
                LevelAction::Match
            }
        )
    }
    fn update_level_data(&mut self, price: Price, quantity: Quantity, action: LevelAction) {
        let data = self.data.entry(price).or_default();

        match action {
            LevelAction::Add => {
                data.count = data.count.saturating_add(1);
                data.quantity = data.quantity.saturating_add(quantity);
            }
            LevelAction::Remove => {
                data.count = data.count.saturating_sub(1);
                data.quantity = data.quantity.saturating_sub(quantity);
            }
            LevelAction::Match => {
                data.quantity = data.quantity.saturating_sub(quantity);
            }
        }

        if data.count == 0 {
            self.data.remove(&price);
        }
    }

    pub fn size(&self) -> usize {
        self.orders.len()
    }

    pub fn get_order_infos(&self) -> OrderbookLevelInfo {
        let mut bid_infos: LevelInfos = Vec::with_capacity(self.bids.len());
        let mut ask_infos: LevelInfos = Vec::with_capacity(self.asks.len());

        for (&price, order_ids) in self.bids.iter().rev() {
            let quantity: Quantity = order_ids
                .iter()
                .filter_map(|id| self.orders.get(id))
                .map(|o| o.remaining_quantity())
                .sum();
            bid_infos.push(LevelInfo::new(price, quantity));
        }

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

    fn prune_good_till_cancel(&mut self) {
        // Goodtillcancel orders, cex is open 24/7, so no opening/closing auctions
    }
    
}


impl Default for Orderbook {
    fn default() -> Self {
        Self::new()
    }
}