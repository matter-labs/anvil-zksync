# `anvil-zksync`

A single binary **local ZK chain node** that can impersonate _any_ ZKsync powered or Elastic Network
ZK chain. Ideal for integration tests, fork based debugging, quick prototyping, and offline
experiments.

## Installation

```bash
brew tap matter-labs/anvil-zksync https://github.com/matter-labs/anvil-zksync.git 
brew install anvil-zksync
```

## Usage

```bash // [anvil-zksync]
anvil-zksync [OPTIONS] [COMMAND]
```

## Quick start

```bash // [run with user traces]
# 1. Fresh network that auto‑mines
anvil-zksync run -vv
```

```bash // [fork ZKsync Era mainnet]
# 2. Fork mainnet at the latest block
anvil-zksync fork mainnet
```

```bash // [Replay tx from Abstract]
# 3. Replay tx from Abstract mainnet with user vm traces
anvil-zksync -vv replay_tx --fork-url abstract \
0xea5a45976f8013fd937ac267211fa575bc5968350a0ae3d8932bad75651d933c
```

## Commands

| Command       | Purpose                                      | Docs                       |
| ------------- | -------------------------------------------- | -------------------------- |
| **run**       | Start a brand new empty network              | [`run`](./run.md)             |
| **fork**      | Fork an existing chain into a local instance | [`fork`](./fork.md)           |
| **replay_tx** | Fork + replay a historical L2 transaction    | [`replay_tx`](./replay_tx.md) |
| **help**      | Show help for any command                    | -                          |

## Global options

### Chain initialization

| Flag                             | Description                           | Default |
| -------------------------------- | ------------------------------------- | ------- |
| `--timestamp <NUM>`              | Override genesis block timestamp      | -       |
| `--init <PATH>`                  | Load full `genesis.json` definition   | -       |
| `--state <PATH>`                 | Load then dump snapshot on exit       | -       |
| `-s, --state-interval <SECONDS>` | Auto-dump state every _n_ seconds     | -       |
| `--dump-state <PATH>`            | Dump state snapshot on exit only      | -       |
| `--preserve-historical-states`   | Keep in-memory states for past blocks | off     |
| `--load-state <PATH>`            | Restore from an existing snapshot     | -       |

### Mining & mempool

| Flag              | Description                   | Default     |
| ----------------- | ----------------------------- | ----------- |
| `--no-mining`     | Mine on demand only           | `auto-mine` |
| `--order <order>` | Transaction ordering strategy | `fifo`      |

### General

| Flag                      | Description                             |
| ------------------------- | --------------------------------------- |
| `--offline`               | Disable **all** network requests        |
| `--health-check-endpoint` | Expose `GET /health` returning `200 OK` |
| `--config-out <FILE>`     | Write effective JSON config to disk     |
| `-h, --help`              | Show help                               |
| `-V, --version`           | Show version                            |

### Network

| Flag              | Description                               | Default   |
| ----------------- | ----------------------------------------- | --------- |
| `--port <PORT>`   | RPC port                                  | `8011`    |
| `--host <IP>`     | Bind address (env `ANVIL_ZKSYNC_IP_ADDR`) | `0.0.0.0` |
| `--chain-id <ID>` | Chain ID                                  | `260`     |

### Debugging

| Flag                          | Description                                          | Values / Notes                         |
| ----------------------------- | ---------------------------------------------------- | -------------------------------------- |
| `--show-node-config[=<bool>]` | Print node config on startup                         | `true`                                 |
| `--show-storage-logs <mode>`  | Storage log details                                  | `none`, `read`, `write`, `paid`, `all` |
| `--show-vm-details <mode>`    | VM execution details                                 | `none`, `all`                          |
| `--show-gas-details <mode>`   | Gas cost breakdown                                   | `none`, `all`                          |
| `-v, --verbosity…`            | Increment log detail (`-vvv` = system + user traces) | up to `-vvvvv`                         |

### Gas configuration

