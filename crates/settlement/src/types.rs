use std::time::SystemTime;
use uuid::Uuid;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use matching_engine::{Fill, Side};
use crate::batcher::FlushReason;

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

/// A batch of fills ready for simulation and on-chain submission.
#[derive(Debug)]
pub struct SettlementBatch {
    /// Monotonic batch ID (used as the on-chain batchId).
    pub id: u64,
    /// Fills in this batch.
    pub fills: Vec<PendingFill>,
    /// Why this batch was flushed (for metrics).
    pub flush_reason: FlushReason,
    /// Wall-clock time the batch was created.
    pub created_at: SystemTime,
}

impl SettlementBatch {
    pub fn fill_count(&self) -> usize {
        self.fills.len()
    }

    pub fn age_ms(&self) -> u64 {
        self.created_at
            .elapsed()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

/// Result of submitting a batch on-chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementReceipt {
    pub batch_id: u64,
    pub tx_hash: String,
    pub block_number: u64,
    pub gas_used: u64,
    pub fill_count: usize,
    pub settled_at: SystemTime,
}

/// Current status of the pipeline — exposed via health endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStatus {
    pub pending_fills:     usize,
    pub batches_submitted: u64,
    pub last_tx_hash:      Option<String>,
    pub current_gas_wei:   u128,
    pub circuit_breaker:   bool,
}
