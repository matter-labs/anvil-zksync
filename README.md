<div align="center">
<a href="https://era.zksync.io/docs/tools/testing/era-test-node.html">

![era-test-node](./.github/assets/era_test_node_banner_light.png#gh-light-mode-only)
![Webb Logo](./.github/assets/era_test_node_banner_dark.png#gh-dark-mode-only)
</a>

  </div>

# 🚀 zkSync Era In-Memory Node 🚀

This crate provides an in-memory node that supports forking the state from other networks.

The goal of this crate is to offer a fast solution for integration testing, bootloader and system contract testing, and
prototyping.

🔗 For a detailed walkthrough, refer to the [official documentation](https://era.zksync.io/docs/tools/testing/era-test-node.html).

## 📌 Overview

The In-Memory Node is designed for local testing and uses an in-memory database for storing state information. It also employs simplified hashmaps for tracking blocks and transactions. When in fork mode, it fetches missing storage data from a remote source if not available locally. Additionally, it uses the remote server (openchain) to resolve the ABI and topics to human-readable names.

## ⚠️ Caution

Please note that `era-test-node` is still in its **alpha** stage. Some features might not be fully supported yet and may not work as intended. However, it is open-sourced, and contributions are welcome!

## 📊 Limitations & Features

| 🚫 Limitations                                  | ✅ Features                                                 |
| ----------------------------------------------- | ----------------------------------------------------------- |
| No communication between Layer 1 and Layer 2.   | Can fork the state of mainnet, testnet, or custom network.  |
| Many APIs are not yet implemented.              | Can replay existing mainnet or testnet transactions.        |
| No support for accessing historical data.       | Uses local bootloader and system contracts.                 |
| Only one transaction allowed per Layer 1 batch. | Operates deterministically in non-fork mode.                |
| Fixed values returned for zk Gas estimation.    | Starts up quickly with pre-configured 'rich' accounts.      |
| Redeploy requires MetaMask cache reset.         | Supports hardhat's console.log debugging.                   |
|                                                 | Resolves names of ABI functions and Events using openchain. |

## 🛠 Prerequisites

1. **Rust**: `era-test-node` is written in Rust. Ensure you have Rust installed on your machine. [Download Rust here](https://www.rust-lang.org/tools/install).

2. **Other Dependencies**: This crate relies on rocksDB. If you face any compile errors due to rocksDB, install the necessary dependencies with:
   ```bash
   apt-get install -y cmake pkg-config libssl-dev clang
   ```

## 📥 Installation & Setup

1. Install `era-test-node`:
   ```bash
   cargo install --git https://github.com/matter-labs/era-test-node.git --locked
   ```
2. Start the node:
   ```bash
   era_test_node run
   ```
## 📃 Logging

The node may be started in either of `debug`, `info`, `warn` or `error` logging levels via the `--log` option:
```bash
era_test_node --log=error run
```

Additionally, the file path can be provided via the `--log-file-path` option (defaults to `./era_test_node.log`):
```bash
era_test_node --log=error --log-file-path=run.log run
```

## 📃 Caching

The node will cache certain network request by default to disk in the `.cache` directory. Alternatively the caching can be disabled or set to in-memory only
via the `--cache=none|memory|disk` parameter. 

```bash
era_test_node --cache=none run
```

```bash
era_test_node --cache=memory run
```

Additionally when using `--cache=disk`, the cache directory may be specified via `--cache-dir` and the cache may
be reset on startup via `--reset-cache` parameters.
```bash
era_test_node --cache=disk --cache-dir=/tmp/foo --reset-cache run
```

## 🌐 Network Details

- L2 RPC: http://localhost:8011
- Network Id: 260

> Note: The existing implementation does not support communication with Layer 1. As a result, an L1 RPC is not available.

## 🍴 Forking Networks

To fork the mainnet:

```bash
era_test_node fork mainnet
```

> Tip: You can also fork the testnet with `era_test_node fork testnet`.

## 🔄 Replay Remote Transactions Locally

If you wish to replay a remote transaction locally for deep debugging, use the following command:

```bash
era_test_node replay_tx testnet 0x7f039bcbb1490b855be37e74cf2400503ad57f51c84856362f99b0cbf1ef478a
```

## 📞 Sending Network Calls

You can send network calls against a running `era-test-node`. For example, to check the testnet LINK balance or mainnet USDT, use `curl` or `foundry-zksync`.

```bash
curl -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_call","params":[{"to":"0x40609141Db628BeEE3BfAB8034Fc2D8278D0Cc78", "data":"0x06fdde03"}, "latest"],"id":1}' http://localhost:8011
```

## 🔍 Seeing more details of the transactions

By default, the tool is just printing the basic information about the executed transactions (like status, gas used etc).

But with --show-calls flag, it can print more detailed call traces, and with --resolve-hashes, it will ask openchain for ABI names.

```bash
era_test_node --show-calls=user --resolve-hashes replay_tx testnet 0x7f039bcbb1490b855be37e74cf2400503ad57f51c84856362f99b0cbf1ef478a

Executing 0x7f039bcbb1490b855be37e74cf2400503ad57f51c84856362f99b0cbf1ef478a
┌─────────────────────────┐
│   TRANSACTION SUMMARY   │
└─────────────────────────┘
Transaction: SUCCESS
Initiator: 0x55362182242a4de20ea8a0ec055b2134bb24e23d
Payer: 0x55362182242a4de20ea8a0ec055b2134bb24e23d
Gas - Limit: 797128 | Used: 351250 | Refunded: 445878

==== Console logs: 

==== 18 call traces.  Use --show-calls flag or call config_setShowCalls to display more info.
Call(Normal) 0x55362182242a4de20ea8a0ec055b2134bb24e23d           validateTransaction(bytes32, bytes32, tuple)   730485
  Call(Normal) 0x0000000000000000000000000000000000000001                 0xbb1e83e6   698040
Call(Normal) 0x55362182242a4de20ea8a0ec055b2134bb24e23d           payForTransaction(bytes32, bytes32, tuple)   703647
Call(Normal) 0x55362182242a4de20ea8a0ec055b2134bb24e23d           executeTransaction(bytes32, bytes32, tuple)   647199
    Call(Mimic) 0x6eef3310e09df3aa819cc2aa364d4f3ad2e6ffe3           swapExactETHForTokens(uint256,address[],address,uint256)   596358
      Call(Normal) 0x053f26a020de152a947b8ba7d8974c85c5fc5b81           getPair(address,address)   577647

```

## 💰 Using Rich Wallets

For testing and development purposes, the `era-test-node` comes pre-configured with a set of 'rich' wallets. These wallets are loaded with test funds, allowing you to simulate transactions and interactions without the need for real assets.

Here's a list of the available rich wallets:

| Account Address                              | Private Key                                                          |
| -------------------------------------------- | -------------------------------------------------------------------- |
| `0x36615Cf349d7F6344891B1e7CA7C72883F5dc049` | `0x7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110` |
| `0xa61464658AfeAf65CccaaFD3a512b69A83B77618` | `0xac1e735be8536c6534bb4f17f06f6afc73b2b5ba84ac2cfb12f7461b20c0bbe3` |
| `0x0D43eB5B8a47bA8900d84AA36656c92024e9772e` | `0xd293c684d884d56f8d6abd64fc76757d3664904e309a0645baf8522ab6366d9e` |
| `0xA13c10C0D5bd6f79041B9835c63f91de35A15883` | `0x850683b40d4a740aa6e745f889a6fdc8327be76e122f5aba645a5b02d0248db8` |
| `0x8002cD98Cfb563492A6fB3E7C8243b7B9Ad4cc92` | `0xf12e28c0eb1ef4ff90478f6805b68d63737b7f33abfa091601140805da450d93` |
| `0x4F9133D1d3F50011A6859807C837bdCB31Aaab13` | `0xe667e57a9b8aaa6709e51ff7d093f1c5b73b63f9987e4ab4aa9a5c699e024ee8` |
| `0xbd29A1B981925B94eEc5c4F1125AF02a2Ec4d1cA` | `0x28a574ab2de8a00364d5dd4b07c4f2f574ef7fcc2a86a197f65abaec836d1959` |
| `0xedB6F5B4aab3dD95C7806Af42881FF12BE7e9daa` | `0x74d8b3a188f7260f67698eb44da07397a298df5427df681ef68c45b34b61f998` |
| `0xe706e60ab5Dc512C36A4646D719b889F398cbBcB` | `0xbe79721778b48bcc679b78edac0ce48306a8578186ffcb9f2ee455ae6efeace1` |
| `0xE90E12261CCb0F3F7976Ae611A29e84a6A85f424` | `0x3eb15da85647edd9a1159a4a13b9e7c56877c4eb33f614546d4db06a51868b1c` |

Feel free to use these wallets in your tests, but remember, they are for development purposes only and should not be used in production or with real assets.

## 🔧 Supported APIs

See our list of [Supported APIs here](SUPPORTED_APIS.md).

## 🤝 Contributing

We welcome contributions from the community! If you're interested in contributing to the zkSync Era In-Memory Node, please take a look at our [CONTRIBUTING.md](./.github/CONTRIBUTING.md) for guidelines and details on the process.

Thank you for making zkSync Era In-Memory Node better! 🙌
