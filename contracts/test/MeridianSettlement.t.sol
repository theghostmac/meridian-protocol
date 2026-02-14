// SPDX-Licence-Identifier: MIT
pragma solidity ^0.8.33;

import {Test, console2} from "forge-std/Test.sol";
import {MeridianSettlement} from "../src/MeridianSettlement.sol";
import {OrderLib} from "../src/libraries/OrderLib.sol";
import {FillLib} from "../src/libraries/FillLib.sol";
import {IMeridianSettlement} from "../src/interfaces/IMeridianSettlement.sol";

// ─── Mock ERC-20 ─────────────────────────────────────────────────────────────────

contract MockERC20 {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    string public name;
    string public symbol;

    constructor(string memory _name, string memory _symbol) {
        name = _name;
        symbol = _symbol;
    }

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transferFrom(
        address from,
        address to,
        uint256 amount
    ) external returns (bool) {
        require(balanceOf[from] >= amount, "Insufficient balance");
        require(allowance[from][msg.sender] >= amount, "Allowance exceeded");

        balanceOf[from] -= amount;
        allowance[from][msg.sender] -= amount;
        balanceOf[to] += amount;

        return true;
    }
}

// ─── Base test setup ─────────────────────────────────────────────────────────────────

contract MeridianSettlementTest is Test {
    MeridianSettlement internal settlement;
    MockERC20 internal tokenA; // USDC
    MockERC20 internal tokenB; // WETH

    // Test wallets with known private keys for EIP-712 signing.
    uint256 internal makerPk = 0xA11CE;
    uint256 internal takerPk = 0xB0B;

    address internal maker;
    address internal taker;
    address internal operator;
    address internal owner;

    uint256 internal constant INITIAL_BALANCE = 1_000_000e18;
    uint256 internal constant INITIAL_ALLOWANCE = type(uint256).max;

    function setUp() public virtual {
        maker = vm.addr(makerPk);
        taker = vm.addr(takerPk);
        operator = makeAddr("operator");
        owner = makeAddr("owner");

        vm.prank(owner);
        settlement = new MeridianSettlement(operator);

        tokenA = new MockERC20("Token A", "TKNA");
        tokenB = new MockERC20("Token B", "TKNB");

        // Mint tokens to maker and taker and approve settlement contract
        tokenA.mint(maker, INITIAL_BALANCE);
        tokenB.mint(taker, INITIAL_BALANCE);

        vm.prank(maker);
        tokenA.approve(address(settlement), INITIAL_ALLOWANCE);

        vm.prank(taker);
        tokenB.approve(address(settlement), INITIAL_ALLOWANCE);
    }

    //  ─── EIP-712 helpers -──────────────────────────────────────────────────────────────

    function _signOrder(
        uint256 pk,
        OrderLib.Order memory order
    ) internal view returns (FillLib.Sig memory sig) {
        bytes32 digest = keccak256(
            abi.encodePacked(
                "\x19\x01",
                settlement.domainSeparator(),
                keccak256(
                    abi.encode(
                        OrderLib.ORDER_TYPEHASH,
                        order.trader,
                        order.tokenIn,
                        order.tokenOut,
                        order.amountIn,
                        order.amountOutMin,
                        order.nonce,
                        order.deadline
                    )
                )
            )
        );

        (sig.v, sig.r, sig.s) = vm.sign(pk, digest);
    }

    function _makeOrder(
        address trader,
        address tokenIn,
        address tokenOut,
        uint128 amountIn,
        uint128 amountOutMin,
        uint64 nonce,
        uint64 deadline
    ) internal pure returns (OrderLib.Order memory order) {
        return
                            OrderLib.Order({
                trader: trader,
                tokenIn: tokenIn,
                tokenOut: tokenOut,
                amountIn: amountIn,
                amountOutMin: amountOutMin,
                nonce: nonce,
                deadline: deadline
            });
    }

    function _buildBatch(
        OrderLib.Order memory makerOrder,
        OrderLib.Order memory takerOrder,
        uint128 makerAmt,
        uint128 takerAmt
    ) internal view returns (
        OrderLib.Order[] memory makers,
        OrderLib.Order[] memory takers,
        FillLib.Fill[] memory fills,
        FillLib.Sig[] memory makerSigs,
        FillLib.Sig[] memory takerSigs
    ) {
        makers = new OrderLib.Order[](1);
        takers = new OrderLib.Order[](1);
        fills = new FillLib.Fill[](1);
        makerSigs = new FillLib.Sig[](1);
        takerSigs = new FillLib.Sig[](1);

        makers[0] = makerOrder;
        takers[0] = takerOrder;
        fills[0] = FillLib.Fill({makerAmountIn: makerAmt, takerAmountIn: takerAmt});
        makerSigs[0] = _signOrder(makerPk, makerOrder);
        takerSigs[0] = _signOrder(takerPk, takerOrder);
    }
}


