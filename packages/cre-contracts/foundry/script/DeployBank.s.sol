// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {Bank} from "../src/Bank.sol";

/// @notice Deploys the Bank implementation and a UUPS proxy pointing to it.
///
/// Required env vars:
///   DEPLOYER_PRIVATE_KEY  — deployer's private key (hex, 0x-prefixed)
///   ADMIN_ADDRESS         — address that receives DEFAULT_ADMIN_ROLE + ADMIN_ROLE
///   PAUSER_ADDRESS        — address that receives PAUSER_ROLE
///   SYNC_USD_PROXY        — SyncUSD proxy address (output of DeploySyncUSD.s.sol)
///   FEE_COLLECTOR         — fee collector address (may be 0x0 to disable)
///
/// Usage (run after DeploySyncUSD.s.sol):
///   forge script script/DeployBank.s.sol --rpc-url base_sepolia --broadcast --verify
///   forge script script/DeployBank.s.sol --rpc-url arbitrum_sepolia --broadcast --verify
contract DeployBank is Script {
    function run() external returns (address proxy, address implementation) {
        uint256 deployerKey   = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address admin         = vm.envAddress("ADMIN_ADDRESS");
        address pauser        = vm.envAddress("PAUSER_ADDRESS");
        address syncUsd       = vm.envAddress("SYNC_USD_PROXY");
        address feeCollector  = vm.envOr("FEE_COLLECTOR", address(0));

        vm.startBroadcast(deployerKey);

        // 1. Deploy implementation (initializers disabled by constructor)
        Bank impl = new Bank();

        // 2. Encode initializer call
        bytes memory initData = abi.encodeCall(
            Bank.initialize,
            (admin, pauser, syncUsd, feeCollector)
        );

        // 3. Deploy UUPS proxy
        ERC1967Proxy proxyContract = new ERC1967Proxy(address(impl), initData);
        proxy = address(proxyContract);

        vm.stopBroadcast();

        implementation = address(impl);

        console2.log("Bank implementation :", implementation);
        console2.log("Bank proxy          :", proxy);
        console2.log("Admin               :", admin);
        console2.log("Pauser              :", pauser);
        console2.log("SyncUSD proxy       :", syncUsd);
        console2.log("Fee collector       :", feeCollector);
    }
}
