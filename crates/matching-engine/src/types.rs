use std::collections::BTreeMap;
use uuid::Uuid;

/// Which side of the book an order sits on.
enum Side {
    /// Buy side.
    Bid,
    /// Sell side.
    Ask,
}

/// Unique order identifier - wrapping UUID for type safety.
pub struct OrderId(pub Uuid);

/// A trading pair, e.g. SOL/USDT
pub struct TradingPair {
    pub base: String,
    pub quote: String,
}

/// Order type - we start with Limit only, Market comes later.
pub enum OrderType {
    Limit,
    Market,
}

struct Order {
    pub id: OrderId,
    pub pair: TradingPair,
    pub side: Side,
    pub order_type: OrderType,
}

struct Fill {

}

struct OrderBook {
    pub orders: BTreeMap<Order, Side>,
}