use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::Formatter;
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

impl fmt::Display for Side {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Side::Bid => write!(f, "bid"),
            Side::Ask => write!(f, "ask"),
        }
    }
}

/// Unique order identifier - wrapping UUID for type safety.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Order {
    /// Returns true if the order still has quantity to be filled and isn't cancelled.
    pub fn is_active(&self) -> bool {
        self.status == OrderStatus::Open || self.status == OrderStatus::PartiallyFilled
    }

    /// Updates the order state after a fill.
    pub fn fill(&mut self, qty: Decimal) {
        self.quantity_remaining -= qty;

        if self.quantity_remaining <= Decimal::ZERO {
            self.status = OrderStatus::Filled;
            self.quantity_remaining = Decimal::ZERO;
        } else {
            self.status = OrderStatus::PartiallyFilled;
        }
    }
}

/// A single matched fill - produced by the engine when two orders cross.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub id: Uuid,
    /// The passive (resting) order that was sitting in the book.
    pub maker_order_id: OrderId,
    /// The aggressive (incoming) order that triggered the match.
    pub taker_order_id: OrderId,

    pub pair: TradingPair,

    /// Price at which the fill executed (maker's price)
    pub price: Decimal,

    /// Quantity that was exchanged.
    pub quantity: Decimal,

    /// Which side the taker was on.
    pub taker_side: Side,

    pub timestamp_ns: u64,
}

impl Fill {
    pub fn new(
        maker_order_id: OrderId,
        taker_order_id: OrderId,
        pair: TradingPair,
        price: Decimal,
        quantity: Decimal,
        taker_side: Side,
        timestamp_ns: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            maker_order_id,
            taker_order_id,
            pair,
            price,
            quantity,
            taker_side,
            timestamp_ns,
        }
    }

    /// Notional value of this fill (price * quantity).
    pub fn notional(&self) -> Decimal {
        self.price * self.quantity
    }
}

/// Result returned by the engine after processing an order.
pub struct MatchResult {
    pub order_id: OrderId,
    pub fills: Vec<Fill>,
    pub status: OrderStatus,
}

/// A single price level's aggregated data for market depth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelSnapshot {
    pub price: Decimal,
    pub quantity: Decimal,
}

/// A snapshot of the order book's liquidity at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookSnapshot {
    pub bids: Vec<LevelSnapshot>,
    pub asks: Vec<LevelSnapshot>,
    pub timestamp_ns: u64,
}