use std::collections::HashMap;

use crate::commands::{
    Command, CommandOutput, EngineError, InstrumentId,
};
use crate::orderbook::level_info::OrderbookLevelInfo;
use crate::orderbook::orderbook::Orderbook;
use crate::orderbook::types::Price;

pub struct Engine {
    books: HashMap<InstrumentId, Orderbook>,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            books: HashMap::new(),
        }
    }

    /// Register an instrument; returns true if a new book was created,
    /// false if the instrument was already registered (idempotent).
    pub fn register_instrument(&mut self, instrument_id: InstrumentId) -> bool {
        if self.books.contains_key(&instrument_id) {
            return false;
        }
        self.books.insert(instrument_id, Orderbook::new(instrument_id));
        true
    }

    pub fn is_registered(&self, instrument_id: InstrumentId) -> bool {
        self.books.contains_key(&instrument_id)
    }

    pub fn registered_instruments(&self) -> impl Iterator<Item = InstrumentId> + '_ {
        self.books.keys().copied()
    }

    pub fn execute(&mut self, cmd: Command) -> Result<CommandOutput, EngineError> {
        let instrument_id = cmd.instrument_id();
        let book = self
            .books
            .get_mut(&instrument_id)
            .ok_or(EngineError::UnknownInstrument(instrument_id))?;

        let output = match cmd {
            Command::PlaceOrder { order, .. } => CommandOutput::PlaceOrder(book.add_order(order)),
            Command::CancelOrder { order_id, .. } => {
                CommandOutput::CancelOrder(book.cancel_order(order_id))
            }
            Command::CancelOrders { order_ids, .. } => {
                CommandOutput::CancelOrders(book.cancel_orders(order_ids))
            }
            Command::ModifyOrder { modify, .. } => {
                CommandOutput::ModifyOrder(book.modify_order(modify))
            }
        };
        Ok(output)
    }

    pub fn get_orderbook_state(&self, instrument_id: InstrumentId) -> Option<OrderbookLevelInfo> {
        self.books.get(&instrument_id).map(|b| b.get_order_infos())
    }

    pub fn best_bid(&self, instrument_id: InstrumentId) -> Option<Price> {
        self.books.get(&instrument_id).and_then(|b| b.best_bid())
    }

    pub fn best_ask(&self, instrument_id: InstrumentId) -> Option<Price> {
        self.books.get(&instrument_id).and_then(|b| b.best_ask())
    }

    pub fn order_count(&self, instrument_id: InstrumentId) -> Option<usize> {
        self.books.get(&instrument_id).map(|b| b.size())
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
