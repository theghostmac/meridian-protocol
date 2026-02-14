// SPDX-License-Identifier: MIT
pragma solidity ^0.8.33;

/// @title IMeridianSettlement
/// @notice Public interface for the Meridian batch settlement contract.
interface IMeridianSettlement {
    // --- Events ─────────────────────────────────────────────────────────────────
    // Indexed fields chosen deliberately:
    // - maker/taker: needed for per-trader fill history queries
    // - tokenIn:     needed for per-token volume queries
    // Non-indexed amounts stay in the log data (cheaper, and still decodable).

    // @notice Emitted for every fill successfully settled on-chain.
    // @dev    Consumers (indexers) will index by maker, taker, and tokenIn.
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

    /// @notice Emitted when a settlement is successfully executed.
    event OrderCancelled(
        address indexed trader,
        uint64          nonce
    );

    /// @notice Emitted when a settlement operator is rotated.
    event OperatorUpdated(
        address indexed oldOperator,
        address indexed newOperator
    );

    /// @notice Emitted when a batch is settled - will be used for batch-level analytics.
    event BatchSettled(
        uint256 indexed batchId,
        uint256         fillCount,
        uint256         gasUsed
    );

    // ─── Errors ───────────────────────────────────────────────────────────────

    error Unauthorized(address caller);
    error NonceAlreadyUsed(address trader, uint64 nonce);
    error BatchEmpty();
    error ArrayLengthMismatch();
    error TransferFailed(address token, address from, address to, uint256 amount);

    // ─── Core functions ────────────────────────────────────────────────────────

    
}
