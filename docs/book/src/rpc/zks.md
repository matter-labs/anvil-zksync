# `zks_*` namespace

These calls expose Elastic Network specific data such as L1→L2 fees, bridge contract addresses, and
batch details.

Unless marked ✗, they behave the same as on a public ZKsync endpoint.

```bash // [Example]
curl -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"zks_L1ChainId","params":[]}'
```

## Method index

### Fees & gas

| Method                                            | ✓ / ✗ | Purpose                     |
| ------------------------------------------------- | ----- | --------------------------- |
| [`zks_estimateFee`](#zks_estimatefee)             | ✓     | Fee estimate for an L2 tx   |
| [`zks_estimateGasL1ToL2`](#zks_estimategasl1tol2) | ✓     | Gas estimate for L1→L2 call |
| `zks_getFeeParams`                                | ✗     | Current fee params          |

### Blocks & batches

| Method                                                        | ✓ / ✗ | Purpose                 |
| ------------------------------------------------------------- | ----- | ----------------------- |
| [`zks_getBlockDetails`](#zks_getblockdetails)                 | ✓     | Extra zkSync block info |
| [`zks_getRawBlockTransactions`](#zks_getrawblocktransactions) | ✓     | Raw txs in a block      |
| `zks_getL1BatchBlockRange`                                    | ✗     | Block range in batch    |
| `zks_getL1BatchDetails`                                       | ✗     | Batch details           |
| `zks_L1BatchNumber`                                           | ✗     | Latest L1 batch number  |

### Proofs

| Method                                           | ✓ / ✗ | Purpose                    |
| ------------------------------------------------ | ----- | -------------------------- |
| [`zks_getL2ToL1LogProof`](#zks_getl2to1logproof) | ✓     | Proof for L2→L1 log        |
| `zks_getL2ToL1MsgProof`                          | ✗     | Proof for L1 Messenger msg |
| `zks_getProof`                                   | ✗     | Storage Merkle proof       |

### Accounts & tokens

| Method                                                    | ✓ / ✗ | Purpose            |
| --------------------------------------------------------- | ----- | ------------------ |
| [`zks_getAllAccountBalances`](#zks_getallaccountbalances) | ✓     | All token balances |
| [`zks_getConfirmedTokens`](#zks_getconfirmedtokens)       | ✓     | Token list slice   |
| [`zks_getBaseTokenL1Address`](#zks_getbasetokenl1address) | ✓     | Base‑token L1 addr |

### Bridges & contracts

| Method                                                  | ✓ / ✗ | Purpose                  |
| ------------------------------------------------------- | ----- | ------------------------ |
| [`zks_getBridgeContracts`](#zks_getbridgecontracts)     | ✓     | Default bridge addrs     |
| [`zks_getBridgehubContract`](#zks_getbridgehubcontract) | ✓     | Bridgehub addr           |
| `zks_getMainContract`                                   | ✗     | zkSync Era main contract |
| `zks_getTimestampAsserter`                              | ✗     | Timestamp asserter       |
| `zks_getL2Multicall3`                                   | ✗     | Multicall3 addr          |

### Misc & system

| Method                                     | ✓ / ✗ | Purpose                 |
| ------------------------------------------ | ----- | ----------------------- |
| [`zks_L1ChainId`](#zks_l1chainid)          | ✓     | Underlying L1 chain‑id  |
| `zks_getBatchFeeInput`                     | ✗     | Current batch fee input |
| `zks_getL1GasPrice`                        | ✗     | Current L1 gas price    |
| `zks_sendRawTransactionWithDetailedOutput` | ✗     | Tx with trace output    |

## Method reference

> Full schemas live in the [official zkSync JSON-RPC docs ↗︎](https://era.zksync.io/docs/api/).

### zks_estimateFee <a id="zks_estimatefee" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,"method":"zks_estimateFee",
        "params":[{
          "from":"0x…","to":"0x…","data":"0x…"
        }]
      }'
```

### zks_estimateGasL1ToL2 <a id="zks_estimategasl1tol2" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,"method":"zks_estimateGasL1ToL2",
        "params":[{
          "from":"0x…","to":"0x…","gasPerPubdata":"0x0"
        }]
      }'
```

### zks_getAllAccountBalances <a id="zks_getallaccountbalances" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,"method":"zks_getAllAccountBalances",
        "params":["0x…account…"]
      }'
```

### zks_getBridgeContracts <a id="zks_getbridgecontracts" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"zks_getBridgeContracts","params":[]}'
```

### zks_getBridgehubContract <a id="zks_getbridgehubcontract" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"zks_getBridgehubContract","params":[]}'
```

### zks_getBlockDetails <a id="zks_getblockdetails" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"zks_getBlockDetails","params":["0x1a"]}'
```

### zks_getBytecodeByHash <a id="zks_getbytecodebyhash" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"zks_getBytecodeByHash","params":["0x…hash…"]}'
```

### zks_getConfirmedTokens <a id="zks_getconfirmedtokens" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"zks_getConfirmedTokens","params":[0, 50]}'
```

### zks_getBaseTokenL1Address <a id="zks_getbasetokenl1address" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"zks_getBaseTokenL1Address","params":[]}'
```

### zks_getL2ToL1LogProof <a id="zks_getl2to1logproof" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{
        "jsonrpc":"2.0","id":1,"method":"zks_getL2ToL1LogProof",
        "params":["0x…txHash…", 0]
      }'
```

### zks_getRawBlockTransactions <a id="zks_getrawblocktransactions" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"zks_getRawBlockTransactions","params":["0x1a"]}'
```

### zks_getTransactionDetails <a id="zks_gettransactiondetails" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"zks_getTransactionDetails","params":["0x…txHash…"]}'
```

### zks_L1ChainId <a id="zks_l1chainid" />

```bash
curl -s -X POST http://localhost:8011 \
  -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"zks_L1ChainId","params":[]}'
```

## Unimplemented stubs

The following methods are not yet implemented and will return `Method not found`:

- `zks_getBatchFeeInput`
- `zks_getFeeParams`
- `zks_getL1BatchBlockRange`
- `zks_getL1BatchDetails`
- `zks_getL1GasPrice`
- `zks_getL2ToL1MsgProof`
- `zks_getMainContract`
- `zks_getProof`
- `zks_getProtocolVersion`
- `zks_getTestnetPaymaster`
- `zks_sendRawTransactionWithDetailedOutput`
- `zks_getTimestampAsserter`
- `zks_getL2Multicall3`

## See also

- [`eth_*`](./eth.md) — Ethereum compatible base methods
- [`debug_*`](./misc.md) — Execution traces & state inspection
- [`anvil_*`](./anvil.md) — Node testing helpers
