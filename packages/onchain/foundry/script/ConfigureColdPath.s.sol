// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {Bank} from "../src/Bank.sol";

/// @notice Configures cold-path rebalance controls for one Bank deployment.
///
/// Required env vars:
///   DEPLOYER_PRIVATE_KEY       — must hold ADMIN_ROLE on Bank
///   BANK_PROXY                 — Bank proxy address
///   MAX_REBALANCE_AMOUNT       — cap in SyncUSD base units; launch default is 5% of total supply
///   CCIP_ROUTER                — Chainlink CCIP router authorized to deliver inbound messages
///   DEST_CHAIN_ID              — outbound CCIP destination selector to allowlist
///   SOURCE_CHAIN_ID            — inbound CCIP source selector to allowlist
///   SOURCE_BANK_CONTRACT       — inbound source Bank contract to allowlist
contract ConfigureColdPath is Script {
    function run() external {
        uint256 deployerKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        Bank bank = Bank(vm.envAddress("BANK_PROXY"));
        uint256 maxRebalanceAmount = vm.envUint("MAX_REBALANCE_AMOUNT");
        address ccipRouter = vm.envAddress("CCIP_ROUTER");
        uint64 destChainId = uint64(vm.envUint("DEST_CHAIN_ID"));
        uint64 sourceChainId = uint64(vm.envUint("SOURCE_CHAIN_ID"));
        address sourceBank = vm.envAddress("SOURCE_BANK_CONTRACT");

        vm.startBroadcast(deployerKey);

        bank.setMaxRebalanceAmount(maxRebalanceAmount);
        bank.setCcipRouter(ccipRouter);
        bank.setAllowlistedDestChain(destChainId, true);
        bank.setAllowlistedSourceContract(sourceChainId, sourceBank, true);

        vm.stopBroadcast();

        require(bank.maxRebalanceAmount() == maxRebalanceAmount, "ConfigureColdPath: cap not set");
        require(bank.ccipRouter() == ccipRouter, "ConfigureColdPath: router not set");
        require(bank.allowlistedDestChains(destChainId), "ConfigureColdPath: dest not allowlisted");
        require(bank.allowlistedSourceContracts(sourceChainId, sourceBank), "ConfigureColdPath: source not allowlisted");

        console2.log("Cold path maxRebalanceAmount:", maxRebalanceAmount);
        console2.log("CCIP router                 :", ccipRouter);
        console2.log("Allowlisted destination chain:", destChainId);
        console2.log("Allowlisted source chain     :", sourceChainId);
        console2.log("Allowlisted source Bank      :", sourceBank);
    }
}