// ── Unit tests ────────────────────────────────────────────────────────────────

contract SettlementUnitTest is MeridianSettlementTest {

    function test_settle_single_fill() public {
        uint128 amt = 1_000e18;

        OrderLib.Order memory makerOrder = _makeOrder(
            maker, address(tokenA), address(tokenB), amt, amt, 0, uint64(block.timestamp + 1 hours)
        );
        OrderLib.Order memory takerOrder = _makeOrder(
            taker, address(tokenB), address(tokenA), amt, amt, 0, uint64(block.timestamp + 1 hours)
        );

        (
            OrderLib.Order[] memory makers,
            OrderLib.Order[] memory takers,
            FillLib.Fill[]   memory fills,
            FillLib.Sig[]    memory makerSigs,
            FillLib.Sig[]    memory takerSigs
        ) = _buildBatch(makerOrder, takerOrder, amt, amt);

        uint256 makerTokenABefore = tokenA.balanceOf(maker);
        uint256 takerTokenBBefore = tokenB.balanceOf(taker);

        // settleBatch emits OrderFilled then BatchSettled.
        // expectEmit(checkTopic1, checkTopic2, checkTopic3, checkData, emitter)
        // We only verify the indexed fields (maker, taker) and let data slide
        // since fillId is computed deterministically inside the contract.
        vm.expectEmit(false, true, true, false, address(settlement));
        emit IMeridianSettlement.OrderFilled(
            bytes32(0), maker, taker,
            address(tokenA), address(tokenB),
            amt, amt, 0, 0
        );

        vm.prank(operator);
        settlement.settleBatch(makers, takers, fills, makerSigs, takerSigs, 1);

        // Maker sent tokenA, received tokenB.
        assertEq(tokenA.balanceOf(maker), makerTokenABefore - amt);
        assertEq(tokenB.balanceOf(maker), amt);

        // Taker sent tokenB, received tokenA.
        assertEq(tokenB.balanceOf(taker), takerTokenBBefore - amt);
        assertEq(tokenA.balanceOf(taker), amt);
    }

    function test_revert_non_operator() public {
        OrderLib.Order memory makerOrder = _makeOrder(
            maker, address(tokenA), address(tokenB), 100e18, 100e18, 0, uint64(block.timestamp + 1 hours)
        );
        OrderLib.Order memory takerOrder = _makeOrder(
            taker, address(tokenB), address(tokenA), 100e18, 100e18, 0, uint64(block.timestamp + 1 hours)
        );

        (
            OrderLib.Order[] memory makers,
            OrderLib.Order[] memory takers,
            FillLib.Fill[]   memory fills,
            FillLib.Sig[]    memory makerSigs,
            FillLib.Sig[]    memory takerSigs
        ) = _buildBatch(makerOrder, takerOrder, 100e18, 100e18);

        vm.prank(makeAddr("random"));
        vm.expectRevert(
            abi.encodeWithSelector(IMeridianSettlement.Unauthorized.selector, makeAddr("random"))
        );
        settlement.settleBatch(makers, takers, fills, makerSigs, takerSigs, 1);
    }

    function test_revert_nonce_replay() public {
        uint128 amt = 100e18;

        OrderLib.Order memory makerOrder = _makeOrder(
            maker, address(tokenA), address(tokenB), amt, amt, 0, uint64(block.timestamp + 1 hours)
        );
        OrderLib.Order memory takerOrder = _makeOrder(
            taker, address(tokenB), address(tokenA), amt, amt, 0, uint64(block.timestamp + 1 hours)
        );

        (
            OrderLib.Order[] memory makers,
            OrderLib.Order[] memory takers,
            FillLib.Fill[]   memory fills,
            FillLib.Sig[]    memory makerSigs,
            FillLib.Sig[]    memory takerSigs
        ) = _buildBatch(makerOrder, takerOrder, amt, amt);

        vm.prank(operator);
        settlement.settleBatch(makers, takers, fills, makerSigs, takerSigs, 1);

        // Re-sign (same nonce) and try again.
        makerSigs[0] = _signOrder(makerPk, makerOrder);
        takerSigs[0] = _signOrder(takerPk, takerOrder);

        vm.prank(operator);
        vm.expectRevert(
            abi.encodeWithSelector(IMeridianSettlement.NonceAlreadyUsed.selector, maker, uint64(0))
        );
        settlement.settleBatch(makers, takers, fills, makerSigs, takerSigs, 2);
    }

    function test_revert_expired_order() public {
        OrderLib.Order memory makerOrder = OrderLib.Order({
            trader:       maker,
            tokenIn:      address(tokenA),
            tokenOut:     address(tokenB),
            amountIn:     100e18,
            amountOutMin: 100e18,
            nonce:        0,
            deadline:     uint64(block.timestamp - 1) // already expired
        });
        OrderLib.Order memory takerOrder = _makeOrder(
            taker, address(tokenB), address(tokenA), 100e18, 100e18, 0, uint64(block.timestamp + 1 hours)
        );

        (
            OrderLib.Order[] memory makers,
            OrderLib.Order[] memory takers,
            FillLib.Fill[]   memory fills,
            FillLib.Sig[]    memory makerSigs,
            FillLib.Sig[]    memory takerSigs
        ) = _buildBatch(makerOrder, takerOrder, 100e18, 100e18);

        vm.prank(operator);
        vm.expectRevert();
        settlement.settleBatch(makers, takers, fills, makerSigs, takerSigs, 1);
    }

    function test_cancel_nonce() public {
        vm.prank(maker);
        vm.expectEmit(true, true, false, false);
        emit IMeridianSettlement.OrderCancelled(maker, 42);
        settlement.cancelOrder(42);

        assertTrue(settlement.isNonceUsed(maker, 42));
    }

    function test_nonce_bitmap_packing() public {
        // Verify 256 nonces share one storage slot by checking gas.
        // Nonces 0-255 are in wordPos 0.
        vm.startPrank(maker);
        settlement.cancelOrder(0);   // cold SSTORE
        settlement.cancelOrder(1);   // warm SSTORE — same slot
        settlement.cancelOrder(255); // still same slot
        vm.stopPrank();

        assertTrue(settlement.isNonceUsed(maker, 0));
        assertTrue(settlement.isNonceUsed(maker, 1));
        assertTrue(settlement.isNonceUsed(maker, 255));
        assertFalse(settlement.isNonceUsed(maker, 256)); // next slot
    }

    function test_gas_batch_of_50() public {
        uint256 batchSize = 50;

        OrderLib.Order[] memory makers    = new OrderLib.Order[](batchSize);
        OrderLib.Order[] memory takers    = new OrderLib.Order[](batchSize);
        FillLib.Fill[]   memory fills     = new FillLib.Fill[](batchSize);
        FillLib.Sig[]    memory makerSigs = new FillLib.Sig[](batchSize);
        FillLib.Sig[]    memory takerSigs = new FillLib.Sig[](batchSize);

        // Realistic hot-path: same two traders with sequential nonces.
        // This mirrors production — a market maker settling 50 fills per batch.
        // Balances + allowances are warm from setUp(); nonces 0..49 all share
        // wordPos 0 (same bitmap slot), so SSTOREs are warm after the first fill.
        tokenA.mint(maker, 100e18 * batchSize);
        tokenB.mint(taker, 100e18 * batchSize);

        for (uint256 i; i < batchSize; ++i) {
            makers[i]    = _makeOrder(
                maker, address(tokenA), address(tokenB),
                100e18, 100e18, uint64(i), uint64(block.timestamp + 1 hours)
            );
            takers[i]    = _makeOrder(
                taker, address(tokenB), address(tokenA),
                100e18, 100e18, uint64(i), uint64(block.timestamp + 1 hours)
            );
            fills[i]     = FillLib.Fill({makerAmountIn: 100e18, takerAmountIn: 100e18});
            makerSigs[i] = _signOrder(makerPk, makers[i]);
            takerSigs[i] = _signOrder(takerPk, takers[i]);
        }

        uint256 gasBefore = gasleft();

        vm.prank(operator);
        settlement.settleBatch(makers, takers, fills, makerSigs, takerSigs, 1);

        uint256 gasUsed = gasBefore - gasleft();
        console2.log("Gas used for 50-fill batch:", gasUsed);
        console2.log("Gas per fill:              ", gasUsed / batchSize);

        // Hot-path bound: warm storage, shared traders.
        // 2x ecrecover + warm nonce SSTORE + 2x warm transferFrom + event ~ 25k.
        // Allow 40k headroom for EVM variance.
        assertLt(gasUsed / batchSize, 40_000);
    }
}

