// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/**
 * @dev This contract is for basic demonstration purposes only. It should not be used in production.
 * It is for the convenience of the ERC20FixedPaymaster.sol contract and its corresponding test file.
 */
contract MyERC20 is ERC20 {
  uint8 private _decimals;

  constructor(
    string memory name,
    string memory symbol,
    uint8 decimals_
  ) payable ERC20(name, symbol) {
    _decimals = decimals_;
  }

  function mint(address _to, uint256 _amount) public returns (bool) {
    _mint(_to, _amount);
    return true;
  }

  function decimals() public view override returns (uint8) {
    return _decimals;
  }

  function burn(address from, uint256 amount) public {
    _burn(from, amount);
  }
}