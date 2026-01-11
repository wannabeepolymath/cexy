// use std::{collections::BTreeMap, marker};

// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
// pub struct OrderId(pub String);






// // #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// // pub enum OrderStatus {
// //     Open,
// //     PartiallyFilled,
// //     Filled,
// //     Cancelled,
// // }

// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
// pub struct Asset(pub String);

// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
// pub struct AssetPair {
//     pub base: Asset,   // SOL
//     pub quote: Asset,  // USDC
// }

//! Type aliases used throughout the orderbook implementation.

/// Price type - signed 32-bit integer to match C++ implementation
pub type Price = i32;

/// Quantity type - unsigned 32-bit integer
pub type Quantity = u32;

/// Order ID type - unsigned 64-bit integer
pub type OrderId = u64;

/// Collection of order IDs
pub type OrderIds = Vec<OrderId>;

/// Invalid price constant (using i32::MIN as a sentinel value since Rust doesn't have NaN for integers)
pub const INVALID_PRICE: Price = i32::MIN;