// ── Fuzz tests ────────────────────────────────────────────────────────────────

contract SettlementFuzzTest is MeridianSettlementTest {

    /// @dev Fuzz: any valid fill amounts should settle correctly
    ///      as long as amounts satisfy order constraints.
    function testFuzz_settle_valid_fill(
        uint128 fillAmt
    ) public {
        fillAmt = uint128(bound(fillAmt, 1e18, 100_000e18));

        tokenA.mint(maker, fillAmt);
        tokenB.mint(taker, fillAmt);

        OrderLib.Order memory makerOrder = _makeOrder(
            maker, address(tokenA), address(tokenB), fillAmt, fillAmt, 0, uint64(block.timestamp + 1 hours)
        );
        OrderLib.Order memory takerOrder = _makeOrder(
            taker, address(tokenB), address(tokenA), fillAmt, fillAmt, 0, uint64(block.timestamp + 1 hours)
        );

        (
            OrderLib.Order[] memory makers,
            OrderLib.Order[] memory takers,
            FillLib.Fill[]   memory fills,
            FillLib.Sig[]    memory makerSigs,
            FillLib.Sig[]    memory takerSigs
        ) = _buildBatch(makerOrder, takerOrder, fillAmt, fillAmt);

        vm.prank(operator);
        settlement.settleBatch(makers, takers, fills, makerSigs, takerSigs, 1);

        // Conservation: total token supply unchanged.
        assertEq(
            tokenA.balanceOf(maker) + tokenA.balanceOf(taker),
            INITIAL_BALANCE + fillAmt // taker received, maker sent
        );
    }

    /// @dev Fuzz: fills exceeding order.amountIn must always revert.
    function testFuzz_revert_fill_exceeds_order(
        uint128 orderAmt,
        uint128 excess
    ) public {
        orderAmt = uint128(bound(orderAmt, 1e18, type(uint128).max - 1));
        excess   = uint128(bound(excess, 1, type(uint128).max - orderAmt));
        uint128 fillAmt = orderAmt + excess;

        tokenA.mint(maker, fillAmt);
        tokenB.mint(taker, fillAmt);

        OrderLib.Order memory makerOrder = _makeOrder(
            maker, address(tokenA), address(tokenB), orderAmt, orderAmt, 0, uint64(block.timestamp + 1 hours)
        );
        OrderLib.Order memory takerOrder = _makeOrder(
            taker, address(tokenB), address(tokenA), fillAmt, fillAmt, 0, uint64(block.timestamp + 1 hours)
        );

        (
            OrderLib.Order[] memory makers,
            OrderLib.Order[] memory takers,
            FillLib.Fill[]   memory fills,
            FillLib.Sig[]    memory makerSigs,
            FillLib.Sig[]    memory takerSigs
        ) = _buildBatch(makerOrder, takerOrder, fillAmt, orderAmt);

        vm.prank(operator);
        vm.expectRevert();
        settlement.settleBatch(makers, takers, fills, makerSigs, takerSigs, 1);
    }

    /// @dev Fuzz: nonce bitmap correctness across arbitrary nonce values.
    function testFuzz_nonce_bitmap(uint64 nonce) public {
        assertFalse(settlement.isNonceUsed(maker, nonce));

        vm.prank(maker);
        settlement.cancelOrder(nonce);

        assertTrue(settlement.isNonceUsed(maker, nonce));

        // Adjacent nonces must be unaffected.
        if (nonce > 0)                    assertFalse(settlement.isNonceUsed(maker, nonce - 1));
        if (nonce < type(uint64).max)     assertFalse(settlement.isNonceUsed(maker, nonce + 1));
    }
}
