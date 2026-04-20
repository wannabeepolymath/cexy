use engine::orderbook::order_type::OrderType;
use engine::orderbook::side::Side;

pub fn parse_side(input: &str) -> Option<Side> {
    match input.trim().to_lowercase().as_str() {
        "buy" => Some(Side::Buy),
        "sell" => Some(Side::Sell),
        _ => None,
    }
}

pub fn parse_order_type(input: &str) -> Option<OrderType> {
    match input.trim().to_lowercase().as_str() {
        "market" => Some(OrderType::Market),
        "limit" | "gtc" | "goodtillcancel" | "good_till_cancel" => Some(OrderType::GoodTillCancel),
        "fak" | "ioc" | "fillandkill" | "fill_and_kill" => Some(OrderType::FillAndKill),
        "fok" | "fillorkill" | "fill_or_kill" => Some(OrderType::FillOrKill),
        "postonly" | "post_only" | "post-only" => Some(OrderType::PostOnly),
        _ => None,
    }
}

pub fn validate_request_identity(
    instrument_id: u32,
    account_id: u64,
    request_id: u64,
) -> Option<&'static str> {
    if instrument_id == 0 {
        return Some("instrument_id must be greater than 0");
    }
    if account_id == 0 {
        return Some("account_id must be greater than 0");
    }
    if request_id == 0 {
        return Some("request_id must be greater than 0");
    }
    None
}
