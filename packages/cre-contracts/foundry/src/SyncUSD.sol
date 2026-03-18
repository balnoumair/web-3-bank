// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {ERC20Upgradeable} from "@openzeppelin/contracts-upgradeable/token/ERC20/ERC20Upgradeable.sol";
import {AccessControlUpgradeable} from "@openzeppelin/contracts-upgradeable/access/AccessControlUpgradeable.sol";
import {PausableUpgradeable} from "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IERC165} from "@openzeppelin/contracts/utils/introspection/IERC165.sol";
import {IBurnMintERC20} from "./interfaces/IBurnMintERC20.sol";

/// @title SyncUSD
/// @notice Multi-chain stablecoin token with CCIP burn-and-mint support.
/// @dev UUPS upgradeable. Roles are gated via AccessControl.
///      - MINTER_ROLE: Bank Contract and CCIP Token Pool — may mint/burn
///      - ADMIN_ROLE:  Timelock — may authorize upgrades
///      - PAUSER_ROLE: Emergency multisig — may pause/unpause
///      - DEFAULT_ADMIN_ROLE: Timelock — may grant/revoke roles
contract SyncUSD is
    Initializable,
    ERC20Upgradeable,
    AccessControlUpgradeable,
    PausableUpgradeable,
    UUPSUpgradeable,
    IBurnMintERC20
{
    // ── Errors ─────────────────────────────────────────────────────────

    error ZeroAddress();

    // ── Roles ──────────────────────────────────────────────────────────

    bytes32 public constant ADMIN_ROLE = keccak256("ADMIN_ROLE");
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    // ── Constructor ────────────────────────────────────────────────────

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    // ── Initializer ────────────────────────────────────────────────────

    /// @notice Initializes the proxy. Called once by the deployer.
    /// @param admin  Receives DEFAULT_ADMIN_ROLE and ADMIN_ROLE (Timelock in production).
    /// @param pauser Receives PAUSER_ROLE (emergency multisig in production).
    /// @dev MINTER_ROLE is not granted here — granted post-deploy via grantRole.
    function initialize(address admin, address pauser) external initializer {
        if (admin == address(0) || pauser == address(0)) revert ZeroAddress();
        __ERC20_init("SyncUSD", "sUSD");
        __AccessControl_init();
        __Pausable_init();

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, pauser);
    }

    // ── ERC-20 overrides ───────────────────────────────────────────────

    /// @inheritdoc ERC20Upgradeable
    function decimals() public pure override returns (uint8) {
        return 6;
    }

    /// @notice Transfers are blocked when the contract is paused.
    function transfer(address to, uint256 amount)
        public
        override(ERC20Upgradeable, IERC20)
        whenNotPaused
        returns (bool)
    {
        return super.transfer(to, amount);
    }

    /// @notice TransferFrom is blocked when the contract is paused.
    function transferFrom(address from, address to, uint256 amount)
        public
        override(ERC20Upgradeable, IERC20)
        whenNotPaused
        returns (bool)
    {
        return super.transferFrom(from, to, amount);
    }

    // ── IBurnMintERC20 ─────────────────────────────────────────────────

    /// @notice Mints `amount` tokens to `to`. Restricted to MINTER_ROLE.
    function mint(address to, uint256 amount)
        external
        override
        onlyRole(MINTER_ROLE)
        whenNotPaused
    {
        _mint(to, amount);
    }

    /// @notice Burns `amount` from msg.sender's balance. Restricted to MINTER_ROLE.
    /// @dev Does NOT inherit ERC20BurnableUpgradeable — that would expose an ungated public burn.
    function burn(uint256 amount)
        external
        override
        onlyRole(MINTER_ROLE)
        whenNotPaused
    {
        _burn(msg.sender, amount);
    }

    /// @notice Burns `amount` from `from`'s balance using caller's allowance.
    /// @dev Caller must have MINTER_ROLE and sufficient allowance from `from`.
    function burnFrom(address from, uint256 amount)
        external
        override
        onlyRole(MINTER_ROLE)
        whenNotPaused
    {
        _spendAllowance(from, msg.sender, amount);
        _burn(from, amount);
    }

    // ── Pause ──────────────────────────────────────────────────────────

    /// @notice Pauses all token transfers, mints, and burns.
    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    /// @notice Unpauses the contract.
    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }

    // ── UUPS ───────────────────────────────────────────────────────────

    /// @dev Only ADMIN_ROLE may authorize upgrades.
    function _authorizeUpgrade(address newImplementation)
        internal
        override
        onlyRole(ADMIN_ROLE)
    {}

    // ── ERC-165 ────────────────────────────────────────────────────────

    /// @notice Returns true for IBurnMintERC20, IERC20, IERC165, and IAccessControl.
    function supportsInterface(bytes4 interfaceId)
        public
        view
        override(AccessControlUpgradeable, IERC165)
        returns (bool)
    {
        return
            interfaceId == type(IBurnMintERC20).interfaceId ||
            interfaceId == type(IERC20).interfaceId ||
            super.supportsInterface(interfaceId);
    }
}
