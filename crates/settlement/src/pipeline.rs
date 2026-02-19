
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use crate::batcher::{Batcher, GasPriceOracle};
use crate::config::PipelineConfig;
use crate::reconciler::Reconciler;
use crate::simulator::Simulator;

use matching_engine::Fill;

pub struct SettlementPipeline {
    config: PipelineConfig,
    batcher: Arc<Mutex<Batcher>>,
    oracle: Arc<Mutex<GasPriceOracle>>,
    reconciler: Reconciler,
    simulator: Simulator,

    /// Receives fills from the matching engine.
    fill_rx: mpsc::Receiver<Fill>,

    /// Pipeline health state.
    circuit_breaker: bool,
    batches_submitted: u64,
    last_tx_hash: Option<String>,
}
