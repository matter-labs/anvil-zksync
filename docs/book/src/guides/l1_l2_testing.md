# L1 → L2 Message Testing with Anvil-ZKsync

This guide shows you how to locally test a cross-chain message from L1 → L2 using `anvil` and `anvil-zksync`.

## Prerequisites

* [`anvil`](https://book.getfoundry.sh/anvil/) installed
* [`anvil-zksync`](https://github.com/matter-labs/anvil-zksync) installed
* Node.js + Hardhat dev setup

---

## 1. Start Local Nodes

Start the **L1** and **L2** nodes in two terminals:

```bash
# L1 (Anvil)
anvil --no-request-size-limit --port 8012

# L2 (Anvil-ZkSync)
anvil-zksync --evm-interpreter --external-l1=http://127.0.0.1:8012
```

The `--evm-interpreter` flag enables deploying EVM contracts to L2, simplifying Hardhat workflows.

You now have:

* L1 RPC at `http://127.0.0.1:8012`
* L2 RPC at `http://127.0.0.1:8011`

---

## 2. Clone the Example Repo

```bash
git clone git@github.com:dutterbutter/cross-chain-example.git
cd cross-chain-example
npm install
```

Repo layout:

```bash
├── contracts             # AccessKey (L1) & Vault (L2)
├── test/l1-l2.spec.ts    # Full e2e test using Hardhat + ethers
```

---

## 3. What the Contracts Do

### `AccessKey.sol` (L1)

Sends a cross-chain message to unlock a vault on L2 via the `Bridgehub`.

### `Vault.sol` (L2)

Accepts an `unlock()` call and marks itself as `unlocked`. Validates the caller using aliased L1 address logic.

---

## Environment Variables

Ensure `.env` contains:

```env
PRIVATE_KEY=0x...      # Use pre-funded dev accounts from anvil-zksync
```

## 4. Run the Test

This test:

* Deploys `AccessKey` to L1
* Deploys `Vault` to L2 with the aliased address of the L1 AccessKey
* Calls `AccessKey.unlockVaultOnL2(...)` to dispatch a message via `Bridgehub`
* Waits for L2 state change to confirm message success

Compile contracts first:

```bash
npm run compile
```

Run the test:

```bash
npm run test
```

---

## Want EraVM Contracts Instead?

If you're interested in deploying **EraVM contracts** instead of EVM contracts using `@matterlabs/hardhat-zksync`, check out the [`eravm-test`](https://github.com/dutterbutter/cross-chain-example/tree/eravm-test) branch of the repository. Everything else stays the same — nodes, config, and test flow.

---

## ✅ Success Criteria

At the end of the test, you should see:

```bash
✓ should deploy AccessKey on L1, Vault on L2, then unlock (XXXXms)
```

And `Vault.isVaultUnlocked()` on L2 should return `true`.
