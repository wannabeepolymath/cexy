use crate::orderbook::orderbook::Orderbook;
use crate::orderbook::order::Order;
use crate::orderbook::order_type::OrderType;
use crate::orderbook::side::Side;
use crate::orderbook::types::{OrderId, Price, Quantity};
use crate::orderbook::order_modify::OrderModify;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_limit_order(id: OrderId, side: Side, price: Price, quantity: Quantity) -> Order {
        Order::new(id, side, OrderType::GoodTillCancel, price, quantity)
    }

    fn make_fak_order(id: OrderId, side: Side, price: Price, quantity: Quantity) -> Order {
        Order::new(id, side, OrderType::FillAndKill, price, quantity)
    }

    fn make_fok_order(id: OrderId, side: Side, price: Price, quantity: Quantity) -> Order {
        Order::new(id, side, OrderType::FillOrKill, price, quantity)
    }

    fn make_market_order(id: OrderId, side: Side, quantity: Quantity) -> Order {
        Order::market(id, side, OrderType::Market, quantity)
    }

    #[test]
    fn new_orderbook_is_empty() {
        let ob = Orderbook::new();
        assert_eq!(ob.size(), 0);
        assert!(ob.best_bid().is_none());
        assert!(ob.best_ask().is_none());
    }

    #[test]
    fn default_orderbook_is_empty() {
        let ob = Orderbook::default();
        assert_eq!(ob.size(), 0);
    }

    #[test]
    fn add_single_bid() {
        let mut ob = Orderbook::new();
        let order = make_limit_order(1, Side::Buy, 100, 10);
        
        let trades = ob.add_order(order);
        
        assert!(trades.is_empty());
        assert_eq!(ob.size(), 1);
        assert_eq!(ob.best_bid(), Some(100));
        assert!(ob.best_ask().is_none());
    }

    #[test]
    fn add_single_ask() {
        let mut ob = Orderbook::new();
        let order = make_limit_order(1, Side::Sell, 100, 10);
        
        let trades = ob.add_order(order);
        
        assert!(trades.is_empty());
        assert_eq!(ob.size(), 1);
        assert!(ob.best_bid().is_none());
        assert_eq!(ob.best_ask(), Some(100));
    }

    #[test]
    fn add_multiple_bids_best_bid_is_highest() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Buy, 100, 10));
        ob.add_order(make_limit_order(2, Side::Buy, 105, 10));
        ob.add_order(make_limit_order(3, Side::Buy, 95, 10));
        
        assert_eq!(ob.size(), 3);
        assert_eq!(ob.best_bid(), Some(105));
    }

    #[test]
    fn add_multiple_asks_best_ask_is_lowest() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 10));
        ob.add_order(make_limit_order(2, Side::Sell, 105, 10));
        ob.add_order(make_limit_order(3, Side::Sell, 95, 10));
        
        assert_eq!(ob.size(), 3);
        assert_eq!(ob.best_ask(), Some(95));
    }

    #[test]
    fn duplicate_order_id_rejected() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Buy, 100, 10));
        let trades = ob.add_order(make_limit_order(1, Side::Buy, 105, 20));
        
        assert!(trades.is_empty());
        assert_eq!(ob.size(), 1);
        assert_eq!(ob.best_bid(), Some(100)); // original order remains
    }

    #[test]
    fn can_match_buy_against_ask() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 10));
        
        assert!(ob.can_match(Side::Buy, 100));  // equal price
        assert!(ob.can_match(Side::Buy, 105));  // higher price
        assert!(!ob.can_match(Side::Buy, 95));  // lower price
    }

    #[test]
    fn can_match_sell_against_bid() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Buy, 100, 10));
        
        assert!(ob.can_match(Side::Sell, 100)); // equal price
        assert!(ob.can_match(Side::Sell, 95));  // lower price
        assert!(!ob.can_match(Side::Sell, 105)); // higher price
    }

    #[test]
    fn can_match_empty_book_returns_false() {
        let ob = Orderbook::new();
        assert!(!ob.can_match(Side::Buy, 100));
        assert!(!ob.can_match(Side::Sell, 100));
    }

    #[test]
    fn simple_match_full_fill() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 10));
        
        let trades = ob.add_order(make_limit_order(2, Side::Buy, 100, 10));
        
        assert_eq!(trades.len(), 1);
        assert_eq!(ob.size(), 0); // both orders fully filled
        assert!(ob.best_bid().is_none());
        assert!(ob.best_ask().is_none());
    }

    #[test]
    fn partial_fill_bid_remaining() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 5));
        
        let trades = ob.add_order(make_limit_order(2, Side::Buy, 100, 10));
        
        assert_eq!(trades.len(), 1);
        assert_eq!(ob.size(), 1); // bid partially filled, remains
        assert_eq!(ob.best_bid(), Some(100));
        assert!(ob.best_ask().is_none());
    }

    #[test]
    fn partial_fill_ask_remaining() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 10));
        
        let trades = ob.add_order(make_limit_order(2, Side::Buy, 100, 5));
        
        assert_eq!(trades.len(), 1);
        assert_eq!(ob.size(), 1); // ask partially filled, remains
        assert!(ob.best_bid().is_none());
        assert_eq!(ob.best_ask(), Some(100));
    }

    #[test]
    fn match_multiple_orders_at_same_level() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 5));
        ob.add_order(make_limit_order(2, Side::Sell, 100, 5));
        
        let trades = ob.add_order(make_limit_order(3, Side::Buy, 100, 10));
        
        assert_eq!(trades.len(), 2); // matched both asks
        assert_eq!(ob.size(), 0);
    }

    #[test]
    fn match_across_price_levels() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 5));
        ob.add_order(make_limit_order(2, Side::Sell, 101, 5));
        
        // Aggressive buy that sweeps both levels
        let trades = ob.add_order(make_limit_order(3, Side::Buy, 101, 10));
        
        assert_eq!(trades.len(), 2);
        assert_eq!(ob.size(), 0);
    }

    #[test]
    fn cancel_order_removes_from_book() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Buy, 100, 10));
        assert_eq!(ob.size(), 1);
        
        ob.cancel_order(1);
        
        assert_eq!(ob.size(), 0);
        assert!(ob.best_bid().is_none());
    }

    #[test]
    fn cancel_nonexistent_order_is_noop() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Buy, 100, 10));
        
        ob.cancel_order(999); // doesn't exist
        
        assert_eq!(ob.size(), 1); // unchanged
    }

    #[test]
    fn cancel_order_cleans_up_empty_level() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Buy, 100, 10));
        ob.cancel_order(1);
        
        // Add a different order - level should be clean
        ob.add_order(make_limit_order(2, Side::Buy, 100, 5));
        assert_eq!(ob.size(), 1);
        assert_eq!(ob.best_bid(), Some(100));
    }

    #[test]
    fn modify_order_changes_price_and_quantity() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Buy, 100, 10));
        
        let modify = OrderModify::new(1, Side::Buy, 105, 20);
        let trades = ob.modify_order(modify);
        
        assert!(trades.is_empty());
        assert_eq!(ob.size(), 1);
        assert_eq!(ob.best_bid(), Some(105));
    }

    #[test]
    fn modify_order_can_trigger_match() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 10));
        ob.add_order(make_limit_order(2, Side::Buy, 95, 10));
        
        // Modify bid to cross the spread
        let modify = OrderModify::new(2, Side::Buy, 100, 10);
        let trades = ob.modify_order(modify);
        
        assert_eq!(trades.len(), 1);
        assert_eq!(ob.size(), 0);
    }

    #[test]
    fn modify_nonexistent_order_is_noop() {
        let mut ob = Orderbook::new();
        let modify = OrderModify::new(999, Side::Buy, 100, 10);
        let trades = ob.modify_order(modify);
        
        assert!(trades.is_empty());
        assert_eq!(ob.size(), 0);
    }

    #[test]
    fn fill_and_kill_rejected_when_no_match() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 10));
        
        // FAK buy at 95 can't match ask at 100
        let trades = ob.add_order(make_fak_order(2, Side::Buy, 95, 10));
        
        assert!(trades.is_empty());
        assert_eq!(ob.size(), 1); // only the original ask
    }

    #[test]
    fn fill_and_kill_accepted_when_can_match() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 10));
        
        // FAK buy at 100 can match
        let trades = ob.add_order(make_fak_order(2, Side::Buy, 100, 5));
        
        assert_eq!(trades.len(), 1);
    }

    #[test]
    fn fill_and_kill_does_not_rest() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 5));

        let trades = ob.add_order(make_fak_order(2, Side::Buy, 100, 10));

        assert_eq!(trades.len(), 1);
        assert_eq!(ob.size(), 0);
        assert!(ob.best_bid().is_none());
        assert!(ob.best_ask().is_none());
    }

    #[test]
    fn fill_or_kill_rejected_when_insufficient_liquidity() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 5));

        let trades = ob.add_order(make_fok_order(2, Side::Buy, 100, 10));

        assert!(trades.is_empty());
        assert_eq!(ob.size(), 1);
        assert_eq!(ob.best_ask(), Some(100));
    }

    #[test]
    fn fill_or_kill_fills_when_enough_liquidity() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 5));
        ob.add_order(make_limit_order(2, Side::Sell, 101, 5));

        let trades = ob.add_order(make_fok_order(3, Side::Buy, 101, 10));

        assert_eq!(trades.len(), 2);
        assert_eq!(ob.size(), 0);
    }

    #[test]
    fn post_only_rejected_if_would_cross() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 10));

        let post_only = Order::new(2, Side::Buy, OrderType::PostOnly, 100, 5);
        let trades = ob.add_order(post_only);

        assert!(trades.is_empty());
        assert_eq!(ob.size(), 1);
        assert_eq!(ob.best_ask(), Some(100));
    }

    #[test]
    fn post_only_accepted_if_not_crossing() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 10));

        let post_only = Order::new(2, Side::Buy, OrderType::PostOnly, 99, 5);
        let trades = ob.add_order(post_only);

        assert!(trades.is_empty());
        assert_eq!(ob.size(), 2);
        assert_eq!(ob.best_bid(), Some(99));
        assert_eq!(ob.best_ask(), Some(100));
    }

    #[test]
    fn market_order_does_not_rest() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 5));

        let trades = ob.add_order(make_market_order(2, Side::Buy, 10));

        assert_eq!(trades.len(), 1);
        assert_eq!(ob.size(), 0);
        assert!(ob.best_bid().is_none());
        assert!(ob.best_ask().is_none());
    }

    #[test]
    fn get_order_infos_empty_book() {
        let ob = Orderbook::new();
        let infos = ob.get_order_infos();
        
        assert!(infos.get_bids().is_empty());
        assert!(infos.get_asks().is_empty());
    }

    #[test]
    fn get_order_infos_with_orders() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Buy, 100, 10));
        ob.add_order(make_limit_order(2, Side::Buy, 100, 5));  // same level
        ob.add_order(make_limit_order(3, Side::Buy, 95, 20));
        ob.add_order(make_limit_order(4, Side::Sell, 105, 15));
        ob.add_order(make_limit_order(5, Side::Sell, 110, 25));
        
        let infos = ob.get_order_infos();
        
        // Bids: highest first
        assert_eq!(infos.get_bids().len(), 2);
        assert_eq!(infos.get_bids()[0].price, 100);
        assert_eq!(infos.get_bids()[0].quantity, 15); // 10 + 5 aggregated
        assert_eq!(infos.get_bids()[1].price, 95);
        assert_eq!(infos.get_bids()[1].quantity, 20);
        
        // Asks: lowest first
        assert_eq!(infos.get_asks().len(), 2);
        assert_eq!(infos.get_asks()[0].price, 105);
        assert_eq!(infos.get_asks()[0].quantity, 15);
        assert_eq!(infos.get_asks()[1].price, 110);
        assert_eq!(infos.get_asks()[1].quantity, 25);
    }

    #[test]
    fn fifo_order_matching() {
        let mut ob = Orderbook::new();
        // Add two asks at same price - order 1 should be matched first
        ob.add_order(make_limit_order(1, Side::Sell, 100, 10));
        ob.add_order(make_limit_order(2, Side::Sell, 100, 10));
        
        let trades = ob.add_order(make_limit_order(3, Side::Buy, 100, 10));
        
        assert_eq!(trades.len(), 1);
        // Order 1 should be fully filled, order 2 remains
        assert_eq!(ob.size(), 1);
        
        // Verify order 2 is still there by checking level info
        let infos = ob.get_order_infos();
        assert_eq!(infos.get_asks().len(), 1);
        assert_eq!(infos.get_asks()[0].quantity, 10);
    }

    #[test]
    fn aggressive_buy_sweeps_multiple_levels() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Sell, 100, 5));
        ob.add_order(make_limit_order(2, Side::Sell, 101, 5));
        ob.add_order(make_limit_order(3, Side::Sell, 102, 5));
        
        // Buy enough to consume first two levels, partial third
        let trades = ob.add_order(make_limit_order(4, Side::Buy, 102, 12));
        
        assert_eq!(trades.len(), 3);
        assert_eq!(ob.size(), 1); // partial ask at 102 remains
        assert_eq!(ob.best_ask(), Some(102));
    }

    #[test]
    fn aggressive_sell_sweeps_multiple_levels() {
        let mut ob = Orderbook::new();
        ob.add_order(make_limit_order(1, Side::Buy, 102, 5));
        ob.add_order(make_limit_order(2, Side::Buy, 101, 5));
        ob.add_order(make_limit_order(3, Side::Buy, 100, 5));
        
        // Sell enough to consume first two levels, partial third
        let trades = ob.add_order(make_limit_order(4, Side::Sell, 100, 12));
        
        assert_eq!(trades.len(), 3);
        assert_eq!(ob.size(), 1); // partial bid at 100 remains
        assert_eq!(ob.best_bid(), Some(100));
    }
}