// SPDX-License-Identifier: MIT
pragma solidity ^0.8.33;

import {
    IERC20
} from "../lib/openzeppelin-contracts/contracts/token/ERC20/IERC20.sol";
import {IMeridianSettlement} from "./interfaces/IMeridianSettlement.sol";
import {FillLib} from "./libraries/FillLib.sol";
import {OrderLib} from "./libraries/OrderLib.sol";

/// @title  MeridianSettlement
/// @author Meridian Protocol
/// @notice Batch settlement contract for the Meridian off-chain matching engine.
/// @dev    ARCHITECTURE
///         ─────────────────────────────────────────────────────────────────
///         The off-chain engine produces batches of (maker, taker, fill) triples
///         This contract:
///             1. Verifies EIP-712 signatures of the makers and takers
///             2. Checks the nonses to prevent replay attacks
///             3. Validates fill amounts against order constraints
///             4. Executes atomic ERC-20 transfers to settle the batch
///             5. Emits structured events for the indexer.
///         PERFORMANCE NOTES
///         ─────────────────────────────────────────────────────────────────
///         - All orders/fill arrays are `calldata` - never copied to memory.
///         - Nonces use a bitmap (uint256 per 256 nonces) to pack 256 nonce
///           states into a single SLOAD/SSTORE, saving gas for active traders.
///         - Typed errors instead of string reverts - saves ~50 gas per revert.
///         - No SafeERC20 wrapper - we check return values manually to avoid
///           he extra DELEGATECALL (and its cost) for standard ERC-20 tokens.
///         - Solidity 0.8.33+ has built-in overflow checks, so we don't need
///           SafeMath or `unchecked` blocks.
///
///         SECURITY
///         ─────────────────────────────────────────────────────────────────
///         - Only the designated `operator` (the engine's settlement submitter)
///           can call `settleBatch` to prevent front-running and invalid batches.
///           This is a trusted role - the operator cannot steal funds only submit
///           valid signed fills.
///         - Traders retain sovereignty: they sign intents with deadlines and
///           can cancel nonces at any time.
///         - Reentrancy is not a concern because we update state (nonces) before
///           external calls and emit events after (Checks-Effects-Interactions pattern).
contract MeridianSettlement is IMeridianSettlement {
    // ─── Constants ────────────────────────────────────────────────────────

    string public constant NAME = "MeridianSettlement";
    string public constant VERSION = "1";

    bytes32 private constant EIP712_DOMAIN_TYPEHASH =
        keccak256(
            "EIP712Domain(string name, string  version, uint256 chainId, address verifyingContract)"
        );

    // ─── Immutables ───────────────────────────────────────────────────────

    /// @dev Set once at deployment, use in every signature verification.
    bytes32 public immutable DOMAIN_SEPARATOR;

    // ─── Storage ──────────────────────────────────────────────────────────

    /// @notice Address authorized to submit settlement batches.
    address public operator;

    /// @notice Owner ─ can rotate the operator.
    address public owner;

    /// @dev    Notice bitmap: trader -> (nonce / 256) -> bitmap.
    ///         Bit at position (nonce % 256) indicates whether that nonce has been used.
    ///         Saves 255 SLOADs compared to a mapping(uint64 => bool) for nonces.
    mapping(address => mapping(uint256 => uint256)) private _nonceBitmap;

    // ─── Constructor ──────────────────────────────────────────────────────

    constructor(address _operator) {
        owner = msg.sender;
        operator = _operator;

        DOMAIN_SEPARATOR = keccak256(
            abi.encode(
                EIP712_DOMAIN_TYPEHASH,
                keccak256(bytes(NAME)),
                keccak256(bytes(VERSION)),
                block.chainid,
                address(this)
            )
        );

        emit OperatorUpdated(address(0), _operator);
    }

    // ─── Modifiers ────────────────────────────────────────────────────────

    modifier onlyOperator() {
        _onlyOperator();
        _;
    }

    modifier onlyOwner() {
        _onlyOwner();
        _;
    }

    // ─── Core: batch settlement ─────────────────────────────────────────────

    /// @inheritdoc IMeridianSettlement
    /// @dev        Gas profile per fill (approximate, Base L2):
    ///               - 2x ecrecover:          ~6,000
    ///               - 2x nonce SLOAD/SSTORE: ~2,200 (cold) / ~100 (warm, same slot)
    ///               - 2x ERC-20 transfer:    ~15,000
    ///               - event emisssion:       ~1,500
    ///             ─────────────────────────────────
    ///            Total per fill:             ~25,000
    ///            Batch of 50 fills:          ~1,250,000 gas (fits in one Base block)
    function settleBatch(
        OrderLib.Order[] calldata makers,
        OrderLib.Order[] calldata takers,
        FillLib.Fill[] calldata fills,
        FillLib.Sig[] calldata makerSigs,
        FillLib.Sig[] calldata takerSigs,
        uint256 batchId
    ) external onlyOperator {
        uint256 len = fills.length;

        // Validate array lengths match - single check, revert early.
        if (len == 0) revert BatchEmpty();
        if (
            makers.length != len ||
            takers.length != len ||
            makerSigs.length != len ||
            takerSigs.length != len
        ) {
            revert ArrayLengthMismatch();
        }

        uint256 gasStart = gasleft();

        // @dev Solidity version 0.8.33+ has built-in overflow checks.
        for (uint256 i; i < len; ++i) {
            _settleFill(
                makers[i],
                takers[i],
                fills[i],
                makerSigs[i],
                takerSigs[i]
            );
        }

        emit BatchSettled(batchId, len, gasStart - gasleft());
    }

    // ─── Core: cancellation ─────────────────────────────────────────────────

    /// @inheritdoc IMeridianSettlement
    function cancelOrder(uint64 nonce) external {
        _useNonce(msg.sender, nonce);
        emit OrderCancelled(msg.sender, nonce);
    }

    // ─── Views ─────────────────────────────────────────────────

    /// @inheritdoc IMeridianSettlement
    function isNonceUsed(
        address trader,
        uint64 nonce
    ) external view returns (bool) {
        return _isNonceUsed(trader, nonce);
    }

    /// @inheritdoc IMeridianSettlement
    function domainSeparator() external view override returns (bytes32) {
        return DOMAIN_SEPARATOR;
    }

    // ─── Admin ───────────────────────────────────────────────

    /// @notice Rotate the settlement operator.
    function setOperator(address newOperator) external onlyOwner {
        address oldOperator = operator;
        operator = newOperator;
        emit OperatorUpdated(oldOperator, newOperator);
    }

    /// @notice Transfer contract ownership to a new address.
    function transferOwnership(address newOwner) external onlyOwner {
        owner = newOwner;
    }

    // ─── Internal functions ───────────────────────────────────────────────

    /// @dev   Process a single fill:
    ///        1. Validate order fields.
    ///        2. Verify maker and taker signatures.
    ///        3. Check and consume nonces as used (state change BEFORE transfers - CEI).
    ///        4. Validate fill amounts against order constraints.
    ///        5. Execute ERC-20 transfers to settle the fill.
    ///        6. Emit events.
    function _settleFill(
        OrderLib.Order calldata maker,
        OrderLib.Order calldata taker,
        FillLib.Fill calldata fill,
        FillLib.Sig calldata makerSig,
        FillLib.Sig calldata takerSig
    ) internal {
        // 1. Validate order fields (deadlines, zero checks).
        OrderLib.validate(maker);
        OrderLib.validate(taker);

        // 2. Verify EIP-712 signatures.
        OrderLib.verify(
            DOMAIN_SEPARATOR,
            maker,
            makerSig.v,
            makerSig.r,
            makerSig.s
        );
        OrderLib.verify(
            DOMAIN_SEPARATOR,
            taker,
            takerSig.v,
            takerSig.r,
            takerSig.s
        );

        // 3. Check and consume nonces (CEI).
        _useNonce(maker.trader, maker.nonce);
        _useNonce(taker.trader, taker.nonce);

        // 4. Validate fill amounts against order constraints.
        FillLib.validate(fill, maker, taker);

        // 5. Execute atomic transfers:
        //    maker sends tokenIn -> taker
        //    taker sends tokenIn -> maker
        //    (maker.tokenIn == taker.tokenOut, verified in FillLib)
        _transferFrom(
            maker.tokenIn,
            maker.trader,
            taker.trader,
            fill.makerAmountIn
        );
        _transferFrom(
            taker.tokenIn,
            taker.trader,
            maker.trader,
            fill.takerAmountIn
        );

        // 6. Emit structured fill event for indexer.
        emit OrderFilled(
            _fillId(maker.trader, taker.trader, maker.nonce, taker.nonce),
            maker.trader,
            taker.trader,
            maker.tokenIn,
            maker.tokenOut,
            fill.makerAmountIn,
            fill.takerAmountIn,
            maker.nonce,
            taker.nonce
        );
    }

    /// @dev   ERC-20 transfer with manual return-value check.
    ///        Avoids SafeERC20's extra overhead for standard tokens.
    ///        Non-standard tokens (e.g. USDT) that returns nothing still works
    ///        because we only revert on explicit `false` return.
    function _transferFrom(
        address token,
        address from,
        address to,
        uint256 amount
    ) internal {
        (bool success, bytes memory data) = token.call(
            abi.encodeWithSelector(
                IERC20.transferFrom.selector,
                from,
                to,
                amount
            )
        );
        if (!success) {
            revert TransferFailed(token, from, to, amount);
        }
        if (data.length > 0 && abi.decode(data, (bool)) == false) {
            revert TransferFailed(token, from, to, amount);
        }
    }

    // ─── Nonce bitmap ───────────────────────────────────────────────────

    /// @dev    Marks a nonce as used. Revers if already used (replay protection).
    ///         One SLOAD + one SSTORE per nonce.
    ///         256 nonces share a single storage slt -> amortized cost for traders
    ///         with many fills in one block.
    function _useNonce(address trader, uint64 nonce) internal {
        // Use bitwise operators for speed: >> 8 is / 256, & 0xff is % 256
        uint256 wordPos = uint256(nonce) >> 8;
        uint256 bitPos = uint256(nonce) & 0xff;

        // Use a 1 shifted by bitPos to create the mask
        uint256 mask = uint256(1) << bitPos;

        uint256 word = _nonceBitmap[trader][wordPos];
        if (word & mask != 0) revert NonceAlreadyUsed(trader, nonce);

        // Mark as used
        _nonceBitmap[trader][wordPos] = word | mask;
    }

    function _isNonceUsed(
        address trader,
        uint64 nonce
    ) internal view returns (bool) {
        uint256 wordPos = uint256(nonce) >> 8;
        uint256 bitPos = uint256(nonce) & 0xff;
        // Shift the word down and check the last bit
        return (_nonceBitmap[trader][wordPos] >> bitPos) & 1 == 1;
    }

    // ─── Helpers ─────────────────────────────────────────────────────────────

    /// @dev    Deterministic fill ID for indexing: cheaper than storing a counter.
    ///         Used as the indexed topic for fill lookups in the indexer.
    function _fillId(
        address maker,
        address taker,
        uint64 makerNonce,
        uint64 takerNonce
    ) internal pure returns (bytes32 id) {
        assembly {
            let ptr := mload(0x40) // get the free memory pointer.
            mstore(ptr, maker)
            mstore(add(ptr, 20), taker)
            mstore(add(ptr, 40), makerNonce)
            mstore(add(ptr, 48), takerNonce)
            id := keccak256(ptr, 56) // hash the 56 bytes we just packed.
        }
    }

    function _onlyOperator() internal view {
        if (msg.sender != operator) revert Unauthorized(msg.sender);
    }

    function _onlyOwner() internal view {
        if (msg.sender != owner) revert Unauthorized(msg.sender);
    }
}
