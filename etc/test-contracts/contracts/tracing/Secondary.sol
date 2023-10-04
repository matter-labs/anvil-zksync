// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

contract Secondary {
    uint data;

    constructor(uint _data) {
        data = _data;
    }

    function name() public pure returns (string memory) {
        return "Secondary";
    }

    function multiply(uint256 value) public returns (uint) {
        data = data * value;
        return data;
    }
}
