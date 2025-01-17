// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.8.0;

contract Fib {
    function fib(uint256 n) public pure returns (uint256) {
        return n <= 1 ? 1 : fib(n - 1) + fib(n - 2);
    }
}
