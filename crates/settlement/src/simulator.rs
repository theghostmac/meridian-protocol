use alloy::consensus::EnvKzgSettings::Default;
use revm::database::InMemoryDB;
use revm::primitives::bytes::Bytes;
use revm::primitives::U256;
use revm::state::{AccountInfo, Bytecode};
use tracing::{debug, instrument};
use crate::errors::PipelineError;
use crate::types::SettlementBatch;

/// Pre-flight simulator using revm.
///
/// Before submitting a batch on-chain, we simulate it locally.
/// This catches issues early:
///     - Expired order deadlines
///     - Nonces already consumed
///     - Insufficient token balances/allowances
///     - Gas limit violations
///
/// Simulation runs in microseconds locally vs. risking a failed
/// on-chain tx that wastes gas and requires investigations.
pub struct Simulator {
    /// Maximum gas we'll allow a simulated batch to consume.
    gas_limit: u64,
}

/// Gas estimation result from a local simulation.
#[derive(Debug)]
pub struct SimulationResult {
    /// Estimated gas the transaction will consume.
    pub gas_used: u64,

    /// Whether the simulation succeeded.
    pub success: bool,

    /// Revert reason bytes, if the call reverted.
    pub revert_data: Option<Vec<u8>>
}

impl Simulator {
    pub fn new(p0: u64) -> _ {
        todo!()
    }

    /// Simulate a settlement batch against a local EVM fork.
    ///
    /// In production this would fork from the current Base state
    /// via `eth_call` with a state override. Here we demonstrate
    /// the revm integration pattern clearly.
    #[instrument(skip(self, batch, calldate), fields(batch_id = batch.id))]
    pub fn simulate(
        &self,
        batch: &SettlementBatch,
        calldata: Bytes,
        contract_address: revm::primitives::Address,
        caller: revm::primitives::Address,
    ) -> Result<SimulationResult, PipelineError> {
        debug!(
            batch_id = batch.id,
            fill_count = batch.fill_count(),
            "simulating batch"
        );

        // Build an in-memory EVM database.
        // In production: fork from Base via `eth_getProof` + state override.
        let mut db = InMemoryDB::default();

        // Fund the caller so simulation doesn't fail on balance checks.
        db.insert_account_info(
            caller,
            AccountInfo {
                balance: U256::from(10u128.pow(18)), // 1 ETH
                nonce: 0,
                ..Default::default()
            },
        );

        // Insert a minimal contract stub so the call target exists.
        // In production: fetch actual bytecode via eth_getCode.
        db.insert_account_info(
            contract_address,
            AccountInfo {
                balance: U256::ZERO,
                nonce: 1,
                code_hash: revm::primitives::KECCAK_EMPTY,
                code: Some(Bytecode::new()),
            },
        );

        // Build and execute the EVM.
        let mut evm = Evm::builder()
            .with_db(db)
            .
    }
}