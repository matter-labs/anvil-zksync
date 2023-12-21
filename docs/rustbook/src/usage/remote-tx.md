# Replay Remote Transactions

If you wish to replay a remote transaction locally for deep debugging, use the following command:

```sh
# Usage: era_test_node replay_tx <NETWORK> <TX>
era_test_node replay_tx sepolia-testnet 0x7119045573862797257e4441ff48bf5a3bc4d133a00d167c18dc955eda12cfac
```

For more detailed transaction information, such as call traces, add the `--show-calls` flag. If you want to see ABI names, add the `--resolve-hashes` flag. Here's an example:

```sh
# Usage: era_test_node replay_tx <NETWORK> <TX>
era_test_node --show-calls=user --resolve-hashes replay_tx sepolia-testnet 0x7119045573862797257e4441ff48bf5a3bc4d133a00d167c18dc955eda12cfac
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

Here's an example of what you could see when `--show-calls` and `--resolve-hashes` are configured:

```log
Creating fork from "https://sepolia.era.zksync.dev:443" L1 block: L1BatchNumber(4513) L2 block: 14945 with timestamp 1703064786, L1 gas price 61083275326 and protocol version: Some(Version19)
Starting network with chain id: L2ChainId(300)
Running 1 transactions (one per batch)

Validating 0x7119045573862797257e4441ff48bf5a3bc4d133a00d167c18dc955eda12cfac
Executing 0x7119045573862797257e4441ff48bf5a3bc4d133a00d167c18dc955eda12cfac
┌─────────────────────────┐
│   TRANSACTION SUMMARY   │
└─────────────────────────┘
Transaction: SUCCESS
Initiator: 0x4eaf936c172b5e5511959167e8ab4f7031113ca3
Payer: 0x4eaf936c172b5e5511959167e8ab4f7031113ca3
Gas - Limit: 2_487_330 | Used: 969_330 | Refunded: 1_518_000
Use --show-gas-details flag or call config_setShowGasDetails to display more info

==== Console logs: 

==== 22 call traces.  Use --show-calls flag or call config_setShowCalls to display more info.
  Call(Normal) 0x4eaf936c172b5e5511959167e8ab4f7031113ca3           validateTransaction(bytes32, bytes32, tuple)   1830339
    Call(Normal) 0x0000000000000000000000000000000000000001                 0x89c19e9b   1766835
  Call(Normal) 0x4eaf936c172b5e5511959167e8ab4f7031113ca3           payForTransaction(bytes32, bytes32, tuple)   1789767
  Call(Normal) 0x4eaf936c172b5e5511959167e8ab4f7031113ca3           executeTransaction(bytes32, bytes32, tuple)   1671012
      Call(Mimic) 0x5d4fb5385ed95b65d1cd6a10ed9549613481ab2f           0x   1443393

==== 4 events
EthToken System Contract                  
  Topics:
    Transfer(address,address,uint256)
    0x0000000000000000000000004eaf936c172b5e5511959167e8ab4f7031113ca3
    0x0000000000000000000000000000000000000000000000000000000000008001
  Data (Hex): 0x000000000000000000000000000000000000000000000000000c31dac51a6200

EthToken System Contract                  
  Topics:
    Transfer(address,address,uint256)
    0x0000000000000000000000000000000000000000000000000000000000008001
    0x0000000000000000000000004eaf936c172b5e5511959167e8ab4f7031113ca3
  Data (Hex): 0x0000000000000000000000000000000000000000000000000009fc4d1bd4ad00

EthToken System Contract                  
  Topics:
    Transfer(address,address,uint256)
    0x0000000000000000000000004eaf936c172b5e5511959167e8ab4f7031113ca3
    0x0000000000000000000000005d4fb5385ed95b65d1cd6a10ed9549613481ab2f
  Data (Hex): 0x0000000000000000000000000000000000000000000000000090f705956a4008

EthToken System Contract                  
  Topics:
    Transfer(address,address,uint256)
    0x0000000000000000000000000000000000000000000000000000000000008001
    0x0000000000000000000000004eaf936c172b5e5511959167e8ab4f7031113ca3
  Data (Hex): 0x000000000000000000000000000000000000000000000000000159273ab13800
```