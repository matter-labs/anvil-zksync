# Testing with anvil-zksync

Use Hardhat, Mocha & Chai to write and run tests against a live `anvil-zksync` node.

## 1. Scaffold a Hardhat project

```bash
npm init -y
npm install --save-dev hardhat
npx hardhat
```

## 2. Install ZKsync & test libraries

Install the ZKsync Hardhat plugin, ethers provider, and test tools:

:::code-group

```bash [pnpm]
pnpm add -D @matterlabs/hardhat-zksync zksync-ethers ethers mocha chai @types/mocha @types/chai
```

```bash [npm]
npm install --save-dev @matterlabs/hardhat-zksync zksync-ethers ethers mocha chai @types/mocha @types/chai
```

```bash [yarn]
yarn add -D @matterlabs/hardhat-zksync zksync-ethers ethers mocha chai @types/mocha @types/chai
```

```bash [bun]
bun add -d @matterlabs/hardhat-zksync zksync-ethers ethers mocha chai @types/mocha @types/chai
```

:::

## 3. Configure Hardhat

Edit `hardhat.config.ts` (or `.js`) to point tests at your local node:

```ts
import '@matterlabs/hardhat-zksync';

export default {
  zksolc: { version: '1.5.13', settings: {} },
  defaultNetwork: 'anvil_zksync',
  networks: {
    hardhat: { zksync: true },
    anvil_zksync: {
      url: 'http://localhost:8011',
      ethNetwork: 'http://localhost:8545',
      zksync: true,
    },
  },
  solidity: { version: '0.8.17' },
};
```

## 4. Add a test script

In your `package.json`, add:

```json
"scripts": {
  "test": "mocha --recursive --exit"
}
```

## 5. Write a sample test

Create `test/Greeter.test.ts`:

```ts
import { expect } from 'chai';
import { Wallet, Provider } from 'zksync-ethers';
import * as hre from 'hardhat';
import { Deployer } from '@matterlabs/hardhat-zksync';

const RICH_PK = '0x7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110';

describe('Greeter', function () {
  it('deploys and greets', async function () {
    const provider = Provider.getDefaultProvider();
    const wallet = new Wallet(RICH_PK, provider);
    const deployer = new Deployer(hre, wallet);

    // compile & deploy
    const artifact = await deployer.loadArtifact('Greeter');
    const greeter = await deployer.deploy(artifact, ['Hello, zkSync!']);

    // call and assert
    expect(await greeter.greet()).to.equal('Hello, zkSync!');
  });
});
```

## 6. Run your node & tests

1. **Terminal A:**

   ```bash
   anvil-zksync
   ```

2. **Terminal B:**

   ```bash
   npm test
   ```

Your tests will connect to `http://localhost:8011` and run against your local ZK chain node.
