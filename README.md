# 🚀 anvil-zksync 🚀

> [!IMPORTANT]  
> This repository has been renamed from **era-test-node** to **anvil-zksync**. All references to the previous name have been updated to reflect this change.

This crate provides an in-memory node that supports forking the state from other networks.

The goal of this crate is to offer a fast solution for integration testing, bootloader and system contract testing, and prototyping.

🔗 **For a detailed walkthrough, refer to the following resources:**

- [Official documentation: Anvil-ZKsync](https://docs.zksync.io/build/test-and-debug/in-memory-node)
- [Foundry Book: Anvil for zkSync](https://foundry-book.zksync.io/reference/anvil-zksync/)
- [Rust Book: Anvil-ZKsync](https://matter-labs.github.io/anvil-zksync/anvil_zksync/index.html)

## 📌 Overview

`anvil-zksync` is designed for local testing and uses an in-memory database for storing state information. It also employs simplified hashmaps for tracking blocks and transactions. When in fork mode, it fetches missing storage data from a remote source if not available locally. Additionally, it uses the remote server (openchain) to resolve the ABI and topics to human-readable names.

## 📊 Limitations & Features

| 🚫 Limitations                                  | ✅ Features                                                 |
| ----------------------------------------------- | ----------------------------------------------------------- |
| Cannot fork state with L1-L2 communication     | Can fork the state of mainnet, testnet, or custom network.  |
| Limited support for accessing historical data.  | Uses local bootloader and system contracts.                 |
| Only one block allowed per Layer 1 batch.       | Operates deterministically in non-fork mode.                |
| Redeploy requires MetaMask cache reset.         | Supports hardhat's console.log debugging.                   |
|                                                 | Resolves names of ABI functions and Events using openchain. |
|                                                 | Can replay existing mainnet or testnet transactions.        |
|                                                 | Starts up quickly with pre-configured 'rich' accounts.      |

## 🛠 Prerequisites

1. **Rust**: `anvil-zksync` is written in Rust. Ensure you have Rust installed on your machine. [Download Rust here](https://www.rust-lang.org/tools/install).

2. **Other Dependencies**: This crate relies on rocksDB. If you face any compile errors due to rocksDB, install the necessary dependencies with:
   ```bash
   apt-get install -y cmake pkg-config libssl-dev clang
   ```

## 📥 Installation & Setup

### Using the installation script

1. Install via `foundryup-zksync` as described [here](https://foundry-book.zksync.io/getting-started/installation):
  ```
  curl -L https://raw.githubusercontent.com/matter-labs/foundry-zksync/main/install-foundry-zksync | bash
  ```

This will install `forge`, `cast` and `anvil-zksync`.

3. Start the node:
   ```bash
   anvil-zksync
   ```

### Manually

1. Download `anvil-zksync` from latest [Release](https://github.com/matter-labs/anvil-zksync/releases/latest)

2. Extract the binary and mark as executable:
   ```bash
   tar xz -f anvil-zksync.tar.gz -C /usr/local/bin/
   chmod +x /usr/local/bin/anvil-zksync
   ```

3. Start the node:
   ```bash
   anvil-zksync
   ```

## 🧑‍💻 Running Locally

1. Compile Rust project and start the node:
   ```bash
   make run
   ```

## 📡 L1–L2 Communication

`anvil-zksync` supports L1-L2 communication by either spawning a new [Anvil](https://book.getfoundry.sh/anvil/) L1 node or using an existing one.

### 1. Spawn a Local L1 Node

Use the `--spawn-l1` flag to launch an Anvil L1 node on a specified port (defaults to `8012` if no port is provided):

```bash
anvil-zksync --spawn-l1
# or specify a different port:
anvil-zksync --spawn-l1 9000
```

This command relies on Anvil being installed. To install Anvil, please refer to documentation [here](https://book.getfoundry.sh/getting-started/installation).

### 2. Connect to an External L1 Node

If you already have an Anvil L1 node running, ensure it was started with the `--no-request-size-limit` option:

```bash
anvil --no-request-size-limit
```

Then, provide its JSON-RPC endpoint to `anvil-zksync` via:

```bash
anvil-zksync --external-l1 http://localhost:8545
```

> **Note:** The `--spawn-l1` and `--external-l1` flags cannot be used together because they are mutually exclusive.

## 📄 System Contracts

The system contract within the node can be specified via the `--dev-system-contracts` option.
It can take one of the following options:

- `built-in`: Use the compiled built-in contracts
- `built-in-no-verify`: Use the compiled built-in contracts, but without signature verification
- `local`: Load contracts from `ZKSYNC_HOME` or specify path using `--system-contracts-path`

**Example:**

```bash
anvil-zksync --dev-system-contracts local --system-contracts-path ./system-contracts --protocol-version 28
```

## 📃 Logging

The node may be started in either of `debug`, `info`, `warn` or `error` logging levels via the `--log` option:
```bash
anvil-zksync --log=error run
```

Additionally, the file path can be provided via the `--log-file-path` option (defaults to `./anvil-zksync.log`):
```bash
anvil-zksync --log=error --log-file-path=run.log run
```

The logging can be configured during runtime via the [`config_setLogLevel`](./SUPPORTED_APIS.md#config_setloglevel) and [`config_setLogging`](./SUPPORTED_APIS.md#config_setlogging) methods.

## 📊 Telemetry

Anonymous usage data is collected **only if you opt in** when first prompted. You can opt out at any time by editing or removing the telemetry configuration file:

- macOS (Darwin):

  ```bash
  $HOME/Library/Application Support/com.matter-labs.zksync-tooling/telemetry.json
  ```

- Linux:

  ```bash
  $XDG_CONFIG_HOME/zksync-tooling/telemetry.json
  or
  $HOME/.config/zksync-tooling/telemetry.json
  ```

**What we collect**:

- Basic usage statistics  
- Error reports  
- Platform information  

**What we do NOT collect**:

- Personal information  
- Sensitive configuration  
- Private keys or addresses

## 📃 Caching

The node will cache certain network request by default to disk in the `.cache` directory. Alternatively the caching can be disabled or set to in-memory only
via the `--cache=none|memory|disk` parameter.

```bash
anvil-zksync --cache=none run
```

```bash
anvil-zksync --cache=memory run
```

Additionally when using `--cache=disk`, the cache directory may be specified via `--cache-dir` and the cache may
be reset on startup via `--reset-cache` parameters.
```bash
anvil-zksync --cache=disk --cache-dir=/tmp/foo --reset-cache run
```

## 🌐 Network Details

- L2 RPC: http://localhost:8011
- Network Id: 260

> Note: The existing implementation does not support communication with Layer 1. As a result, an L1 RPC is not available.

## 🍴 Forking Networks

To fork the mainnet:

```bash
# Available options mainnet, sepolia-testnet, abstract, abstract-testnet, sophon, sophon-testnet
anvil-zksync fork --fork-url mainnet
```

## 🔄 Replay Remote Transactions Locally

If you wish to replay a remote transaction locally for deep debugging, use the following command:

```bash
anvil-zksync replay_tx --fork-url <network> <transaction_hash>
```

Example:

```bash
anvil-zksync --show-calls=all --resolve-hashes=true replay_tx --fork-url sepolia-testnet \
0x0d53f06d3f3734d1f2dd6456c2f9de05d333be0b83ea6caed2a52f4103849fe4
```

## Replacing bytecodes

You can also replace / override the contract bytecode with the local version. This is especially useful if you are replaying some mainnet transactions and would like to see how they would behave on the different bytecode. Or when you want to fork mainnet to see how your code would
behave on mainnet state.

You have to prepare a directory, with files in format `0xabc..93f.json` that contain the json outputs that you can get from zkout directories from your compiler.

Then you have to add `--override-bytecodes-dir=XX` flag to point at that directory. See the `example_override` dir for more details.

```bash
anvil-zksync --override-bytecodes-dir=example_override --show-storage-logs all fork --fork-url mainnet
```

## 📞 Sending Network Calls

You can send network calls against a running `anvil-zksync`. For example, to check the testnet LINK balance or mainnet USDT, use `curl` or `foundry-zksync`.

```bash
curl -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_call","params":[{"to":"0x40609141Db628BeEE3BfAB8034Fc2D8278D0Cc78", "data":"0x06fdde03"}, "latest"],"id":1}' http://localhost:8011
```

## 🔍 Seeing more details of the transactions

By default, the tool is just printing the basic information about the executed transactions (like status, gas used etc).

To dive deeper into execution details, incremental verbosity flags can now be used to unlock rich debugging output.

### 📢 Verbosity Levels

Use the `-v` flag repeatedly (`-vv`, `-vvv`, etc.) to increase the level of detail shown in logs. Each level adds more granular VM tracing insights:

`-vv`: User-level calls, User L1-L2 logs, and event traces
`-vvv`:Adds system-level calls, System logs, and event traces
`-vvvv`: Includes system and user calls, system and user events, and precompiles

```bash
anvil-zksync -vv \
replay_tx --fork-url sepolia-testnet \
0x7119045573862797257e4441ff48bf5a3bc4d133a00d167c18dc955eda12cfac

✅ [SUCCESS] Hash: 0x7119045573862797257e4441ff48bf5a3bc4d133a00d167c18dc955eda12cfac
Initiator: 0x4eaf936c172b5e5511959167e8ab4f7031113ca3
Payer: 0x4eaf936c172b5e5511959167e8ab4f7031113ca3
Gas Limit: 2_487_330 | Used: 127_813 | Refunded: 2_359_517
Paid: 0.0000242845 ETH (127813 gas * 0.19000000 gwei)
Refunded: 0.0004483082 ETH

Traces:
  [18539] 0x4eaf936c172b5e5511959167e8ab4f7031113ca3::validateTransaction(0x7119045573862797257e4441ff48bf5a3bc4d133a00d167c18dc955eda12cfac, 0x89c19e9b41956859a89a7a263bcefdc9f7836a4001b258f8e5d23d2df73d201e, (2, 449216752821327364111873762331949023955812170915 [4.492e47], 532713687062943204947393888873391921348219087663 [5.327e47], 2487330 [2.487e6], 50000 [5e4], 1380000000 [1.38e9], 1000000000 [1e9], 0, 26, 40804000000000008 [4.08e16], [0, 0, 0, 0], 0x, 0x52da4d91f482131dde68bc08f1a1d8ebb3297ed7ac94e70e9f0f0270dc3c8f8b5b1f0a2e1cfcae078251d507f80fee2f8434af2b199b44536d21a7d72fb779771b, [], 0x, 0x))
    └─ ← [Success] 202bcce700000000000000000000000000000000000000000000000000000000
  [8497] 0x4eaf936c172b5e5511959167e8ab4f7031113ca3::payForTransaction(0x7119045573862797257e4441ff48bf5a3bc4d133a00d167c18dc955eda12cfac, 0x89c19e9b41956859a89a7a263bcefdc9f7836a4001b258f8e5d23d2df73d201e, (2, 449216752821327364111873762331949023955812170915 [4.492e47], 532713687062943204947393888873391921348219087663 [5.327e47], 2487330 [2.487e6], 50000 [5e4], 1380000000 [1.38e9], 1000000000 [1e9], 0, 26, 40804000000000008 [4.08e16], [0, 0, 0, 0], 0x, 0x52da4d91f482131dde68bc08f1a1d8ebb3297ed7ac94e70e9f0f0270dc3c8f8b5b1f0a2e1cfcae078251d507f80fee2f8434af2b199b44536d21a7d72fb779771b, [], 0x, 0x))
    └─ ← [Success]
  [10622] 0x4eaf936c172b5e5511959167e8ab4f7031113ca3::executeTransaction(0x7119045573862797257e4441ff48bf5a3bc4d133a00d167c18dc955eda12cfac, 0x89c19e9b41956859a89a7a263bcefdc9f7836a4001b258f8e5d23d2df73d201e, (2, 449216752821327364111873762331949023955812170915 [4.492e47], 532713687062943204947393888873391921348219087663 [5.327e47], 2487330 [2.487e6], 50000 [5e4], 1380000000 [1.38e9], 1000000000 [1e9], 0, 26, 40804000000000008 [4.08e16], [0, 0, 0, 0], 0x, 0x52da4d91f482131dde68bc08f1a1d8ebb3297ed7ac94e70e9f0f0270dc3c8f8b5b1f0a2e1cfcae078251d507f80fee2f8434af2b199b44536d21a7d72fb779771b, [], 0x, 0x))
    ├─ [204] 0x5d4fb5385ed95b65d1cd6a10ed9549613481ab2f::fallback{value: 40804000000000008}() [mimiccall]
    │   └─ ← [Success]
    └─ ← [Success] 00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000
```

You can use the following options to get more granular information during transaction processing:

- `--show-storage-logs <SHOW_STORAGE_LOGS>`: Show storage log information.
  [default: none]
  [possible values: none, read, paid, write, all]

- `--show-vm-details <SHOW_VM_DETAILS>`: Show VM details information.
  [default: none]
  [possible values: none, all]

- `--show-gas-details <SHOW_GAS_DETAILS>`: Show Gas details information.
  [default: none]
  [possible values: none, all]

Example:

```bash
anvil-zksync --show-storage-logs=all --show-vm-details=all --show-gas-details=all run
```

## 💰 Using Rich Wallets

For testing and development purposes, the `anvil-zksync` comes pre-configured with a set of 'rich' wallets. These wallets are loaded with test funds, allowing you to simulate transactions and interactions without the need for real assets.

Here's a list of the available rich wallets:
```
Rich Accounts
========================
(0) 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 (10000 ETH)
(1) 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 (10000 ETH)
(2) 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC (10000 ETH)
(3) 0x90F79bf6EB2c4f870365E785982E1f101E93b906 (10000 ETH)
(4) 0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65 (10000 ETH)
(5) 0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc (10000 ETH)
(6) 0x976EA74026E726554dB657fA54763abd0C3a0aa9 (10000 ETH)
(7) 0x14dC79964da2C08b23698B3D3cc7Ca32193d9955 (10000 ETH)
(8) 0x23618e81E3f5cdF7f54C3d65f7FBc0aBf5B21E8f (10000 ETH)
(9) 0xa0Ee7A142d267C1f36714E4a8F75612F20a79720 (10000 ETH)
```

Feel free to use these wallets in your tests, but remember, they are for development purposes only and should not be used in production or with real assets.

## 🔧 Supported APIs

See our list of [Supported APIs here](SUPPORTED_APIS.md).

## 🤖 CI/CD Testing with GitHub Actions

A GitHub Action is available for integrating `anvil-zksync` into your CI/CD environments. This action offers high configurability and streamlines the process of testing your applications in an automated way.

You can find this GitHub Action in the marketplace [here](https://github.com/marketplace/actions/anvil-zksync-action).

### 📝 Example Usage

Below is an example `yaml` configuration to use the `anvil-zksync` GitHub Action in your workflow:

```yml
name: Run anvil-zksync Action

on:
  push:
    branches: [ main ]

jobs:
  build:
    runs-on: ubuntu-latest

    steps:
    - name: Checkout code
      uses: actions/checkout@v2

    - name: Run anvil-zksync
      uses: dutterbutter/anvil-zksync-action@latest
```

## 🤝 Contributing

We welcome contributions from the community! If you're interested in contributing to the anvil-zksync, please take a look at our [CONTRIBUTING.md](./.github/CONTRIBUTING.md) for guidelines and details on the process.

Thank you for making anvil-zksync better! 🙌
