// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IERC165} from "@openzeppelin/contracts/utils/introspection/IERC165.sol";

/// @notice Chainlink CCIP burn-and-mint interface.
/// @dev Vendored verbatim from Chainlink's canonical definition.
///      The CCIP Token Pool expects this interface to be satisfied by the token.
interface IBurnMintERC20 is IERC20, IERC165 {
    function mint(address account, uint256 amount) external;
    function burn(uint256 amount) external;
    function burnFrom(address account, uint256 amount) external;
}
