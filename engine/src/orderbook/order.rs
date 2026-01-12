use crate::orderbook::{order_type::OrderType, side::Side, types::{OrderId, Price, Quantity}};
use thiserror::Error;
use std::collections::VecDeque;

#[derive(Debug, Error)]
pub enum OrderError {
    #[error("Order ({0}) cannot be filled for more than its remaining quantity")]
    OverFill(OrderId),
    #[error("Order ({0}) cannot have its price adjusted, only market orders can")]
    InvalidPriceAdjustment(OrderId),
}

#[derive(Debug, Clone)]
pub struct Order {
    pub order_id: OrderId,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Price,
    pub initial_quantity: Quantity,
    pub remaining_quantity: Quantity,
}

impl Order {
    pub fn new(
        order_id: OrderId,
        side: Side,
        order_type: OrderType,
        price: Price,
        quantity: Quantity,
    ) -> Self {
        Self {
            order_id,
            side,
            order_type,
            price,
            initial_quantity: quantity,
            remaining_quantity: quantity,
        }
    }

     pub fn order_id(&self) -> OrderId {
        self.order_id
    }

    pub fn side(&self) -> Side {
        self.side
    }

    pub fn price(&self) -> Price {
        self.price
    }

    pub fn order_type(&self) -> OrderType {
        self.order_type
    }

    pub fn initial_quantity(&self) -> Quantity {
        self.initial_quantity
    }

    pub fn remaining_quantity(&self) -> Quantity {
        self.remaining_quantity
    }

    pub fn filled_quantity(&self) -> Quantity {
        self.initial_quantity - self.remaining_quantity
    }

    pub fn is_filled(&self) -> bool {
        self.remaining_quantity == 0
    }

    pub fn fill(&mut self, quantity: Quantity) {

        if quantity > self.remaining_quantity {
            eprintln!("Cannot fill more than the remaining quantity: {} > {}", quantity, self.remaining_quantity);
            return;
        }

        self.remaining_quantity -= quantity;
    }
}

pub type OrderList = VecDeque<Order>;
