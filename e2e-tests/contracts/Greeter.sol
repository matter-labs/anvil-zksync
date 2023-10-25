//SPDX-License-Identifier: Unlicense
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/access/Ownable.sol";
import "hardhat/console.sol";

contract Greeter is Ownable {
    string private greeting;
    event LogString(string value);

    constructor(string memory _greeting) {
        greeting = _greeting;
    }

    function greet() public view returns (string memory) {
        return greeting;
    }

    function setGreeting(string memory _greeting) public {
        console.log("setGreeting called");
        console.log(_greeting);
        emit LogString(string.concat("Greeting is being updated to ", _greeting));

        require(
            keccak256(abi.encodePacked((_greeting))) != keccak256(abi.encodePacked(("test"))),
            "Received a test value"
        );
        greeting = _greeting;
    }

    function setGreetingByOwner(string memory _greeting) public onlyOwner {
        console.log("setGreetingByOwner called");
        console.log(_greeting);
        emit LogString(string.concat("Greeting is being updated to ", _greeting));
        greeting = _greeting;
    }
}
