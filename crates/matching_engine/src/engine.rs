use std::collections::HashMap;
use std::time::SystemTime;
use rust_decimal::Decimal;
use crate::errors::EngineError;
use crate::orderbook::OrderBook;
use crate::types::{Fill, MatchResult, Order, OrderBookSnapshot, OrderId, OrderStatus, Side, TradingPair};


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
        self.validate(&order)?;

        let key = pair_key(&order.pair);
        let book = self.books.get_mut(&key)
            .ok_or_else(|| EngineError::InvalidOrder(format!("unknown pair: {}", order.pair)))?;

        let now_ns = now_ns();
        let mut fills = Vec::new();

        match order.side {
            Side::Bid => match_bid(&mut order, book, &mut fills, now_ns),
            Side::Ask => match_ask(&mut order, book, &mut fills, now_ns),
        }

        // If there's remaining quantity, rest the order in the book
        let status = order.status;
        if order.is_active() {
            book.add_order(order.clone());
        }

        Ok(MatchResult {
            order_id: order.id,
            fills,
            status,
        })
    }

    /// Cancel a resting order by ID.
    pub fn cancel(&mut self, pair: &TradingPair, order_id: OrderId) -> Result<Order, EngineError> {
        let key = pair_key(pair);
        let book = self.books.get_mut(&key)
            .ok_or_else(|| EngineError::InvalidOrder(format!("unknown pair: {}", pair)))?;

        book.remove_order(order_id)
            .ok_or(EngineError::OrderNotFound(order_id))
            .map(|mut o| {
                o.status = OrderStatus::Cancelled;
                o
            })
    }

    /// Get a depth snapshot for a pair.
    pub fn depth(&self, pair: &TradingPair, level: usize) -> Option<OrderBookSnapshot> {
        let key = pair_key(pair);
        self.books.get(&key)
            .map(|b| b.depth_snapshot(level))
    }

    /// Best bid/ask for a pair.
    pub fn best_prices(&self, pair: &TradingPair) -> Option<(Option<Decimal>, Option<Decimal>)> {
        let key = pair_key(pair);
        self.books.get(&key).map(|b| (b.best_bid(), b.best_ask()))
    }

    // ----- private ---------------------------------

    fn validate(&self, order: &Order) -> Result<(), EngineError> {
        if order.quantity <= Decimal::ZERO {
            return Err(EngineError::InvalidQuantity(order.quantity.to_string()));
        }

        if let Some(price) = order.price {
            if price <= Decimal::ZERO {
                return Err(EngineError::InvalidPrice(price.to_string()));
            }
        } else {
            // Market orders must eventually be supported; for now require price.
            return Err(EngineError::MissingPrice);
        }

        Ok(())
    }
}

impl Default for MatchingEngine {
    fn default() -> Self {
        Self::new()
    }
}

// --- Matching Logic

/// Match an incoming bid (buy) against resting asks.
///
/// Price-time priority:
/// - Walk asks from lowest price upward.
/// - Fill as long as ask_price <= bid_price and quantity remains.
fn match_bid(
    taker: &mut Order,
    book: &mut OrderBook,
    fills: &mut Vec<Fill>,
    now_ns: u64,
) {
    let taker_price = match taker.price {
        Some(p) => p,
        None => return,
    };

    // Collect ask prices that cross with the bid.
    // We collect first to avoid borrow issues on the book.
    loop {
        let best_ask = match book.best_ask() {
            Some(p) => p,
            None => break,
        };

        // No more crosses.
        if best_ask > taker_price {
            break;
        }

        // Get the front order at this price level.
        let maker_id = match book.asks.front_order_at(&best_ask) {
            Some(id) => id,
            None => break,
        };

        let fill_qty = {
            let maker = match book.orders.get_mut(&maker_id) {
                Some(o) => o,
                None => break,
            };

            let qty = taker.quantity_remaining.min(maker.quantity_remaining);
            maker.fill(qty);
            qty
        };

        taker.fill(fill_qty);

        fills.push(Fill::new(
            maker_id,
            taker.id,
            taker.pair.clone(),
            best_ask,
            fill_qty,
            Side::Bid,
            now_ns,
        ));

        // If the maker is fully filled, remove it from the book.
        let maker_filled = book
            .orders
            .get(&maker_id)
            .map(|o| !o.is_active())
            .unwrap_or(false);

        if maker_filled {
            book.remove_order(maker_id);
        }

        if !taker.is_active() {
            break;
        }
    }
}

/// Match an incoming ask (sell) against resting bids.
///
/// Walk bids from highest price downward.
/// Fill as long as bid_price >= ask_price and quantity remains.
fn match_ask(
    taker: &mut Order,
    book: &mut OrderBook,
    fills: &mut Vec<Fill>,
    now_ns: u64,
) {
    let taker_price = match taker.price {
        Some(p) => p,
        None => return,
    };

    loop {
        let best_bid = match book.best_bid() {
            Some(p) => p,
            None => break,
        };

        // No more crosses.
        if best_bid < taker_price {
            break;
        }

        let maker_id = match book.bids.front_order_at(&best_bid) {
            Some(id) => id,
            None => break,
        };

        let fill_qty = {
            let maker = match book.orders.get_mut(&maker_id) {
                Some(o) => o,
                None => break,
            };

            let qty = taker.quantity_remaining.min(maker.quantity_remaining);
            maker.fill(qty);
            qty
        };

        taker.fill(fill_qty);

        fills.push(Fill::new(
            maker_id,
            taker.id,
            taker.pair.clone(),
            best_bid,
            fill_qty,
            Side::Ask,
            now_ns,
        ));

        let maker_filled = book
            .orders
            .get(&maker_id)
            .map(|o| !o.is_active())
            .unwrap_or(false);

        if maker_filled {
            book.remove_order(maker_id);
        }

        if !taker.is_active() {
            break;
        }
    }
}


// --- Helpers ---------------------------------------

fn pair_key(pair: &TradingPair) -> String {
    format!("{}/{}", pair.base.to_uppercase(), pair.quote.to_uppercase())
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos() as u64
}