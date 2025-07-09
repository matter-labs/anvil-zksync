# Getting Started

Get up and running in four steps: start your node, interact via RPC, deploy a contract, and replay a
transaction.

## 1. Start your local node

By default, `run` is the active command:

```bash
anvil-zksync
```

- **RPC endpoint:** `http://localhost:8011`
- **Dev accounts:** 10 pre-funded wallets

To see VM traces and call stacks, add `-vv` (increase verbosity level for more detail):

```bash
anvil-zksync -vv
```

## 2. Send an RPC call

Use `curl` or Foundry's `cast` against your local node:

```bash
curl -s -X POST http://localhost:8011 \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}'
```

```bash
cast block-number --rpc-url http://localhost:8011
```

## 3. Deploy a contract

Compile with zkSync support, then deploy via `forge`:

```bash
forge build --zksync
forge create \
  contracts/Greeter.sol:Greeter \
  --constructor-args "Hello, zkSync!" \
  --rpc-url http://localhost:8011 \
  --chain 260 \
  --zksync
```

## 4. Replay a historical transaction

Fork and replay any on-chain tx locally, with verbose tracing:

```bash
anvil-zksync -vv replay_tx \
  --fork-url mainnet \
  0xe12b1924faa45881beb324adca1f8429d553d7ad56a2a030dcebea715e1ec1e4
```

## Next steps

- [CLI commands](../cli/index.md)
- [RPC methods](../rpc/index.md)
- [Guides](../guides/local_hardhat_testing.md)
