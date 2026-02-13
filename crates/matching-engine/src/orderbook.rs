use std::collections::{BTreeMap, HashMap, VecDeque};
use rust_decimal::Decimal;
use crate::types::{Order, OrderId};

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
pub struct HalfBook {
    /// price -> price level (FIFO queue of order IDs)
    pub levels: BTreeMap<Decimal, PriceLevel>
}

impl HalfBook {
    /// Insert an order into this half-book.
    pub fn insert(&mut self, order: &Order) {
        let price = order.price.expect("limit order must have price");
        let level = self.levels.entry(price).or_default();
        level.add(order.id, order.quantity_remaining);
    }
}

pub struct OrderBook {
    pub bid: HalfBook,
    pub ask: HalfBook,
    /// All live orders by ID for fast lookup during matching.
    pub orders: HashMap<OrderId, Order>
}