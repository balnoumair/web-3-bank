// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {BankContract} from "../src/BankContract.sol";

/// @notice Deploys the BankContract implementation and a UUPS proxy pointing to it.
///
/// Required env vars:
///   DEPLOYER_PRIVATE_KEY  — deployer's private key (hex, 0x-prefixed)
///   ADMIN_ADDRESS         — address that receives DEFAULT_ADMIN_ROLE + ADMIN_ROLE
///   PAUSER_ADDRESS        — address that receives PAUSER_ROLE
///   USDC_ADDRESS          — USDC token address on the target chain
///   SYNC_USD_PROXY        — SyncUSD proxy address (output of DeploySyncUSD.s.sol)
///
/// Usage (run after DeploySyncUSD.s.sol):
///   forge script script/DeployBankContract.s.sol --rpc-url base_sepolia --broadcast --verify
///   forge script script/DeployBankContract.s.sol --rpc-url arbitrum_sepolia --broadcast --verify
contract DeployBankContract is Script {
    function run() external returns (address proxy, address implementation) {
        uint256 deployerKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address admin     = vm.envAddress("ADMIN_ADDRESS");
        address pauser    = vm.envAddress("PAUSER_ADDRESS");
        address usdc      = vm.envAddress("USDC_ADDRESS");
        address syncUsd   = vm.envAddress("SYNC_USD_PROXY");

        vm.startBroadcast(deployerKey);

        // 1. Deploy implementation (initializers disabled by constructor)
        BankContract impl = new BankContract();

        // 2. Encode initializer call
        bytes memory initData = abi.encodeCall(
            BankContract.initialize,
            (admin, pauser, usdc, syncUsd)
        );

        // 3. Deploy UUPS proxy
        ERC1967Proxy proxyContract = new ERC1967Proxy(address(impl), initData);
        proxy = address(proxyContract);

        vm.stopBroadcast();

        implementation = address(impl);

        console2.log("BankContract implementation :", implementation);
        console2.log("BankContract proxy          :", proxy);
        console2.log("Admin                       :", admin);
        console2.log("Pauser                      :", pauser);
        console2.log("USDC                        :", usdc);
        console2.log("SyncUSD proxy               :", syncUsd);
    }
}
