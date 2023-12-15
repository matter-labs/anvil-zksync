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
| [`CONFIG`](#config-namespace) | [`config_getCurrentTimestamp`](#config_getcurrenttimestamp) | `SUPPORTED` | Gets the value of `current_timestamp` for the node |
| [`CONFIG`](#config-namespace) | [`config_setResolveHashes`](#config_setresolvehashes) | `SUPPORTED` | Updates `resolve-hashes` to call OpenChain for human-readable ABI names in call traces |
| [`CONFIG`](#config-namespace) | [`config_setShowCalls`](#config_setshowcalls) | `SUPPORTED` | Updates `show_calls` to print more detailed call traces |
| [`CONFIG`](#config-namespace) | [`config_setShowStorageLogs`](#config_setshowstoragelogs) | `SUPPORTED` | Updates `show_storage_logs` to print storage log reads/writes |
| [`CONFIG`](#config-namespace) | [`config_setShowVmDetails`](#config_setshowvmdetails) | `SUPPORTED` | Updates `show_vm_details` to print more detailed results from vm execution |
| [`CONFIG`](#config-namespace) | [`config_setShowGasDetails`](#config_setshowgasdetails) | `SUPPORTED` | Updates `show_gas_details` to print more details about gas estimation and usage |
| [`CONFIG`](#config-namespace) | [`config_setLogLevel`](#config_setloglevel) | `SUPPORTED` | Sets the logging level for the node and only displays the node logs. |
| [`CONFIG`](#config-namespace) | [`config_setLogging`](#config_setlogging) | `SUPPORTED` | Sets the fine-tuned logging levels for the node and any of its dependencies |
| [`DEBUG`](#debug-namespace) | [`debug_traceCall`](#debug_tracecall) | `SUPPORTED` | Performs a call and returns structured traces of the execution |
| [`DEBUG`](#debug-namespace) | [`debug_traceBlockByHash`](#debug_traceblockbyhash) | `SUPPORTED` | Returns structured traces for operations within the block of the specified block hash |
| [`DEBUG`](#debug-namespace) | [`debug_traceBlockByNumber`](#debug_traceblockbynumber) | `SUPPORTED` | Returns structured traces for operations within the block of the specified block number |
| [`DEBUG`](#debug-namespace) | [`debug_traceTransaction`](#debug_tracetransaction) | `SUPPORTED` | Returns a structured trace of the execution of the specified transaction |
| `ETH` | `eth_accounts` | `SUPPORTED` | Returns a list of addresses owned by client |
| [`ETH`](#eth-namespace) | [`eth_chainId`](#eth_chainid) | `SUPPORTED` | Returns the currently configured chain id <br />_(default is `260`)_ |
| `ETH` | `eth_coinbase` | `NOT IMPLEMENTED` | Returns the client coinbase address |
| [`ETH`](#eth-namespace) | [`eth_estimateGas`](#eth_estimategas) | `SUPPORTED` | Generates and returns an estimate of how much gas is necessary for the transaction to complete |
| [`ETH`](#eth-namespace) | [`eth_feeHistory`](#eth_feehistory) | `SUPPORTED` | Returns a collection of historical block gas data <br />_(hardcoded with gas price of `250_000_000`)_ |
| [`ETH`](#eth-namespace) | [`eth_gasPrice`](#eth_gasprice) | `SUPPORTED` | Returns the current price per gas in wei <br />_(hardcoded to `250_000_000`)_ |
| [`ETH`](#eth-namespace) | [`eth_getBalance`](#eth_getbalance) | `SUPPORTED` | Returns the balance of the account of given address |
| [`ETH`](#eth-namespace) | [`eth_getBlockByHash`](#eth_getblockbyhash) | `SUPPORTED` | Returns information about a block by block hash |
| [`ETH`](#eth-namespace) | [`eth_getBlockByNumber`](#eth_getblockbynumber) | `SUPPORTED` | Returns information about a block by block number |
| [`ETH`](#eth-namespace) | [`eth_getBlockTransactionCountByHash`](#eth_getblocktransactioncountbyhash) | `SUPPORTED` | Number of transactions in a block from a block matching the given block hash |
| [`ETH`](#eth-namespace) | [`eth_getBlockTransactionCountByNumber`](#eth_getblocktransactioncountbynumber) | `SUPPORTED` | Number of transactions in a block from a block matching the given block number |
| `ETH` | `eth_getCompilers` | `NOT IMPLEMENTED` | Returns a list of available compilers |
| [`ETH`](#eth-namespace) | [`eth_getTransactionByHash`](#eth_gettransactionbyhash) | `SUPPORTED` | Returns the information about a transaction requested by transaction hash |
| [`ETH`](#eth-namespace) | [`eth_getTransactionCount`](#eth_gettransactioncount) | `SUPPORTED` | Returns the number of transactions sent from an address |
| [`ETH`](#eth-namespace) | [`eth_blockNumber`](#eth_blocknumber) | `SUPPORTED` | Returns the number of the most recent block |
| [`ETH`](#eth-namespace) | [`eth_call`](#eth_call) | `SUPPORTED` | Executes a new message call immediately without creating a transaction on the block chain |
| [`ETH`](#eth-namespace) | [`eth_sendRawTransaction`](#eth_sendrawtransaction) | `SUPPORTED` | Creates new message call transaction or a contract creation for signed transactions |
| [`ETH`](#eth-namespace) | [`eth_getCode`](#eth_getcode) | `SUPPORTED` | Returns code at a given address |
| [`ETH`](#eth-namespace) | [`eth_getFilterChanges`](#`eth_getfilterchanges) | `SUPPORTED` | Polling method for a filter, which returns an array of logs, block hashes, or transaction hashes, depending on the filter type, which occurred since last poll |
| [`ETH`](#eth-namespace) | [`eth_getFilterLogs`](#eth_getfilterlogs) | `SUPPORTED` | Returns an array of all logs matching filter with given id |
| [`ETH`](#eth-namespace) | [`eth_getLogs`](#eth_getlogs) | `SUPPORTED` | Returns an array of all logs matching a given filter object |
| `ETH` | `eth_getProof` | `NOT IMPLEMENTED` | Returns the details for the account at the specified address and block number, the account's Merkle proof, and the storage values for the specified storage keys with their Merkle-proofs |
| [`ETH`](#eth-namespace) | [`eth_getStorageAt`](#eth_getstorageat) | `SUPPORTED` | Returns the value from a storage position at a given address |
| [`ETH`](#eth-namespace) | [`eth_getTransactionByBlockHashAndIndex`](#eth_gettransactionbyblockhashandindex) | `SUPPORTED` | Returns information about a transaction by block hash and transaction index position |
| [`ETH`](#eth-namespace) | [`eth_getTransactionByBlockNumberAndIndex`](#eth_gettransactionbyblocknumberandindex) | `SUPPORTED` | Returns information about a transaction by block number and transaction index position |
| [`ETH`](#eth-namespace) | [`eth_getTransactionReceipt`](#eth_gettransactionreceipt) | `SUPPORTED` | Returns the receipt of a transaction by transaction hash |
| `ETH` | `eth_getUncleByBlockHashAndIndex` | `NOT IMPLEMENTED` | Returns information about a uncle of a block by hash and uncle index position |
| `ETH` | `eth_getUncleByBlockNumberAndIndex` | `NOT IMPLEMENTED` | Returns information about a uncle of a block by hash and uncle index position |
| `ETH` | `eth_getUncleCountByBlockHash` | `NOT IMPLEMENTED` | Returns the number of uncles in a block from a block matching the given block hash |
| `ETH` | `eth_getUncleCountByBlockNumber` | `NOT IMPLEMENTED` | Returns the number of uncles in a block from a block matching the given block hash |
| `ETH` | `eth_getWork` | `NOT IMPLEMENTED` | Returns: An Array with the following elements<br /> 1: DATA, 32 Bytes - current block header pow-hash<br /> 2: DATA, 32 Bytes - the seed hash used for the DAG.<br /> 3: DATA, 32 Bytes - the boundary condition ("target"), 2^256 / difficulty |
| `ETH` | `eth_hashrate` | `NOT IMPLEMENTED` | Returns the number of hashes per second that the node is mining with |
| `ETH` | `eth_maxPriorityFeePerGas` | `NOT IMPLEMENTED` | Returns a `maxPriorityFeePerGas` value suitable for quick transaction inclusion |
| `ETH` | `eth_mining` | `NOT IMPLEMENTED` | Returns `true` if client is actively mining new blocks |
| [`ETH`](#eth-namespace) | [`eth_newBlockFilter`](#`eth_newblockfilter) | `SUPPORTED` | Creates a filter in the node, to notify when a new block arrives |
| [`ETH`](#eth-namespace) | [`eth_newFilter`](#`eth_newfilter) | `SUPPORTED` | Creates a filter object, based on filter options, to notify when the state changes (logs) |
| [`ETH`](#eth-namespace) | [`eth_newPendingTransactionFilter`](#`eth_newpendingtransactionfilter) | `SUPPORTED` | Creates a filter in the node, to notify when new pending transactions arrive |
| [`ETH`](#eth-namespace) | [`eth_protocolVersion`](#eth_protocolversion) | `SUPPORTED` | Returns the current ethereum protocol version |
| [`ETH`](#eth-namespace) | [`eth_sendTransaction`](#eth_sendtransaction) | `SUPPORTED` | Creates new message call transaction or a contract creation, if the data field contains code |
| `ETH` | `eth_sign` | `NOT IMPLEMENTED` | The sign method calculates an Ethereum specific signature with: `sign(keccak256("\x19Ethereum Signed Message:\n" + message.length + message)))` |
| `ETH` | `eth_signTransaction` | `NOT IMPLEMENTED` | Signs a transaction that can be submitted to the network at a later time using `eth_sendRawTransaction` |
| `ETH` | `eth_signTypedData` | `NOT IMPLEMENTED` | Identical to `eth_signTypedData_v4` |
| `ETH` | `eth_signTypedData_v4` | `NOT IMPLEMENTED` | Returns `Promise<string>: Signature`. As in `eth_sign`, it is a hex encoded 129 byte array starting with `0x`. |
| `ETH` | `eth_submitHashrate` | `NOT IMPLEMENTED` | Used for submitting mining hashrate |
| `ETH` | `eth_submitWork` | `NOT IMPLEMENTED` | Used for submitting a proof-of-work solution |
| `ETH` | `eth_subscribe` | `NOT IMPLEMENTED` | Starts a subscription to a particular event |
| [`ETH`](#eth-namespace) | [`eth_syncing`](#eth_syncing) | `SUPPORTED` | Returns an object containing data about the sync status or `false` when not syncing |
| [`ETH`](#eth-namespace) | [`eth_uninstallFilter`](#`eth_uninstallfilter) | `SUPPORTED` | Uninstalls a filter with given id |
| `ETH` | `eth_unsubscribe` | `NOT IMPLEMENTED` | Cancel a subscription to a particular event |
| `EVM` | `evm_addAccount` | `NOT IMPLEMENTED` | Adds any arbitrary account |
| [`EVM`](#evm-namespace) | [`evm_increaseTime`](#evm_increasetime) | `SUPPORTED` | Jump forward in time by the given amount of time, in seconds |
| [`EVM`](#evm-namespace) | [`evm_mine`](#evm_mine) | `SUPPORTED` | Force a single block to be mined |
| `EVM` | `evm_removeAccount` | `NOT IMPLEMENTED` | Removes an account |
| [`EVM`](#evm-namespace) | [`evm_revert`](#evm_revert) | `SUPPORTED` | Revert the state of the blockchain to a previous snapshot |
| `EVM` | `evm_setAccountBalance` | `NOT IMPLEMENTED` | Sets the given account's balance to the specified WEI value |
| `EVM` | `evm_setAccountCode` | `NOT IMPLEMENTED` | Sets the given account's code to the specified data |
| `EVM` | `evm_setAccountNonce` | `NOT IMPLEMENTED` | Sets the given account's nonce to the specified value |
| `EVM` | `evm_setAccountStorageAt` | `NOT IMPLEMENTED` | Sets the given account's storage slot to the specified data |
| `EVM` | `evm_setAutomine` | `NOT IMPLEMENTED` | Enables or disables the automatic mining of new blocks with each new transaction submitted to the network |
| `EVM` | `evm_setBlockGasLimit` | `NOT IMPLEMENTED` | Sets the Block Gas Limit of the network |
| `EVM` | `evm_setIntervalMining` | `NOT IMPLEMENTED` | Enables (with a numeric argument greater than 0) or disables (with a numeric argument equal to 0), the automatic mining of blocks at a regular interval of milliseconds, each of which will include all pending transactions |
| [`EVM`](#evm-namespace) | [`evm_setNextBlockTimestamp`](#evm_setnextblocktimestamp) | `SUPPORTED` | Works like `evm_increaseTime`, but takes the exact timestamp that you want in the next block, and increases the time accordingly |
| [`EVM`](#evm-namespace) | [`evm_setTime`](#evm_settime) | `SUPPORTED` | Sets the internal clock time to the given timestamp |
| [`EVM`](#evm-namespace) | [`evm_snapshot`](#evm_snapshot) | `SUPPORTED` | Snapshot the state of the blockchain at the current block |
| `HARDHAT` | `hardhat_addCompilationResult` | `NOT IMPLEMENTED` | Add information about compiled contracts |
| `HARDHAT` | `hardhat_dropTransaction` | `NOT IMPLEMENTED` | Remove a transaction from the mempool |
| [`HARDHAT`](#hardhat-namespace) | [`hardhat_impersonateAccount`](#hardhat_impersonateaccount) | `SUPPORTED` | Impersonate an account |
| `HARDHAT` | `hardhat_getAutomine` | `NOT IMPLEMENTED` | Returns `true` if automatic mining is enabled, and `false` otherwise |
| `HARDHAT` | `hardhat_metadata` | `NOT IMPLEMENTED` | Returns the metadata of the current network |
| [`HARDHAT`](#hardhat-namespace) | [`hardhat_mine`](#hardhat_mine) | Mine any number of blocks at once, in constant time |
| `HARDHAT` | `hardhat_reset` | `NOT IMPLEMENTED` | Resets the state of the network |
| [`HARDHAT`](#hardhat-namespace) | [`hardhat_setBalance`](#hardhat_setbalance) | `SUPPORTED` | Modifies the balance of an account |
| [`HARDHAT`](#hardhat-namespace) | [`hardhat_setCode`](#hardhat_setcode) | `SUPPORTED` | Sets the bytecode of a given account |
| `HARDHAT` | `hardhat_setCoinbase` | `NOT IMPLEMENTED` | Sets the coinbase address |
| `HARDHAT` | `hardhat_setLoggingEnabled` | `NOT IMPLEMENTED` | Enables or disables logging in Hardhat Network |
| `HARDHAT` | `hardhat_setMinGasPrice` | `NOT IMPLEMENTED` | Sets the minimum gas price |
| `HARDHAT` | `hardhat_setNextBlockBaseFeePerGas` | `NOT IMPLEMENTED` | Sets the base fee per gas for the next block |
| `HARDHAT` | `hardhat_setPrevRandao` | `NOT IMPLEMENTED` | Sets the PREVRANDAO value of the next block |
| [`HARDHAT`](#hardhat-namespace) | [`hardhat_setNonce`](#hardhat_setnonce) | `SUPPORTED` | Sets the nonce of a given account |
| `HARDHAT` | `hardhat_setStorageAt` | `NOT IMPLEMENTED` | Sets the storage value at a given key for a given account |
| [`HARDHAT`](#hardhat-namespace) | [`hardhat_stopImpersonatingAccount`](#hardhat_stopimpersonatingaccount) | `SUPPORTED` | Stop impersonating an account after having previously used `hardhat_impersonateAccount` |
| [`NETWORK`](#network-namespace) | [`net_version`](#net_version) | `SUPPORTED` | Returns the current network id <br />_(default is `260`)_ |
| [`NETWORK`](#network-namespace) | [`net_peerCount`](#net_peercount) | `SUPPORTED` | Returns the number of peers currently connected to the client <br/>_(hard-coded to `0`)_ |
| [`NETWORK`](#network-namespace) | [`net_listening`](#net_listening) | `SUPPORTED` | Returns `true` if the client is actively listening for network connections <br />_(hard-coded to `false`)_ |
| [`WEB3`](#web3-namespace) | [`web3_clientVersion`](#web3_clientversion) | `SUPPORTED` | Returns `zkSync/v2.0` |
| [`ZKS`](#zks-namespace) | [`zks_estimateFee`](#zks_estimateFee) | `SUPPORTED` | Gets the Fee estimation data for a given Request |
| `ZKS` | `zks_estimateGasL1ToL2` | `NOT IMPLEMENTED` | Estimate of the gas required for a L1 to L2 transaction |
| [`ZKS`](#zks-namespace) | [`zks_getAllAccountBalances`](#zks_getallaccountbalances) | `SUPPORTED` | Returns all balances for confirmed tokens given by an account address |
| [`ZKS`](#zks-namespace) | [`zks_getBridgeContracts`](#zks_getbridgecontracts) | `SUPPORTED` | Returns L1/L2 addresses of default bridges |
| [`ZKS`](#zks-namespace) | [`zks_getBlockDetails`](#zks_getblockdetails) | `SUPPORTED` | Returns additional zkSync-specific information about the L2 block |
| `ZKS` | `zks_getBytecodeByHash` | `NOT IMPLEMENTED` | Returns bytecode of a transaction given by its hash |
| [`ZKS`](#zks-namespace) | [`zks_getConfirmedTokens`](#zks_getconfirmedtokens) | `SUPPORTED` | Returns [address, symbol, name, and decimal] information of all tokens within a range of ids given by parameters `from` and `limit` |
| `ZKS` | `zks_getL1BatchBlockRange` | `NOT IMPLEMENTED` | Returns the range of blocks contained within a batch given by batch number |
| `ZKS` | `zks_getL1BatchDetails` | `NOT IMPLEMENTED` | Returns data pertaining to a given batch |
| `ZKS` | `zks_getL2ToL1LogProof` | `NOT IMPLEMENTED` | Given a transaction hash, and an index of the L2 to L1 log produced within the transaction, it returns the proof for the corresponding L2 to L1 log |
| `ZKS` | `zks_getL2ToL1MsgProof` | `NOT IMPLEMENTED` | Given a block, a sender, a message, and an optional message log index in the block containing the L1->L2 message, it returns the proof for the message sent via the L1Messenger system contract |
| `ZKS` | `zks_getMainContract` | `NOT IMPLEMENTED` | Returns the address of the zkSync Era contract |
| [`ZKS`](#zks-namespace) | [`zks_getRawBlockTransactions`](#zks_getrawblocktransactions) | `SUPPORTED` | Returns data of transactions in a block |
| `ZKS` | `zks_getTestnetPaymaster` | `NOT IMPLEMENTED` | Returns the address of the testnet paymaster |
| [`ZKS`](#zks-namespace) | [`zks_getTokenPrice`](#zks_getTokenPrice) | `SUPPORTED` | Gets the USD price of a token <br />_(`ETH` is hard-coded to `1_500`, while some others are `1`)_ |
| [`ZKS`](#zks-namespace) | [`zks_getTransactionDetails`](#zks_gettransactiondetails) | `SUPPORTED` | Returns data from a specific transaction given by the transaction hash |
| `ZKS` | `zks_L1BatchNumber` | `NOT IMPLEMENTED` | Returns the latest L1 batch number |
| `ZKS` | `zks_L1ChainId` | `NOT IMPLEMENTED` | Returns the chain id of the underlying L1 |

## `CONFIG NAMESPACE`

### `config_getShowCalls`

[source](src/node/config.rs)

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

### `config_getCurrentTimestamp`

[source](src/node/config.rs)

Gets the value of `current_timestamp` for the node

#### Arguments

+ _NONE_

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_getCurrentTimestamp","params": []}'
```

### `config_setShowCalls`

[source](src/node/config.rs)

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

[source](src/node/config.rs)

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

[source](src/node/config.rs)

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

[source](src/node/config.rs)

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

[source](src/node/config.rs)

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

### `config_setLogLevel`

[source](src/node/config.rs)

Sets the logging level for the node and only displays the node logs.

#### Arguments

+ `level: LogLevel ('trace', 'debug', 'info', 'warn', 'error')`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_setLogLevel","params": ["trace"]}'
```

### `config_setLogging`

[source](src/node/config.rs)

Sets the fine-tuned logging levels for the node and any of its dependencies. 
The directive format is comma-separated `module=level` for any number of modules.

#### Arguments

+ `directive: String (module=level,other_module=level)`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "config_setLogging","params": ["era_test_node=trace,hyper=debug"]}'
```

## `DEBUG NAMESPACE`

### `debug_traceCall`

[source](src/node/debug.rs)

The `debug_traceCall` is similar to `eth_call` but returns call traces for each call.

Currently calls can only be traced on the latest block. This is the default and hence the block argument can be omitted.

The third argument mirrors the [`TraceConfig` of go-ethereum](https://geth.ethereum.org/docs/interacting-with-geth/rpc/ns-debug#traceconfig), but with the restriction that the only supported tracer is `CallTracer`. Memory, Stack and Storage traces are not supported.

#### Arguments

+ `transaction: Transaction`

+ `block: BlockNumber`

+ `tracer: TracerConfig`

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
      "method": "debug_traceCall",
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

### `debug_traceTransaction`

[source](src/node/debug.rs)

Returns call traces for the transaction with given hash.

Currently only transactions executed on the dev node itself (ie, not from upstream when using fork mode) can be traced.

The third argument mirrors the [`TraceConfig` of go-ethereum](https://geth.ethereum.org/docs/interacting-with-geth/rpc/ns-debug#traceconfig), but with the restriction that the only supported tracer is `CallTracer`. Memory, Stack and Storage traces are not supported.

#### Arguments

- `tx_hash: H256`

- `options: TracerConfig` (optional)

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
      "method": "debug_traceTransaction",
      "params": [
        "0xd3a94ff697a573cb174ecce05126e952ecea6dee051526a3e389747ff86b0d99",
        { "tracer": "callTracer", "tracerConfig": { "onlyTopCall": true } }
      ]
  }'
```

### `debug_traceBlockByHash`

[source](src/node/debug.rs)

Returns call traces for each transaction within a given block.

Currently only transactions from blocks mined on the dev node itself (ie, not from upstream when using fork mode) can be traced.

The third argument mirrors the [`TraceConfig` of go-ethereum](https://geth.ethereum.org/docs/interacting-with-geth/rpc/ns-debug#traceconfig), but with the restriction that the only supported tracer is `CallTracer`. Memory, Stack and Storage traces are not supported.

#### Arguments

- `blockHash: H256`

- `options: TracerConfig` (optional)

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
      "method": "debug_traceBlockByHash",
      "params": [
        "0xd3a94ff697a573cb174ecce05126e952ecea6dee051526a3e389747ff86b0d99",
        { "tracer": "callTracer", "tracerConfig": { "onlyTopCall": true } }
      ]
  }'
```

### `debug_traceBlockByNumber`

[source](src/node/debug.rs)

Returns call traces for each transaction within a given block.

Currently only transactions from blocks mined on the dev node itself (ie, not from upstream when using fork mode) can be traced.

The third argument mirrors the [`TraceConfig` of go-ethereum](https://geth.ethereum.org/docs/interacting-with-geth/rpc/ns-debug#traceconfig), but with the restriction that the only supported tracer is `CallTracer`. Memory, Stack and Storage traces are not supported.

#### Arguments

- `blockNumber: BlockNumber`

- `options: TracerConfig` (optional)

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
      "method": "debug_traceBlockByNumber",
      "params": [
        "latest",
        { "tracer": "callTracer", "tracerConfig": { "onlyTopCall": true } }
      ]
  }'
```

## `NETWORK NAMESPACE`

### `net_version`

[source](src/node/net.rs)

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

[source](src/node/net.rs)

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

[source](src/node/net.rs)

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

### `eth_accounts`

[source](src/node/eth.rs)

Returns a list of addresses owned by client

#### Arguments

+ _NONE_

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "eth_accounts","params": []}'
```

### `eth_chainId`

[source](src/node/eth.rs)

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

[source](src/node/eth.rs)

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

### `eth_feeHistory`

[source](src/node/eth.rs)

Returns the fee history for a given range of blocks

#### Arguments

+ `block_count: U64`
+ `newest_block: BlockNumber`
+ `reward_percentiles: Vec<f32>`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "eth_feeHistory","params": ["0x1", "latest", [25, 50 , 75]]}'
```

### `eth_gasPrice`

[source](src/node/eth.rs)

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

[source](src/node/eth.rs)

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

### `eth_getBlockByHash`

[source](src/node/eth.rs)

Returns information about a block by block hash

#### Arguments

+ `hash: H256`

+ `full: boolean`

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
    "method": "eth_getBlockByHash",
    "params": ["0x0000000000000000000000000000000000000000000000000000000000000008", false]
}'
```

### `eth_getBlockByNumber`

[source](src/node/eth.rs)

Returns information about a block by block number

#### Arguments

+ `block: BlockNumber`

+ `full: boolean`

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
    "method": "eth_getBlockByNumber",
    "params": ["latest", true]
}'
```

### `eth_getBlockTransactionCountByHash`

[source](src/node/eth.rs)

Number of transactions in a block from a block matching the given block hash

#### Arguments

+ `block_hash: H256`

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
    "method": "eth_getBlockTransactionCountByHash",
    "params": ["0x0000000000000000000000000000000000000000000000000000000000000008"]
}'
```

### `eth_getBlockTransactionCountByNumber`

[source](src/node/eth.rs)

Number of transactions in a block from a block matching the given block number

#### Arguments

+ `block_number: BlockNumber`

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
    "method": "eth_getBlockTransactionCountByNumber",
    "params": ["latest"]
}'
```


### `eth_getFilterChanges`

[source](src/node/eth.rs)

Polling method for a filter, which returns an array of logs, block hashes, or transaction hashes, depending on the filter type, which occurred since last poll

#### Arguments

+ `id: U256`

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
    "method": "eth_getFilterChanges",
    "params": ["0x1"]
}'
```

### `eth_newBlockFilter`

[source](src/node/eth.rs)

Creates a filter in the node, to notify when a new block arrives

#### Arguments

+ _NONE__

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
    "method": "eth_newBlockFilter"
}'
```

### `eth_newFilter`

[source](src/node/eth.rs)

Creates a filter object, based on filter options, to notify when the state changes (logs)

#### Arguments

+ `filter: Filter`

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
    "method": "eth_newFilter",
    "params": [{
      "fromBlock": "0xa", 
      "toBlock": "0xff", 
      "address": "0x6b175474e89094c44da98b954eedeac495271d0f",
      "topics": []
    }]
}'
```

### `eth_newPendingTransactionFilter`

[source](src/node/eth.rs)

Creates a filter in the node, to notify when new pending transactions arrive

#### Arguments

+ _NONE__

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
    "method": "eth_newPendingTransactionFilter"
}'
```

### `eth_uninstallFilter`

[source](src/node/eth.rs)

Uninstalls a filter with given id

#### Arguments

+ `id: U256`

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
    "method": "eth_uninstallFilter",
    "params": ["0x1"]
}'
```

### `eth_getFilterLogs`

[source](src/node/eth.rs)

Returns an array of all logs matching filter with given id

#### Arguments

+ `id: U256`

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
    "method": "eth_getFilterLogs",
    "params": ["0x1"]
}'
```

### `eth_getLogs`

[source](src/node/eth.rs)

Returns an array of all logs matching a filter

#### Arguments

+ `filter: Filter`

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
    "method": "eth_getLogs",
    "params": [{
      "fromBlock": "0xa",
      "toBlock": "0xff",
      "address": "0x6b175474e89094c44da98b954eedeac495271d0f",
      "topics": []
    }]
}'
```

### `eth_getCode`

[source](src/node/eth.rs)

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

[source](src/node/eth.rs)

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

[source](src/node/eth.rs)

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

[source](src/node/eth.rs)

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

[source](src/node/eth.rs)

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

[source](src/node/eth.rs)

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

[source](src/node/eth.rs)

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

[source](src/node/eth.rs)

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

### `eth_getStorageAt`

[source](src/node/eth.rs)

Returns the value from a storage position at a given address.

#### Arguments

+ `address: H160`
+ `position: U256`
+ `blockNumber: BlockIdVariant`

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
    "method": "eth_getStorageAt",
    "params": ["0x123456789abcdef123456789abcdef1234567890", "0x0", "latest"]
}'
```

### `eth_getTransactionByBlockHashAndIndex`

[source](src/node/eth.rs)

Returns information about a transaction by block hash and transaction index position

#### Arguments

+ `block_hash: H256`
+ `index: U64`

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
    "method": "eth_getTransactionByBlockHashAndIndex",
    "params": ["0x0000000000000000000000000000000000000000000000000000000000000008", "0x1"]
}'
```

### `eth_getTransactionByBlockNumberAndIndex`

[source](src/node/eth.rs)

Returns information about a transaction by block number and transaction index position

#### Arguments

+ `block_number: BlockNumber`
+ `index: U64`

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
    "method": "eth_getTransactionByBlockNumberAndIndex",
    "params": ["latest", "0x1"]
}'
```

### `eth_protocolVersion`

[source](src/node/eth.rs)

Returns the current ethereum protocol version.

#### Arguments

+ _NONE_

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
    "method": "eth_protocolVersion"
}'
```

### `eth_sendTransaction`

[source](src/node/eth.rs)

Creates new message call transaction or a contract creation, if the data field contains code.

#### Arguments

+ `transaction: TransactionRequest`

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
    "method": "eth_sendTransaction",
    "params": ["..."]
}'
```

## `HARDHAT NAMESPACE`

### `hardhat_setBalance`

[source](src/node/hardhat.rs)

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

[source](src/node/hardhat.rs)

Modifies an account's nonce by overwriting it.
The new nonce must be greater than the existing nonce.

#### Arguments

+ `address: Address` - The `Address` whose nonce is to be changed
+ `nonce: U256` - The new nonce

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

### `hardhat_mine`

[source](src/node/hardhat.rs)

Sometimes you may want to advance the latest block number of the network by a large number of blocks.
One way to do this would be to call the evm_mine RPC method multiple times, but this is too slow if you want to mine thousands of blocks.
The hardhat_mine method can mine any number of blocks at once, in constant time. (It exhibits the same performance no matter how many blocks are mined.)

#### Arguments

+ `num_blocks: U64` - The number of blocks to mine. (Optional: defaults to 1)
+ `interval: U646` - The interval between the timestamps of each block, in seconds. (Optional: defaults to 1)

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
    "id": "2",
    "method": "hardhat_mine",
    "params": [
        "0xaa",
        "0x100"
    ]
}'

```
### `hardhat_impersonateAccount`

[source](src/node/hardhat.rs)

Begin impersonating account- subsequent transactions sent to the node will be committed as if they were initiated by the supplied address.

#### Arguments

- `address: Address` - The address to begin impersonating

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
    "id": "2",
    "method": "hardhat_impersonateAccount",
    "params": [
        "0x364d6D0333432C3Ac016Ca832fb8594A8cE43Ca6"
    ]
}'
```

### `hardhat_stopImpersonatingAccount`

[source](src/node/hardhat.rs)

Stop impersonating account, should be used after calling `hardhat_impersonateAccount`.
Since we only impersonate one account at a time, the `address` argument is ignored and the current
impersonated account (if any) is cleared.

#### Arguments

- `address: Address` - (Optional) Argument accepted for compatibility and will be ignored

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{
    "jsonrpc": "2.0",
    "id": "2",
    "method": "hardhat_stopImpersonatingAccount",
    "params": [
        "0x364d6D0333432C3Ac016Ca832fb8594A8cE43Ca6"
    ]
}'
```

### `hardhat_setCode`

[source](src/node/hardhat.rs)

Sets the code for a given address.

#### Arguments

+ `address: Address` - The `Address` whose code will be updated
+ `code: Bytes` - The code to set to

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
      "method": "hardhat_setCode",
      "params": [
        "0x36615Cf349d7F6344891B1e7CA7C72883F5dc049",
        [1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1]
      ]
  }'
```

## `EVM NAMESPACE`

### `evm_mine`

[source](src/node/evm.rs)

Mines an empty block

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "evm_mine","params": []
}'
```

### `evm_increaseTime`

[source](src/node/evm.rs)

Increase the current timestamp for the node

#### Arguments

+ `time_delta_seconds: U64`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "evm_increaseTime","params": [10]}'
```

### `evm_setNextBlockTimestamp`

[source](src/node/evm.rs)

Sets the timestamp of the next block but doesn't mine one..

#### Arguments

+ `timestamp: U64`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "evm_setNextBlockTimestamp","params": [1672527600]}'
```

### `evm_setTime`

[source](src/node/evm.rs)

Set the current timestamp for the node. Warning: This will allow you to move _backwards_ in time, which 
may cause new blocks to appear to be mined before old blocks. This will result in an invalid state.

#### Arguments

+ `time: U64`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "evm_setTime","params": [1672527600]}'
```

### `evm_snapshot`

[source](src/node/evm.rs)

Snapshot the state of the blockchain at the current block.

#### Arguments

+ _NONE_

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "evm_snapshot"}'
```

### `evm_revert`

[source](src/node/evm.rs)

Revert the state of the blockchain to a previous snapshot

#### Arguments

+ `snapshot_id: U64`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "evm_revert","params": ["0x1"]}'
```

## `WEB3 NAMESPACE`

### `web3_clientVersion`

[source](src/node/web3.rs)

Returns the client version

#### Arguments

+ _NONE_

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
    "method": "web3_clientVersion"
  }'
```

## `ZKS NAMESPACE`

### `zks_estimateFee`

[source](src/node/zks.rs)

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

[source](src/node/zks.rs)

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

### `zks_getTransactionDetails`

[source](src/node/zks.rs)

Returns data from a specific transaction given by the transaction hash.

#### Arguments

+ `transactionHash: H256`

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0","id": "1","method": "zks_getTransactionDetails","params": ["0xa5d62a85561295ed58f8daad4e9442691e6da4301a859f364d28a02917d6e04d"]}'
```

### `zks_getBlockDetails`

[source](src/node/zks.rs)

Returns additional zkSync-specific information about the L2 block.

#### Arguments

+ `block: u32` - The number of the block

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0", "id": 1, "method": "zks_getBlockDetails", "params": [ 140599 ]}'
```

### `zks_getBridgeContracts`

[source](src/node/zks.rs)

Returns L1/L2 addresses of default bridges.

#### Arguments

+ _NONE_

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
      "method": "zks_getBridgeContracts"
  }'
```

### `zks_getRawBlockTransactions`

[source](src/node/zks.rs)

Returns data of transactions in a block.

#### Arguments

+ `block: u32` - The number of the block

#### Status

`SUPPORTED`

#### Example

```bash
curl --request POST \
  --url http://localhost:8011/ \
  --header 'content-type: application/json' \
  --data '{"jsonrpc": "2.0", "id": 1, "method": "zks_getRawBlockTransactions", "params": [ 140599 ]}'
```

### `zks_getConfirmedTokens`

[source](src/zks.rs)

Get list of the tokens supported by ZkSync Era. The tokens are returned in alphabetical order by their symbol. This means that the token id is its position in an alphabetically sorted array of tokens.

#### Arguments

+ `from: u32` - Offset of tokens
+ `limit: u8` - Limit of number of tokens to return

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
    "method": "zks_getConfirmedTokens",
    "params": [0, 100]
}'
```

### `zks_getAllAccountBalances`

[source](src/zks.rs)

Get all known balances for a given account.

#### Arguments

+ `address: Address` - The user address with balances to check.

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
    "method": "zks_getAllAccountBalances",
    "params": ["0x364d6D0333432C3Ac016Ca832fb8594A8cE43Ca6"]
}'
```
