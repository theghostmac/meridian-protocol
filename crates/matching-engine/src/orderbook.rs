use crate::types::{Order, OrderId, Side};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, HashMap, VecDeque};

/// A price level in the book - holds all orders at a given price
/// in FIFO (time priority) order.
#[derive(Default)]
pub struct PriceLevel {
    pub orders: VecDeque<OrderId>,
    pub total_quantity: Decimal,
}

impl PriceLevel {
    pub fn add(&mut self, order_id: OrderId, quantity: Decimal) {
        self.orders.push_back(order_id);
        self.total_quantity += quantity;
    }

    pub fn remove_front(&mut self) -> Option<OrderId> {
        self.orders.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }
}

/// One side of the order book (bids or asks)
///
/// Bids: BTreeMap in reverse order (highest price = best bid).
/// Asks: BTreeMap in natural order (lowest price = best ask).
///
/// Using a single struct for both, the engine will control sort direction.
#[derive(Default)]
pub struct HalfBook {
    /// price -> price level (FIFO queue of order IDs)
    pub levels: BTreeMap<Decimal, PriceLevel>,
}

impl HalfBook {
    /// Insert an order into this half-book.
    pub fn insert(&mut self, order: &Order) {
        let price = order.price.expect("limit order must have price");
        let level = self.levels.entry(price).or_default();
        level.add(order.id, order.quantity_remaining);
    }

    /// Remove an order from a price level. CLeans up empty levels.
    pub fn remove(&mut self, price: Decimal, order_id: OrderId) {
        if let Some(level) = self.levels.get_mut(&price) {
            level.orders.retain(|id| *id != order_id);
            if level.is_empty() {
                self.levels.remove(&price);
            }
        }
    }

    /// Best bid = highest price in the book
    pub fn best_bid_price(&self) -> Option<Decimal> {
        self.levels.keys().next_back().copied()
    }

    /// Best ask - lowest price in the book.
    pub fn best_ask_price(&self) -> Option<Decimal> {
        self.levels.keys().next().copied()
    }

    /// Peek at the front order ID at a given price level.
    pub fn front_order_at(&self, price: &Decimal) -> Option<OrderId> {
        self.levels
            .get(price)
            .and_then(|l| l.orders.front().copied())
    }
}

/// The full order book for a single trading pair.
///
/// Maintains:
/// - Bids half-book (buy side)
/// - asks half-book (sell side)
/// - an order map for O(1) order lookup by ID
#[derive(Default)]
pub struct OrderBook {
    pub bids: HalfBook,
    pub asks: HalfBook,
    /// All live orders by ID for fast lookup during matching.
    pub orders: HashMap<OrderId, Order>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new resting order to the order book.
    pub fn add_order(&mut self, order: Order) {
        match order.side {
            Side::Bid => self.bids.insert(&order),
            Side::Ask => self.asks.insert(&order),
        }
        self.orders.insert(order.id, order);
    }

    /// Remove an order from the book (cancellation or post-fill cleanup).
    pub fn remove_order(&mut self, order_id: OrderId) -> Option<Order> {
        let order = self.orders.remove(&order_id)?;
        let price = order.price.expect("resting order must have a price");

        match order.side {
            Side::Bid => self.bids.remove(price, order_id),
            Side::Ask => self.asks.remove(price, order_id),
        }

        Some(order)
    }

    /// Get a mutable reference to an order (used for fill updates)
    pub fn get_order_mut(&mut self, order_id: &OrderId) -> Option<&mut Order> {
        self.orders.get_mut(order_id)
    }

    /// Best bid price currently in the book.
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.best_bid_price()
    }

    /// Best ask price currently in the book.
    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.best_ask_price()
    }

    /// The spread: best_ask - best_bid. None if either side isempty.
    pub fn spread(&self) -> Option<Decimal> {
        Some(self.best_ask()? - self.best_bid()?)
    }

    /// Mid price: (best_bid + best_ask) / 2.
    pub fn mid_price(&self) -> Option<Decimal> {
        Some((self.best_bid()? + self.best_ask()?) / Decimal::TWO)
    }

    /// Snapshot of the top N price levels for each side.
    /// Returns (bids, asks) as Vec<(price, total_qty)>.
    pub fn depth_snapshot(
        &self,
        levels: usize,
    ) -> (Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>) {
        let bids: Vec<(Decimal, Decimal)> = self
            .bids
            .levels
            .iter()
            .rev() // highest first.
            .take(levels)
            .map(|(price, level)| (*price, level.total_quantity))
            .collect();

        let asks: Vec<(Decimal, Decimal)> = self
            .asks
            .levels
            .iter() // lowest first
            .take(levels)
            .map(|(price, level)| (*price, level.total_quantity))
            .collect();

        (bids, asks)
    }
}
