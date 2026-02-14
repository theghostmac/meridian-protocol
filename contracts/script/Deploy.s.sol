// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console2} from "forge-std/Script.sol";
import {MeridianSettlement} from "../src/MeridianSettlement.sol";

/// @notice Deploy MeridianSettlement to a target chain.
/// @dev    Run with:
///         forge script script/Deploy.s.sol \
///           --rpc-url $RPC_URL \
///           --broadcast \
///           --verify \
///           -vvvv
contract DeployMeridianSettlement is Script {
    function run() external {
        address operator = vm.envAddress("OPERATOR_ADDRESS");

        vm.startBroadcast();

        MeridianSettlement settlement = new MeridianSettlement(operator);

        console2.log("MeridianSettlement deployed at: ", address(settlement));
        console2.log("Operator:                       ", operator);
        console2.log("Domain separator:               ");
        console2.logBytes32(settlement.domainSeparator());

        vm.stopBroadcast();
    }
}
