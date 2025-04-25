# `fork`

Create a **local `anvil-zksync` node pre-loaded with live state** from a remote network. All
subsequent transactions are mined locally, so you can experiment freely without touching the real
chain.

## Synopsis

```bash
anvil-zksync fork --fork-url <FORK_URL> [OPTIONS]
```

> `--fork-url` is **required**. It accepts either an HTTP/S endpoint or a short-hand alias such as:
> `mainnet`, `sepolia-testnet`, `abstract`, etc.

### Named chain aliases

| Alias              | RPC endpoint                      |
| ------------------ | --------------------------------- |
| `mainnet`          | `https://mainnet.era.zksync.io`   |
| `sepolia-testnet`  | `https://sepolia.era.zksync.dev`  |
| `abstract`         | `https://api.mainnet.abs.xyz/`    |
| `abstract-testnet` | `https://api.testnet.abs.xyz`     |
| `sophon`           | `https://rpc.sophon.xyz/`         |
| `sophon-testnet`   | `https://rpc.testnet.sophon.xyz/` |

## Options

| Flag                             | Description                                                    |
| -------------------------------- | -------------------------------------------------------------- |
| `--fork-url <FORK_URL>`          | Network to fork from (HTTP/S endpoint or alias). **Required.** |
| `--fork-block-number <BLOCK>`    | Import state at a specific **block number**.                   |
| `--fork-transaction-hash <HASH>` | Import state just **before** a given transaction.              |

## Behavior

- The forked state is loaded **once** at startup; changes made locally never propagate back to the
  remote network.
- If neither `--fork-block-number` nor `--fork-transaction-hash` is supplied, `anvil-zksync` fetches
  the **latest** block.
- All global flags (logging, gas, cache, etc.) still apply.

## Examples

### 1. Fork Era mainnet at the latest block

```bash
anvil-zksync fork --fork-url mainnet
```

### 2. Fork a local node over HTTP

```bash
anvil-zksync fork --fork-url http://127.0.0.1:3050
```

### 3. Fork mainnet at block 59,473,098

```bash
anvil-zksync fork --fork-url mainnet --fork-block-number 59473098
```

### 4. Fork Abstract

```bash
anvil-zksync fork --fork-url abstract
```

## See also

- [`run`](./run.md) — start a clean chain
- [`replay_tx`](./replay_tx.md) — fork & replay a specific transaction
- [`CLI Overview`](./index.md) — global flags and usage
