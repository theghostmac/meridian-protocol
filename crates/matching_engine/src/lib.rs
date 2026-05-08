mod orderbook;
mod types;
mod engine;
mod errors;

// Re-export the primary public interface.
pub use engine::MatchingEngine;
pub use errors::EngineError;
pub use types::{Fill, MatchResult, Order, OrderId, OrderStatus, OrderType, Side, TradingPair};