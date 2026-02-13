use std::fmt;
use std::fmt::Formatter;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Which side of the book an order sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    /// Buy side.
    Bid,
    /// Sell side.
    Ask,
}

/// Unique order identifier - wrapping UUID for type safety.
#[derive(Debug, Copy, Clone)]
pub struct OrderId(pub Uuid);

impl OrderId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for OrderId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A trading pair, e.g. SOL/USDT
pub struct TradingPair {
    pub base: String,
    pub quote: String,
}

impl TradingPair {
    pub fn new(base: impl Into<String>, quote: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            quote: quote.into(),
        }
    }
}

impl fmt::Display for TradingPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

/// Order type - we start with Limit only, Market comes later.
pub enum OrderType {
    Limit,
    Market,
}

/// Current lifecycle state of an order.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
}

/// The core order type submitted to the engine.
///
/// Uses `Decimal` for price/quantity to avoid floating-point
/// precision issues.
pub struct Order {
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

pub struct Fill {
    pub id: Uuid,
    /// The passive (resting) order that was sitting in the book.
    pub maker_order_id: OrderId,
    /// The aggressive (incoming) order that triggered the match.
    pub taker_order_id: OrderId,

    pub pair: TradingPair,

    /// Price at which the fill executed (maker's price)
    pub price: Decimal,

    /// Which side the taker was on.
    pub taker_side: Side,

    pub timestamp_ns: u64,
}

/// Result returned by the engine after processing an order.
pub struct MatchResult {
    pub order_id: OrderId,
    pub fills: Vec<Fill>,
    pub status: OrderStatus,
}