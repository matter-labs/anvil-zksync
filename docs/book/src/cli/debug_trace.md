# `debug-trace`

Spin up an **anvil-zksync** node that forks a remote network and attaches to the **debug API** for a given transaction.

Outputs either:

* A **formatted call trace** (human-readable), or
* The **raw debug JSON** (if requested).

Perfect for diagnosing **why a transaction reverted**, inspecting **nested calls**, or analyzing **low-level VM execution**.

---

## Synopsis

```bash // [debug-trace]
anvil-zksync debug-trace --rpc-url <FORK_URL> <TX>
```

Both parameters are **required**:

* `--rpc-url <FORK_URL>` - remote endpoint or chain alias
* `<TX>` - L2 **transaction hash** to debug

---

### Named chain aliases

| Alias                            | RPC endpoint                                  |
| -------------------------------- | --------------------------------------------- |
| `era`, `mainnet`                 | `https://mainnet.era.zksync.io`               |
| `era-testnet`, `sepolia-testnet` | `https://sepolia.era.zksync.dev`              |
| `abstract`                       | `https://api.mainnet.abs.xyz`                 |
| `abstract-testnet`               | `https://api.testnet.abs.xyz`                 |
| `sophon`                         | `https://rpc.sophon.xyz`                      |
| `sophon-testnet`                 | `https://rpc.testnet.sophon.xyz`              |
| `cronos`                         | `https://mainnet.zkevm.cronos.org`            |
| `cronos-testnet`                 | `https://testnet.zkevm.cronos.org`            |
| `lens`                           | `https://rpc.lens.xyz`                        |
| `lens-testnet`                   | `https://rpc.testnet.lens.xyz`                |
| `openzk`                         | `https://rpc.openzk.net`                      |
| `openzk-testnet`                 | `https://openzk-testnet.rpc.caldera.xyz/http` |
| `zkcandy`                        | `https://rpc.zkcandy.io`                      |

---

## Arguments

| Name   | Description                                       |
| ------ | ------------------------------------------------- |
| `<TX>` | Transaction hash (`0x…`, 32 bytes). **Required.** |

---

## Options

| Flag                   | Description                                                                 |
| ---------------------- | --------------------------------------------------------------------------- |
| `--rpc-url <FORK_URL>` | Network to fork from (endpoint or alias). **Required.**                     |
| `--only-top`           | Restrict trace output to **only the top-level call** (skip internal calls). |
| `--raw`                | Print **raw debug JSON** instead of formatted trace.                        |

---

## Behavior

1. Queries the **debug\_traceTransaction** RPC using the URL and TX provided.
2. Depending on flags:

   * `--only-top`: prints only the root call.
   * `--raw`: dumps the raw JSON response.
   * Otherwise: prints a **formatted tree trace** with calls, gas, and status.

---

## Examples

### 1. Debug a successful swap tx on Era mainnet

```bash
anvil-zksync debug-trace \
  --rpc-url mainnet \
  0x977b31d564042b7e14044c5d1fd7c1f95454e8f9ef643febd40a9c0d082d09cb
```

### 2. Show only the top-level call

```bash
anvil-zksync debug-trace \
  --rpc-url mainnet \
  --only-top \
  0x977b31d564042b7e14044c5d1fd7c1f95454e8f9ef643febd40a9c0d082d09cb
```

### 3. Dump the raw debug JSON

```bash
anvil-zksync debug-trace \
  --rpc-url mainnet \
  --raw \
  0x977b31d564042b7e14044c5d1fd7c1f95454e8f9ef643febd40a9c0d082d09cb
```

---

## See also

* [`replay_tx`](./replay_tx.md) — fork & re-execute a transaction
* [`fork`](./fork.md) — fork without replay
* [`run`](./run.md) — fresh, empty chain
* [CLI overview](./index.md) — global flags and usage
