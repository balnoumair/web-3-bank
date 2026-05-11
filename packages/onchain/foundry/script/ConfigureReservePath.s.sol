// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {Bank} from "../src/Bank.sol";
import {IReserveBridge} from "../src/interfaces/IReserveBridge.sol";

/// @notice Configure reserve-path controls for one Bank deployment. Run **per chain**.
///
/// What this script does on the source Bank:
///   1. Registers the deployed `IReserveBridge` adapter via `setReserveBridge`.
///   2. Sets `maxReserveRebalanceAmount` (spec: 5% of total USDC reserves).
///   3. Sets the destination chain as allowlisted and registers the destination Bank.
///   4. Grants `RESERVE_REBALANCER_ROLE` to the Treasury reserve-ops signer.
///
/// **Per-adapter configuration** (CCTP domain map, LZ EID map, remote adapters, signer set,
/// threshold) is governed on the adapter contract itself. Use one of:
///   - For CCTP: `cast send <adapter> "setChainDomain(uint64,uint32)" ...` then
///                `setRemoteAdapter(uint32,address)` and `setBank(address)`.
///   - For Tempo: `setChainEid(uint64,uint32)`, `setRemoteAdapter(uint32,bytes32)`,
///                `setSigners(address[],uint8)`, `setBank(address)`.
/// Those calls vary too much per chain pair to hard-code here; see the runbook.
///
/// Required env vars:
///   DEPLOYER_PRIVATE_KEY    — holds ADMIN_ROLE on Bank
///   BANK_PROXY              — Bank proxy address on this chain
///   RESERVE_BRIDGE          — deployed IReserveBridge adapter for this chain
///   MAX_RESERVE_REBALANCE   — cap in USDC base units (6 decimals); spec default 5% of reserves
///   DEST_CHAIN_ID           — destination chain to allowlist
///   DEST_BANK_CONTRACT      — destination Bank address (the adapter passes this as destReserve)
///   RESERVE_RELAYER_ADDRESS — receives RESERVE_REBALANCER_ROLE; ideally distinct from cold-path relayer
contract ConfigureReservePath is Script {
    function run() external {
        uint256 deployerKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        Bank bank = Bank(vm.envAddress("BANK_PROXY"));
        IReserveBridge adapter = IReserveBridge(vm.envAddress("RESERVE_BRIDGE"));
        uint256 maxAmount = vm.envUint("MAX_RESERVE_REBALANCE");
        uint64 destChainId = uint64(vm.envUint("DEST_CHAIN_ID"));
        address destBank = vm.envAddress("DEST_BANK_CONTRACT");
        address reserveRelayer = vm.envAddress("RESERVE_RELAYER_ADDRESS");

        vm.startBroadcast(deployerKey);

        bank.setReserveBridge(adapter);
        bank.setMaxReserveRebalanceAmount(maxAmount);
        bank.setReserveDestination(destChainId, destBank);
        bank.setAllowlistedDestChain(destChainId, true);
        bank.grantRole(bank.RESERVE_REBALANCER_ROLE(), reserveRelayer);

        vm.stopBroadcast();

        require(
            address(bank.reserveBridge()) == address(adapter),
            "ConfigureReservePath: adapter not set"
        );
        require(
            bank.maxReserveRebalanceAmount() == maxAmount,
            "ConfigureReservePath: cap not set"
        );
        require(
            bank.reserveDestinations(destChainId) == destBank,
            "ConfigureReservePath: dest reserve not registered"
        );
        require(
            bank.allowlistedDestChains(destChainId),
            "ConfigureReservePath: dest not allowlisted"
        );
        require(
            bank.hasRole(bank.RESERVE_REBALANCER_ROLE(), reserveRelayer),
            "ConfigureReservePath: role not granted"
        );

        console2.log("Reserve adapter    :", address(adapter));
        console2.log("Max amount (USDC base):", maxAmount);
        console2.log("Allowlisted dest   :", destChainId);
        console2.log("Dest Bank          :", destBank);
        console2.log("Reserve relayer    :", reserveRelayer);
    }
}
