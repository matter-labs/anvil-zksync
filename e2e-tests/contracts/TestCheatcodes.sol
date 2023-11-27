// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

contract TestCheatcodes {
  event LogNonce(bytes data);
  address constant CHEATCODE_ADDRESS = 0x7109709ECfa91a80626fF3989D68f67F5b1DD12D;

  function testDeal(address account, uint256 amount) external {
    uint balanceBefore = address(account).balance;
    (bool success, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("deal(address,uint256)", account, amount));
    uint balanceAfter = address(account).balance;
    require(balanceAfter == amount, "balance mismatch");
    require(balanceAfter != balanceBefore, "balance unchanged");
    require(success, "deal failed");
  }

  function testEtch(address target, bytes calldata code) external {
    (bool success, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("etch(address,bytes)", target, code));
    require(success, "etch failed");
    (success, ) = target.call(abi.encodeWithSignature("setGreeting(string)", "hello world"));
    require(success, "setGreeting failed");
  }

  function testGetSetNonce(address account, uint64 nonce) external {
    (bool success, bytes memory data) = CHEATCODE_ADDRESS.call(
      abi.encodeWithSignature("setNonce(address,uint64)", account, nonce)
    );
    require(success, "setNonce failed");
    (success, data) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("getNonce(address)", account));
    require(success, "getNonce failed");
    emit LogNonce(data);
    // uint64 finalNonce = abi.decode(data, (uint64));
    // emit LogNonce();
    // emit LogNonce(finalNonce);
    // Console.log("nonce: %s", finalNonce);
    // require(finalNonce == nonce, "nonce mismatch");
  }

  function warp(uint256 timestamp) external {
    uint256 initialTimestamp = block.timestamp;
    require(timestamp != initialTimestamp, "timestamp must be different than current block timestamp");

    (bool success, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("warp(uint256)", timestamp));
    require(success, "warp failed");

    uint256 finalTimestamp = block.timestamp;
    require(finalTimestamp == timestamp, "timestamp was not changed");
  }
}
