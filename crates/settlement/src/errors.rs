use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    // ── Batch errors ──────────────────────────────────────────────────────────
    #[error("batch is empty — nothing to settle")]
    EmptyBatch,

    #[error("batch size {0} exceeds maximum {1}")]
    BatchTooLarge(usize, usize),

    // ── Simulation errors ─────────────────────────────────────────────────────
    #[error("revm simulation failed: {reason}")]
    SimulationFailed { reason: String },

    #[error("simulation reverted with data: {revert_data}")]
    SimulationReverted { revert_data: String },

    #[error("simulation gas estimate {estimated} exceeds limit {limit}")]
    GasLimitExceeded { estimated: u64, limit: u64 },

    // ── Submission errors ─────────────────────────────────────────────────────
    #[error("transaction submission failed: {0}")]
    SubmissionFailed(String),

    #[error("transaction reverted on-chain: tx={tx_hash}, reason={reason}")]
    TransactionReverted { tx_hash: String, reason: String },

    #[error("transaction confirmation timeout after {seconds}s: tx={tx_hash}")]
    ConfirmationTimeout { seconds: u64, tx_hash: String },

    #[error("nonce too low — likely a race condition, retry")]
    NonceTooLow,

    // ── Gas errors ────────────────────────────────────────────────────────────
    #[error("gas price {current_wei} exceeds circuit breaker {max_wei}")]
    GasPriceTooHigh { current_wei: u128, max_wei: u128 },

    #[error("gas oracle unavailable: {0}")]
    GasOracleError(String),

    // ── Reconciliation errors ─────────────────────────────────────────────────
    #[error("on-chain fill count {on_chain} does not match expected {expected}")]
    ReconciliationMismatch { on_chain: usize, expected: usize },

    #[error("fill {fill_id} not found in on-chain events")]
    MissingOnChainFill { fill_id: String },

    // ── Provider errors ───────────────────────────────────────────────────────
    #[error("RPC provider error: {0}")]
    ProviderError(String),

    #[error("ABI encoding error: {0}")]
    AbiError(String),

    // ── Config errors ─────────────────────────────────────────────────────────
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Categorize errors for retry logic.
impl PipelineError {
    /// Should this error trigger an automatic retry?
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::NonceTooLow
                | Self::ConfirmationTimeout { .. }
                | Self::ProviderError(_)
                | Self::GasOracleError(_)
        )
    }

    /// Should this error pause the pipeline (circuit breaker)?
    pub fn is_circuit_breaker(&self) -> bool {
        matches!(
            self,
            Self::GasPriceTooHigh { .. }
                | Self::TransactionReverted { .. }
                | Self::ReconciliationMismatch { .. }
        )
    }
}