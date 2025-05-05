# `eth_*` namespace

Standard Ethereum JSON-RPC calls supported by **anvil-zksync**.  
Unless marked ✗, behavior matches the same call on an actual ZK chain endpoint.

```bash // [Example]
curl -X POST http://localhost:8011 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}'
```

## Method index

### Accounts & keys

| Method                                                | ✓ / ✗ | Purpose           |
| ----------------------------------------------------- | ----- | ----------------- |
| [`eth_accounts`](#eth_accounts)                       | ✓     | List dev accounts |
| `eth_coinbase`                                        | ✗     | Coinbase address  |
| [`eth_getTransactionCount`](#eth_gettransactioncount) | ✓     | Nonce for address |

### Blocks & chain

| Method                                          | ✓ / ✗ | Purpose                                  |
| ----------------------------------------------- | ----- | ---------------------------------------- |
| [`eth_blockNumber`](#eth_blocknumber)           | ✓     | Latest block height                      |
| [`eth_chainId`](#eth_chainid)                   | ✓     | Configured chain ID&nbsp;(`260` default) |
| [`eth_getBlockByNumber`](#eth_getblockbynumber) | ✓     | Block by number                          |
| [`eth_getBlockByHash`](#eth_getblockbyhash)     | ✓     | Block by hash                            |
| `eth_getUncleByBlockNumberAndIndex`             | ✗     | Uncle data                               |
| `eth_getUncleCountByBlockHash`                  | ✗     | Uncle count                              |

### Transactions

| Method                                                    | ✓ / ✗ | Purpose                               |
| --------------------------------------------------------- | ----- | ------------------------------------- |
| [`eth_sendRawTransaction`](#eth_sendrawtransaction)       | ✓     | Broadcast signed tx                   |
| [`eth_sendTransaction`](#eth_sendtransaction)             | ✓     | Broadcast _unsigned_ tx (dev wallets) |
| [`eth_getTransactionByHash`](#eth_gettransactionbyhash)   | ✓     | Tx by hash                            |
| [`eth_getTransactionReceipt`](#eth_gettransactionreceipt) | ✓     | Tx receipt                            |
| [`eth_estimateGas`](#eth_estimategas)                     | ✓     | Gas estimate                          |
| [`eth_call`](#eth_call)                                   | ✓     | Stateless call                        |
| `eth_sign`                                                | ✗     | Sign message                          |
| `eth_signTypedData`                                       | ✗     | Sign typed data                       |

### Logs & filters

| Method                                          | ✓ / ✗ | Purpose                     |
| ----------------------------------------------- | ----- | --------------------------- |
| [`eth_getLogs`](#eth_getlogs)                   | ✓     | Logs by filter object       |
| [`eth_newFilter`](#eth_newfilter)               | ✓     | Create log filter           |
| [`eth_getFilterChanges`](#eth_getfilterchanges) | ✓     | Poll filter                 |
| [`eth_uninstallFilter`](#eth_uninstallfilter)   | ✓     | Remove filter               |
| `eth_subscribe`                                 | ✗     | Open websocket subscription |

### Gas & fees

| Method                              | ✓ / ✗ | Purpose                                    |
| ----------------------------------- | ----- | ------------------------------------------ |
| [`eth_gasPrice`](#eth_gasprice)     | ✓     | Current gas price (hardcoded `50_000_000`) |
| [`eth_feeHistory`](#eth_feehistory) | ✓     | Historical fee data (stubbed)              |
| `eth_maxPriorityFeePerGas`          | ✗     | EIP-1559 priority fee                      |

### Misc & sync

| Method                                        | ✓ / ✗ | Purpose                       |
| --------------------------------------------- | ----- | ----------------------------- |
| [`eth_syncing`](#eth_syncing)                 | ✓     | Always `false` (instant sync) |
| [`eth_protocolVersion`](#eth_protocolversion) | ✓     | Protocol version              |
| `eth_hashrate`                                | ✗     | Miner hashrate                |

## Method reference

> We keep this section lightweight — for full parameter and return definitions see the official
> Ethereum JSON-RPC [spec ↗︎](https://ethereum.org/en/developers/docs/apis/json-rpc/).

### eth_accounts <a id="eth_accounts" />

Returns a list of addresses owned by the local node (e.g. dev accounts).

```bash filename="eth_accounts.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"eth_accounts","params":[]}'
```

### eth_getTransactionCount <a id="eth_gettransactioncount" />

Returns the nonce (transaction count) for an address.

```bash filename="eth_getTransactionCount.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_getTransactionCount",
        "params":["0x6fC1E2F6c7381BF9b7205F3a14e0ccabe9d9a8F8", "latest"]
      }'
```

> Replace the address with one from `eth_accounts` if you're running dev mode.

### eth_blockNumber <a id="eth_blocknumber" />

Returns the latest block height.

```bash filename="eth_blockNumber.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}'
```

### eth_chainId <a id="eth_chainid" />

Returns the chain ID of the node (default: `260` / `0x104`).

```bash filename="eth_chainId.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}'
```

### eth_getBlockByNumber <a id="eth_getblockbynumber" />

Returns block details by block number.

```bash filename="eth_getBlockByNumber.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_getBlockByNumber",
        "params":["latest", true]
      }'
```

> `true` returns full transaction objects; `false` returns only tx hashes.

### eth_getBlockByHash <a id="eth_getblockbyhash" />

Returns block details by hash.

```bash filename="eth_getBlockByHash.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_getBlockByHash",
        "params":["0xabc123...blockHash...", true]
      }'
```

> Replace the hash with a real block hash from your node (e.g. via `eth_getLogs` or block explorer).

### eth_sendRawTransaction <a id="eth_sendrawtransaction" />

Broadcasts a raw signed transaction.  
Use this with pre-signed transactions (e.g. from Foundry or viem).

```bash filename="eth_sendRawTransaction.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_sendRawTransaction",
        "params":["0x02f8..."]  // replace with actual signed tx
      }'
```

<Callout title="Auto-mining">
  Unless the node is started with <code>--no-mining</code>, the transaction is mined immediately.
</Callout>

### eth_sendTransaction <a id="eth_sendtransaction" />

Broadcasts an **unsigned** transaction using dev accounts (in local mode only).  
Automatically signs and sends the tx.

```bash filename="eth_sendTransaction.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_sendTransaction",
        "params":[{
          "from": "0x6fC1E2F6c7381BF9b7205F3a14e0ccabe9d9a8F8",
          "to": "0x0000000000000000000000000000000000000000",
          "value": "0x2386F26FC10000"  // 0.01 ETH
        }]
      }'
```

### eth_getTransactionByHash <a id="eth_gettransactionbyhash" />

Returns a transaction by its hash.

```bash filename="eth_getTransactionByHash.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_getTransactionByHash",
        "params":["0x...transactionHash..."]
      }'
```

### eth_getTransactionReceipt <a id="eth_gettransactionreceipt" />

Returns the receipt for a transaction hash (after it has been mined).

```bash filename="eth_getTransactionReceipt.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_getTransactionReceipt",
        "params":["0x...transactionHash..."]
      }'
```

### eth_estimateGas <a id="eth_estimategas" />

Estimates how much gas a transaction will consume.

```bash filename="eth_estimateGas.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_estimateGas",
        "params":[{
          "from": "0x6fC1E2F6c7381BF9b7205F3a14e0ccabe9d9a8F8",
          "to": "0x0000000000000000000000000000000000000000",
          "data": "0x"
        }]
      }'
```

### eth_call <a id="eth_call" />

Simulates a read-only call to a contract (does not submit a tx).

```bash filename="eth_call.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_call",
        "params":[{
          "from": "0x6fC1E2F6c7381BF9b7205F3a14e0ccabe9d9a8F8",
          "to": "0x0000000000000000000000000000000000000000",
          "data": "0x..."
        }, "latest"]
      }'
```

> Replace `data` with the ABI-encoded call data for the method you wish to simulate.

### eth_getLogs <a id="eth_getlogs" />

Returns logs matching the specified filter object.

```bash filename="eth_getLogs.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_getLogs",
        "params":[{
          "fromBlock": "0x0",
          "toBlock": "latest",
          "address": "0x0000000000000000000000000000000000000000",
          "topics": []
        }]
      }'
```

### eth_newFilter <a id="eth_newfilter" />

Creates a new log filter on the node and returns its ID.

```bash filename="eth_newFilter.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_newFilter",
        "params":[{
          "fromBlock": "0x0",
          "toBlock": "latest",
          "address": "0x0000000000000000000000000000000000000000",
          "topics": []
        }]
      }'
```

### eth_getFilterChanges <a id="eth_getfilterchanges" />

Polls the filter for new changes (e.g. logs or block hashes since last call).

```bash filename="eth_getFilterChanges.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_getFilterChanges",
        "params":["0x1"]
      }'
```

> Replace `0x1` with the filter ID returned by `eth_newFilter`.

### eth_uninstallFilter <a id="eth_uninstallfilter" />

Removes a previously created filter.

```bash filename="eth_uninstallFilter.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_uninstallFilter",
        "params":["0x1"]
      }'
```

> After removal, the filter ID becomes invalid and cannot be polled.

### eth_gasPrice <a id="eth_gasprice" />

Returns the current gas price.  
In `anvil-zksync`, this is hardcoded to `50_000_000` gwei (`0x2FAF080`).

```bash filename="eth_gasPrice.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"eth_gasPrice","params":[]}'
```

### eth_feeHistory <a id="eth_feehistory" />

Returns a stubbed gas fee history.  
Parameters include block count, reference block, and optional reward percentiles.

```bash filename="eth_feeHistory.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"eth_feeHistory",
        "params":["0x5", "latest", [25, 50, 75]]
      }'
```

### eth_syncing <a id="eth_syncing" />

Returns `false` — the node is always considered fully synced.

```bash filename="eth_syncing.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"eth_syncing","params":[]}'
```

### eth_protocolVersion <a id="eth_protocolversion" />

Returns the Ethereum protocol version as a hex string.

```bash filename="eth_protocolVersion.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"eth_protocolVersion","params":[]}'
```

## Unimplemented stubs

The following methods currently return **`Method not found`**:

- `eth_getCompilers`
- `eth_sign`
- `eth_subscribe`
- `eth_hashrate`
- `eth_maxPriorityFeePerGas`
- `eth_coinbase`
- `eth_signTypedData`
- `eth_getUncleByBlockNumberAndIndex`
- `eth_getUncleCountByBlockHash`
- `eth_getUncleByBlockHashAndIndex`
- `eth_getUncleCountByBlockNumber`
- `eth_getUncleByBlockNumberAndIndex`

---

## See also

- [`zks_*`](./zks.md) — ZKsync specific extensions
- [`debug_*`](./misc.md) — Execution traces & state inspection
- [`anvil_*`](./anvil.md) — Node testing helpers
