#[cfg(test)]
mod tests {
    use crate::commands::{Command, CommandOutput, EngineError, ModifyOrderReject};
    use crate::engine::Engine;
    use crate::orderbook::order::Order;
    use crate::orderbook::order_modify::OrderModify;
    use crate::orderbook::order_type::OrderType;
    use crate::orderbook::side::Side;

    #[test]
    fn execute_place_order_accepts_command_metadata() {
        let mut engine = Engine::new();
        assert!(engine.register_instrument(1));
        let order = Order::new(1, Side::Buy, OrderType::GoodTillCancel, 100, 10);

        let out = engine
            .execute(Command::PlaceOrder {
                instrument_id: 1,
                account_id: 42,
                request_id: 99,
                order,
            })
            .expect("registered instrument should route");

        match out {
            CommandOutput::PlaceOrder(Ok(success)) => {
                assert!(success.trades.is_empty());
                assert_eq!(engine.order_count(1), Some(1));
            }
            _ => panic!("expected successful place order output"),
        }
    }

    #[test]
    fn execute_modify_rejects_side_change() {
        let mut engine = Engine::new();
        engine.register_instrument(1);
        let order = Order::new(1, Side::Buy, OrderType::GoodTillCancel, 100, 10);
        let _ = engine.execute(Command::PlaceOrder {
            instrument_id: 1,
            account_id: 42,
            request_id: 100,
            order,
        });

        let out = engine
            .execute(Command::ModifyOrder {
                instrument_id: 1,
                account_id: 42,
                request_id: 101,
                modify: OrderModify::new(1, Side::Sell, 101, 10),
            })
            .expect("registered instrument should route");

        match out {
            CommandOutput::ModifyOrder(Err(ModifyOrderReject::SideChangeNotAllowed)) => {}
            _ => panic!("expected side-change rejection"),
        }
    }

    #[test]
    fn execute_rejects_unknown_instrument() {
        let mut engine = Engine::new();
        engine.register_instrument(1);

        let order = Order::new(1, Side::Buy, OrderType::GoodTillCancel, 100, 10);
        let err = engine
            .execute(Command::PlaceOrder {
                instrument_id: 9,
                account_id: 42,
                request_id: 1,
                order,
            })
            .expect_err("unknown instrument must be rejected");

        assert_eq!(err, EngineError::UnknownInstrument(9));
        assert_eq!(engine.order_count(9), None);
    }

    #[test]
    fn register_instrument_is_idempotent() {
        let mut engine = Engine::new();
        assert!(engine.register_instrument(7));
        assert!(!engine.register_instrument(7));
        assert!(engine.is_registered(7));
    }

    #[test]
    fn instruments_are_isolated() {
        let mut engine = Engine::new();
        engine.register_instrument(1);
        engine.register_instrument(2);

        let buy = Order::new(1, Side::Buy, OrderType::GoodTillCancel, 100, 10);
        let sell = Order::new(2, Side::Sell, OrderType::GoodTillCancel, 100, 10);

        engine
            .execute(Command::PlaceOrder {
                instrument_id: 1,
                account_id: 1,
                request_id: 1,
                order: buy,
            })
            .unwrap();
        engine
            .execute(Command::PlaceOrder {
                instrument_id: 2,
                account_id: 1,
                request_id: 2,
                order: sell,
            })
            .unwrap();

        // No cross-instrument match should have occurred.
        assert_eq!(engine.order_count(1), Some(1));
        assert_eq!(engine.order_count(2), Some(1));
        assert_eq!(engine.best_bid(1), Some(100));
        assert_eq!(engine.best_ask(1), None);
        assert_eq!(engine.best_bid(2), None);
        assert_eq!(engine.best_ask(2), Some(100));
    }
}
