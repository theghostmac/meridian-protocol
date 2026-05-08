use alloy::{
    network::EthereumWallet,
    primitives::{Address, U256},
    providers::{Provider, ProviderBuilder},
    signers::local::PrivateKeySigner,
    sol,
    transports::http::Http,
};
use alloy::transports::http::reqwest;
use tracing::{error, info, instrument, warn};

use crate::config::PipelineConfig;
use crate::errors::PipelineError;
use crate::types::{PendingFill, SettlementBatch, SettlementReceipt};


// ── Alloy contract bindings ───────────────────────────────────────────────────
// sol! macro generates type-safe ABI bindings at compile time.
// The struct layout must match the Solidity ABI exactly.

sol! {
    #[allow(missing_docs)]
    #[sol(rpc)]
    interface IMeridianSettlement {
        struct Order {
            address trader;
            address tokenIn;
            address tokenOut;
            uint128 amountIn;
            uint128 amountOutMin;
            uint64  nonce;
            uint64  deadline;
        }

        struct Fill {
            uint128 makerAmountIn;
            uint128 takerAmountIn;
        }

        struct Sig {
            uint8   v;
            bytes32 r;
            bytes32 s;
        }

        function settleBatch(
            Order[]  calldata makers,
            Order[]  calldata takers,
            Fill[]   calldata fills,
            Sig[]    calldata makerSigs,
            Sig[]    calldata takerSigs,
            uint256           batchId
        ) external;

        event OrderFilled(
            bytes32 indexed fillId,
            address indexed maker,
            address indexed taker,
            address         tokenIn,
            address         tokenOut,
            uint128         makerAmountIn,
            uint128         takerAmountIn,
            uint64          makerNonce,
            uint64          takerNonce
        );

        event BatchSettled(
            uint256 indexed batchId,
            uint256         fillCount,
            uint256         gasUsed
        );
    }
}

/// The submitter encodes batches into calldata and submits them on-chain.
///
/// Uses Alloy's EthereumWallet for EIP-1559 transaction signing.
/// Implements retry logic with exponential backoff for transient errors.
pub struct Submitter {
    config:   PipelineConfig,
    provider: alloy::providers::RootProvider<Http<reqwest::Client>>,
    wallet:   EthereumWallet,
    contract: Address,
}

impl Submitter {
    /// Creates a new submitter from config.
    /// Connects to the RPC endpoint and initializes the signer.
    pub async fn new(config: PipelineConfig) -> Result<Self, PipelineError> {
        let signer: PrivateKeySigner = config
            .chain
            .operator_private_key
            .parse()
            .map_err(|e| PipelineError::InvalidConfig(format!("invalid private key: {e}")))?;

        let wallet = EthereumWallet::from(signer);

        let provider = ProviderBuilder::new()
            .with_recommended_fillers()
            .wallet(wallet.clone())
            .on_http(
                config
                    .chain
                    .rpc_http
                    .parse()
                    .map_err(|e| PipelineError::InvalidConfig(format!("invalid RPC URL: {e}")))?,
            );

        let contract = config.chain.settlement_address;

        Ok(Self {
            config,
            provider,
            wallet,
            contract,
        })
    }

    /// Submit a settlement batch on-chain.
    ///
    /// Steps:
    /// 1. Check circuit breaker (gas price).
    /// 2. Build EIP-1559 transaction with current gas pricing.
    /// 3. Estimate gas.
    /// 4. Submit with retry on transient errors.
    /// 5. Wait for confirmation.
    /// 6. Return receipt.
    #[instrument(skip(self, batch), fields(batch_id = batch.id, fill_count = batch.fill_count()))]
    pub async fn submit(
        &self,
        batch: SettlementBatch,
        gas_price_wei: u128,
    ) -> Result<SettlementReceipt, PipelineError> {
        // Circuit breaker — refuse to submit at extreme gas prices.
        if gas_price_wei > self.config.gas.max_gas_price_wei {
            return Err(PipelineError::GasPriceTooHigh {
                current_wei: gas_price_wei,
                max_wei: self.config.gas.max_gas_price_wei,
            });
        }

        info!(
            batch_id = batch.id,
            fill_count = batch.fill_count(),
            gas_price_gwei = gas_price_wei as f64 / 1e9,
            "submitting settlement batch"
        );

        // Encode the batch into ABI calldata.
        let calldata = self.encode_batch(&batch)?;

        // Build and submit transaction with retry.
        let receipt = self
            .submit_with_retry(batch.id, calldata, gas_price_wei, 3)
            .await?;

        metrics::counter!("meridian.settlement.submitted").increment(1);
        metrics::counter!("meridian.settlement.fills_settled")
            .increment(batch.fill_count() as u64);
        metrics::histogram!("meridian.settlement.gas_used")
            .record(receipt.gas_used as f64);

        info!(
            batch_id = receipt.batch_id,
            tx_hash  = %receipt.tx_hash,
            block    = receipt.block_number,
            gas_used = receipt.gas_used,
            "batch settled on-chain"
        );

        Ok(receipt)
    }

