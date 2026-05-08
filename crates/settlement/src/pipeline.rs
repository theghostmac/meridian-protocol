
use std::sync::{Arc, Mutex};
use alloy::signers::k256::sha2::digest::Mac;
use tokio::sync::mpsc;
use tokio::time::{interval, MissedTickBehavior};
use tracing::{error, info, instrument, warn};
use crate::batcher::{Batcher, GasPriceOracle};
use crate::config::PipelineConfig;
use crate::reconciler::Reconciler;
use crate::simulator::Simulator;

use matching_engine::Fill;
use crate::errors::PipelineError;
use crate::submitter::Submitter;
use crate::types::{PendingFill, PipelineStatus};

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

impl SettlementPipeline {
    /// Create a new pipeline. Returns (pipeline, fill_sender).
    /// The caller sends fills via fill_sender; the pipeline consumes them.
    pub fn new(config: PipelineConfig) -> (Self, mpsc::Sender<Fill>) {
        let (fill_tx, fill_rx) = mpsc::channel(10_000);

        let batcher = Arc::new(Mutex::new(Batcher::new(config.batch.clone())));
        let oracle = Arc::new(Mutex::new(
            GasPriceOracle::new(config.gas.rolling_window_size)
        ));
        let simulator = Simulator::new(config.gas.gas_limit);

        let pipeline = Self {
            config,
            batcher,
            oracle,
            reconciler: Reconciler::new(),
            simulator,
            fill_rx,
            circuit_breaker: false,
            batches_submitted: 0,
            last_tx_hash: None,
        };

        (pipeline, fill_tx)
    }

    /// Run the pipeline indefinitely.
    ///
    /// Spawns two concurrent loops:
    ///  - Fill ingestion loop: receives fills, pushes to batcher.
    ///  - Flush loop: polls batcher on a tick, flushes when ready.
    pub async fn run(mut self) -> Result<(), PipelineError> {
        info!("settlement pipeline starting");

        // Submitter requires async init (connects to RPC).
        let submitter = Submitter::new(self.config.clone()).await?;
        let submitter = Arc::new(submitter);

        // Tick interval for checking flush conditions.
        // Fine-grained enough to respect max_batch_age accurately.
        let tick_ms = self.config.batch.max_batch_age.as_millis() as u64 / 4;
        let mut ticker = interval(std::time::Duration::from_millis(tick_ms.max(25)));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // New fill arrived from the matching engine.
                Some(fill) = self.fill_rx.recv() => {
                    if self.circuit_breaker {
                        warn!(fill_id = %fill.id, "circuit breaker open, dropping fill");
                        metrics::counter!("meridian.fills.dropped").increment(1);
                        continue;
                    }

                    let pending: PendingFill = fill.into();
                    self.batcher.lock().await.push(pending);
                    metrics::counter!("meridian.fills.received").increment(1);
                }

                // Tick — check if we should flush.
                _ = ticker.tick() => {
                    if self.circuit_breaker {
                        continue;
                    }

                    self.maybe_flush(&submitter).await;
                }
            }
        }
    }

    /// Check flush conditions and execute if warranted.
    #[instrument(skip(self, submitter))]
    async fn maybe_flush(&mut self, submitter: &Arc<Submitter>) {
        let oracle = self.oracle.lock().await;
        let flush_reason = self.batcher.lock().await.should_flush(&oracle);
        drop(oracle);

        let reason = match flush_reason {
            Some(r) => r,
            None    => return,
        };

        let batch = match self.batcher.lock().await.flush(reason) {
            Some(b) => b,
            None => return,
        };

        let fill_count = batch.fill_count();
        let batch_id = batch.id,
        let gas_wei = self.oracle.lock().await.current_wei;

        info!(
            batch_id,
            fill_count,
            flush_reason = reason.as_str(),
            "flushing batch"
        );

        // Keep a copy of fills for reconciliation.
        let fills_copy: Vec<PendingFill> = batch.fills.clone();

        // Simulate locally before submitting.
        // TODO: pass real calldata once order structs are wired.
        let sim_gas = Simulator::estimate_gas_heuristic(fill_count);
        info!(batch_id, estimated_gas = sim_gas, "pre-flight gas estimate");

        // Submit on-chain.
        let submitter = Arc::clone(submitter);
        let reconciler_ref = &self.reconciler;

        match submitter.submit(batch, gas_wei).await {
            Ok(receipt) => {
                self.batches_submitted += 1;
                self.last_tx_hash = Some(receipt.tx_hash.clone());

                // Reconcile async — don't block the main loop.
                match reconciler_ref.verify(&fills_copy, &receipt).await {
                    Ok(report) => {
                        info!(
                            batch_id  = report.batch_id,
                            confirmed = report.confirmed_fills,
                            "settlement reconciled"
                        );
                    }
                    Err(e) => {
                        error!(batch_id, error = %e, "reconciliation failed — opening circuit breaker");
                        self.circuit_breaker = true;
                        metrics::counter!("meridian.circuit_breaker.opened").increment(1);
                    }
                }
            }

            Err(e) if e.is_circuit_breaker() => {
                error!(batch_id, error = %e, "circuit breaker error — pausing pipeline");
                self.circuit_breaker = true;
                metrics::counter!("meridian.circuit_breaker.opened").increment(1);
            }

            Err(e) => {
                warn!(batch_id, error = %e, "submission failed (non-fatal)");
                metrics::counter!("meridian.settlement.failed").increment(1);
            }
        }
    }

    /// Snapshot of current pipeline state for health checks.
    pub async fn statuc(&self) -> PipelineStatus {
        PipelineStatus {
            pending_fills: self.batcher.lock().await.pending_count(),
            batches_submitted: self.batches_submitted,
            last_tx_hash: self.last_tx_hash.clone(),
            current_gas_wei: self.oracle.lock().await.current_wei,
            circuit_breaker: self.circuit_breaker,
        }
    }
}