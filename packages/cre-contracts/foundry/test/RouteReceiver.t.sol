// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console} from "forge-std/Test.sol";
import {RouteReceiver} from "../src/RouteReceiver.sol";

contract RouteReceiverTest is Test {
    RouteReceiver public receiver;

    address public owner = address(this);
    address public publisher = address(0x1);
    address public unauthorized = address(0x2);
    address public publisher2 = address(0x3);

    event RoutePublished(
        string indexed runIdHash,
        string runId,
        string customerId,
        string recommendedChain,
        uint256 score,
        uint256 timestamp
    );

    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);

    function setUp() public {
        receiver = new RouteReceiver();
        receiver.addPublisher(publisher);
    }

    // ── Authorized publish + event emission ────────────────────────────

    function test_authorizedPublisherCanWriteRouteAndEmitEvent() public {
        vm.prank(publisher);

        vm.expectEmit(true, false, false, true);
        emit RoutePublished(
            "run-001",
            "run-001",
            "customer-1",
            "base-sepolia",
            82,
            1740000000
        );

        receiver.publishRoute(
            "run-001",
            "customer-1",
            "base-sepolia",
            82,
            1740000000
        );
    }

    // ── Unauthorized address reverts ───────────────────────────────────

    function test_unauthorizedAddressReverts() public {
        vm.prank(unauthorized);

        vm.expectRevert("RouteReceiver: not authorized publisher");
        receiver.publishRoute(
            "run-002",
            "customer-1",
            "base-sepolia",
            75,
            1740000001
        );
    }

    // ── Duplicate runId is rejected ────────────────────────────────────

    function test_duplicateRunIdReverts() public {
        vm.startPrank(publisher);

        receiver.publishRoute(
            "run-003",
            "customer-1",
            "base-sepolia",
            82,
            1740000000
        );

        vm.expectRevert("RouteReceiver: runId already published");
        receiver.publishRoute(
            "run-003",
            "customer-1",
            "base-sepolia",
            82,
            1740000000
        );

        vm.stopPrank();
    }

    // ── getLatestRoute returns correct data ────────────────────────────

    function test_getLatestRouteReturnsCorrectData() public {
        vm.prank(publisher);
        receiver.publishRoute(
            "run-004",
            "customer-1",
            "base-sepolia",
            90,
            1740000010
        );

        (
            string memory runId,
            string memory recommendedChain,
            uint256 score,
            uint256 timestamp
        ) = receiver.getLatestRoute("customer-1");

        assertEq(runId, "run-004");
        assertEq(recommendedChain, "base-sepolia");
        assertEq(score, 90);
        assertEq(timestamp, 1740000010);
    }

    // ── Multiple customers store independently ─────────────────────────

    function test_multipleCustomersStoreIndependently() public {
        vm.startPrank(publisher);

        receiver.publishRoute(
            "run-005",
            "customer-1",
            "base-sepolia",
            80,
            1740000020
        );
        receiver.publishRoute(
            "run-006",
            "customer-2",
            "arbitrum-sepolia",
            95,
            1740000021
        );

        vm.stopPrank();

        (string memory runId1, string memory chain1, uint256 score1, ) =
            receiver.getLatestRoute("customer-1");
        (string memory runId2, string memory chain2, uint256 score2, ) =
            receiver.getLatestRoute("customer-2");

        assertEq(runId1, "run-005");
        assertEq(chain1, "base-sepolia");
        assertEq(score1, 80);

        assertEq(runId2, "run-006");
        assertEq(chain2, "arbitrum-sepolia");
        assertEq(score2, 95);
    }

    // ── Owner can add and remove publishers ────────────────────────────

    function test_ownerCanAddPublisher() public {
        receiver.addPublisher(publisher2);

        vm.prank(publisher2);
        receiver.publishRoute(
            "run-007",
            "customer-1",
            "base-sepolia",
            70,
            1740000030
        );

        (string memory runId, , , ) = receiver.getLatestRoute("customer-1");
        assertEq(runId, "run-007");
    }

    function test_ownerCanRemovePublisher() public {
        receiver.removePublisher(publisher);

        vm.prank(publisher);
        vm.expectRevert("RouteReceiver: not authorized publisher");
        receiver.publishRoute(
            "run-008",
            "customer-1",
            "base-sepolia",
            60,
            1740000040
        );
    }

    function test_nonOwnerCannotAddPublisher() public {
        vm.prank(unauthorized);
        vm.expectRevert("RouteReceiver: not owner");
        receiver.addPublisher(unauthorized);
    }

    function test_nonOwnerCannotRemovePublisher() public {
        vm.prank(unauthorized);
        vm.expectRevert("RouteReceiver: not owner");
        receiver.removePublisher(publisher);
    }

    // ── isRunPublished returns correct value ───────────────────────────

    function test_isRunPublishedReturnsFalseForUnknownRun() public view {
        assertFalse(receiver.isRunPublished("run-unknown"));
    }

    function test_isRunPublishedReturnsTrueAfterPublish() public {
        vm.prank(publisher);
        receiver.publishRoute(
            "run-009",
            "customer-1",
            "base-sepolia",
            85,
            1740000050
        );

        assertTrue(receiver.isRunPublished("run-009"));
    }

    // ── Address(0) guard ──────────────────────────────────────────────

    function test_addPublisherRevertsOnZeroAddress() public {
        vm.expectRevert("RouteReceiver: zero address");
        receiver.addPublisher(address(0));
    }

    // ── Ownership transfer ────────────────────────────────────────────

    function test_ownerCanTransferOwnership() public {
        address newOwner = address(0x99);

        vm.expectEmit(true, true, false, false);
        emit OwnershipTransferred(owner, newOwner);

        receiver.transferOwnership(newOwner);
        assertEq(receiver.owner(), newOwner);
    }

    function test_transferOwnershipRevertsOnZeroAddress() public {
        vm.expectRevert("RouteReceiver: zero address");
        receiver.transferOwnership(address(0));
    }

    function test_nonOwnerCannotTransferOwnership() public {
        vm.prank(unauthorized);
        vm.expectRevert("RouteReceiver: not owner");
        receiver.transferOwnership(unauthorized);
    }
}
