// SPDX-License-Identifier: MIT
pragma solidity ^0.8.33;

/// @title   OrderLib
/// @notice  EIP-712 type hashing and signature verification for Meridian orders.
/// @dev     All struct are tightly packed. No dynamic types in the hash to
///          keep verification gas minimal.
library OrderLib {
    // ─── Type Strings ────────────────────────────────────────────────────────

    /// @dev EIP-712 typehash for a signed Order intent.
    ///      Fields ordered by type size (descending) to minimize padding.
    bytes32 internal constant ORDER_TYPEHASH = keccak256(
        "Order("
           "address trader,"
           "address tokenIn,"
           "address tokenOut,"
           "uint128 amountIn,"
           "uint128 amountOutMin,"
           "uint64 nonce,"
           "uint64 deadline"
        ")"
    );

    // ─── Structs ─────────────────────────────────────────────────────────────

    /// @notice  A signed intent submitted by a trader.
    /// @dev     Packed into two 32-byte slots after the addresses:
    ///          slot0: trader (20) + 12 byte padding -> address
    ///          slot1: tokenIn (20) + 12 bytes padding -> address
    ///          slot2: tokenOut (20) + 12 bytes padding -> address
    ///          slot3: amountIn (16) + amountOutMin (16) -> uint128, uint128
    ///          slot4: nonce (8) + deadline (8 + 16 bytes padding
    struct Order {
        address trader;
        address tokenIn;
        address tokenOut;
        uint128 amountIn;
        uint128 amountOutMin;
        uint64  nonce;
        uint64  deadline;
    }

    // ─── Hashing ─────────────────────────────────────────────────────────────

    /// @notice  Compute the EIP-712 struct hash of an Order.
    /// @dev     Uses abi.encode (not encodePacked) per EIP-712 spec.
    ///          All fixed-size types - no dynamic length issues.
    function hash(Order calldata order) internal pure returns (bytes32) {
        return keccak256(abi.encode(
            ORDER_TYPEHASH,
            order.trader,
            order.tokenIn,
            order.tokenOut,
            order.amountIn,
            order.amountOutMin,
            order.nonce,
            order.deadline
        ));
    }

    // ─── Signature Verification ─────────────────────────────────────────────

    /// @notice Recover the signer of an EIP-712 digest.
    /// @param  domainSeparator  The contract's EIP-712 domain separator.
    /// @param  order            The order struct to verify.
    /// @param  v, r, s          The ECDSA components of the signature.
    ///  @return signer           The recovered address, that signed the order.
    function recover(
        bytes32 domainSeparator,
        Order calldata order,
        uint8 v,
        bytes32 r,
        bytes32 s
    ) internal pure returns (address signer) {
        bytes32 digest = keccak256(abi.encodePacked(
            "\x19\x01",
            domainSeparator,
            hash(order)
        ));
        signer = ecrecover(digest, v, r, s);
        // ecrecover returns address(0) on failure, which we can check in the caller.
    }

    /// @notice Validate that the order's signer matches order.trader.
    /// @dev    Reverts with a typed error if signature is invalid or signer doesn't match.
    function verify(
        bytes32 domainSeparator,
        Order calldata order,
        uint8 v,
        bytes32 r,
        bytes32 s
    ) internal pure {
        address signer = recover(domainSeparator, order, v, r, s);
        if (signer == address(0) || signer != order.trader) {
            revert InvalidSignature(order.trader, signer);
        }
    }

    // ─── Validation ────────────────────────────────────────────────────────────

    /// @notice Validate order fields before settlement.
    function validate(Order calldata order) internal view {
        if (order.trader == address(0)) revert ZeroAddress();
        if (order.tokenIn == address(0) || order.tokenOut == address(0)) revert ZeroAddress();
        if (order.amountIn == 0 || order.amountOutMin == 0) revert ZeroAmount();
        if (block.timestamp > order.deadline) revert OrderExpired(order.deadline, uint64(block.timestamp));
    }
    
    // ─── Errors ─────────────────────────────────────────────────────────────

    error InvalidSignature(address expected, address recovered);
    error ZeroAddress();
    error ZeroAmount();
    error OrderExpired(uint64 deadline, uint64 currentTime);
}
