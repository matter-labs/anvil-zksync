# 🔧 Supported APIs for In-Memory Node 🔧

> ⚠️ **WORK IN PROGRESS**: This list is non-comprehensive and being updated

## Supported APIs Table

| Namespace | API | Status | Description |
| --- | --- | --- | --- |
| [`CONFIG`](#config-namespace) | [`config_getShowCalls`](#config_getshowcalls) | SUPPORTED | Gets the current value of `show_calls` that's originally set with `--show-calls` option |
| [`CONFIG`](#config-namespace) | [`config_setShowCalls`](#config_setshowcalls) | SUPPORTED | Updates `show_calls` to print more detailed call traces |
| [`CONFIG`](#config-namespace) | [`config_setResolveHashes`](#config_setresolvehashes) | SUPPORTED | Updates `resolve-hashes` to call OpenChain for human-readable ABI names in call traces |
| [`NETWORK`](#network-namespace) | [`net_version`](#net_version) | SUPPORTED | Returns the current network id |
| [`NETWORK`](#network-namespace) | [`net_peerCount`](#net_peercount) |SUPPORTED | Returns the number of peers currently connected to the client |
| [`NETWORK`](#network-namespace) | [`net_listening`](#net_listening) | SUPPORTED | Returns `true` if the client is actively listening for network connections |
| `DEBUG` | `debug_storageRangeAt` | NOT IMPLEMENTED | Attempts to replay the transaction as it was executed on the network and return storage data |
| `DEBUG` | `debug_traceTransaction` | NOT IMPLEMENTED | Attempt to run the transaction in the exact same manner as it was executed on the network |
| `HARDHAT` | `hardhat_addCompilationResult` | NOT IMPLEMENTED | Add information about compiled contracts |
| `HARDHAT` | `hardhat_dropTransaction` | NOT IMPLEMENTED | Remove a transaction from the mempool |
| `HARDHAT` | `hardhat_impersonateAccount` | NOT IMPLEMENTED | Impersonate an account |
| `HARDHAT` | `hardhat_getAutomine` | NOT IMPLEMENTED | Returns true if automatic mining is enabled, and false otherwise |
| `HARDHAT` | `hardhat_metadata` | NOT IMPLEMENTED | Returns the metadata of the current hardhat network |
| `HARDHAT` | `hardhat_mine` | NOT IMPLEMENTED | Can mine any number of blocks at once, in constant time |
| `HARDHAT` | `hardhat_reset` | NOT IMPLEMENTED | Resets the state of the network |
| `HARDHAT` | `hardhat_setBalance` | NOT IMPLEMENTED | Enables or disables automatic mining |
| `HARDHAT` | `hardhat_setCode` | NOT IMPLEMENTED | Sets the bytecode of a given account |
| `HARDHAT` | `hardhat_setCoinbase` | NOT IMPLEMENTED | Sets the coinbase address |
| `HARDHAT` | `hardhat_setLoggingEnabled` | NOT IMPLEMENTED | Enables or disables logging |
| `HARDHAT` | `hardhat_setMinGasPrice` | NOT IMPLEMENTED | Sets the minimum gas price |
| `HARDHAT` | `hardhat_setNextBlockBaseFeePerGas` | NOT IMPLEMENTED | Sets the base fee per gas for the next block |
| `HARDHAT` | `hardhat_setPrevRandao` | NOT IMPLEMENTED | Sets the RANDAO value of the next block |
| `HARDHAT` | `hardhat_setNonce` | NOT IMPLEMENTED | Sets the nonce of a given account |
| `HARDHAT` | `hardhat_setStorageAt` | NOT IMPLEMENTED | Sets the storage value at a given key for a given account |
| `HARDHAT` | `hardhat_stopImpersonatingAccount` | Stop impersonating an account after having previously used |
| `EVM` | `evm_addAccount` | NOT IMPLEMENTED | Adds any arbitrary to the namespace |
| `EVM` | `evm_increaseTime` | NOT IMPLEMENTED | Jump forward in time by the given amount of time |
| `EVM` | `evm_mine` | NOT IMPLEMENTED | Force a single block to be mined |
| `EVM` | `evm_removeAccount` | NOT IMPLEMENTED | Removes an account from the namespace |
| `EVM` | `evm_revert` | NOT IMPLEMENTED | Revert the state of the blockchain to a previous snapshot |
| `EVM` | `evm_setAccountBalance` | NOT IMPLEMENTED | Sets the given account's balance to the specified WEI value |
| `EVM` | `evm_setAccountCode` | NOT IMPLEMENTED | Sets the given account's code to the specified data |
| `EVM` | `evm_setAccountNonce` | NOT IMPLEMENTED | Sets the given account's nonce to the specified value |
| `EVM` | `evm_setAccountStorageAt` | NOT IMPLEMENTED | Sets the given account's storage slot to the specified data |
| `EVM` | `evm_setTime` | NOT IMPLEMENTED | Sets the internal clock time to the given timestamp |
| `EVM` | `evm_snapshot` | NOT IMPLEMENTED | Snapshot the state of the blockchain at the current block |
| [`ETH`](#eth-namespace) | [`eth_chainId`](#eth_chainid) | SUPPORTED | Returns the currently configured chain id |
| [`ETH`](#eth-namespace) | [`eth_estimateGas`](#eth_estimategas) | PARTIALLY | Generates and returns an estimate of how much gas is necessary for the transaction to complete |
| [`ETH`](#eth-namespace) | [`eth_gasPrice`](#eth_gasprice) | IMPLEMENTED | Returns the current price per gas in wei |
| [`ETH`](#eth-namespace) | [`eth_getBalance`](#eth_getbalance) | IMPLEMENTED | Returns the balance of the account of given address |
| [`ETH`](#eth-namespace) | [`eth_getBlockByNumber`](#eth_getblockbynumber) | PARTIALLY | Returns information about a block by block number |
| [`ETH`](#eth-namespace) | `eth_getBlockByHash` | NOT IMPLEMENTED | Returns information about a block by block hash |
| [`ETH`](#eth-namespace) | [`eth_getTransactionByHash`](#eth_gettransactionbyhash) | IMPLEMENTED | Returns the information about a transaction requested by transaction hash |
| `ETH` | `eth_accounts` | NOT IMPLEMENTED | Returns a list of addresses owned by client |
| `ETH` | `eth_blockNumber` | NOT IMPLEMENTED | Returns the number of the most recent block |
| `ETH` | `eth_call` | NOT IMPLEMENTED | Executes a new message call immediately without creating a transaction on the block chain |
| `ETH` | `eth_coinbase` | NOT IMPLEMENTED | Returns the client coinbase address |
| `ETH` | `eth_feeHistory` | NOT IMPLEMENTED | Returns a collection of historical block gas data |
| `ETH` | `eth_getBlockTransactionCountByHash` | NOT IMPLEMENTED | Number of transactions in a block from a block matching the given block hash |
| `ETH` | `eth_getBlockTransactionCountByNumber` | NOT IMPLEMENTED | Number of transactions in a block from a block matching the given block number |
| `ETH` | `eth_getCode` | NOT IMPLEMENTED | Returns code at a given address |
| `ETH` | `eth_getCompilers` | NOT IMPLEMENTED | Returns a list of available compilers |
| `ETH` | `eth_getFilterChanges` | NOT IMPLEMENTED | Polling method for a filter, which returns an array of logs, block hashes, or transaction hashes, depending on the filter type, which occurred since last poll |
| `ETH` | `eth_getFilterLogs` | NOT IMPLEMENTED | Returns an array of all logs matching filter with given id |
| `ETH` | `eth_getLogs` | NOT IMPLEMENTED | Returns an array of all logs matching a given filter object |
| `ETH` | `eth_getProof` | NOT IMPLEMENTED | Returns the details for the account at the specified address and block number, the account's Merkle proof, and the storage values for the specified storage keys with their Merkle-proofs |
| `ETH` | `eth_getStorageAt` | NOT IMPLEMENTED | Returns the value from a storage position at a given address |
| `ETH` | `eth_getTransactionByBlockHashAndIndex` | NOT IMPLEMENTED | Returns information about a transaction by block hash and transaction index position |
| `ETH` | `eth_getTransactionByBlockNumberAndIndex` | NOT IMPLEMENTED | Returns information about a transaction by block number and transaction index position |
| `ETH` | `eth_getTransactionCount` | NOT IMPLEMENTED | Returns the number of transactions sent from an address |
| `ETH` | `eth_getTransactionReceipt` | NOT IMPLEMENTED | Returns the receipt of a transaction by transaction hash |
| `ETH` | `eth_getUncleByBlockHashAndIndex` | NOT IMPLEMENTED | Returns information about a uncle of a block by hash and uncle index position |
| `ETH` | `eth_getUncleByBlockNumberAndIndex` | NOT IMPLEMENTED | Returns information about a uncle of a block by hash and uncle index position |
| `ETH` | `eth_getUncleCountByBlockHash` | NOT IMPLEMENTED | Returns the number of uncles in a block from a block matching the given block hash |
| `ETH` | `eth_getUncleCountByBlockNumber` | NOT IMPLEMENTED | Returns the number of uncles in a block from a block matching the given block hash |
| `ETH` | `eth_getWork` | NOT IMPLEMENTED | Returns: An Array with the following elements 1: DATA, 32 Bytes - current block header pow-hash 2: DATA, 32 Bytes - the seed hash used for the DAG. 3: DATA, 32 Bytes - the boundary condition ("target"), 2^256 / difficulty |
| `ETH` | `eth_hashrate` | NOT IMPLEMENTED | Returns the number of hashes per second that the node is mining with |
| `ETH` | `eth_maxPriorityFeePerGas` | NOT IMPLEMENTED | Returns a `maxPriorityFeePerGas` value suitable for quick transaction inclusion |
| `ETH` | `eth_mining` | NOT IMPLEMENTED | Returns true if client is actively mining new blocks |
| `ETH` | `eth_newBlockFilter` | NOT IMPLEMENTED | Creates a filter in the node, to notify when a new block arrives |
| `ETH` | `eth_newFilter` | NOT IMPLEMENTED | Creates a filter object, based on filter options, to notify when the state changes (logs) |
| `ETH` | `eth_newPendingTransactionFilter` | NOT IMPLEMENTED | Creates a filter in the node, to notify when new pending transactions arrive |
| `ETH` | `eth_protocolVersion` | NOT IMPLEMENTED | Returns the current ethereum protocol version |
| `ETH` | `eth_sendRawTransaction` | NOT IMPLEMENTED | Creates new message call transaction or a contract creation for signed transactions |
| `ETH` | `eth_sendTransaction` | NOT IMPLEMENTED | Creates new message call transaction or a contract creation, if the data field contains code |
| `ETH` | `eth_sign` | NOT IMPLEMENTED | The sign method calculates an Ethereum specific signature with: `sign(keccak256("\x19Ethereum Signed Message:\n" + message.length + message)))` |
| `ETH` | `eth_signTransaction` | NOT IMPLEMENTED | Signs a transaction that can be submitted to the network at a later time using `eth_sendRawTransaction` |
| `ETH` | `eth_signTypedData` | NOT IMPLEMENTED | Identical to eth_signTypedData_v4 |
| `ETH` | `eth_signTypedData_v4` | NOT IMPLEMENTED | Returns `Promise<string>: Signature`. As in eth_sign, it is a hex encoded 129 byte array starting with `0x`. |
| `ETH` | `eth_submitHashrate` | NOT IMPLEMENTED | Used for submitting mining hashrate |
| `ETH` | `eth_submitWork` | NOT IMPLEMENTED | Used for submitting a proof-of-work solution |
| `ETH` | `eth_subscribe` | NOT IMPLEMENTED | Starts a subscription to a particular event |
| `ETH` | `eth_syncing` | NOT IMPLEMENTED | Returns an object containing data about the sync status or `false` when not syncing |
| `ETH` | `eth_uninstallFilter` | NOT IMPLEMENTED | Uninstalls a filter with given id |
| `ETH` | `eth_accounts` | NOT IMPLEMENTED | Returns a list of addresses owned by client |
| `ETH` | `eth_unsubscribe` | NOT IMPLEMENTED | Cancel a subscription to a particular event |

## Key

The `status` options are:

+ `SUPPORTED` - Basic support is complete
+ `PARTIALLY` - Partial support and a description including more specific details
+ `NOT IMPLEMENTED` - Currently not supported/implemented

## `CONFIG NAMESPACE`

### `config_getShowCalls`

[source](src/configuration_api.rs)

Gets the current value of `show_calls` that's originally set with `--show-calls` option

#### Arguments

+ _NONE_

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_getShowCalls","params": []}'
```

### `config_setShowCalls`

[source](src/configuration_api.rs)

Updates `show_calls` to print more detailed call traces

#### Arguments

+ `value: String ('None', 'User', 'System', 'All')`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_setShowCalls","params": ["all"]}'
```

### `config_setResolveHashes`

[source](src/configuration_api.rs)

Updates `resolve-hashes` to call OpenChain for human-readable ABI names in call traces

#### Arguments

+ `value: boolean`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_setResolveHashes","params": [true]}'
```

## `NETWORK NAMESPACE`

### `net_version`

[source](src/network_api.rs)

Returns the current network id

#### Arguments

+ _NONE_

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "net_version","params": []}'
```

### `net_peerCount`

[source](src/network_api.rs)

Returns the number of connected peers

#### Arguments

+ _NONE_

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "net_peerCount","params": []}'
```

### `net_listening`

[source](src/network_api.rs)

Returns `true` if the node is listening for connections

#### Arguments

+ _NONE_

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "net_listening","params": []}'
```

## `ETH NAMESPACE`

### `eth_chainId`

[source](src/eth_api.rs)

Returns the current chain id

#### Arguments

+ _NONE_

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "eth_chainId","params": []}'
```

### `eth_estimateGas`

[source](src/eth_api.rs)

Generates and returns an estimate of how much gas is necessary to allow the transaction to complete

#### Arguments

+ `transaction: Transaction`

#### Status

`PARTIALLY`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "eth_estimateGas",
    "params": [{
      "0x0000000000000000000000000000000000000000": true
    }]
  }'
```

### `eth_gasPrice`

[source](src/eth_api.rs)

Returns the current price per gas in wei

#### Arguments

+ _NONE_

#### Status

`PARTIALLY`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "eth_gasPrice","params": []}'
```

### `eth_getBalance`

[source](src/eth_api.rs)

Returns the balance of the account of given address

#### Arguments

+ `address: Address`

+ `block: BlockNumber`

#### Status

`IMPLEMENTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "eth_getBalance",
    "params": ["0x0000000000000000000000000000000000000000", "latest"]
}'
```

### `eth_getBlockByNumber`

[source](src/eth_api.rs)

Returns information about a block by block number

#### Arguments

+ `block: BlockNumber`

+ `full: boolean`

#### Status

`PARTIALLY`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "eth_getBlockByNumber",
    "params": ["latest", true]
}'
```

### `eth_getCode`

[source](src/eth_api.rs)

Returns code at a given address

#### Arguments

+ `address: Address`

+ `block: BlockNumber`

#### Status

`IMPLEMENTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "eth_getCode",
    "params": ["0x0000000000000000000000000000000000000000", "latest"]
}'
```

### `eth_getTransactionByHash`

[source](src/eth_api.rs)

Returns the information about a transaction requested by transaction hash

#### Arguments

+ `hash: Hash`

#### Status

`IMPLEMENTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "eth_getTransactionByHash",
    "params": ["0x0000000000000000000000000000000000000000000000000000000000000000"]
}'
```