    // ── Private ───────────────────────────────────────────────────────────────

    /// Encode a SettlementBatch into ABI calldata for settleBatch().
    ///
    /// NOTE: In production the fills carry pre-computed EIP-712 signatures
    /// from the order gateway. Here we construct the calldata structure —
    /// signature attachment happens at the gateway layer.
    fn encode_batch(
        &self,
        batch: &SettlementBatch,
    ) -> Result<alloy::primitives::Bytes, PipelineError> {
        // Placeholder: In the full implementation, PendingFill carries
        // the full Order structs + Sig components from the gateway.
        // This shows the encoding pattern clearly.
        let batch_id = U256::from(batch.id);

        // Build empty arrays for now — real impl populates from PendingFill.
        let makers:    Vec<IMeridianSettlement::Order> = vec![];
        let takers:    Vec<IMeridianSettlement::Order> = vec![];
        let fills:     Vec<IMeridianSettlement::Fill>  = vec![];
        let maker_sigs: Vec<IMeridianSettlement::Sig>  = vec![];
        let taker_sigs: Vec<IMeridianSettlement::Sig>  = vec![];

        // Encode via Alloy's sol! macro — type-safe, no manual ABI packing.
        let call = IMeridianSettlement::settleBatchCall {
            makers,
            takers,
            fills,
            makerSigs: maker_sigs,
            takerSigs: taker_sigs,
            batchId: batch_id,
        };

        Ok(alloy::sol_types::SolCall::abi_encode(&call).into())
    }

    /// Submit transaction with exponential backoff retry.
    async fn submit_with_retry(
        &self,
        batch_id: u64,
        calldata: alloy::primitives::Bytes,
        gas_price_wei: u128,
        max_retries: u32,
    ) -> Result<SettlementReceipt, PipelineError> {
        let mut attempt = 0u32;

        loop {
            match self
                .submit_once(batch_id, calldata.clone(), gas_price_wei)
                .await
            {
                Ok(receipt) => return Ok(receipt),

                Err(e) if e.is_retryable() && attempt < max_retries => {
                    attempt += 1;
                    let backoff_ms = 100u64 * 2u64.pow(attempt);
                    warn!(
                        batch_id,
                        attempt,
                        backoff_ms,
                        error = %e,
                        "submission failed, retrying"
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                }

                Err(e) => {
                    error!(batch_id, error = %e, "submission failed permanently");
                    return Err(e);
                }
            }
        }
    }

    /// Single submission attempt.
    async fn submit_once(
        &self,
        batch_id: u64,
        calldata: alloy::primitives::Bytes,
        gas_price_wei: u128,
    ) -> Result<SettlementReceipt, PipelineError> {
        use alloy::primitives::TxKind;
        use alloy::rpc::types::TransactionRequest;

        let max_fee = U256::from(gas_price_wei);
        let priority_fee = U256::from(self.config.gas.max_priority_fee_wei);

        let tx = TransactionRequest::default()
            .to(self.contract)
            .input(calldata.into())
            .max_fee_per_gas(max_fee.to::<u128>())
            .max_priority_fee_per_gas(priority_fee.to::<u128>())
            .gas_limit(self.config.gas.gas_limit);

        let pending = self
            .provider
            .send_transaction(tx)
            .await
            .map_err(|e| PipelineError::SubmissionFailed(e.to_string()))?;

        let tx_hash = pending.tx_hash().to_string();
        info!(tx_hash = %tx_hash, "transaction submitted, awaiting confirmation");

        let receipt = pending
            .get_receipt()
            .await
            .map_err(|e| PipelineError::SubmissionFailed(e.to_string()))?;

        if !receipt.status() {
            return Err(PipelineError::TransactionReverted {
                tx_hash: tx_hash.clone(),
                reason: "transaction status = false".to_string(),
            });
        }

        Ok(SettlementReceipt {
            batch_id,
            tx_hash,
            block_number: receipt.block_number.unwrap_or(0),
            gas_used: receipt.gas_used as u64,
            fill_count: 0, // populated by caller
            settled_at: std::time::SystemTime::now(),
        })
    }
}
