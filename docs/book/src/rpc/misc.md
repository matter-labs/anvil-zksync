# `evm_*`, `net_*`, `web3_*`, and `debug_*` namespaces

These helper methods cover developer tooling (`evm_*`), network status (`net_*`), client metadata
(`web3_*`), and execution tracing (`debug_*`).

Use them alongside the core [`eth_*`](./eth.md) and [`zks_*`](./zks.md) calls for full
functionality.

```bash // [Example: trace a contract call]
curl -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"debug_traceCall","params":[{ "to":"0x…", "data":"0x…"}, "latest", {}]}'
```

## Method index

### `evm_*` — Developer utility calls

| Method                                                    | ✓/✗ | Purpose                    |
| --------------------------------------------------------- | --- | -------------------------- |
| [`evm_snapshot`](#evm_snapshot)                           | ✓   | Take blockchain snapshot   |
| [`evm_revert`](#evm_revert)                               | ✓   | Revert to a snapshot       |
| [`evm_increaseTime`](#evm_increasetime)                   | ✓   | Jump forward in time       |
| [`evm_mine`](#evm_mine)                                   | ✓   | Mine a single block        |
| [`evm_setNextBlockTimestamp`](#evm_setnextblocktimestamp) | ✓   | Set next block's timestamp |
| [`evm_setTime`](#evm_settime)                             | ✓   | Override internal clock    |
| [`evm_setAccountNonce`](#evm_setaccountnonce)             | ✓   | Set account nonce          |

### `net_*` — Network diagnostics

| Method                            | ✓/✗ | Purpose                         |
| --------------------------------- | --- | ------------------------------- |
| [`net_version`](#net_version)     | ✓   | Returns network ID (`260`)      |
| [`net_peerCount`](#net_peercount) | ✓   | Number of connected peers (`0`) |
| [`net_listening`](#net_listening) | ✓   | Is P2P listening? (`false`)     |

### `web3_*` — Client metadata

| Method                                      | ✓/✗ | Purpose               |
| ------------------------------------------- | --- | --------------------- |
| [`web3_clientVersion`](#web3_clientversion) | ✓   | Returns `zkSync/v2.0` |

### `debug_*` — Execution tracing

| Method                                                  | ✓/✗ | Purpose                            |
| ------------------------------------------------------- | --- | ---------------------------------- |
| [`debug_traceCall`](#debug_tracecall)                   | ✓   | Trace a single call                |
| [`debug_traceBlockByHash`](#debug_traceblockbyhash)     | ✓   | Trace all ops in a block by hash   |
| [`debug_traceBlockByNumber`](#debug_traceblockbynumber) | ✓   | Trace all ops in a block by number |
| [`debug_traceTransaction`](#debug_tracetransaction)     | ✓   | Trace a single transaction by hash |

## Method reference

### evm_snapshot <a id="evm_snapshot" />

```bash filename="evm_snapshot.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"evm_snapshot","params":[]}'
```

### evm_revert <a id="evm_revert" />

```bash filename="evm_revert.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"evm_revert","params":["0x1"]}'
```

> Use the snapshot ID returned by `evm_snapshot`.

### evm_increaseTime <a id="evm_increasetime" />

```bash filename="evm_increaseTime.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"evm_increaseTime","params":[3600]}'
```

### evm_mine <a id="evm_mine" />

```bash filename="evm_mine.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"evm_mine","params":[]}'
```

### evm_setNextBlockTimestamp <a id="evm_setnextblocktimestamp" />

```bash filename="evm_setNextBlockTimestamp.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"evm_setNextBlockTimestamp","params":[1700000000]}'
```

### evm_setTime <a id="evm_settime" />

```bash filename="evm_setTime.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"evm_setTime","params":[1700000000]}'
```

### evm_setAccountNonce <a id="evm_setaccountnonce" />

```bash filename="evm_setAccountNonce.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"evm_setAccountNonce","params":["0x…addr…", "0xA"]}'
```

### net_version <a id="net_version" />

```bash filename="net_version.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"net_version","params":[]}'
```

### net_peerCount <a id="net_peercount" />

```bash filename="net_peerCount.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"net_peerCount","params":[]}'
```

### net_listening <a id="net_listening" />

```bash filename="net_listening.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"net_listening","params":[]}'
```

### web3_clientVersion <a id="web3_clientversion" />

```bash filename="web3_clientVersion.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"web3_clientVersion","params":[]}'
```

### debug_traceCall <a id="debug_tracecall" />

```bash filename="debug_traceCall.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,"method":"debug_traceCall",
        "params":[{ "to":"0x…","data":"0x…"}, "latest", {}]
      }'
```

<Callout title="Note">
  `debug_traceCall` returns full VM execution traces. Responses can be large.
</Callout>

### debug_traceBlockByHash <a id="debug_traceblockbyhash" />

```bash filename="debug_traceBlockByHash.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,"method":"debug_traceBlockByHash",
        "params":["0x…blockHash…", {}]
      }'
```

### debug_traceBlockByNumber <a id="debug_traceblockbynumber" />

```bash filename="debug_traceBlockByNumber.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"debug_traceBlockByNumber","params":["latest", {}]}'
```

### debug_traceTransaction <a id="debug_tracetransaction" />

```bash filename="debug_traceTransaction.sh" // [!code hl]
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"debug_traceTransaction","params":["0x…txHash…", {}]}'
```

## Unimplemented stubs

- `evm_addAccount`
- `evm_removeAccount`
- `evm_setAccountBalance`
- `evm_setAccountCode`
- `evm_setAccountStorageAt`
- `evm_setAutomine`
- `evm_setBlockGasLimit`
- `evm_setIntervalMining`

## See also

- [`eth_*`](./eth.md) — Ethereum compatible base methods
- [`zks_*`](./zks.md) — ZKsync specific extensions
- [`hardhat_*`](./hardhat.md) — Hardhat style helpers
