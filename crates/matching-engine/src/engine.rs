use std::collections::HashMap;
use crate::orderbook::OrderBook;
use crate::types::TradingPair;

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
}

// ---- Helpers ---------------------------------------

fn pair_key(pair: &TradingPair) -> String {
    format!("{}/{}", pair.base.to_uppercase(), pair.quote.to_uppercase())
}