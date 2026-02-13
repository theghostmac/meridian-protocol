use std::collections::HashMap;
use crate::errors::EngineError;
use crate::orderbook::OrderBook;
use crate::types::{MatchResult, Order, TradingPair};


/// The matching engine.
///
/// Maintains one order book per trading pair.
/// Processes orders synchronously in a deterministic loop -
/// no async, no locks at this layer. Concurrency is handled
/// by the caller (a Tokio task per pair).
pub struct MatchingEngine {
    /// One book per trading pair.
    books: HashMap<String, OrderBook>,
}

impl MatchingEngine {
    pub fn new() -> Self {
       Self {
           books: HashMap::new(),
       }
    }

    /// Register a new trading pair. Must be called before submitting orders.
    pub fn add_pair(&mut self, pair: &TradingPair) {
        let key = pair_key(pair);
        self.books.entry(key).or_insert_with(OrderBook::new);
    }

    /// Submit an order to the engine.
    ///
    /// The engine will:
    /// 1. Validate the order.
    /// 2. Attempt to match it against the opposite side of the book.
    /// 3. If unfilled quantity remains, rest it in the book.
    /// 4. Return a MatchResult with all fills produced.
    pub fn submit(&mut self, mut order: Order) -> Result<MatchResult, EngineError> {
        self.validate(&order?;)
    }


}

// ---- Helpers ---------------------------------------

fn pair_key(pair: &TradingPair) -> String {
    format!("{}/{}", pair.base.to_uppercase(), pair.quote.to_uppercase())
}