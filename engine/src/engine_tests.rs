#[cfg(test)]
mod tests {
    use crate::commands::{Command, CommandOutput, EngineError, ModifyOrderReject};
    use crate::engine::Engine;
    use crate::events::{Event, Events, RejectReason};
    use crate::orderbook::order::Order;
    use crate::orderbook::order_modify::OrderModify;
    use crate::orderbook::order_type::OrderType;
    use crate::orderbook::side::Side;
    use crate::orderbook::types::OrderId;

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

        match out.output {
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

        match out.output {
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

    // -- Event emission tests -------------------------------------------------

    fn place_limit(
        engine: &mut Engine,
        instrument_id: u32,
        order_id: OrderId,
        side: Side,
        order_type: OrderType,
        price: i32,
        quantity: u32,
        request_id: u64,
    ) -> Events {
        engine
            .execute(Command::PlaceOrder {
                instrument_id,
                account_id: 1,
                request_id,
                order: Order::new(order_id, side, order_type, price, quantity),
            })
            .expect("registered instrument should route")
            .events
    }

    fn cancel(
        engine: &mut Engine,
        instrument_id: u32,
        order_id: OrderId,
        request_id: u64,
    ) -> Events {
        engine
            .execute(Command::CancelOrder {
                instrument_id,
                account_id: 1,
                request_id,
                order_id,
            })
            .expect("registered instrument should route")
            .events
    }

    fn cancel_many(
        engine: &mut Engine,
        instrument_id: u32,
        order_ids: Vec<OrderId>,
        request_id: u64,
    ) -> Events {
        engine
            .execute(Command::CancelOrders {
                instrument_id,
                account_id: 1,
                request_id,
                order_ids,
            })
            .expect("registered instrument should route")
            .events
    }

    fn modify(
        engine: &mut Engine,
        instrument_id: u32,
        order_id: OrderId,
        side: Side,
        price: i32,
        quantity: u32,
        request_id: u64,
    ) -> Events {
        engine
            .execute(Command::ModifyOrder {
                instrument_id,
                account_id: 1,
                request_id,
                modify: OrderModify::new(order_id, side, price, quantity),
            })
            .expect("registered instrument should route")
            .events
    }

    /// Assert every event belongs to `instrument_id` and that their `seq`
    /// values are strictly increasing, starting at `expected_first_seq`.
    fn assert_seq_contiguous(events: &[Event], instrument_id: u32, expected_first_seq: u64) {
        for (offset, ev) in events.iter().enumerate() {
            assert_eq!(ev.instrument_id(), instrument_id, "event {ev:?}");
            assert_eq!(
                ev.seq(),
                expected_first_seq + offset as u64,
                "event {ev:?} has unexpected seq"
            );
        }
    }

    #[test]
    fn place_order_emits_accepted_then_top_of_book_update() {
        let mut engine = Engine::new();
        engine.register_instrument(1);

        let events = place_limit(&mut engine, 1, 1, Side::Buy, OrderType::GoodTillCancel, 100, 10, 1);

        assert_eq!(events.len(), 2, "expected OrderAccepted + TopOfBookUpdated");
        assert_seq_contiguous(&events, 1, 0);

        match &events[0] {
            Event::OrderAccepted {
                order_id,
                side,
                price,
                quantity,
                ..
            } => {
                assert_eq!(*order_id, 1);
                assert_eq!(*side, Side::Buy);
                assert_eq!(*price, 100);
                assert_eq!(*quantity, 10);
            }
            other => panic!("expected OrderAccepted, got {other:?}"),
        }
        match &events[1] {
            Event::TopOfBookUpdated {
                best_bid, best_ask, ..
            } => {
                assert_eq!(*best_bid, Some(100));
                assert_eq!(*best_ask, None);
            }
            other => panic!("expected TopOfBookUpdated, got {other:?}"),
        }
    }

    #[test]
    fn place_order_rejected_emits_order_rejected_only() {
        let mut engine = Engine::new();
        engine.register_instrument(1);
        // Seed an order so the follow-up duplicate rejects.
        place_limit(&mut engine, 1, 1, Side::Buy, OrderType::GoodTillCancel, 100, 10, 1);

        let events = place_limit(&mut engine, 1, 1, Side::Buy, OrderType::GoodTillCancel, 100, 10, 2);

        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::OrderRejected {
                order_id, reason, ..
            } => {
                assert_eq!(*order_id, 1);
                assert_eq!(*reason, RejectReason::DuplicateOrderId);
            }
            other => panic!("expected OrderRejected, got {other:?}"),
        }
    }

    #[test]
    fn crossing_order_emits_trade_execution_events() {
        let mut engine = Engine::new();
        engine.register_instrument(1);
        // Resting ask at 100 qty 10.
        place_limit(&mut engine, 1, 1, Side::Sell, OrderType::GoodTillCancel, 100, 10, 1);
        // Crossing buy at 100 qty 10 -> full match.
        let events = place_limit(&mut engine, 1, 2, Side::Buy, OrderType::GoodTillCancel, 100, 10, 2);

        assert_eq!(
            events.len(),
            3,
            "expected OrderAccepted + TradeExecuted + TopOfBookUpdated, got {events:?}"
        );
        assert!(matches!(events[0], Event::OrderAccepted { order_id: 2, .. }));
        match &events[1] {
            Event::TradeExecuted { trade, .. } => {
                assert_eq!(trade.price(), 100);
                assert_eq!(trade.quantity(), 10);
                assert_eq!(trade.maker_order_id(), 1);
                assert_eq!(trade.taker_order_id(), 2);
                assert_eq!(trade.maker_side(), Side::Sell);
                assert_eq!(trade.instrument_id(), 1);
            }
            other => panic!("expected TradeExecuted, got {other:?}"),
        }
        match &events[2] {
            Event::TopOfBookUpdated { best_bid, best_ask, .. } => {
                // Both sides of the book are empty after the full match.
                assert_eq!(*best_bid, None);
                assert_eq!(*best_ask, None);
            }
            other => panic!("expected TopOfBookUpdated, got {other:?}"),
        }
    }

    #[test]
    fn ioc_residual_emits_cancel_after_partial_fill() {
        let mut engine = Engine::new();
        engine.register_instrument(1);
        // Resting ask qty 5.
        place_limit(&mut engine, 1, 1, Side::Sell, OrderType::GoodTillCancel, 100, 5, 1);
        // FAK buy qty 10 crosses, partially fills, residual is auto-cancelled.
        let events = place_limit(&mut engine, 1, 2, Side::Buy, OrderType::FillAndKill, 100, 10, 2);

        assert_eq!(
            events.len(),
            4,
            "expected Accepted + Trade + Canceled + TopOfBook, got {events:?}"
        );
        assert!(matches!(events[0], Event::OrderAccepted { order_id: 2, .. }));
        assert!(matches!(events[1], Event::TradeExecuted { .. }));
        match &events[2] {
            Event::OrderCanceled {
                order_id,
                remaining_quantity,
                ..
            } => {
                assert_eq!(*order_id, 2);
                assert_eq!(*remaining_quantity, 5);
            }
            other => panic!("expected OrderCanceled, got {other:?}"),
        }
        assert!(matches!(events[3], Event::TopOfBookUpdated { .. }));
    }

    #[test]
    fn cancel_order_emits_cancel_then_top_of_book() {
        let mut engine = Engine::new();
        engine.register_instrument(1);
        place_limit(&mut engine, 1, 1, Side::Buy, OrderType::GoodTillCancel, 100, 10, 1);

        let events = cancel(&mut engine, 1, 1, 2);

        assert_eq!(events.len(), 2);
        match &events[0] {
            Event::OrderCanceled {
                order_id,
                remaining_quantity,
                ..
            } => {
                assert_eq!(*order_id, 1);
                assert_eq!(*remaining_quantity, 10);
            }
            other => panic!("expected OrderCanceled, got {other:?}"),
        }
        match &events[1] {
            Event::TopOfBookUpdated { best_bid, best_ask, .. } => {
                assert_eq!(*best_bid, None);
                assert_eq!(*best_ask, None);
            }
            other => panic!("expected TopOfBookUpdated, got {other:?}"),
        }
    }

    #[test]
    fn cancel_unknown_order_emits_no_events() {
        let mut engine = Engine::new();
        engine.register_instrument(1);

        let events = cancel(&mut engine, 1, 42, 1);
        assert!(events.is_empty(), "expected no events, got {events:?}");
    }

    #[test]
    fn cancel_orders_emits_one_cancel_per_order_and_single_top_of_book() {
        let mut engine = Engine::new();
        engine.register_instrument(1);
        place_limit(&mut engine, 1, 1, Side::Buy, OrderType::GoodTillCancel, 100, 10, 1);
        place_limit(&mut engine, 1, 2, Side::Buy, OrderType::GoodTillCancel, 99, 10, 2);
        place_limit(&mut engine, 1, 3, Side::Buy, OrderType::GoodTillCancel, 98, 10, 3);

        // Cancel two existing orders + one unknown id; the unknown id must not
        // produce any event but the batch should still emit a single TOB update.
        let events = cancel_many(&mut engine, 1, vec![1, 2, 999], 4);

        let canceled: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::OrderCanceled { .. }))
            .collect();
        assert_eq!(canceled.len(), 2);

