// SPDX-License-Identifier: MIT
pragma solidity ^0.8.33;

import { FillLib } from "../libraries/FillLib.sol";
import { OrderLib } from "../libraries/OrderLib.sol";

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

    /// @notice   Settles a batch of matched fills from the off-chain engine.
    /// @param    makers     Maker order intents (resting side).
    /// @param    takers     Taker order intents (aggressive side).
    /// @param    fills      Exact fill amounts computed by engine for each maker/taker pair.
    /// @param    makerSigs  EIP-712 Signatures of the maker orders.
    /// @param    takerSigs  EIP-712 Signatures of the taker orders.
    /// @param    batchId    Off-chain identifier for the batch, emitted in BatchSettled event.
    function settleBatch(
        OrderLib.Order[] calldata makers,
        OrderLib.Order[] calldata takers,
        FillLib.Fill[] calldata fills,
        FillLib.Sig[] calldata makerSigs,
        FillLib.Sig[] calldata takerSigs,
        uint256 batchId
    ) external;

    /// @notice   Cancel a nonce to prevent a signed order from being settled on-chain.
    /// @dev      Called by the trader directly.  Gas: 1 SSTORE to set the bit in the bitmap.
    /// @param    nonce   The nonce to cancel.
    function cancelOrder(uint64 nonce) external;

    /// @notice   Check if a nonce has been used or cancelled.
    /// @dev      Used by off-chain engine to check order validity before including in a batch.
    function isNonceUsed(address trader, uint64 nonce) external view returns (bool);

    /// @notice EIP-712 domain separator for this contract for easy access by off-chain engine and other callers.
    function domainSeparator() external view returns (bytes32);
}
