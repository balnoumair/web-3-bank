// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {IAccessControl} from "@openzeppelin/contracts/access/IAccessControl.sol";
import {SyncUSD} from "../src/SyncUSD.sol";
import {Bank} from "../src/Bank.sol";

/// @notice Grants post-deployment roles so the contracts can interact:
///   1. MINTER_ROLE on SyncUSD  → Bank proxy  (deposit mints SyncUSD)
///   2. RELAYER_ROLE on Bank    → relayer EOA/contract
///   3. allowToken(USDC)        → Bank proxy  (whitelist USDC for deposit/withdrawal)
///
/// Required env vars:
///   DEPLOYER_PRIVATE_KEY   — must hold DEFAULT_ADMIN_ROLE on both contracts
///   SYNC_USD_PROXY         — SyncUSD proxy address
///   BANK_PROXY             — Bank proxy address
///   RELAYER_ADDRESS        — address that will call releaseHotPath
///   USDC_ADDRESS           — USDC token address on this chain
///
/// Usage (run after both proxy deployments):
///   forge script script/AssignRoles.s.sol --rpc-url base_sepolia --broadcast
///   forge script script/AssignRoles.s.sol --rpc-url arbitrum_sepolia --broadcast
contract AssignRoles is Script {
    function run() external {
        uint256 deployerKey    = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address syncUsdProxy   = vm.envAddress("SYNC_USD_PROXY");
        address bankProxy      = vm.envAddress("BANK_PROXY");
        address relayer        = vm.envAddress("RELAYER_ADDRESS");
        address usdc           = vm.envAddress("USDC_ADDRESS");

        SyncUSD syncUsd = SyncUSD(syncUsdProxy);
        Bank    bank    = Bank(bankProxy);

        bytes32 MINTER_ROLE  = syncUsd.MINTER_ROLE();
        bytes32 RELAYER_ROLE = bank.RELAYER_ROLE();

        vm.startBroadcast(deployerKey);

        // Grant Bank the right to mint/burn SyncUSD
        syncUsd.grantRole(MINTER_ROLE, bankProxy);
        console2.log("Granted MINTER_ROLE  on SyncUSD to Bank    :", bankProxy);

        // Grant relayer the right to execute hot-path releases
        bank.grantRole(RELAYER_ROLE, relayer);
        console2.log("Granted RELAYER_ROLE on Bank    to relayer  :", relayer);

        // Whitelist USDC as an accepted deposit/withdrawal token
        bank.allowToken(usdc);
        console2.log("Allowed token on Bank               (USDC)  :", usdc);

        vm.stopBroadcast();

        // ── Verification ──────────────────────────────────────────────
        require(
            syncUsd.hasRole(MINTER_ROLE, bankProxy),
            "AssignRoles: MINTER_ROLE not set"
        );
        require(
            bank.hasRole(RELAYER_ROLE, relayer),
            "AssignRoles: RELAYER_ROLE not set"
        );
        require(
            bank.allowedTokens(usdc),
            "AssignRoles: USDC not allowed"
        );

        console2.log("Role assignment verified.");
    }
}