        let tob_updates: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::TopOfBookUpdated { .. }))
            .collect();
        assert_eq!(tob_updates.len(), 1, "expected exactly one TopOfBookUpdated");

        match tob_updates[0] {
            Event::TopOfBookUpdated { best_bid, best_ask, .. } => {
                assert_eq!(*best_bid, Some(98));
                assert_eq!(*best_ask, None);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn modify_unknown_emits_order_rejected() {
        let mut engine = Engine::new();
        engine.register_instrument(1);

        let events = modify(&mut engine, 1, 42, Side::Buy, 100, 10, 1);
        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::OrderRejected { order_id, reason, .. } => {
                assert_eq!(*order_id, 42);
                assert_eq!(*reason, RejectReason::OrderNotFound);
            }
            other => panic!("expected OrderRejected, got {other:?}"),
        }
    }

    #[test]
    fn modify_with_side_change_emits_order_rejected() {
        let mut engine = Engine::new();
        engine.register_instrument(1);
        place_limit(&mut engine, 1, 1, Side::Buy, OrderType::GoodTillCancel, 100, 10, 1);

        let events = modify(&mut engine, 1, 1, Side::Sell, 100, 10, 2);
        assert_eq!(events.len(), 1);
        match &events[0] {
            Event::OrderRejected { order_id, reason, .. } => {
                assert_eq!(*order_id, 1);
                assert_eq!(*reason, RejectReason::SideChangeNotAllowed);
            }
            other => panic!("expected OrderRejected, got {other:?}"),
        }
    }

    #[test]
    fn modify_emits_cancel_then_accept_then_top_of_book() {
        let mut engine = Engine::new();
        engine.register_instrument(1);
        place_limit(&mut engine, 1, 1, Side::Buy, OrderType::GoodTillCancel, 100, 10, 1);

        let events = modify(&mut engine, 1, 1, Side::Buy, 105, 15, 2);

        assert_eq!(
            events.len(),
            3,
            "expected Canceled + Accepted + TopOfBookUpdated, got {events:?}"
        );
        match &events[0] {
            Event::OrderCanceled { order_id, remaining_quantity, .. } => {
                assert_eq!(*order_id, 1);
                assert_eq!(*remaining_quantity, 10);
            }
            other => panic!("expected OrderCanceled, got {other:?}"),
        }
        match &events[1] {
            Event::OrderAccepted {
                order_id, side, price, quantity, ..
            } => {
                assert_eq!(*order_id, 1);
                assert_eq!(*side, Side::Buy);
                assert_eq!(*price, 105);
                assert_eq!(*quantity, 15);
            }
            other => panic!("expected OrderAccepted, got {other:?}"),
        }
        match &events[2] {
            Event::TopOfBookUpdated { best_bid, .. } => {
                assert_eq!(*best_bid, Some(105));
            }
            other => panic!("expected TopOfBookUpdated, got {other:?}"),
        }
    }

    #[test]
    fn event_seq_is_monotonic_across_commands_per_instrument() {
        let mut engine = Engine::new();
        engine.register_instrument(1);

        let mut all = Vec::new();
        all.extend(place_limit(
            &mut engine, 1, 1, Side::Buy, OrderType::GoodTillCancel, 100, 10, 1,
        ));
        all.extend(place_limit(
            &mut engine, 1, 2, Side::Sell, OrderType::GoodTillCancel, 101, 10, 2,
        ));
        all.extend(cancel(&mut engine, 1, 1, 3));
        all.extend(modify(&mut engine, 1, 2, Side::Sell, 102, 5, 4));

        // Seq must be 0, 1, 2, ... across the full stream on this instrument.
        assert_seq_contiguous(&all, 1, 0);
    }

    #[test]
    fn event_seq_is_independent_across_instruments() {
        let mut engine = Engine::new();
        engine.register_instrument(1);
        engine.register_instrument(2);

        let a = place_limit(&mut engine, 1, 1, Side::Buy, OrderType::GoodTillCancel, 100, 10, 1);
        let b = place_limit(&mut engine, 2, 1, Side::Sell, OrderType::GoodTillCancel, 200, 10, 2);

        // Both books start their seq numbering at 0 independently.
        assert_seq_contiguous(&a, 1, 0);
        assert_seq_contiguous(&b, 2, 0);
    }
}
