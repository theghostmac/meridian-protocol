use std::collections::VecDeque;
use std::time::Instant;
use crate::config::BatchConfig;

use tracing::{debug, info, instrument, warn};
use crate::types::PendingFill;

/// Rolling gas price tracker for dynamic flush decisions.
///
/// Maintains a sliding window of recent base fee samples.
/// Used to detect when gas is cheap relative to recent history.
#[derive(Debug)]
pub struct GasPriceOracle {
    /// Recent base fee samples (wei), oldest first.
    samples: VecDeque<u128>,
    /// Maximum number of samples to retain.
    window_size: usize,
    /// Current base fee from the latest block.
    pub current_wei: u128,
}

impl GasPriceOracle {
    pub fn new(window_size: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(window_size),
            window_size,
            current_wei: 0,
        }
    }

    /// Record a new base fee sample (call on each new block).
    pub fn record(&mut self, base_fee_wei: u128) {
        if self.samples.len() >= self.window_size {
            self.samples.pop_front();
        }
        self.samples.push_back(base_fee_wei);
        self.current_wei = base_fee_wei;
    }

    /// Rolling average of recent base fees.
    pub fn rolling_average(&self) -> Option<u128> {
        if self.samples.is_empty() {
            return None;
        }

        Some(self.samples.iter().sum::<u128>() / self.samples.len() as u128)
    }

    /// True if current gas is below `factor` x rolling average.
    /// E.g. factor=0.85 -> true when gas is 15% below recent average.
    pub fn is_below_average(&self, factor: u64) -> bool {
        self.rolling_average()
            .map(|avg| self.current_wei < (avg as f64 * factor) as u128)
            .unwrap_or(false)
    }

    /// True if current gas is at or below the absolute threshold.
    pub fn is_opportunistic(&self, threshold_wei: u128) -> bool {
        self.current_wei <= threshold_wei && self.current_wei > 0
    }
}

/// Reasons why a batch was flushed. Used for metrics and logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushReason {
    /// Reached the hard fill count cap.
    MaxSizeReached,
    /// Oldest fill exceeded the age threshold.
    MaxAgeReached,
    /// Gas price dropped to or below the absolute threshold.
    OpportunisticGas,
    /// Gas price dropped below rolling average by the configured factor.
    BelowAverageGas,
    /// Manual/external flush request.
    Manual,
}

impl FlushReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MaxSizeReached => "max_size",
            Self::MaxAgeReached => "max_age",
            Self::OpportunisticGas => "opportunistic_gas",
            Self::BelowAverageGas => "below_average_gas",
            Self::Manual => "manual",
        }
    }
}

/// The core batcher -- accumulates fills and decides when to flush.
///
/// The flush decision logic (in order of priority):
///   1. fills >= max_batch_size              → always flush (hard cap)
///   2. oldest_fill_age >= max_batch_age     → flush (latency cap)
///   3. gas <= opportunistic_threshold       → flush if fills >= min_for_gas
///   4. gas < rolling_avg * below_avg_factor → flush if fills >= min_for_gas
#[derive(Debug)]
pub struct Batcher {
    config: BatchConfig,
    pending: Vec<PendingFill>,
    oldest_fill_at: Option<Instant>,
    batch_counter: u64,
}

impl Batcher {
    pub fn new(config: BatchConfig) -> Self {
        Self {
            config,
            pending: Vec::new(),
            oldest_fill_at: None,
            batch_counter: 0,
        }
    }

    /// Add a fill to the pending queue.
    #[instrument(skip(self, fill), fields(fill_id = %fill.id))]
    pub fn push(&mut self, fill: PendingFill) {
        if self.pending.is_empty() {
            self.oldest_fill_at = Some(Instant::now());
        }
        debug!(
            fill_id = %fill.id,

        )
    }

    /// Evaluate whether a flush should happen given current gas conditions.
    /// Return the flush reason if a flush is warranted, None otherwise.
    #[instrument(skip(self, oracle))]
    pub fn should_flush(&self, oracle: &GasPriceOracle) -> Option<FlushReason> {
        let count = self.pending.len();

        if count == 0 {
            return None;
        }

        // 1. Hard size cap — always flush regardless of gas.
        if count >= self.config.max_batch_size {
            debug!(count, "flush: max size reached");
            return Some(FlushReason::MaxSizeReached);
        }

        // 2. Age cap — prevents indefinite latency during quiet periods.
        if let Some(oldest) = self.oldest_fill_at {
            if oldest.elapsed() >= self.config.max_batch_age {
                debug!(
                    age_ms = oldest.elapsed().as_millis(),
                    "flush: max age reached"
                );
                return Some(FlushReason::MaxAgeReached);
            }
        }

        // 3 & 4. Gas-triggered flushes — only worthwhile with enough fills.
        let min = self.config.min_batch_size_for_gas_trigger;
        if count >= min {
            // Absolute opportunistic threshold.
            if oracle.is_opportunistic(crate::config::GasConfig::OPPORTUNISTIC_DEFAULT) {
                debug!(
                    gas_wei = oracle.current_wei,
                    count,
                    "flush: opportunistic gas"
                );
                return Some(FlushReason::OpportunisticGas);
            }

            // Below rolling average.
            if oracle.is_below_average(0.85) {
                debug!(
                    gas_wei = oracle.current_wei,
                    avg_wei = oracle.rolling_average(),
                    count,
                    "flush: gas below rolling average"
                );
                return Some(FlushReason::BelowAverageGas);
            }
        }

        None
    }

    /// Drain pending fills into a SettlementBatch and reset state.
    /// Only call after `should_flush` returns Some.
    pub fn flush(&mut self, reason: FlushReason) -> Option<SettlementBatch> {
        
    }
}

