// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

contract TestCheatcodes {
  address constant CHEATCODE_ADDRESS = 0x7109709ECfa91a80626fF3989D68f67F5b1DD12D;

  function testAddr(uint256 privateKey, address addr) external {
    (bool success, bytes memory data) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("addr(uint256)", privateKey));
    require(success, "addr failed");
    address recovered = abi.decode(data, (address));
    require(recovered == addr, "address mismatch");
  }

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

  function testRoll(uint256 blockNumber) external {
    uint256 initialBlockNumber = block.number;
    require(blockNumber != initialBlockNumber, "block number must be different than current block number");

    (bool success, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("roll(uint256)", blockNumber));
    require(success, "roll failed");

    uint256 finalBlockNumber = block.number;
    require(finalBlockNumber == blockNumber, "block number was not changed");
  }

  function testSetNonce(address account, uint64 nonce) external {
    (bool success, bytes memory data) = CHEATCODE_ADDRESS.call(
      abi.encodeWithSignature("setNonce(address,uint64)", account, nonce)
    );
    require(success, "setNonce failed");
    (success, data) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("getNonce(address)", account));
    require(success, "getNonce failed");
    uint64 finalNonce = abi.decode(data, (uint64));
    require(finalNonce == nonce, "nonce mismatch");
  }

  function testStartPrank(address account) external {
    address original_msg_sender = msg.sender;
    address original_tx_origin = tx.origin;

    PrankVictim victim = new PrankVictim();

    victim.assertCallerAndOrigin(
      address(this),
      "startPrank failed: victim.assertCallerAndOrigin failed",
      original_tx_origin,
      "startPrank failed: victim.assertCallerAndOrigin failed"
    );

    (bool success1, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("startPrank(address)", account));
    require(success1, "startPrank failed");

    require(msg.sender == account, "startPrank failed: msg.sender unchanged");
    require(tx.origin == original_tx_origin, "startPrank failed tx.origin changed");
    victim.assertCallerAndOrigin(
      account,
      "startPrank failed: victim.assertCallerAndOrigin failed",
      original_tx_origin,
      "startPrank failed: victim.assertCallerAndOrigin failed"
    );

    (bool success2, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("stopPrank()"));
    require(success2, "stopPrank failed");

    require(msg.sender == original_msg_sender, "stopPrank failed: msg.sender didn't return to original");
    require(tx.origin == original_tx_origin, "stopPrank failed tx.origin changed");
    victim.assertCallerAndOrigin(
      address(this),
      "startPrank failed: victim.assertCallerAndOrigin failed",
      original_tx_origin,
      "startPrank failed: victim.assertCallerAndOrigin failed"
    );
  }

  function testStartPrankWithOrigin(address account, address origin) external {
    address original_msg_sender = msg.sender;
    address original_tx_origin = tx.origin;

    PrankVictim victim = new PrankVictim();

    victim.assertCallerAndOrigin(
      address(this),
      "startPrank failed: victim.assertCallerAndOrigin failed",
      original_tx_origin,
      "startPrank failed: victim.assertCallerAndOrigin failed"
    );

    (bool success1, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("startPrank(address,address)", account, origin));
    require(success1, "startPrank failed");

    require(msg.sender == account, "startPrank failed: msg.sender unchanged");
    require(tx.origin == origin, "startPrank failed: tx.origin unchanged");
    victim.assertCallerAndOrigin(
      account,
      "startPrank failed: victim.assertCallerAndOrigin failed",
      origin,
      "startPrank failed: victim.assertCallerAndOrigin failed"
    );

    (bool success2, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("stopPrank()"));
    require(success2, "stopPrank failed");

    require(msg.sender == original_msg_sender, "stopPrank failed: msg.sender didn't return to original");
    require(tx.origin == original_tx_origin, "stopPrank failed: tx.origin didn't return to original");
    victim.assertCallerAndOrigin(
      address(this),
      "startPrank failed: victim.assertCallerAndOrigin failed",
      original_tx_origin,
      "startPrank failed: victim.assertCallerAndOrigin failed"
    );
  }

  function testWarp(uint256 timestamp) external {
    uint256 initialTimestamp = block.timestamp;
    require(timestamp != initialTimestamp, "timestamp must be different than current block timestamp");

    (bool success, ) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("warp(uint256)", timestamp));
    require(success, "warp failed");

    uint256 finalTimestamp = block.timestamp;
    require(finalTimestamp == timestamp, "timestamp was not changed");
  }

  function testStore(bytes32 slot, bytes32 value) external {
    testStoreTarget testStoreInstance = new testStoreTarget();
    testStoreInstance.testStoredValue(0);

    (bool success, ) = CHEATCODE_ADDRESS.call(
      abi.encodeWithSignature("store(address,bytes32,bytes32)", address(testStoreInstance), slot, value)
    );
    require(success, "store failed");

    testStoreInstance.testStoredValue(value);
  }

  function testLoad(bytes32 slot) external {
    TestLoadTarget testLoadTarget = new TestLoadTarget();
    (bool success, bytes memory data) = CHEATCODE_ADDRESS.call(abi.encodeWithSignature("load(address,bytes32)", address(testLoadTarget), slot));
    require(success, "load failed");
    bytes32 loadedValue = abi.decode(data, (bytes32));
    require(loadedValue == bytes32(uint256(1337)), "address mismatch");
  }
}

contract TestLoadTarget {
  bytes32 public testValue = bytes32(uint256(1337)); //slot 0
}

contract testStoreTarget {
  bytes32 public testValue = bytes32(uint256(0)); //slot 0

  function testStoredValue(bytes32 value) public view {
    require(testValue == value, "testValue was not stored correctly");
  }
}

contract PrankVictim {
  function assertCallerAndOrigin(
    address expectedSender,
    string memory senderMessage,
    address expectedOrigin,
    string memory originMessage
  ) public view {
    require(msg.sender == expectedSender, senderMessage);
    require(tx.origin == expectedOrigin, originMessage);
  }
}
