# 🔧 Supported APIs for In-Memory Node 🔧

> ⚠️ **WORK IN PROGRESS**: This list is non-comprehensive and being updated

## Supported APIs Table

| Category | API | Status | Description |
| --- | --- | --- | --- |
| [`CONFIG NAMESPACE`](#config-namespace) | [`config_getShowCalls`](#config_getShowCalls) | SUPPORTED | Gets the current value of `show_calls` that's originally set with `--show-calls` option |
| [`CONFIG NAMESPACE`](#config-namespace) | [`config_setShowCalls`](#config_setShowCalls) | SUPPORTED | Updates `show_calls` to print more detailed call traces |
| [`CONFIG NAMESPACE`](#config-namespace) | [`config_setResolveHashes`](#config_setResolveHashes) | SUPPORTED | Updates `resolve-hashes` to call OpenChain for human-readable ABI names in call traces |
| [`NETWORK METHODS`](#network-methods) | [`net_getVersion`](#net-getVersion) | NOT IMPLEMENTED | Returns the current network id |
| [`NETWORK METHODS`](#network-methods) | [`net_getMetadata`](#net-getMetadata) | NOT IMPLEMENTED | Returns the current network metadata |
| [`NETWORK METHODS`](#network-methods) | [`net_fastForward`](#net-fastForward) | NOT IMPLEMENTED | Fast forwards the clock by a given number of seconds (also known as `mine`) |
| [`NETWORK METHODS`](#network-methods) | [`net_resetFork`](#net-resetFork) | NOT IMPLEMENTED | Resets the fork choice to a given block |
| [`NETWORK METHODS`](#network-methods) | [`net_setBalance`](#net-setBalance) | NOT IMPLEMENTED | Sets the balance of a given account |
| [`NETWORK METHODS`](#network-methods) | [`net_setCode`](#net-setCode) | NOT IMPLEMENTED | Sets the bytecode of a given account |
| [`NETWORK METHODS`](#network-methods) | [`net_setMinGasPrice`](#net-setMinGasPrice) | NOT IMPLEMENTED | Sets the minimum gas price |
| [`NETWORK METHODS`](#network-methods) | [`net_setNonce`](#net-setNonce) | NOT IMPLEMENTED | Sets the nonce of a given account |
| [`NETWORK METHODS`](#network-methods) | [`net_setNextBlockBaseFeePerGas`](#net-setNextBlockBaseFeePerGas) | NOT IMPLEMENTED | Sets the base fee per gas for the next block |
| [`NETWORK METHODS`](#network-methods) | [`net_setStorageAt`](#net-setStorageAt) | NOT IMPLEMENTED | Sets the storage value at a given key for a given account |
| [`NETWORK METHODS`](#network-methods) | [`net_snapshot`](#net-snapshot) | NOT IMPLEMENTED | Takes a snapshot of the current state and returns its id |
| [`NETWORK METHODS`](#network-methods) | [`net_getBlockByHash`](#net-getBlockByHash) | NOT IMPLEMENTED | Returns the block with the given hash |
| [`NETWORK METHODS`](#network-methods) | [`net_getBlockByNumber`](#net-getBlockByNumber) | NOT IMPLEMENTED | Returns the block with the given number |
| [`NETWORK METHODS`](#network-methods) | [`net_getBlockTransactionCountByHash`](#net-getBlockTransactionCountByHash) | NOT IMPLEMENTED | Returns the number of transactions in the block with the given hash |
| [`ACCOUNT METHODS`](#account-methods) | [`account_getCode`](#account-getCode) | NOT IMPLEMENTED | Returns the bytecode of a given account |
| [`ACCOUNT METHODS`](#account-methods) | [`account_getNonce`](#account-getNonce) | NOT IMPLEMENTED | Returns the nonce of a given account |
| [`ACCOUNT METHODS`](#account-methods) | [`account_getStorageAt`](#account-getStorageAt) | NOT IMPLEMENTED | Returns the storage value at a given key for a given account |
| [`ACCOUNT METHODS`](#account-methods) | [`account_getBalance`](#account-getBalance) | NOT IMPLEMENTED | Returns the balance of a given account |
| [`ACCOUNT METHODS`](#account-methods) | [`account_getAccounts`](#account-getAccounts) | NOT IMPLEMENTED | Returns the list of accounts |
| [`ACCOUNT METHODS`](#account-methods) | [`account_createAccount`](#account-createAccount) | NOT IMPLEMENTED | Creates a new account |
| [`ACCOUNT METHODS`](#account-methods) | [`account_importAccount`](#account-importAccount) | NOT IMPLEMENTED | Imports an account |
| [`ACCOUNT METHODS`](#account-methods) | [`account_exportAccount`](#account-exportAccount) | NOT IMPLEMENTED | Exports an account |
| [`ACCOUNT METHODS`](#account-methods) | [`account_sendTransaction`](#account-sendTransaction) | NOT IMPLEMENTED | Sends a transaction |
| [`ACCOUNT METHODS`](#account-methods) | [`account_signTransaction`](#account-signTransaction) | NOT IMPLEMENTED | Signs a transaction |

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
