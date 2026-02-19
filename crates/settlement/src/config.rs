
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

}

#[derive(Debug, Clone)]
pub struct GasConfig {

}