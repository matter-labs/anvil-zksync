// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import "./Secondary.sol";

contract Primary {
    Secondary secondary;

    constructor(address _secondary) {
        secondary = Secondary(_secondary);
    }

    function name() public pure returns (string memory) {
        return "Primary";
    }

    function calculate(uint256 value) public returns (uint) {
        return secondary.multiply(value);
    }

    function shouldRevert() public view returns (uint) {
        return secondary.shouldRevert();
    }
}
