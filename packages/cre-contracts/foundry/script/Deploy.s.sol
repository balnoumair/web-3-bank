// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Script, console2} from "forge-std/Script.sol";
import {RouteReceiver} from "../src/RouteReceiver.sol";

contract Deploy is Script {
    function run() external returns (RouteReceiver receiver) {
        uint256 deployerPrivateKey = vm.envUint("DEPLOYER_PRIVATE_KEY");
        address deployer = vm.addr(deployerPrivateKey);

        vm.startBroadcast(deployerPrivateKey);

        receiver = new RouteReceiver();
        receiver.addPublisher(deployer);

        vm.stopBroadcast();

        console2.log("Deployed RouteReceiver at:", address(receiver));
        console2.log("Authorized publisher:", deployer);
    }
}
