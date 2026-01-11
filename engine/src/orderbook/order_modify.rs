use crate::orderbook::{order::{OrderPointer, Order, create_order}, order_type::OrderType, side::Side, types::{OrderId, Price, Quantity}};


#[derive(Debug, Clone)]
pub struct OrderModify {
    pub order_id: OrderId,
    pub side: Side,
    pub price: Price,
    pub quantity: Quantity,
}

impl OrderModify {
    pub fn new(order_id: OrderId, side: Side, price: Price, quantity: Quantity) -> Self {
        Self {
            order_id,
            side,
            price,
            quantity,
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

    pub fn quantity(&self) -> Quantity {
        self.quantity
    }

    pub fn modify(&self, order_type: OrderType) -> OrderPointer {
        create_order(Order::new(
            self.order_id, 
            self.side,
            order_type,
            self.price,
            self.quantity,
        ))
    }
}