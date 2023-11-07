// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

contract TestCheatcodes {
    address constant CHEATCODE_ADDRESS =
        0x7109709ECfa91a80626fF3989D68f67F5b1DD12D;

    function deal() external {
        (bool success, ) = CHEATCODE_ADDRESS.call(
            abi.encodeWithSignature("deal(address,uint)", msg.sender, 1_000_000)
        );
        require(success, "deal failed");
    }

    function testDeal(address account) external {
        uint balanceBefore = address(account).balance;
        (bool success, ) = CHEATCODE_ADDRESS.call(
            abi.encodeWithSignature("deal(address,uint)", account, 1_000_000)
        );
        require(success, "deal failed");
        uint balanceAfter = address(account).balance;
        require(balanceAfter == balanceBefore + 1_000_000, "balance mismatch");
    }
}
