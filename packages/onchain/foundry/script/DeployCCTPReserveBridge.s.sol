// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {
    CCTPReserveBridge,
    ITokenMessenger,
    IMessageTransmitter
} from "../src/CCTPReserveBridge.sol";

/// @notice Deploy the CCTPReserveBridge adapter on a CCTP-supported chain.
///
/// Run **per chain** that should host a Bank Contract with USDC reserve bridging via CCTP.
/// After this script, see `ConfigureReservePath.s.sol` to wire the adapter into the Bank
/// and to register paired remote adapters across chains.
///
/// Required env vars:
///   DEPLOYER_PRIVATE_KEY — deployer's private key (hex, 0x-prefixed)
///   ADMIN_ADDRESS        — receives DEFAULT_ADMIN_ROLE on the adapter
///   USDC_ADDRESS         — canonical USDC ERC-20 on this chain
///   CCTP_TOKEN_MESSENGER — Circle TokenMessenger address (per Circle docs)
///   CCTP_MESSAGE_TRANSMITTER — Circle MessageTransmitter address
///   LOCAL_CCTP_DOMAIN    — CCTP domain id for this chain (Ethereum=0, Avalanche=1, OP=2, Arb=3, Base=6, …)
contract DeployCCTPReserveBridge is Script {
    function run() external returns (address adapter) {
        uint256 deployerKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address admin = vm.envAddress("ADMIN_ADDRESS");
        address usdc = vm.envAddress("USDC_ADDRESS");
        address tokenMessenger = vm.envAddress("CCTP_TOKEN_MESSENGER");
        address messageTransmitter = vm.envAddress("CCTP_MESSAGE_TRANSMITTER");
        uint32 localDomain = uint32(vm.envUint("LOCAL_CCTP_DOMAIN"));

        vm.startBroadcast(deployerKey);
        CCTPReserveBridge bridge = new CCTPReserveBridge(
            IERC20(usdc),
            ITokenMessenger(tokenMessenger),
            IMessageTransmitter(messageTransmitter),
            localDomain,
            admin
        );
        vm.stopBroadcast();

        adapter = address(bridge);

        console2.log("CCTPReserveBridge   :", adapter);
        console2.log("Admin               :", admin);
        console2.log("USDC                :", usdc);
        console2.log("TokenMessenger      :", tokenMessenger);
        console2.log("MessageTransmitter  :", messageTransmitter);
        console2.log("Local CCTP domain   :", localDomain);
    }
}
