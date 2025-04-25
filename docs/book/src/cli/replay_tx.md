# `replay_tx`

Spin up an **anvil-zksync** node that forks a remote network and immediately re-executes a chosen L2
transaction.

Perfect for step-through debugging or gas cost inspection of real world transactions and contract
interactions.

## Synopsis

```bash // [replay_tx]
anvil-zksync replay_tx --fork-url <FORK_URL> <TX>
```

Both parameters are **required**:

- `--fork-url <FORK_URL>` - remote endpoint or chain alias
- `<TX>` - L2 **transaction hash** to replay

### Named chain aliases

| Alias              | RPC endpoint                      |
| ------------------ | --------------------------------- |
| `mainnet`          | `https://mainnet.era.zksync.io`   |
| `sepolia-testnet`  | `https://sepolia.era.zksync.dev`  |
| `abstract`         | `https://api.mainnet.abs.xyz/`    |
| `abstract-testnet` | `https://api.testnet.abs.xyz`     |
| `sophon`           | `https://rpc.sophon.xyz/`         |
| `sophon-testnet`   | `https://rpc.testnet.sophon.xyz/` |

## Arguments

| Name   | Description                                       |
| ------ | ------------------------------------------------- |
| `<TX>` | Transaction hash (`0x…`, 32 bytes). **Required.** |

## Options

| Flag                    | Description                                             |
| ----------------------- | ------------------------------------------------------- |
| `--fork-url <FORK_URL>` | Network to fork from (endpoint or alias). **Required.** |

All **global flags** (verbosity, cache, gas tuning, etc.) are also available.

## Behavior

1. Downloads block state up to (but not including) `<TX>`.
2. Replays the transaction **locally**, reproducing calldata & timestamp.
3. Provides full VM traces, logs, and storage-diff when `-vv` or higher is enabled.

## Examples

### 1. Replay a swap transaction from Era mainnet

```bash
anvil-zksync replay_tx \
  --fork-url mainnet \
  0xe56fd585309971c7c68b19c6c75a39c4b450731f9884c7b73e13276bb6db9b5b
```

### 2. Replay a tx from a local node

```bash
anvil-zksync replay_tx \
  --fork-url http://127.0.0.1:3050 \
  0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef
```

### 3. Replay a failed transaction with user VM tracing

```bash
anvil-zksync -vv replay_tx \
  --fork-url mainnet \
  0x9419f4eb8d553dd4f4d254badfd28efb0b2416c4b0eb26076f90e1226501a3e0
```

## See also

- [`fork`](./fork.md) — fork without replay
- [`run`](./run.md) — fresh, empty chain
- [CLI overview](./index.md) — global flags and usage
