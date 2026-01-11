#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Market,
    Limit,
    FillAndKill,
    FillOrKill,
    PostOnly,
}