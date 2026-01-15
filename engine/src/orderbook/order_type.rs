#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Market,
    GoodTillCancel,
    FillAndKill,
    FillOrKill,
    PostOnly,
}