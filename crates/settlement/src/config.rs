use std::time::Duration;

/// Chain-level and pipeline-level config.
///
/// Loaded from env. var. or a config file at startup.
/// All gas values are in wei.
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    /// Chain configuration.
    pub chain: ChainConfig,

    /// Batch flush strategy parameters,
    pub batch: BatchConfig,

    /// Gas price oracle and submission parameters.
    pub gas: GasConfig,
}

#[derive(Debug, Clone)]
pub struct ChainConfig {
    /// Chain ID - Base mainnet = 8453, Base Sepolia = 84532.
    pub chain_id: u64,

    /// HTTP RPC endpoint (Primary)
    pub rpc_http: String,

    /// WebSocket RPC endpoint (for gas price subscriptions)
    pub rpc_ws: String,

    /// Deployed MeridianSettlement contract address
    pub settlement_address: alloy::primitives::Address,

    /// Operator private key (hex, without 0x prefix).
    pub operator_private_key: String,
}

#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Flush when the batch reaches this many fills.
    /// Hard cap - never exceed regardless of gas price.
    pub max_batch_size: usize,

    /// Flush if the oldest fill in the batch is older than this.
    /// Prevents latency from building up during low-volume periods.
    pub max_batch_age: Duration,

    /// Minimum fills to include in a gas-triggered flush.
    /// Avoids submitting tiny batches even when gas is cheap.
    pub min_batch_size_for_gas_trigger: usize,
}

#[derive(Debug, Clone)]
pub struct GasConfig {
    /// Flush immediately if base fee (wei) is at or below this threshold.
    /// Set to u1238::MAX to disable gas-price triggering.
    pub opportunistic_gas_threshold_wei: u128,

    /// How many recent base-fee samples to keep for rolling average.
    pub rolling_window_size: usize,

    /// Flush if current gas price is below rolling_avg * this factor.
    /// E.g. 0.85 = flush when gas is 15% below recent average.
    pub below_average_factor: f64,

    /// Maximum gas price (wei) at which we'll submit at all.
    /// Circuit breaker — pause settlement if gas spikes unexpectedly.
    pub max_gas_price_wei: u128,

    /// Gas limit per settleBatch call.
    /// Base L2: generous limit since L2 gas is cheap.
    pub gas_limit: u64,

    /// EIP-1559 priority fee (tip) in wei.
    pub max_priority_fee_wei: u128,
}

impl PipelineConfig {
    /// Sensible defaults for Base Sepolia testnet.
    pub fn base_sepolia_defaults() -> Self {
        Self {
            chain: ChainConfig {
                chain_id: 84532,
                rpc_http: std::env::var("BASE_RPC_HTTP")
                    .unwrap_or_else(|_| "https://sepolia.base.org".to_string()),
                rpc_ws: std::env::var("BASE_RPC_WS")
                    .unwrap_or_else(|_| "wss://sepolia.base.org".to_string()),
                settlement_address: std::env::var("SETTLEMENT_ADDRESS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(alloy::primitives::Address::ZERO),
                operator_private_key: std::env::var("OPERATOR_PRIVATE_KEY")
                    .unwrap_or_default(),
            },
            batch: BatchConfig {
                max_batch_size: 50,
                max_batch_age: Duration::from_millis(500),
                min_batch_size_for_gas_trigger: 5,
            },
            gas: GasConfig {
                // Base Sepolia: ~0.001 gwei threshold (effectively always trigger)
                opportunistic_gas_threshold_wei: 1_000_000,
                rolling_window_size: 20,
                below_average_factor: 0.85,
                // Circuit breaker: 10 gwei max on Base
                max_gas_price_wei: 10_000_000_000,
                gas_limit: 3_000_000,
                // Base Sepolia: 1 wei tip is sufficient
                max_priority_fee_wei: 1_000_000,
            },
        }
    }

    /// Defaults for Base mainnet.
    pub fn base_mainnet_defaults() -> Self {
        let mut cfg = Self::base_sepolia_defaults();
        cfg.chain.chain_id = 8453;
        cfg.chain.rpc_http = std::env::var("BASE_RPC_HTTP")
            .unwrap_or_else(|_| "https://mainnet.base.org".to_string());
        cfg.chain.rpc_ws = std::env::var("BASE_RPC_WS")
            .unwrap_or_else(|_| "wss://mainnet.base.org".to_string());
        // Mainnet: tighter gas discipline
        cfg.gas.opportunistic_gas_threshold_wei = 100_000_000; // 0.1 gwei
        cfg.gas.max_gas_price_wei = 50_000_000_000;            // 50 gwei
        cfg.gas.max_priority_fee_wei = 1_000_000_000;          // 1 gwei
        cfg
    }
}

impl GasConfig {
    /// Default opportunistic threshold used by the batcher.
    /// Exposed as a constant so batcher.rs doesn't import a full config.
    pub const OPPORTUNISTIC_DEFAULT: u128 = 1_000_000; // 0.001 gwei
}
