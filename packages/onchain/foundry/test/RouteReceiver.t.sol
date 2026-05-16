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
        uint256 timestamp,
        uint256 blockTimestamp
    );

    event OwnershipTransferStarted(address indexed previousOwner, address indexed newOwner);
    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    event ChainDecommissioningMarked(uint256 indexed chainId);
    event ChainDecommissionFinalized(uint256 indexed chainId);

    function setUp() public {
        receiver = new RouteReceiver();
        receiver.addPublisher(publisher);
    }

    // ── Authorized publish + event emission ────────────────────────────

    function test_authorizedPublisherCanWriteRouteAndEmitEvent() public {
        vm.prank(publisher);

        vm.expectEmit(true, false, false, true);
        emit RoutePublished("run-001", "run-001", "customer-1", "base-sepolia", 82, 1740000000, block.timestamp);

        receiver.publishRoute("run-001", "customer-1", "base-sepolia", 82, 1740000000);
    }

    // ── Unauthorized address reverts ───────────────────────────────────

    function test_unauthorizedAddressReverts() public {
        vm.prank(unauthorized);

        vm.expectRevert("RouteReceiver: not authorized publisher");
        receiver.publishRoute("run-002", "customer-1", "base-sepolia", 75, 1740000001);
    }

    // ── Duplicate runId is rejected (separate namespaces) ─────────────

    function test_duplicateRunIdReverts() public {
        vm.startPrank(publisher);

        receiver.publishRoute("run-003", "customer-1", "base-sepolia", 82, 1740000000);

        vm.expectRevert("RouteReceiver: runId already published");
        receiver.publishRoute("run-003", "customer-1", "base-sepolia", 82, 1740000000);

        vm.stopPrank();
    }

    function test_routeAndActivationNamespacesAreIndependent() public {
        // Same runId can appear in route namespace and activation namespace independently
        vm.startPrank(publisher);

        receiver.publishRoute("run-shared", "customer-1", "base-sepolia", 82, 1740000000);

        // Same runId in activation namespace must succeed — separate mapping
        receiver.publishActivationState(
            "run-shared", "customer-1", 5000, "base-sepolia", "arbitrum-sepolia", 1740000000
        );

        vm.stopPrank();

        assertTrue(receiver.isRouteRunPublished("run-shared"));
        assertTrue(receiver.isActivationRunPublished("run-shared"));
    }

    function test_duplicateActivationRunIdReverts() public {
        vm.startPrank(publisher);

        receiver.publishActivationState("act-001", "customer-1", 5000, "base-sepolia", "arbitrum-sepolia", 1740000000);

        vm.expectRevert("RouteReceiver: runId already published");
        receiver.publishActivationState("act-001", "customer-1", 5000, "base-sepolia", "arbitrum-sepolia", 1740000000);

        vm.stopPrank();
    }

    // ── getLatestRoute returns correct data ────────────────────────────

    function test_getLatestRouteReturnsCorrectData() public {
        vm.prank(publisher);
        receiver.publishRoute("run-004", "customer-1", "base-sepolia", 90, 1740000010);

        (string memory runId, string memory recommendedChain, uint256 score, uint256 timestamp) =
            receiver.getLatestRoute("customer-1");

        assertEq(runId, "run-004");
        assertEq(recommendedChain, "base-sepolia");
        assertEq(score, 90);
        assertEq(timestamp, 1740000010);
    }

    // ── Multiple customers store independently ─────────────────────────

    function test_multipleCustomersStoreIndependently() public {
        vm.startPrank(publisher);

        receiver.publishRoute("run-005", "customer-1", "base-sepolia", 80, 1740000020);
        receiver.publishRoute("run-006", "customer-2", "arbitrum-sepolia", 95, 1740000021);

        vm.stopPrank();

        (string memory runId1, string memory chain1, uint256 score1,) = receiver.getLatestRoute("customer-1");
        (string memory runId2, string memory chain2, uint256 score2,) = receiver.getLatestRoute("customer-2");

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
        receiver.publishRoute("run-007", "customer-1", "base-sepolia", 70, 1740000030);

        (string memory runId,,,) = receiver.getLatestRoute("customer-1");
        assertEq(runId, "run-007");
    }

    function test_ownerCanRemovePublisher() public {
        receiver.removePublisher(publisher);

        vm.prank(publisher);
        vm.expectRevert("RouteReceiver: not authorized publisher");
        receiver.publishRoute("run-008", "customer-1", "base-sepolia", 60, 1740000040);
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

    // ── isRouteRunPublished / isActivationRunPublished ─────────────────

    function test_isRouteRunPublishedReturnsFalseForUnknownRun() public view {
        assertFalse(receiver.isRouteRunPublished("run-unknown"));
    }

    function test_isRouteRunPublishedReturnsTrueAfterPublish() public {
        vm.prank(publisher);
        receiver.publishRoute("run-009", "customer-1", "base-sepolia", 85, 1740000050);

        assertTrue(receiver.isRouteRunPublished("run-009"));
        assertFalse(receiver.isActivationRunPublished("run-009"));
    }

    function test_isActivationRunPublishedReturnsTrueAfterPublish() public {
        vm.prank(publisher);
        receiver.publishActivationState("act-002", "customer-1", 5000, "base-sepolia", "arbitrum-sepolia", 1740000050);

        assertTrue(receiver.isActivationRunPublished("act-002"));
        assertFalse(receiver.isRouteRunPublished("act-002"));
    }

    // ── Address(0) guard ──────────────────────────────────────────────

    function test_addPublisherRevertsOnZeroAddress() public {
        vm.expectRevert("RouteReceiver: zero address");
        receiver.addPublisher(address(0));
    }

    // ── String length validation ──────────────────────────────────────

    function test_publishRouteRevertsOnLongRunId() public {
        string memory longStr = new string(129);
        vm.prank(publisher);
        vm.expectRevert(RouteReceiver.StringTooLong.selector);
        receiver.publishRoute(longStr, "customer-1", "base-sepolia", 82, 1740000000);
    }

    function test_publishActivationRevertsOnLongRunId() public {
        string memory longStr = new string(129);
        vm.prank(publisher);
        vm.expectRevert(RouteReceiver.StringTooLong.selector);
        receiver.publishActivationState(longStr, "customer-1", 5000, "base", "arb", 1740000000);
    }

    // ── Emergency pause ───────────────────────────────────────────────

    function test_ownerCanPause() public {
        receiver.pause();
        assertTrue(receiver.paused());
    }

    function test_ownerCanUnpause() public {
        receiver.pause();
        receiver.unpause();
        assertFalse(receiver.paused());
    }

    function test_nonOwnerCannotPause() public {
        vm.prank(unauthorized);
        vm.expectRevert("RouteReceiver: not owner");
        receiver.pause();
    }

    function test_publishRouteRevertsWhenPaused() public {
        receiver.pause();

        vm.prank(publisher);
        vm.expectRevert(RouteReceiver.ContractPaused.selector);
        receiver.publishRoute("run-010", "customer-1", "base-sepolia", 82, 1740000000);
    }

    function test_publishActivationRevertsWhenPaused() public {
        receiver.pause();

        vm.prank(publisher);
        vm.expectRevert(RouteReceiver.ContractPaused.selector);
        receiver.publishActivationState("act-003", "customer-1", 5000, "base", "arb", 1740000000);
    }

    function test_publishRouteWorksAfterUnpause() public {
        receiver.pause();
        receiver.unpause();

        vm.prank(publisher);
        receiver.publishRoute("run-011", "customer-1", "base-sepolia", 82, 1740000000);

        assertTrue(receiver.isRouteRunPublished("run-011"));
    }

    // ── Two-step ownership transfer ────────────────────────────────────

    function test_transferOwnershipSetsPendingOwner() public {
        address newOwner = address(0x99);

        vm.expectEmit(true, true, false, false);
        emit OwnershipTransferStarted(owner, newOwner);

        receiver.transferOwnership(newOwner);

        assertEq(receiver.pendingOwner(), newOwner);
        assertEq(receiver.owner(), owner); // unchanged until acceptOwnership
    }

    function test_acceptOwnershipCompletesTransfer() public {
        address newOwner = address(0x99);
        receiver.transferOwnership(newOwner);

        vm.expectEmit(true, true, false, false);
        emit OwnershipTransferred(owner, newOwner);

        vm.prank(newOwner);
        receiver.acceptOwnership();

        assertEq(receiver.owner(), newOwner);
        assertEq(receiver.pendingOwner(), address(0));
    }

    function test_acceptOwnershipRevertsForNonPendingOwner() public {
        address newOwner = address(0x99);
        receiver.transferOwnership(newOwner);

        vm.prank(unauthorized);
        vm.expectRevert("RouteReceiver: not pending owner");
        receiver.acceptOwnership();
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

    function test_newOwnerCanManagePublishersAfterTransfer() public {
        address newOwner = address(0x99);
        receiver.transferOwnership(newOwner);
        vm.prank(newOwner);
        receiver.acceptOwnership();

        vm.prank(newOwner);
        receiver.addPublisher(publisher2);
        assertTrue(receiver.publishers(publisher2));
    }

    // ── Chain decommissioning ─────────────────────────────────────────

    function test_ownerCanMarkAndFinalizeDecommission() public {
        vm.expectEmit(true, false, false, false);
        emit ChainDecommissioningMarked(84532);
        receiver.markDecommissioning(84532);

        (bool draining, bool decommissioned) = receiver.getChainDecommissionStatus(84532);
        assertTrue(draining);
        assertFalse(decommissioned);

        vm.expectEmit(true, false, false, false);
        emit ChainDecommissionFinalized(84532);
        receiver.finalizeDecommission(84532);

        (draining, decommissioned) = receiver.getChainDecommissionStatus(84532);
        assertTrue(draining);
        assertTrue(decommissioned);
    }

    function test_nonOwnerCannotMarkOrFinalizeDecommission() public {
        vm.prank(unauthorized);
        vm.expectRevert("RouteReceiver: not owner");
        receiver.markDecommissioning(84532);

        vm.prank(unauthorized);
        vm.expectRevert("RouteReceiver: not owner");
        receiver.finalizeDecommission(84532);
    }

    function test_finalizeRequiresDrainingState() public {
        vm.expectRevert(abi.encodeWithSelector(RouteReceiver.ChainNotDraining.selector, uint256(84532)));
        receiver.finalizeDecommission(84532);
    }

    function test_decommissionedCannotBeUnsetOrFinalizedAgain() public {
        receiver.markDecommissioning(84532);
        receiver.finalizeDecommission(84532);

        vm.expectRevert(abi.encodeWithSelector(RouteReceiver.ChainAlreadyDecommissioned.selector, uint256(84532)));
        receiver.markDecommissioning(84532);

        vm.expectRevert(abi.encodeWithSelector(RouteReceiver.ChainAlreadyDecommissioned.selector, uint256(84532)));
        receiver.finalizeDecommission(84532);
    }

    function test_creActivationCannotAffectDecommissionedChain() public {
        receiver.markDecommissioning(84532);
        receiver.finalizeDecommission(84532);

        vm.prank(publisher);
        vm.expectRevert(
            abi.encodeWithSelector(RouteReceiver.ActivationTouchesDecommissionedChain.selector, uint256(84532))
        );
        receiver.publishActivationState("act-decom-active", "customer-1", 5000, "84532,421614", "", 1740000000);

        vm.prank(publisher);
        vm.expectRevert(
            abi.encodeWithSelector(RouteReceiver.ActivationTouchesDecommissionedChain.selector, uint256(84532))
        );
        receiver.publishActivationState("act-decom-inactive", "customer-1", 5000, "421614", "84532", 1740000001);

        (bool draining, bool decommissioned) = receiver.getChainDecommissionStatus(84532);
        assertTrue(draining);
        assertTrue(decommissioned);
    }

    function test_getLatestRouteWithChainStateExposesThirdState() public {
        receiver.markDecommissioning(84532);
        receiver.finalizeDecommission(84532);

        vm.prank(publisher);
        receiver.publishRoute("run-with-state", "customer-1", "base-sepolia", 82, 1740000000);

        (
            string memory runId,
            string memory recommendedChain,
            uint256 score,
            uint256 timestamp,
            bool draining,
            bool decommissioned
        ) = receiver.getLatestRouteWithChainState("customer-1", 84532);

        assertEq(runId, "run-with-state");
        assertEq(recommendedChain, "base-sepolia");
        assertEq(score, 82);
        assertEq(timestamp, 1740000000);
        assertTrue(draining);
        assertTrue(decommissioned);
    }
}
