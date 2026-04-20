#[cfg(test)]
mod tests {
    use crate::commands::{Command, CommandOutput, ModifyOrderReject};
    use crate::engine::Engine;
    use crate::orderbook::order::Order;
    use crate::orderbook::order_modify::OrderModify;
    use crate::orderbook::order_type::OrderType;
    use crate::orderbook::side::Side;

    #[test]
    fn execute_place_order_accepts_command_metadata() {
        let mut engine = Engine::new();
        let order = Order::new(1, Side::Buy, OrderType::GoodTillCancel, 100, 10);

        let out = engine.execute(Command::PlaceOrder {
            instrument_id: 1,
            account_id: 42,
            request_id: 99,
            order,
        });

        match out {
            CommandOutput::PlaceOrder(Ok(success)) => {
                assert!(success.trades.is_empty());
                assert_eq!(engine.order_count(), 1);
            }
            _ => panic!("expected successful place order output"),
        }
    }

    #[test]
    fn execute_modify_rejects_side_change() {
        let mut engine = Engine::new();
        let order = Order::new(1, Side::Buy, OrderType::GoodTillCancel, 100, 10);
        let _ = engine.execute(Command::PlaceOrder {
            instrument_id: 1,
            account_id: 42,
            request_id: 100,
            order,
        });

        let out = engine.execute(Command::ModifyOrder {
            instrument_id: 1,
            account_id: 42,
            request_id: 101,
            modify: OrderModify::new(1, Side::Sell, 101, 10),
        });

        match out {
            CommandOutput::ModifyOrder(Err(ModifyOrderReject::SideChangeNotAllowed)) => {}
            _ => panic!("expected side-change rejection"),
        }
    }
}

