# `run`

Launch an Elastic Network ZK chain for integration tests, debugging, or rapid prototyping.

## Synopsis

```bash
anvil-zksync [OPTIONS]
```

## Key options

| Flag                 | Description                                       | Default         |
| -------------------- | ------------------------------------------------- | --------------- |
| `--no-mining`        | Mine blocks only when RPC clients call `evm_mine` | _auto-mine_     |
| `--block-time <sec>` | Seal blocks at a fixed interval                   | _instant on tx_ |
| `--state <PATH>`     | Load + dump snapshot on exit                      | -               |
| `--timestamp <NUM>`  | Override genesis timestamp                        | _now_           |
| `-a, --accounts <N>` | Pre-funded dev accounts                           | `10`            |
| `--balance <ETH>`    | Balance per dev account                           | `10000`         |
| `-v, --verbosity…`   | Show increasingly detailed traces                 | -               |

All **global flags** (network, gas, logging, cache, etc.) are also accepted.

## Experimental L1 mode 🚧

Enable a companion **Layer 1 Anvil** while running your L2 node.  
Useful for testing bridge flows and deposit/withdraw paths.

| Flag                         | Description                                                  | Default |
| ---------------------------- | ------------------------------------------------------------ | ------- |
| `--spawn-l1[=<port>]`        | Start an L1 Anvil instance on the given port                 | `8012`  |
| `--external-l1 <URL>`        | Use an **external** L1 JSON-RPC endpoint instead of spawning | -       |
| `--auto-execute-l1[=<bool>]` | Auto-execute L1 batches after L2 sealing                     | `false` |

> ⚠️ _L1 support is marked **UNSTABLE**; interfaces and behavior may change between releases._

### L1 quick start

```bash
anvil-zksync --spawn-l1 --auto-execute-l1 -vv
```

### Connect to an existing L1 node

```bash
anvil-zksync --external-l1 http://127.0.0.1:8545
```

When running the L1 node (anvil) alongside `anvil-zksync`,
make sure to start it with the `--no-request-size-limit` flag to avoid data payload issues:

```bash
anvil --no-request-size-limit
```

## Behavior

- Starts with a **clean L2 state** (no contracts, no history).
- Auto-mines each submitted transaction unless you pass `--no-mining` or set a fixed `--block-time`.
- Generates deterministic dev accounts unless you provide a custom mnemonic.
- When `--spawn-l1` or `--external-l1` is enabled, cross-chain calls target that L1 endpoint and
  deposit logs are emitted as usual.

## Examples

### 1. Quick start with verbose traces

```bash
anvil-zksync -vv
```

### 2. Manual mining workflow

```bash
anvil-zksync --no-mining
# elsewhere:
cast rpc evm_mine
```

### 3. Pre-load a snapshot & dump on exit

```bash
anvil-zksync --state ./snapshot.json
```

## See also

- [`fork`](./fork.md) — fork an existing network
- [`replay_tx`](./replay_tx.md) — fork & replay a transaction
- [CLI overview](./index.md) — all global flags and usage
