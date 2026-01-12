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

pub type Price = i32;
pub type Quantity = u32;
pub type OrderId = u64;
pub type OrderIds = Vec<OrderId>;
pub const INVALID_PRICE: Price = i32::MIN;
