# Introduction

Welcome to the **anvil-zkSync** documentation—your single stop for spinning up a local ZK chain node, forking mainnet or testnets, and debugging smart-contract workflows.

> **At a glance:**  
> • **Fast in-memory node** for Elastic Network ZK chains 
> • Drop-in *anvil* CLI parity (forking, snapshots, impersonation, …)  
> • Rich RPC surface for integration tests and L1↔L2 scenarios

## Documentation map

- **[CLI Reference](./cli/index.md)**
- **[RPC Reference](./rpc/index.md)**
- **[Guides](./guides/local_hardhat_testing.md)**

## Getting started in 30 seconds

```bash
# 1. Install via Cargo (recommended)
brew tap matter-labs/anvil-zksync https://github.com/matter-labs/anvil-zksync.git 
brew install anvil-zksync

# 2. Launch a fresh local node
anvil-zksync run

# 3. Or fork Era mainnet at the latest block
anvil-zksync fork --fork-url mainnet
```

Once running, visit [`http://localhost:8011`](http://localhost:8011).

## Where to next?

* **New to zkSync?** Start with [Getting Started](./intro/getting-started.md) for a quick architectural primer.  
* **Deep dive into flags?** Head over to the [CLI Reference](./cli/index.md).  
* **Building an L1↔L2 bridge?** Check the [Guides](./guides/local_hardhat_testing.md) section.
* **Looking for Rust documentation index?** Head over to the [Rust API](./rustdoc/anvil_zksync/index.html) for a complete list of public APIs.

Thank you for using **anvil-zkSync** —happy building!  
If you spot an issue or have an idea for improvement, open an issue or [pull request]() on GitHub.