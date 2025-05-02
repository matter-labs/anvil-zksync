# Why anvil-zksync

Developers building on Elastic Network chains need a local node that **understands EraVM**, supports
rapid iteration, and handles cross-chain workflows. **anvil-zksync** fills that gap.

Why not just use `anvil`?

Anvil is great but it is built to target Ethereum's EVM but **EraVM is not EVM-equivalent**. EraVM
is a custom virtual machine that powers ZKsync Era and other Elastic Network chains. It has its own
opcodes, gas costs, and execution model. This means that anvil's EVM-based execution environment is
not compatible with EraVM bytecode.

**anvil-zksync** is a drop-in replacement for Anvil, delivering the same fast local development
workflow—but with native EraVM support.

## Rapid testing, debugging & prototyping

- **Spin up a fresh network** in milliseconds with `anvil-zksync run`.
- **Fork any Elastic Network ZK chain** at the latest block or a historic block number with `fork`.
- **Auto-mine or mine on demand**, so you can exercise blocks exactly when you need them.
- **Verbose VM tracing** helps you introspect failing txs, gas usage, and internal state.

## Forking Elastic Network ZK chains

With `--fork-url` you can point at any supported network `mainnet`, `sepolia-testnet`, `abstract`,
`sophon`, etc. and get a local fork that includes all state (accounts, contracts, balances). Perfect
for:

- **Integration tests**
- **Debugging live issues**
- **Prototyping**
- **dApp development**
- **Gas cost analysis**

## L2→L1 communication

`anvil-zksync` supports local L2-L1 communication:

- **L1→L2 deposits** and **L2→L1 withdrawals**
- **L2→L1 messaging**
- **Priority queue transactions**
- **Upgrade and governance txs**

## Custom base-token support

Not every chain uses ETH for gas.

Configure a **custom base token symbol** and **conversion ratio**, and all gas estimations and
balances will “just work” for non-ETH base tokens.

## Protocol & system-contract development

For protocol engineers:

- **Test system contracts and bootloader changes** locally quickly and easily
- **Experiment with new protocol versions** (`--protocol-version`)
- **Validate compression, pubdata costs, and bytecode overrides**

## Emulate EVM usage

Starting in the v27 protocol upgrade, `anvil-zksync` supports running the EVM interpreter on top of
EraVM. This is useful if you need to:

- Test, debug, and prototype with EVM contracts on EraVM
- Compare EraVM vs EVM interpreter behavior

```bash
anvil-zksync --emulate-evm --protocol-version 27
```

## Replay and trace historical transactions

Replay any transaction:

```bash
anvil-zksync -vv replay_tx \
  --fork-url mainnet \
  0xe12b1924faa45881beb324adca1f8429d553d7ad56a2a030dcebea715e1ec1e4
```