| Flag                       | Description                     |
| -------------------------- | ------------------------------- |
| `--l1-gas-price <wei>`     | Custom L1 gas price             |
| `--l2-gas-price <wei>`     | Custom L2 gas price             |
| `--l1-pubdata-price <wei>` | Custom pubdata price            |
| `--price-scale-factor <x>` | Price estimation multiplier     |
| `--limit-scale-factor <x>` | Gas limit estimation multiplier |

### System

| Flag                                    | Description                     | Default / Values |
| --------------------------------------- | ------------------------------- | ---------------- |
| `--override-bytecodes-dir <DIR>`        | Override deployed bytecodes     | -                |
| `--enforce-bytecode-compression=<bool>` | Enforce compression             | `false`          |
| `--dev-system-contracts <mode>`         | Built‑in / local / no-security  | `built-in`       |
| `--system-contracts-path <PATH>`        | Custom system contract build    | -                |
| `--protocol-version <N>`                | Protocol version for new blocks | `26`             |
| `--emulate-evm`                         | Enable EVM emulation            | false            |

### Logging

| Flag                     | Description             | Default            |
| ------------------------ | ----------------------- | ------------------ |
| `--log <level>`          | Log level               | `info`             |
| `--log-file-path <PATH>` | Persist logs            | `anvil-zksync.log` |
| `--silent[=<bool>]`      | Suppress startup banner | `false`            |

### Cache

| Flag                   | Description                                               | Default  |
| ---------------------- | --------------------------------------------------------- | -------- |
| `--cache <option>`     | Cache backend. Available options: `none`, `memory`,`disk` | `disk`   |
| `--reset-cache=<bool>` | Wipe local cache on start                                 | `false`  |
| `--cache-dir <DIR>`    | Cache directory                                           | `.cache` |

### Accounts

| Flag                            | Description                             | Default           |
| ------------------------------- | --------------------------------------- | ----------------- |
| `-a, --accounts <N>`            | Dev accounts to generate                | `10`              |
| `--balance <ETH>`               | Balance per dev account                 | `10000`           |
| `--mnemonic <PHRASE>`           | Custom BIP-39 mnemonic                  | -                 |
| `--mnemonic-random[=<words>]`   | Generate random mnemonic                | `12` words        |
| `--mnemonic-seed-unsafe <seed>` | Derive from seed (**testing only**)     | -                 |
| `--derivation-path <path>`      | HD derivation path                      | `m/44'/60'/0'/0/` |
| `--auto-impersonate`            | Unlock any sender (aka `--auto-unlock`) | -                 |

### Block sealing

| Flag                     | Description                                    | Default |
| ------------------------ | ---------------------------------------------- | ------- |
| `-b, --block-time <sec>` | Fixed block interval. If unset, seal instantly | -       |

### Server

| Flag                       | Description                        | Default |
| -------------------------- | ---------------------------------- | ------- |
| `--allow-origin <origins>` | CORS `Access-Control-Allow-Origin` | `*`     |
| `--no-cors`                | Disable CORS                       | -       |

### L1 (unstable)

| Flag                         | Description                   | Default |
| ---------------------------- | ----------------------------- | ------- |
| `--spawn-l1[=<port>]`        | Start colocated L1 Anvil node | `8012`  |
| `--external-l1 <URL>`        | Use external L1 JSON-RPC      | -       |
| `--auto-execute-l1[=<bool>]` | Auto execute L1 batches       | `false` |

### Custom base token

| Flag                         | Description                          |
| ---------------------------- | ------------------------------------ |
| `--base-token-symbol <SYM>`  | Replace `ETH` symbol                 |
| `--base-token-ratio <ratio>` | Conversion ratio (`40000`, `628/17`) |

## Next steps

- **Start a local chain** → [`run`](./run.md)
- **Fork Elastic Network chains** → [`fork`](./fork.md)
- **Replay transaction** → [`replay_tx`](./replay_tx.md)
