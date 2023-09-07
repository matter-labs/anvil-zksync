# 🔧 Supported APIs for In-Memory Node 🔧

> ⚠️ **WORK IN PROGRESS**: This list is non-comprehensive and being updated. If there is an API that requires additional support, please start by [creating a GitHub Issue](https://github.com/matter-labs/era-test-node/issues/new/choose).

## Key

The `status` options are:

+ `SUPPORTED` - Basic support is complete
+ `PARTIALLY` - Partial support and a description including more specific details
+ `NOT IMPLEMENTED` - Currently not supported/implemented

## Supported APIs Table

| Namespace | API | <div style="width:130px">Status</div> | Description |
| --- | --- | --- | --- |
| [`CONFIG`](#config-namespace) | [`config_getShowCalls`](#config_getshowcalls) | `SUPPORTED` | Gets the current value of `show_calls` that's originally set with `--show-calls` option |
| [`CONFIG`](#config-namespace) | [`config_setResolveHashes`](#config_setresolvehashes) | `SUPPORTED` | Updates `resolve-hashes` to call OpenChain for human-readable ABI names in call traces |
| [`CONFIG`](#config-namespace) | [`config_setShowCalls`](#config_setshowcalls) | `SUPPORTED` | Updates `show_calls` to print more detailed call traces |
| [`CONFIG`](#config-namespace) | [`config_setShowStorageLogs`](#config_setshowstoragelogs) | `SUPPORTED` | Updates `show_storage_logs` to print storage log reads/writes |
| [`CONFIG`](#config-namespace) | [`config_setShowVmDetails`](#config_setshowvmdetails) | `SUPPORTED` | Updates `show_vm_details` to print more detailed results from vm execution |
| [`CONFIG`](#config-namespace) | [`config_setShowGasDetails`](#config_setshowgasdetails) | `SUPPORTED` | Updates `show_gas_details` to print more details about gas estimation and usage |
| `DEBUG` | `debug_traceCall` | `NOT IMPLEMENTED`<br />[GitHub Issue #61](https://github.com/matter-labs/era-test-node/issues/61) | Performs a call and returns structured traces of the execution |
| `DEBUG` | `debug_traceBlockByHash` | `NOT IMPLEMENTED`<br />[GitHub Issue #63](https://github.com/matter-labs/era-test-node/issues/63) | Returns structured traces for operations within the block of the specified block hash |
| `DEBUG` | `debug_traceBlockByNumber` | `NOT IMPLEMENTED`<br />[GitHub Issue #64](https://github.com/matter-labs/era-test-node/issues/64) | Returns structured traces for operations within the block of the specified block number |
| `DEBUG` | `debug_traceTransaction` | `NOT IMPLEMENTED`<br />[GitHub Issue #65](https://github.com/matter-labs/era-test-node/issues/65) | Returns a structured trace of the execution of the specified transaction |
| `ETH` | `eth_accounts` | `NOT IMPLEMENTED`<br />[GitHub Issue #50](https://github.com/matter-labs/era-test-node/issues/50) | Returns a list of addresses owned by client |
| [`ETH`](#eth-namespace) | [`eth_chainId`](#eth_chainid) | `SUPPORTED` | Returns the currently configured chain id <br />_(default is `260`)_ |
| `ETH` | `eth_coinbase` | `NOT IMPLEMENTED` | Returns the client coinbase address |
| [`ETH`](#eth-namespace) | [`eth_estimateGas`](#eth_estimategas) | `SUPPORTED` | Generates and returns an estimate of how much gas is necessary for the transaction to complete |
| `ETH` | `eth_feeHistory` | `NOT IMPLEMENTED` | Returns a collection of historical block gas data |
| [`ETH`](#eth-namespace) | [`eth_gasPrice`](#eth_gasprice) | `SUPPORTED` | Returns the current price per gas in wei <br />_(hardcoded to `250_000_000`)_ |
| [`ETH`](#eth-namespace) | [`eth_getBalance`](#eth_getbalance) | `SUPPORTED` | Returns the balance of the account of given address |
| `ETH` | `eth_getBlockByHash` | `NOT IMPLEMENTED`<br />[GitHub Issue #25](https://github.com/matter-labs/era-test-node/issues/25) | Returns information about a block by block hash |
| [`ETH`](#eth-namespace) | [`eth_getBlockByNumber`](#eth_getblockbynumber) | `PARTIALLY`<br />[GitHub Issue #71](https://github.com/matter-labs/era-test-node/issues/71) | Returns information about a block by block number<br /> ⚠️ _Support not available for `earliest`, `pending`, or block numbers other than the current block number_ |
| `ETH` | `eth_getBlockTransactionCountByHash` | `NOT IMPLEMENTED`<br />[GitHub Issue #44](https://github.com/matter-labs/era-test-node/issues/44) | Number of transactions in a block from a block matching the given block hash |
| `ETH` | `eth_getBlockTransactionCountByNumber` | `NOT IMPLEMENTED`<br />[GitHub Issue #43](https://github.com/matter-labs/era-test-node/issues/43) | Number of transactions in a block from a block matching the given block number |
| `ETH` | `eth_getCompilers` | `NOT IMPLEMENTED` | Returns a list of available compilers |
| [`ETH`](#eth-namespace) | [`eth_getTransactionByHash`](#eth_gettransactionbyhash) | `SUPPORTED` | Returns the information about a transaction requested by transaction hash |
| [`ETH`](#eth-namespace) | [`eth_getTransactionCount`](#eth_gettransactioncount) | `SUPPORTED` | Returns the number of transactions sent from an address |
| [`ETH`](#eth-namespace) | [`eth_blockNumber`](#eth_blocknumber) | `SUPPORTED` | Returns the number of the most recent block |
| [`ETH`](#eth-namespace) | [`eth_call`](#eth_call) | `SUPPORTED` | Executes a new message call immediately without creating a transaction on the block chain |
| [`ETH`](#eth-namespace) | [`eth_sendRawTransaction`](#eth_sendrawtransaction) | `SUPPORTED` | Creates new message call transaction or a contract creation for signed transactions |
| [`ETH`](#eth-namespace) | [`eth_getCode`](#eth_getcode) | `SUPPORTED` | Returns code at a given address |
| `ETH` | `eth_getFilterChanges` | `NOT IMPLEMENTED`<br />[GitHub Issue #42](https://github.com/matter-labs/era-test-node/issues/42) | Polling method for a filter, which returns an array of logs, block hashes, or transaction hashes, depending on the filter type, which occurred since last poll |
| `ETH` | `eth_getFilterLogs` | `NOT IMPLEMENTED`<br />[GitHub Issue #41](https://github.com/matter-labs/era-test-node/issues/41) | Returns an array of all logs matching filter with given id |
| `ETH` | `eth_getLogs` | `NOT IMPLEMENTED`<br />[GitHub Issue #40](https://github.com/matter-labs/era-test-node/issues/40) | Returns an array of all logs matching a given filter object |
| `ETH` | `eth_getProof` | `NOT IMPLEMENTED` | Returns the details for the account at the specified address and block number, the account's Merkle proof, and the storage values for the specified storage keys with their Merkle-proofs |
| `ETH` | `eth_getStorageAt` | `NOT IMPLEMENTED`<br />[GitHub Issue #45](https://github.com/matter-labs/era-test-node/issues/45) | Returns the value from a storage position at a given address |
| `ETH` | `eth_getTransactionByBlockHashAndIndex` | `NOT IMPLEMENTED`<br />[GitHub Issue #46](https://github.com/matter-labs/era-test-node/issues/46) | Returns information about a transaction by block hash and transaction index position |
| `ETH` | `eth_getTransactionByBlockNumberAndIndex` | `NOT IMPLEMENTED`<br />[GitHub Issue #47](https://github.com/matter-labs/era-test-node/issues/47) | Returns information about a transaction by block number and transaction index position |
| [`ETH`](#eth-namespace) | [`eth_getTransactionReceipt`](#eth_gettransactionreceipt) | `SUPPORTED` | Returns the receipt of a transaction by transaction hash |
| `ETH` | `eth_getUncleByBlockHashAndIndex` | `NOT IMPLEMENTED` | Returns information about a uncle of a block by hash and uncle index position |
| `ETH` | `eth_getUncleByBlockNumberAndIndex` | `NOT IMPLEMENTED` | Returns information about a uncle of a block by hash and uncle index position |
| `ETH` | `eth_getUncleCountByBlockHash` | `NOT IMPLEMENTED` | Returns the number of uncles in a block from a block matching the given block hash |
| `ETH` | `eth_getUncleCountByBlockNumber` | `NOT IMPLEMENTED` | Returns the number of uncles in a block from a block matching the given block hash |
| `ETH` | `eth_getWork` | `NOT IMPLEMENTED` | Returns: An Array with the following elements<br /> 1: DATA, 32 Bytes - current block header pow-hash<br /> 2: DATA, 32 Bytes - the seed hash used for the DAG.<br /> 3: DATA, 32 Bytes - the boundary condition ("target"), 2^256 / difficulty |
| `ETH` | `eth_hashrate` | `NOT IMPLEMENTED` | Returns the number of hashes per second that the node is mining with |
| `ETH` | `eth_maxPriorityFeePerGas` | `NOT IMPLEMENTED` | Returns a `maxPriorityFeePerGas` value suitable for quick transaction inclusion |
| `ETH` | `eth_mining` | `NOT IMPLEMENTED` | Returns `true` if client is actively mining new blocks |
| `ETH` | `eth_newBlockFilter` | `NOT IMPLEMENTED`<br />[GitHub Issue #37](https://github.com/matter-labs/era-test-node/issues/37) | Creates a filter in the node, to notify when a new block arrives |
| `ETH` | `eth_newFilter` | `NOT IMPLEMENTED`<br />[GitHub Issue #36](https://github.com/matter-labs/era-test-node/issues/36) | Creates a filter object, based on filter options, to notify when the state changes (logs) |
| `ETH` | `eth_newPendingTransactionFilter` | `NOT IMPLEMENTED`<br />[GitHub Issue #39](https://github.com/matter-labs/era-test-node/issues/39) | Creates a filter in the node, to notify when new pending transactions arrive |
| `ETH` | `eth_protocolVersion` | `NOT IMPLEMENTED`<br />[GitHub Issue #48](https://github.com/matter-labs/era-test-node/issues/48) | Returns the current ethereum protocol version |
| `ETH` | `eth_sendTransaction` | `NOT IMPLEMENTED` | Creates new message call transaction or a contract creation, if the data field contains code |
| `ETH` | `eth_sign` | `NOT IMPLEMENTED` | The sign method calculates an Ethereum specific signature with: `sign(keccak256("\x19Ethereum Signed Message:\n" + message.length + message)))` |
| `ETH` | `eth_signTransaction` | `NOT IMPLEMENTED` | Signs a transaction that can be submitted to the network at a later time using `eth_sendRawTransaction` |
| `ETH` | `eth_signTypedData` | `NOT IMPLEMENTED` | Identical to `eth_signTypedData_v4` |
| `ETH` | `eth_signTypedData_v4` | `NOT IMPLEMENTED` | Returns `Promise<string>: Signature`. As in `eth_sign`, it is a hex encoded 129 byte array starting with `0x`. |
| `ETH` | `eth_submitHashrate` | `NOT IMPLEMENTED` | Used for submitting mining hashrate |
| `ETH` | `eth_submitWork` | `NOT IMPLEMENTED` | Used for submitting a proof-of-work solution |
| `ETH` | `eth_subscribe` | `NOT IMPLEMENTED` | Starts a subscription to a particular event |
| [`ETH`](#eth-namespace) | [`eth_syncing`](#eth_syncing) | `SUPPORTED` | Returns an object containing data about the sync status or `false` when not syncing |
| `ETH` | `eth_uninstallFilter` | `NOT IMPLEMENTED`<br />[GitHub Issue #38](https://github.com/matter-labs/era-test-node/issues/38) | Uninstalls a filter with given id |
| `ETH` | `eth_unsubscribe` | `NOT IMPLEMENTED` | Cancel a subscription to a particular event |
| `EVM` | `evm_addAccount` | `NOT IMPLEMENTED` | Adds any arbitrary account |
| `EVM` | `evm_increaseTime` | `NOT IMPLEMENTED`<br />[GitHub Issue #66](https://github.com/matter-labs/era-test-node/issues/66) | Jump forward in time by the given amount of time, in seconds |
| `EVM` | `evm_mine` | `NOT IMPLEMENTED`<br />[GitHub Issue #67](https://github.com/matter-labs/era-test-node/issues/67) | Force a single block to be mined |
| `EVM` | `evm_removeAccount` | `NOT IMPLEMENTED` | Removes an account |
| `EVM` | `evm_revert` | `NOT IMPLEMENTED`<br />[GitHub Issue #70](https://github.com/matter-labs/era-test-node/issues/70) | Revert the state of the blockchain to a previous snapshot |
| `EVM` | `evm_setAccountBalance` | `NOT IMPLEMENTED` | Sets the given account's balance to the specified WEI value |
| `EVM` | `evm_setAccountCode` | `NOT IMPLEMENTED` | Sets the given account's code to the specified data |
| `EVM` | `evm_setAccountNonce` | `NOT IMPLEMENTED` | Sets the given account's nonce to the specified value |
| `EVM` | `evm_setAccountStorageAt` | `NOT IMPLEMENTED` | Sets the given account's storage slot to the specified data |
| `EVM` | `evm_setAutomine` | `NOT IMPLEMENTED` | Enables or disables the automatic mining of new blocks with each new transaction submitted to the network |
| `EVM` | `evm_setBlockGasLimit` | `NOT IMPLEMENTED` | Sets the Block Gas Limit of the network |
| `EVM` | `evm_setIntervalMining` | `NOT IMPLEMENTED` | Enables (with a numeric argument greater than 0) or disables (with a numeric argument equal to 0), the automatic mining of blocks at a regular interval of milliseconds, each of which will include all pending transactions |
| `EVM` | `evm_setNextBlockTimestamp` | `NOT IMPLEMENTED`<br />[GitHub Issue #68](https://github.com/matter-labs/era-test-node/issues/68) | Works like `evm_increaseTime`, but takes the exact timestamp that you want in the next block, and increases the time accordingly |
| `EVM` | `evm_setTime` | `NOT IMPLEMENTED` | Sets the internal clock time to the given timestamp |
| `EVM` | `evm_snapshot` | `NOT IMPLEMENTED`<br />[GitHub Issue #69](https://github.com/matter-labs/era-test-node/issues/69) | Snapshot the state of the blockchain at the current block |
| `HARDHAT` | `hardhat_addCompilationResult` | `NOT IMPLEMENTED` | Add information about compiled contracts |
| `HARDHAT` | `hardhat_dropTransaction` | `NOT IMPLEMENTED` | Remove a transaction from the mempool |
| `HARDHAT` | `hardhat_impersonateAccount` | `NOT IMPLEMENTED`<br />[GitHub Issue #73](https://github.com/matter-labs/era-test-node/issues/73) | Impersonate an account |
| `HARDHAT` | `hardhat_getAutomine` | `NOT IMPLEMENTED` | Returns `true` if automatic mining is enabled, and `false` otherwise |
| `HARDHAT` | `hardhat_metadata` | `NOT IMPLEMENTED` | Returns the metadata of the current network |
| `HARDHAT` | `hardhat_mine` | `NOT IMPLEMENTED`<br />[GitHub Issue #75](https://github.com/matter-labs/era-test-node/issues/75) | Mine any number of blocks at once, in constant time |
| `HARDHAT` | `hardhat_reset` | `NOT IMPLEMENTED` | Resets the state of the network |
| [`HARDHAT`](#hardhat-namespace) | [`hardhat_setBalance`](#hardhat_setbalance) | `SUPPORTED` | Modifies the balance of an account |
| `HARDHAT` | `hardhat_setCode` | `NOT IMPLEMENTED` | Sets the bytecode of a given account |
| `HARDHAT` | `hardhat_setCoinbase` | `NOT IMPLEMENTED` | Sets the coinbase address |
| `HARDHAT` | `hardhat_setLoggingEnabled` | `NOT IMPLEMENTED` | Enables or disables logging in Hardhat Network |
| `HARDHAT` | `hardhat_setMinGasPrice` | `NOT IMPLEMENTED` | Sets the minimum gas price |
| `HARDHAT` | `hardhat_setNextBlockBaseFeePerGas` | `NOT IMPLEMENTED` | Sets the base fee per gas for the next block |
| `HARDHAT` | `hardhat_setPrevRandao` | `NOT IMPLEMENTED` | Sets the PREVRANDAO value of the next block |
| [`HARDHAT`](#hardhat-namespace) | [`hardhat_setNonce`](#hardhat_setnonce) | `SUPPORTED` | Sets the nonce of a given account |
| `HARDHAT` | `hardhat_setStorageAt` | `NOT IMPLEMENTED` | Sets the storage value at a given key for a given account |
| `HARDHAT` | `hardhat_stopImpersonatingAccount` | `NOT IMPLEMENTED`<br />[GitHub Issue #74](https://github.com/matter-labs/era-test-node/issues/74) | Stop impersonating an account after having previously used `hardhat_impersonateAccount` |
| [`NETWORK`](#network-namespace) | [`net_version`](#net_version) | `SUPPORTED` | Returns the current network id <br />_(default is `260`)_ |
| [`NETWORK`](#network-namespace) | [`net_peerCount`](#net_peercount) | `SUPPORTED` | Returns the number of peers currently connected to the client <br/>_(hard-coded to `0`)_ |
| [`NETWORK`](#network-namespace) | [`net_listening`](#net_listening) | `SUPPORTED` | Returns `true` if the client is actively listening for network connections <br />_(hard-coded to `false`)_ |
| [`ZKS`](#zks-namespace) | [`zks_estimateFee`](#zks_estimateFee) | `SUPPORTED` | Gets the Fee estimation data for a given Request |
| `ZKS` | `zks_estimateGasL1ToL2` | `NOT IMPLEMENTED` | Estimate of the gas required for a L1 to L2 transaction |
| `ZKS` | `zks_getAllAccountBalances` | `NOT IMPLEMENTED` | Returns all balances for confirmed tokens given by an account address |
| `ZKS` | `zks_getBlockDetails` | `NOT IMPLEMENTED` | Returns additional zkSync-specific information about the L2 block |
| `ZKS` | `zks_getBridgeContracts` | `NOT IMPLEMENTED` | Returns L1/L2 addresses of default bridges |
| `ZKS` | `zks_getBytecodeByHash` | `NOT IMPLEMENTED` | Returns bytecode of a transaction given by its hash |
| `ZKS` | `zks_getConfirmedTokens` | `NOT IMPLEMENTED` | Returns [address, symbol, name, and decimal] information of all tokens within a range of ids given by parameters `from` and `limit` |
| `ZKS` | `zks_getL1BatchBlockRange` | `NOT IMPLEMENTED` | Returns the range of blocks contained within a batch given by batch number |
| `ZKS` | `zks_getL1BatchDetails` | `NOT IMPLEMENTED` | Returns data pertaining to a given batch |
| `ZKS` | `zks_getL2ToL1LogProof` | `NOT IMPLEMENTED` | Given a transaction hash, and an index of the L2 to L1 log produced within the transaction, it returns the proof for the corresponding L2 to L1 log |
| `ZKS` | `zks_getL2ToL1MsgProof` | `NOT IMPLEMENTED` | Given a block, a sender, a message, and an optional message log index in the block containing the L1->L2 message, it returns the proof for the message sent via the L1Messenger system contract |
| `ZKS` | `zks_getMainContract` | `NOT IMPLEMENTED` | Returns the address of the zkSync Era contract |
| `ZKS` | `zks_getRawBlockTransactions` | `NOT IMPLEMENTED` | Returns data of transactions in a block |
| `ZKS` | `zks_getTestnetPaymaster` | `NOT IMPLEMENTED` | Returns the address of the testnet paymaster |
| [`ZKS`](#zks-namespace) | [`zks_getTokenPrice`](#zks_getTokenPrice) | `SUPPORTED` | Gets the USD price of a token <br />_(`ETH` is hard-coded to `1_500`, while some others are `1`)_ |
| `ZKS` | `zks_getTransactionDetails` | `NOT IMPLEMENTED` | Returns data from a specific transaction given by the transaction hash |
| `ZKS` | `zks_L1BatchNumber` | `NOT IMPLEMENTED` | Returns the latest L1 batch number |
| `ZKS` | `zks_L1ChainId` | `NOT IMPLEMENTED` | Returns the chain id of the underlying L1 |

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

### `config_setShowStorageLogs`

[source](src/configuration_api.rs)

Updates `show_storage_logs` to print storage log reads/writes

#### Arguments

+ `value: String ('None', 'Read', 'Write', 'All')`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_setShowStorageLogs","params": ["all"]}'
```

### `config_setShowVmDetails`

[source](src/configuration_api.rs)

Updates `show_vm_details` to print more detailed results from vm execution

#### Arguments

+ `value: String ('None', 'All')`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_setShowVmDetails","params": ["all"]}'
```

### `config_setShowGasDetails`

[source](src/configuration_api.rs)

Updates `show_gas_details` to print more details about gas estimation and usage

#### Arguments

+ `value: String ('None', 'All')`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_setShowGasDetails","params": ["all"]}'
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

[source](src/net.rs)

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

[source](src/net.rs)

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

[source](src/net.rs)

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

[source](src/node.rs)

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

[source](src/node.rs)

Generates and returns an estimate of how much gas is necessary to allow the transaction to complete

#### Arguments

+ `transaction: Transaction`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
      "id": "2",
      "method": "eth_estimateGas",
      "params": [{
          "to": "0x36615Cf349d7F6344891B1e7CA7C72883F5dc049",
          "data": "0x0000",
          "from": "0xa61464658AfeAf65CccaaFD3a512b69A83B77618",
          "gas": "0x0000",
          "gasPrice": "0x0000",
          "value": "0x0000",
          "nonce": "0x0000"
      }, "latest"]
  }'
```

### `eth_gasPrice`

[source](src/node.rs)

Returns the current price per gas in wei

#### Arguments

+ _NONE_

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "eth_gasPrice","params": []}'
```

### `eth_getBalance`

[source](src/node.rs)

Returns the balance of the account of given address

#### Arguments

+ `address: Address`

+ `block: BlockNumber`

#### Status

`SUPPORTED`

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

[source](src/node.rs)

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

[source](src/node.rs)

Returns code at a given address

#### Arguments

+ `address: Address`

+ `block: BlockNumber`

#### Status

`SUPPORTED`

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

[source](src/node.rs)

Returns the information about a transaction requested by transaction hash

#### Arguments

+ `hash: Hash`

#### Status

`SUPPORTED`

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

### `eth_getTransactionCount`

[source](src/node.rs)

Returns the number of transactions sent from an address

#### Arguments

+ `address: Address`

+ `block: BlockNumber`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "eth_getTransactionCount",
    "params": ["0x0000000000000000000000000000000000000000", "latest"]
}'
```

### `eth_getTransactionReceipt`

[source](src/node.rs)

Returns the transaction receipt for a given transaction hash

#### Arguments

+ `hash: H256`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "eth_getTransactionReceipt",
    "params": ["0x0000000000000000000000000000000000000000"]
}'
```

### `eth_blockNumber`

[source](src/node.rs)

Returns the number of most recent block

#### Arguments

+ _NONE_

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "eth_blockNumber","params": []}'
```

### `eth_call`

[source](src/node.rs)

Executes a new message call immediately without creating a transaction on the block chain

#### Arguments

+ `transaction: Transaction`

+ `block: BlockNumber`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
      "id": "2",
      "method": "eth_call",
      "params": [{
          "to": "0x36615Cf349d7F6344891B1e7CA7C72883F5dc049",
          "data": "0x0000",
          "from": "0xa61464658AfeAf65CccaaFD3a512b69A83B77618",
          "gas": "0x0000",
          "gasPrice": "0x0000",
          "value": "0x0000",
          "nonce": "0x0000"
      }, "latest"]
  }'
```

### `eth_sendRawTransaction`

[source](src/node.rs)

Creates new message call transaction or a contract creation for signed transactions

#### Arguments

+ `transaction: Transaction`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "eth_sendRawTransaction","params": ["0x0000"]
}'
```

### `eth_syncing`

[source](src/node.rs)

Returns syncing status of the node. This will always return `false`.

#### Arguments

+ _NONE_

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "eth_syncing","params": []
}'
```

## `HARDHAT NAMESPACE`

### `hardhat_setBalance`

[source](src/hardhat.rs)

Sets the balance of the given address to the given balance.

#### Arguments

+ `address: Address` - The `Address` whose balance will be edited
+ `balance: U256` - The balance to set for the given address, in wei

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
      "id": "1",
      "method": "hardhat_setBalance",
      "params": [
        "0x36615Cf349d7F6344891B1e7CA7C72883F5dc049",
        "0x1337"
      ]
  }'
```

### `hardhat_setNonce`

[source](src/hardhat.rs)

Modifies an account's nonce by overwriting it.
The new nonce must be greater than the existing nonce.

#### Arguments

+ `address: Address` - The `Address` whose nonce is to be changed
+ `nonce: U256` - The new nonce

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
      "id": "1",
      "method": "hardhat_setNonce",
      "params": [
        "0x36615Cf349d7F6344891B1e7CA7C72883F5dc049",
        "0x1337"
      ]
  }'
```

## `ZKS NAMESPACE`

### `zks_estimateFee`

[source](src/zks.rs)

Generates and returns an estimate of how much gas is necessary to allow the transaction to complete

#### Arguments

+ `transaction: Transaction`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
      "id": "2",
      "method": "zks_estimateFee",
      "params": [{
          "to": "0x36615Cf349d7F6344891B1e7CA7C72883F5dc049",
          "data": "0x0000",
          "from": "0xa61464658AfeAf65CccaaFD3a512b69A83B77618",
          "gas": "0x0000",
          "gasPrice": "0x0000",
          "value": "0x0000",
          "nonce": "0x0000"
      }]
  }'
```

### `zks_getTokenPrice`

[source](src/zks.rs)

Returns the token price given an Address

#### Arguments

+ `address: Address`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "zks_getTokenPrice","params": ["0x0000000000000000000000000000000000000000"]}'
```
