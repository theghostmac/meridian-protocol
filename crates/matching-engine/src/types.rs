use rust_decimal::Decimal;
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

impl OrderId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

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

pub enum  OrderStatus {
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
}

struct Order {
    pub id: OrderId,
    pub pair: TradingPair,
    pub side: Side,
    pub order_type: OrderType,
    /// Limit price. None for market orders.
    pub price: Option<Decimal>,
    /// Original quantity submitted.
    pub quantity: Decimal,
    /// Remaining unfilled quantity.
    pub quantity_remaining: Decimal,
    pub status: OrderStatus,

    /// Unix timestamp in nanoseconds - used for time priority.
    pub timestamp_ns: u64,

    /// The trader's on-chain address (EVM or Solana).
    pub trader: String,
}

struct Fill {

}

struct OrderBook {
    pub orders: BTreeMap<Order, Side>,
}