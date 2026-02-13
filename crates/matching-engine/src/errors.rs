use thiserror::Error;
use crate::types::OrderId;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("order {0} not found")]
    OrderNotFound(OrderId),

    #[error("order {0} is not active (status: {1})")]
    OrderNotActive(OrderId, String),

    #[error("invalid order: {0}")]
    InvalidOrder(String),

    #[error("limit order missing price")]
    MissingPrice,

    #[error("quantity must be positive, got {0}")]
    InvalidQuantity(String),

    #[error("price must be positive, got {0}")]
    InvalidPrice(String),

    #[error("trading pair mismatch: expected {expected}, got {got}")]
    PairMismatch { expected: String, got: String },
}
