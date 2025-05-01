<div align="center">

# 🚀 anvil‑zksync 🚀

*Elastic power for the Elastic Network – spin up blazing‑fast local ZK chain nodes in a snap*

[![CI Status](https://github.com/matter-labs/anvil-zksync/actions/workflows/checks.yaml/badge.svg)](https://github.com/matter-labs/anvil-zksync/actions/workflows/checks.yaml)
[![Release](https://img.shields.io/github/v/release/matter-labs/anvil-zksync?label=version)](https://github.com/matter-labs/anvil-zksync/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![X: @zksync](https://img.shields.io/badge/follow-@zksync-1DA1F2?logo=x)](https://x.com/zksync)
[![User Book](https://img.shields.io/badge/docs-user%20book-brightgreen)](https://matter-labs.github.io/anvil-zksync/latest/)

</div>

<p align="center">
  <b>
    <a href="https://matter-labs.github.io/anvil-zksync/latest/intro/installation.html">Install</a> ·
    <a href="https://matter-labs.github.io/anvil-zksync/latest/">User Book</a> ·
    <a href="https://matter-labs.github.io/anvil-zksync/latest/rustdoc/anvil_zksync/index.html">Rust Book</a> ·
    <a href="./.github/CONTRIBUTING.md">Contributing</a>
  </b>
</p>

---

## ✨ Features

- **Instant local node** – in‑memory execution for lightning‑fast startup
- **Seamless forking** – mirror any Elastic Network ZK chain or any custom RPC
- **Rich dev accounts** – pre-funded wallets for immediate testing
- **Transaction replay** – debug any live tx with full VM call‑trace introspection
- **Custom base token** – simulate fee tokens & gas ratios for custom base token ZK chains
- **Protocol development** – target non-builtin system contracts for quick protocol testing

## 📦 Installation

Install `anvil-zksync` with your preferred method from the below options:

<details>
<summary><strong>Homebrew</strong></summary>

```bash
brew tap matter-labs/anvil-zksync https://github.com/matter-labs/anvil-zksync.git
brew install anvil-zksync
```

</details>

<details>
<summary><strong>Script (installs forge-zksync, cast-zksync & anvil‑zksync)</strong></summary>

```bash
curl -L https://raw.githubusercontent.com/matter-labs/foundry-zksync/main/install-foundry-zksync | bash
```

</details>

<details>
<summary><strong>Pre‑built binaries</strong></summary>

```bash
# download and unpack the latest release
tar -xzf anvil-zksync_x.y.z_<platform>.tar.gz -C /usr/local/bin
chmod +x /usr/local/bin/anvil-zksync
```

</details>

<details>
<summary><strong>Build from source</strong></summary>

```bash
# clone and build from source
git clone git@github.com:matter-labs/anvil-zksync.git
cd anvil-zksync
cargo build --release
```

</details>

## ⚡️ Quick‑start

**Start a local ZK chain node**

```bash
anvil-zksync run
```

Launches a fast, in-memory ZK chain for local development.

**Fork any Elastic Network chain**

```bash
anvil-zksync fork --fork-url era
```

Mirror mainnet, testnet, or any Elastic ZK chain using a custom RPC.

**Replay transactions**

```bash
anvil-zksync -vv replay_tx --fork-url era 0x0820a939dfe83221f44a6f0f81f8059ec8a7a4e17006965a8b0c146a2c4a00c2
```

Debug live transactions with full VM call‑trace and verbose output.

**Integrate an L1 node**

For L1–L2 interactions, choose one of the following:

- **Spawn a local L1** (defaults to port `8012`):

  ```bash
  anvil-zksync --spawn-l1 [port]
  ```

- **Connect to an external L1** (must be started with `--no-request-size-limit`):

  ```bash
  anvil --no-request-size-limit
  anvil-zksync --external-l1 http://localhost:8545
  ```

> **Note:** `--spawn-l1` and `--external-l1` are mutually exclusive.

**For the full CLI reference and advanced usage, see the user docs: https://matter-labs.github.io/anvil-zksync/latest**

## 🤝 Contributing

Bug reports, fixes and new features are welcome! Please read the [contributing guide](.github/CONTRIBUTING.md) to get started.

## 📜 License

This project is licensed under the terms of the **MIT License** / **Apache License** – see the [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE) and  file for details.
