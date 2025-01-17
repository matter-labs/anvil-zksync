# 🚀 anvil-zksync 🚀

> [!IMPORTANT]  
> This repository has been renamed from **era-test-node** to **anvil-zksync**. All references to the previous name have been updated to reflect this change.

This crate provides an in-memory node that supports forking the state from other networks.

The goal of this crate is to offer a fast solution for integration testing, bootloader and system contract testing, and prototyping.

🔗 **For a detailed walkthrough, refer to the following resources:**

- [Official documentation: Anvil-ZKsync](https://docs.zksync.io/build/test-and-debug/in-memory-node)
- [Foundry Book: Anvil for zkSync](https://foundry-book.zksync.io/reference/anvil-zksync/)
- [Rust Book: Anvil-ZKsync](https://matter-labs.github.io/era-test-node/era_test_node/index.html)

## 📌 Overview

`anvil-zksync` is designed for local testing and uses an in-memory database for storing state information. It also employs simplified hashmaps for tracking blocks and transactions. When in fork mode, it fetches missing storage data from a remote source if not available locally. Additionally, it uses the remote server (openchain) to resolve the ABI and topics to human-readable names.

## ⚠️ Caution

Please note that `anvil-zksync` is still in its **alpha** stage. Some features might not be fully supported yet and may not work as intended. However, it is open-sourced, and contributions are welcome!

## 📊 Limitations & Features

| 🚫 Limitations                                  | ✅ Features                                                 |
| ----------------------------------------------- | ----------------------------------------------------------- |
| No communication between Layer 1 and Layer 2.   | Can fork the state of mainnet, testnet, or custom network.  |
| No support for accessing historical data.       | Uses local bootloader and system contracts.                 |
| Only one transaction allowed per Layer 1 batch. | Operates deterministically in non-fork mode.                |
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

1. Download the installation script and mark as executable:
   ```bash
   curl --proto '=https' -sSf https://raw.githubusercontent.com/matter-labs/anvil-zksync/main/scripts/install.sh > install.sh
   chmod +x install.sh
   ```

2. Run the script with `sudo` (version can optionally be specified via the `--version` argument):
   ```bash
   sudo ./install.sh
   ```

3. Start the node:
   ```bash
   anvil-zksync run
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
   anvil-zksync run
   ```

## 🧑‍💻 Running Locally

1. Compile Rust project and start the node:
   ```bash
   make run
   ```

## 📄 System Contracts

The system contract within the node can be specified via the `--dev-system-contracts` option.
It can take one of the following options:
   * `built-in`: Use the compiled built-in contracts
   * `built-in-no-verify`: Use the compiled built-in contracts, but without signature verification
   * `local`: Load contracts from `ZKSYNC_HOME`

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
anvil-zksync fork fork-url mainnet
```

> Tip: You can also fork the zkSync Sepolia testnet with `anvil-zksync fork fork-url sepolia-testnet`.

## 🔄 Replay Remote Transactions Locally

If you wish to replay a remote transaction locally for deep debugging, use the following command:

```bash
anvil-zksync replay_tx <network> <transaction_hash>
```

## Replacing bytecodes

You can also replace / override the contract bytecode with the local version. This is especially useful if you are replaying some mainnet transactions and would like to see how they would behave on the different bytecode. Or when you want to fork mainnet to see how your code would
behave on mainnet state.

You have to prepare a directory, with files in format `0xabc..93f.json` that contain the json outputs that you can get from zkout directories from your compiler.

Then you have to add `--override-bytecodes-dir=XX` flag to point at that directory. See the `example_override` dir for more details.

```bash
cargo run -- --override-bytecodes-dir=example_override --show-storage-logs all fork mainnet
```

## 📞 Sending Network Calls

You can send network calls against a running `anvil-zksync`. For example, to check the testnet LINK balance or mainnet USDT, use `curl` or `foundry-zksync`.

```bash
curl -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_call","params":[{"to":"0x40609141Db628BeEE3BfAB8034Fc2D8278D0Cc78", "data":"0x06fdde03"}, "latest"],"id":1}' http://localhost:8011
```

## 🔍 Seeing more details of the transactions

By default, the tool is just printing the basic information about the executed transactions (like status, gas used etc).

But with --show-calls flag, it can print more detailed call traces, and with --resolve-hashes, it will ask openchain for ABI names.

```bash
anvil-zksync --show-calls=user --resolve-hashes replay_tx sepolia-testnet 0x7119045573862797257e4441ff48bf5a3bc4d133a00d167c18dc955eda12cfac

Executing 0x7119045573862797257e4441ff48bf5a3bc4d133a00d167c18dc955eda12cfac
┌─────────────────────────┐
│   TRANSACTION SUMMARY   │
└─────────────────────────┘
Transaction: SUCCESS
Initiator: 0x4eaf936c172b5e5511959167e8ab4f7031113ca3
Payer: 0x4eaf936c172b5e5511959167e8ab4f7031113ca3
Gas - Limit: 2_487_330 | Used: 969_330 | Refunded: 1_518_000
Use --show-gas-details flag or call config_setShowGasDetails to display more info

==== Console logs:

==== 22 call traces.  Use --show-calls flag or call config_setShowCalls to display more info.
  Call(Normal) 0x4eaf936c172b5e5511959167e8ab4f7031113ca3           validateTransaction(bytes32, bytes32, tuple)   1830339
    Call(Normal) 0x0000000000000000000000000000000000000001                 0x89c19e9b   1766835
  Call(Normal) 0x4eaf936c172b5e5511959167e8ab4f7031113ca3           payForTransaction(bytes32, bytes32, tuple)   1789767
  Call(Normal) 0x4eaf936c172b5e5511959167e8ab4f7031113ca3           executeTransaction(bytes32, bytes32, tuple)   1671012
      Call(Mimic) 0x5d4fb5385ed95b65d1cd6a10ed9549613481ab2f           0x   1443393
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

This is now even easier with a single flag (`--debug-mode` or `-d`):

```bash
anvil-zksync -d
```

## 💰 Using Rich Wallets

For testing and development purposes, the `anvil-zksync` comes pre-configured with a set of 'rich' wallets. These wallets are loaded with test funds, allowing you to simulate transactions and interactions without the need for real assets.

Here's a list of the available rich wallets:
```
18:02:50  INFO Rich Accounts
18:02:50  INFO ========================
18:02:50  INFO (0) 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 (10000 ETH)
18:02:50  INFO (1) 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 (10000 ETH)
18:02:50  INFO (2) 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC (10000 ETH)
18:02:50  INFO (3) 0x90F79bf6EB2c4f870365E785982E1f101E93b906 (10000 ETH)
18:02:50  INFO (4) 0x15d34AAf54267DB7D7c367839AAf71A00a2C6A65 (10000 ETH)
18:02:50  INFO (5) 0x9965507D1a55bcC2695C58ba16FB37d819B0A4dc (10000 ETH)
18:02:50  INFO (6) 0x976EA74026E726554dB657fA54763abd0C3a0aa9 (10000 ETH)
18:02:50  INFO (7) 0x14dC79964da2C08b23698B3D3cc7Ca32193d9955 (10000 ETH)
18:02:50  INFO (8) 0x23618e81E3f5cdF7f54C3d65f7FBc0aBf5B21E8f (10000 ETH)
18:02:50  INFO (9) 0xa0Ee7A142d267C1f36714E4a8F75612F20a79720 (10000 ETH)
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
