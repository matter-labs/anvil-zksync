//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "hardhat/console.sol";

contract Greeter {
    string private greeting;

    constructor(string memory _greeting) {
        greeting = _greeting;
    }

    function greet() public view returns (string memory) {
        return greeting;
    }

    function setGreeting(string memory _greeting) public {
        console.log("setGreeting called");
        console.log(_greeting);
        require(
            keccak256(abi.encodePacked((_greeting))) != keccak256(abi.encodePacked(("test"))),
            "Received a test value"
        );
        greeting = _greeting;
    }
}
