use crate::commands::{
    CancelOrderResult, CancelOrdersSummary, Command, CommandOutput, ModifyOrderResult,
    PlaceOrderResult,
};
use crate::orderbook::orderbook::Orderbook;
use crate::orderbook::order::Order;
use crate::orderbook::order_modify::OrderModify;
use crate::orderbook::types::{OrderId, Price};
use crate::orderbook::level_info::OrderbookLevelInfo;

pub struct Engine {
    orderbook: Orderbook,
}

impl Engine {
    pub fn new() -> Self {
        Self { orderbook: Orderbook::new() }
    }

    pub fn place_order(&mut self, order: Order) -> PlaceOrderResult {
        self.orderbook.add_order(order)
    }

    pub fn cancel_order(&mut self, order_id: OrderId) -> CancelOrderResult {
        self.orderbook.cancel_order(order_id)
    }

    pub fn cancel_orders(&mut self, order_ids: Vec<OrderId>) -> CancelOrdersSummary {
        self.orderbook.cancel_orders(order_ids)
    }

    pub fn modify_order(&mut self, order_modify: OrderModify) -> ModifyOrderResult {
        self.orderbook.modify_order(order_modify)
    }

    pub fn execute(&mut self, cmd: Command) -> CommandOutput {
        match cmd {
            Command::PlaceOrder { order } => CommandOutput::PlaceOrder(self.place_order(order)),
            Command::CancelOrder { order_id } => CommandOutput::CancelOrder(self.cancel_order(order_id)),
            Command::CancelOrders { order_ids } => {
                CommandOutput::CancelOrders(self.cancel_orders(order_ids))
            }
            Command::ModifyOrder { modify } => CommandOutput::ModifyOrder(self.modify_order(modify)),
        }
    }

    pub fn get_orderbook_state(&self) -> OrderbookLevelInfo {
        self.orderbook.get_order_infos()
    }

    pub fn best_bid(&self) -> Option<Price> {
        self.orderbook.best_bid()
    }

    pub fn best_ask(&self) -> Option<Price> {
        self.orderbook.best_ask()
    }

    pub fn order_count(&self) -> usize {
        self.orderbook.size()
    }
}