// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

contract TestCheatcodes {
  address constant CHEATCODE_ADDRESS = 0x7109709ECfa91a80626fF3989D68f67F5b1DD12D;

  function testDeal(address account) public returns (bool) {
    uint balanceBefore = address(account).balance;
    (bool success, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("deal(address,uint256)", account, 1_000_000));
    require(success, "deal failed");
    uint balanceAfter = address(account).balance;
    require(balanceAfter == balanceBefore + 1_000_000, "balance mismatch");
    return true;
  }

  function testGetSetNonce(address account) public returns (bool) {
    (bool success, bytes memory returnData) = CHEATCODE_ADDRESS.call(
      abi.encodeWithSignature("getNonce(address)", account)
    );
    uint256 nonceBefore = uint256(bytes32(returnData));
    require(success, "getNonce failed");
    require(nonceBefore == 0, "nonce mismatch");
    (success, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("setNonce(address,uint256)", account, 1));
    require(success, "setNonce failed");
    (success, returnData) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("getNonce(address)", account));
    uint256 nonceAfter = uint256(bytes32(returnData));
    require(success, "getNonce failed");
    require(nonceAfter == 1, "nonce mismatch");
    return true;
  }
}
