# Replay Remote Transactions

If you wish to replay a remote transaction locally for deep debugging, use the following command:

```sh
# Usage: era_test_node replay_tx <NETWORK> <TX>
era_test_node replay_tx testnet 0x7f039bcbb1490b855be37e74cf2400503ad57f51c84856362f99b0cbf1ef478a
```

For more detailed transaction information, such as call traces, add the `--show-calls` flag. If you want to see ABI names, add the `--resolve-hashes` flag. Here's an example:

```sh
# Usage: era_test_node replay_tx <NETWORK> <TX>
era_test_node --show-calls=user --resolve-hashes replay_tx testnet 0x7f039bcbb1490b855be37e74cf2400503ad57f51c84856362f99b0cbf1ef478a
```

Alternatively (if your node is already running) you can use `config_setShowCalls` and `config_setResolveHashes` RPC endpoints to configure these values. Here's an example:

```sh
# era_test_node already running...

# Set show-calls to User
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_setShowCalls","params": ["user"]}'

# Enable resolve-hashes
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_setResolveHashes","params": [true]}'
```

Here's an example of what you should expect to see when `--show-calls` and `--resolve-hashes` are configured:

```log
Creating fork from "https://testnet.era.zksync.dev:443" L1 block: L1BatchNumber(94420) L2 block: 8072359 with timestamp 1687337488 and L1 gas price 2500011172
Starting network with chain id: L2ChainId(280)
Running 1 transactions (one per batch)

Executing 0x7f039bcbb1490b855be37e74cf2400503ad57f51c84856362f99b0cbf1ef478a
Transaction: SUCCESS
Initiator: 0x55362182242a4de20ea8a0ec055b2134bb24e23d Payer: 0x55362182242a4de20ea8a0ec055b2134bb24e23d
Gas Limit: 797128 used: 351250 refunded: 445878

==== Console logs:

==== 18 call traces.  Use --show-calls flag to display more info.
Call(Normal) 0x55362182242a4de20ea8a0ec055b2134bb24e23d           validateTransaction(bytes32, bytes32, tuple)   730485
  Call(Normal) 0x0000000000000000000000000000000000000001                 0xbb1e83e6   698040
Call(Normal) 0x55362182242a4de20ea8a0ec055b2134bb24e23d           payForTransaction(bytes32, bytes32, tuple)   703647
Call(Normal) 0x55362182242a4de20ea8a0ec055b2134bb24e23d           executeTransaction(bytes32, bytes32, tuple)   647199
    Call(Mimic) 0x6eef3310e09df3aa819cc2aa364d4f3ad2e6ffe3           swapExactETHForTokens(uint256,address[],address,uint256)   596358
      Call(Normal) 0x053f26a020de152a947b8ba7d8974c85c5fc5b81           getPair(address,address)   577647
      Call(Normal) 0xc0b7c869ba924c05f64333d9caa21f4424eb4b30           getReserves()   570402
        Call(Mimic) 0x8a144308792a23aadb118286ac0dec646f638908           deposit()   534807
      Call(Normal) 0x053f26a020de152a947b8ba7d8974c85c5fc5b81           getPair(address,address)   526050
      Call(Normal) 0x8a144308792a23aadb118286ac0dec646f638908           transfer(address,uint256)   522900
      Call(Normal) 0x053f26a020de152a947b8ba7d8974c85c5fc5b81           getPair(address,address)   497007
      Call(Normal) 0xc0b7c869ba924c05f64333d9caa21f4424eb4b30           swap(uint256,uint256,address,bytes)   492660
        Call(Normal) 0x880f03ca84e6cf0d0871c9818a2981debaba22b3           transfer(address,uint256)   465948
        Call(Normal) 0x880f03ca84e6cf0d0871c9818a2981debaba22b3           balanceOf(address)   432495
        Call(Normal) 0x8a144308792a23aadb118286ac0dec646f638908           balanceOf(address)   430290

==== 9 events
EthToken System Contract                   Transfer(address,address,uint256), 0x0000…e23d, 0x0000…8001
EthToken System Contract                   Transfer(address,address,uint256), 0x0000…e23d, 0x0000…ffe3
EthToken System Contract                   Transfer(address,address,uint256), 0x0000…ffe3, 0x0000…8908
0x8a144308792a23aadb118286ac0dec646f638908 Deposit(address,uint256), 0x0000…ffe3
0x8a144308792a23aadb118286ac0dec646f638908 Transfer(address,address,uint256), 0x0000…ffe3, 0x0000…4b30
0x880f03ca84e6cf0d0871c9818a2981debaba22b3 Transfer(address,address,uint256), 0x0000…4b30, 0x0000…e23d
0xc0b7c869ba924c05f64333d9caa21f4424eb4b30 Sync(uint112,uint112)
0xc0b7c869ba924c05f64333d9caa21f4424eb4b30 Swap(address,uint256,uint256,uint256,uint256,address), 0x0000…ffe3, 0x0000…e23d
EthToken System Contract                   Transfer(address,address,uint256), 0x0000…8001, 0x0000…e23d
```