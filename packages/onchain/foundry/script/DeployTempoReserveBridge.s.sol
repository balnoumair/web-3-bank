// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {TempoReserveBridge, ILayerZeroEndpointV2} from "../src/TempoReserveBridge.sol";

/// @notice Deploy the TempoReserveBridge adapter — used on chains where CCTP is unavailable
///         (notably Tempo). Pairs with another TempoReserveBridge on the counterparty chain via
///         LayerZero v2.
///
/// Required env vars:
///   DEPLOYER_PRIVATE_KEY — deployer's private key (hex, 0x-prefixed)
///   ADMIN_ADDRESS        — receives DEFAULT_ADMIN_ROLE on the adapter
///   USDC_ADDRESS         — USDC ERC-20 on this chain (canonical on EVM, equivalent on Tempo)
///   LZ_ENDPOINT          — LayerZero v2 EndpointV2 address on this chain
///   LOCAL_LZ_EID         — LayerZero EID for this chain
contract DeployTempoReserveBridge is Script {
    function run() external returns (address adapter) {
        uint256 deployerKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address admin = vm.envAddress("ADMIN_ADDRESS");
        address usdc = vm.envAddress("USDC_ADDRESS");
        address lzEndpoint = vm.envAddress("LZ_ENDPOINT");
        uint32 localEid = uint32(vm.envUint("LOCAL_LZ_EID"));

        vm.startBroadcast(deployerKey);
        TempoReserveBridge bridge =
            new TempoReserveBridge(IERC20(usdc), ILayerZeroEndpointV2(lzEndpoint), localEid, admin);
        vm.stopBroadcast();

        adapter = address(bridge);

        console2.log("TempoReserveBridge  :", adapter);
        console2.log("Admin               :", admin);
        console2.log("USDC                :", usdc);
        console2.log("LZ endpoint         :", lzEndpoint);
        console2.log("Local LZ EID        :", localEid);
        console2.log("");
        console2.log("Post-deploy steps:");
        console2.log("  1. Top up the adapter with native gas to pay LZ fees: vm.deal-equivalent on prod.");
        console2.log("  2. Run ConfigureReservePath.s.sol to register chains, remotes, and signers.");
    }
}
