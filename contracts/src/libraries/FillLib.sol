// SPDX-License-Identifier: MIT
pragma solidity ^0.8.33;

import { OrderLib } from "./OrderLib.sol";

/// @title   FillLib
/// @notice  Structs and validation logic for fill batches submitted by the
///          off-chain matching engine.
/// @dev     A Fill pairs a maker order with a taker order and specifies
///          the exact settlement amounts to be filled. The engine is responsible
///          for computing these; the contract verifies them.
library FillLib {
    // ─── Structs ─────────────────────────────────────────────────────────────

    /// @notice  A single matched fill from the off-chain engine.
    /// @dev     Packed into two 32-byte slots for calldata efficiency:
    ///          - makerAmountIn + takerAmountIn fit in one 32-byte slot (2 x uint128)
    ///         - maker/taker orders passed separately in parallel arrays.
    struct Fill {
        /// Amount of tokenIn the maker transfers to the taker.
        uint128 makerAmountIn;
        /// Amount of tokenIn the taker transfers to the maker.
        uint128 takerAmountIn;
    }

    /// @notice Signature components for an order.
    struct Sig {
        uint8 v;
        bytes32 r;
        bytes32 s;
    }

    // ─── Validation Logic ───────────────────────────────────────────────────

    /// @notice  Validate that a fill satisfies both orders' minimum amounts.
    /// @dev     Called per-fill in the settlement loop.
    ///          Reverts with typed errors - no string cost.
    function validate(
        Fill calldata fill,
        OrderLib.Order calldata makerOrder,
        OrderLib.Order calldata takerOrder
    ) internal pure {
        // Amounts must be non-zero.
        if (fill.makerAmountIn == 0) {
            revert FillLib__MakerAmountZero();
        }
        if (fill.takerAmountIn == 0) {
            revert FillLib__TakerAmountZero();
        }

        // Fill amounts cannot exceed order amounts.
        if (fill.makerAmountIn > makerOrder.amountIn) {
            revert FillLib__MakerAmountExceedsOrder(fill.makerAmountIn, makerOrder.amountIn);
        }
        if (fill.takerAmountIn > takerOrder.amountIn) {
            revert FillLib__TakerAmountExceedsOrder(fill.takerAmountIn, takerOrder.amountIn);
        }
        if (fill.makerAmountIn < makerOrder.amountOutMin) {
            revert FillLib__MakerAmountBelowMinimum(fill.makerAmountIn, makerOrder.amountOutMin);
        }
        if (fill.takerAmountIn < takerOrder.amountOutMin) {
            revert FillLib__TakerAmountBelowMinimum(fill.takerAmountIn, takerOrder.amountOutMin);
        }
    }

    // ─── Errors ───────────────────────────────────────────────────────────────

    error FillLib__MakerAmountZero();
    error FillLib__TakerAmountZero();
    error FillLib__MakerAmountExceedsOrder(uint256 fillAmount, uint256 orderAmount);
    error FillLib__TakerAmountExceedsOrder(uint256 fillAmount, uint256 orderAmount);
    error FillLib__MakerAmountBelowMinimum(uint256 fillAmount, uint256 minimum);
    error FillLib__TakerAmountBelowMinimum(uint256 fillAmount, uint256 minimum);
}
