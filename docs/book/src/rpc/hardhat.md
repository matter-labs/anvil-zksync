# `hardhat_*` namespace

Utility methods that mimic **Hardhat Network** behavior (impersonation, snapshot resets, on demand
mining, etc.).  
Perfect for Hardhat test suites, Foundry scripts, or any framework that expects the Hardhat JSON-RPC
surface.

```bash // [Example]
curl -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"hardhat_impersonateAccount","params":["0x…address…"]}'
```

## Method index

### Impersonation

| Method                                                                  | ✓ / ✗ | Purpose                        |
| ----------------------------------------------------------------------- | ----- | ------------------------------ |
| [`hardhat_impersonateAccount`](#hardhat_impersonateaccount)             | ✓     | Start impersonating an address |
| [`hardhat_stopImpersonatingAccount`](#hardhat_stopimpersonatingaccount) | ✓     | Stop impersonation             |

### Mining & automine

| Method                                        | ✓ / ✗ | Purpose                           |
| --------------------------------------------- | ----- | --------------------------------- |
| [`hardhat_getAutomine`](#hardhat_getautomine) | ▲     | Always returns `true` (auto‑mine) |
| [`hardhat_mine`](#hardhat_mine)               | ✓     | Mine _N_ blocks instantly         |

### Chain reset

| Method                            | ✓ / ✗ | Purpose                                    |
| --------------------------------- | ----- | ------------------------------------------ |
| [`hardhat_reset`](#hardhat_reset) | ▲     | Reset chain (blocknumber only when forked) |

### State modification

| Method                                          | ✓ / ✗ | Purpose              |
| ----------------------------------------------- | ----- | -------------------- |
| [`hardhat_setBalance`](#hardhat_setbalance)     | ✓     | Set account balance  |
| [`hardhat_setCode`](#hardhat_setcode)           | ✓     | Set account bytecode |
| [`hardhat_setNonce`](#hardhat_setnonce)         | ✓     | Set account nonce    |
| [`hardhat_setStorageAt`](#hardhat_setstorageat) | ✓     | Set storage slot     |

## Method reference

> Hardhat's full spec is in the [Hardhat Network docs ↗︎](https://hardhat.org/hardhat-network).

### hardhat_impersonateAccount <a id="hardhat_impersonateaccount" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"hardhat_impersonateAccount",
        "params":["0x…targetAddress…"]
      }'
```

### hardhat_stopImpersonatingAccount <a id="hardhat_stopimpersonatingaccount" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"hardhat_stopImpersonatingAccount",
        "params":["0x…targetAddress…"]
      }'
```

### hardhat_getAutomine ▲ <a id="hardhat_getautomine" />

Returns `true`.

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"hardhat_getAutomine","params":[]}'
```

### hardhat_mine <a id="hardhat_mine" />

Mine one block **or** `N` blocks instantly.

```bash
# mine 1 block
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"hardhat_mine","params":[]}'
```

```bash
# mine 5 blocks
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"hardhat_mine","params":["0x5"]}'
```

### hardhat_reset ▲ <a id="hardhat_reset" />

Resets the chain state.  
Blocknumber rewinds only work while running in **fork mode**.

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"hardhat_reset","params":[]}'
```

### hardhat_setBalance <a id="hardhat_setbalance" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"hardhat_setBalance",
        "params":["0x…targetAddress…", "0xDE0B6B3A7640000"]  // 1 ETH
      }'
```

### hardhat_setCode <a id="hardhat_setcode" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"hardhat_setCode",
        "params":["0x…targetAddress…", "0x60006000…"]  // EVM bytecode
      }'
```

### hardhat_setNonce <a id="hardhat_setnonce" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"hardhat_setNonce",
        "params":["0x…targetAddress…", "0xA"]  // nonce = 10
      }'
```

### hardhat_setStorageAt <a id="hardhat_setstorageat" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,
        "method":"hardhat_setStorageAt",
        "params":[
          "0x…targetAddress…",
          "0x0",                    // storage slot
          "0x0000000000000000000000000000000000000000000000000000000000000042"
        ]
      }'
```

## Unimplemented stubs

The following methods are not yet implemented and will return `Method not found`:

- `hardhat_addCompilationResult`
- `hardhat_dropTransaction`
- `hardhat_metadata`
- `hardhat_setCoinbase`
- `hardhat_setLoggingEnabled`
- `hardhat_setMinGasPrice`
- `hardhat_setNextBlockBaseFeePerGas`
- `hardhat_setPrevRandao`

## See also

- [`eth_*`](./eth.md) — Ethereum compatible base methods
- [`zks_*`](./zks.md) — ZKsync specific extensions
- [`anvil_*`](./anvil.md) — node management helpers
