# `anvil_*` namespace

Low level node controls that **anvil-zksync** exposes on top of the standard Ethereum RPC and zks
RPC. They let devs mine blocks on demand, timetravel, impersonate accounts, edit balances/code, and
tweak automine behaviour.

```bash // [Example]
curl -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"anvil_mine","params":[]}'
```

## Method index

### Mining & mempool

| Method                                                          | ✓ / ✗ | Purpose                   |
| --------------------------------------------------------------- | ----- | ------------------------- |
| [`anvil_mine`](#anvil_mine)                                     | ✓     | Mine _N_ blocks instantly |
| [`anvil_mine_detailed`](#anvil_mine_detailed)                   | ✓     | Mine & return extra data  |
| [`anvil_dropTransaction`](#anvil_droptransaction)               | ✓     | Remove tx by hash         |
| [`anvil_dropAllTransactions`](#anvil_dropalltransactions)       | ✓     | Clear mempool             |
| [`anvil_removePoolTransactions`](#anvil_removepooltransactions) | ✓     | Drop txs by sender        |

### Automine & intervals

| Method                                                                      | ✓ / ✗ | Purpose             |
| --------------------------------------------------------------------------- | ----- | ------------------- |
| [`anvil_getAutomine`](#anvil_getautomine)                                   | ✓     | Query automine      |
| [`anvil_setAutomine`](#anvil_setautomine)                                   | ✓     | Toggle automine     |
| [`anvil_setIntervalMining`](#anvil_setintervalmining)                       | ✓     | Mine every _N_ ms   |
| [`anvil_setNextBlockBaseFeePerGas`](#anvil_setnextblockbasefeepergas)       | ✓     | Next block base fee |
| [`anvil_setBlockTimestampInterval`](#anvil_setblocktimestampinterval)       | ✓     | Auto timestamp +Δ   |
| [`anvil_removeBlockTimestampInterval`](#anvil_removeblocktimestampinterval) | ✓     | Clear timestamp Δ   |

### State snapshots

| Method                              | ✓ / ✗ | Purpose                  |
| ----------------------------------- | ----- | ------------------------ |
| [`anvil_snapshot`](#anvil_snapshot) | ✓     | Take snapshot            |
| [`anvil_revert`](#anvil_revert)     | ✓     | Revert to snapshot       |
| [`anvil_reset`](#anvil_reset)       | ✓     | Reset chain - fork aware |

### Time travel

| Method                                                        | ✓ / ✗ | Purpose                 |
| ------------------------------------------------------------- | ----- | ----------------------- |
| [`anvil_setTime`](#anvil_settime)                             | ✓     | Set wall clock          |
| [`anvil_increaseTime`](#anvil_increasetime)                   | ✓     | Jump forward Δ sec      |
| [`anvil_setNextBlockTimestamp`](#anvil_setnextblocktimestamp) | ✓     | Timestamp of next block |

### Accounts & impersonation

| Method                                                              | ✓ / ✗ | Purpose                 |
| ------------------------------------------------------------------- | ----- | ----------------------- |
| [`anvil_impersonateAccount`](#anvil_impersonateaccount)             | ✓     | Start impersonation     |
| [`anvil_stopImpersonatingAccount`](#anvil_stopimpersonatingaccount) | ✓     | Stop impersonation      |
| [`anvil_autoImpersonateAccount`](#anvil_autoimpersonateaccount)     | ✓     | Toggle auto impersonate |
| [`anvil_setBalance`](#anvil_setbalance)                             | ✓     | Set balance             |
| [`anvil_setCode`](#anvil_setcode)                                   | ✓     | Set bytecode            |
| [`anvil_setStorageAt`](#anvil_setstorageat)                         | ✓     | Set storage slot        |
| [`anvil_setNonce`](#anvil_setnonce)                                 | ✓     | Set nonce               |

### Chain parameters & logging

| Method                                                | ✓ / ✗ | Purpose             |
| ----------------------------------------------------- | ----- | ------------------- |
| [`anvil_setChainId`](#anvil_setchainid)               | ✓     | Change `chainId`    |
| [`anvil_setRpcUrl`](#anvil_setrpcurl)                 | ✓     | Hot swap fork URL   |
| [`anvil_setLoggingEnabled`](#anvil_setloggingenabled) | ✓     | Toggle RPC logging  |
| `anvil_setMinGasPrice`                                | ✗     | (pre EIP-1559 only) |

## Method reference

> Full schema lives in the [Anvil docs ↗︎](https://book.getfoundry.sh/anvil/).

### anvil_mine <a id="anvil_mine" />

Mine one or more blocks instantly.

```bash
# mine 1 block
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"anvil_mine","params":[]}'
```

```bash
# mine 12 blocks
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"anvil_mine","params":["0xc"]}'
```

### anvil_mine_detailed <a id="anvil_mine_detailed" />

Same as `anvil_mine` but returns block hash, timestamp, gas used, etc.

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"anvil_mine_detailed","params":[]}'
```

### anvil_getAutomine <a id="anvil_getautomine" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"anvil_getAutomine","params":[]}'
```

### anvil_setAutomine <a id="anvil_setautomine" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"anvil_setAutomine","params":[false]}'
```

### anvil_snapshot <a id="anvil_snapshot" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"anvil_snapshot","params":[]}'
```

Stores the snapshot ID returned. Revert with `anvil_revert`.

### anvil_revert <a id="anvil_revert" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"anvil_revert","params":["0x1"]}'
```

> Snapshot IDs are hex strings (`"0x1"`, `"0x2"`…).

### anvil_impersonateAccount <a id="anvil_impersonateaccount" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"anvil_impersonateAccount","params":["0x…addr…"]}'
```

### anvil_setBalance <a id="anvil_setbalance" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,"method":"anvil_setBalance",
        "params":["0x…addr…", "0x8AC7230489E80000"]  // 10 ETH
      }'
```

### anvil_setRpcUrl <a id="anvil_setrpcurl" />

Hot swap the upstream fork URL (must be same chain).

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"anvil_setRpcUrl","params":["https://mainnet.era.zksync.io"]}'
```

## Unimplemented stubs

The following method is not yet implemented and will return `Method not found`:

- `anvil_setMinGasPrice`

## See also

- [`eth_*`](./eth.md) — Ethereum compatible base methods
- [`hardhat_*`](./hardhat.md) — Hardhat style helpers
- [`zks_*`](./zks.md) — ZKsync specific extensions
