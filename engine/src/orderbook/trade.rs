use crate::orderbook::side::Side;
use crate::orderbook::types::{OrderId, Price, Quantity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trade {
    price: Price,
    quantity: Quantity,
    maker_order_id: OrderId,
    taker_order_id: OrderId,
    maker_side: Side,
    instrument_id: u32,
    seq: u64,
}

impl Trade {
    pub fn new(
        price: Price,
        quantity: Quantity,
        maker_order_id: OrderId,
        taker_order_id: OrderId,
        maker_side: Side,
        instrument_id: u32,
        seq: u64,
    ) -> Self {
        Self {
            price,
            quantity,
            maker_order_id,
            taker_order_id,
            maker_side,
            instrument_id,
            seq,
        }
    }

    pub fn price(&self) -> Price {
        self.price
    }

    pub fn quantity(&self) -> Quantity {
        self.quantity
    }

    pub fn maker_order_id(&self) -> OrderId {
        self.maker_order_id
    }

    pub fn taker_order_id(&self) -> OrderId {
        self.taker_order_id
    }

    pub fn maker_side(&self) -> Side {
        self.maker_side
    }

    pub fn instrument_id(&self) -> u32 {
        self.instrument_id
    }

    pub fn seq(&self) -> u64 {
        self.seq
    }
}

pub type Trades = Vec<Trade>;