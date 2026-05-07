// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title IReserveBridge
/// @notice Adapter interface for moving USDC reserves between Bank contracts.
interface IReserveBridge {
    /// @notice Initiate an outbound reserve bridge from the calling Bank.
    /// @param destChainId Destination chain id.
    /// @param amount Amount of USDC to bridge.
    /// @param destReserve Destination Bank/reserve contract that receives the bridged USDC.
    /// @return messageId Bridge-specific message identifier used for audit and idempotency.
    function bridgeOut(uint64 destChainId, uint256 amount, address destReserve) external returns (bytes32 messageId);

    /// @notice Process an inbound bridge-specific message.
    /// @dev Concrete adapters define their own message encoding.
    /// @return messageId Bridge-specific message identifier.
    function bridgeIn(bytes calldata message) external returns (bytes32 messageId);

    /// @notice Stable bridge type identifier for Treasury audit records.
    function bridgeType() external view returns (bytes32);
}
