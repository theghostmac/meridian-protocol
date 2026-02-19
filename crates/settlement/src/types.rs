use uuid::Uuid;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use matching_engine::{Fill, Side};

/// A fill produced by the matching engine, ready for settlement.
/// Maps 1:1 to matching_engine::Fill but owns its data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingFill {
    pub id: Uuid,
    pub maker_order_id: Uuid,
    pub taker_order_id: Uuid,
    pub price: Decimal,
    pub quantity: Decimal,
    pub taker_side: Side,
    pub timestamp_ns: u64,
}

impl From<matching_engine::Fill> for PendingFill {
    fn from(value: Fill) -> Self {
        Self {
            id: value.id,
            maker_order_id: value.maker_order_id.0,
            taker_order_id: value.taker_order_id.0,
            price: value.price,
            quantity: value.quantity,
            taker_side: value.taker_side,
            timestamp_ns: value.timestamp_ns,
        }
    }
}

