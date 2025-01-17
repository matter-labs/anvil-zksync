# Testing

The node can be used to test different parameters of the system locally.


### Testing bootloader and system contracts
In-memory node allows testing of the currently compiled bootloader and system contracts. 
This makes it possible to examine the effects of changes on already deployed contracts.

> *Note:* These commands assume you have set `$ZKSYNC_HOME` in your shell profile file (e.g. `~/.bash_profile`, `~/.zshrc`) 
to target your local copy of `anvil-zksync`. For instance:

```sh
# Add path here:
export ZKSYNC_HOME=/path/to/anvil-zksync

export PATH=$ZKSYNC_HOME/bin:$PATH
```

Firstly, you will need to preprocess and compile the contracts:
```sh
cd etc/system-contracts
yarn preprocess && yarn hardhat run ./scripts/compile-yul.ts
```

To use the locally compiled bootloader and system contracts, run:
```sh
RUST_LOG=vm=trace anvil-zksync --dev-use-local-contracts fork sepolia-testnet
```

## Writing and running tests locally
This section demonstrates how to author and execute tests locally against `anvil-zksync` using the `mocha` and `chai` testing frameworks.

### Project configuration
Start by creating a new Hardhat project. If you need guidance, follow the [getting started guide](https://era.zksync.io/docs/tools/hardhat/getting-started.html).

To incorporate the test libraries, execute:
```sh
yarn add -D mocha chai @types/mocha @types/chai
```

Add the following lines to your package.json in the root folder:
```json
"scripts": {
    "test": "NODE_ENV=test hardhat test"
}
```

This script makes it possible to run tests in a Hardhat environment with the `NODE_ENV` env variable set as `test`.

### Configuring tests
Adjust `hardhat.config.ts` to use the local node for testing:
> *Note:* Ensure `anvil-zksync` is running in another process before executing `yarn test`.

```ts
import "@matterlabs/hardhat-zksync-deploy";
import "@matterlabs/hardhat-zksync-solc";

module.exports = {
  zksolc: {
    version: "latest",
    settings: {},
  },
  defaultNetwork: "zkSyncTestnet",
  networks: {
    hardhat: {
      zksync: true,
    },
    zkSyncTestnet: {
      url: "http://localhost:8011",
      ethNetwork: "http://localhost:8545",
      zksync: true,
    },
  },
  solidity: {
    version: "0.8.17",
  },
};
```

### Writing test scripts
Now, create your first test! Construct a test/main.test.ts file with the following code:

```ts
import { expect } from "chai";
import { Wallet, Provider, Contract } from "zksync-web3";
import * as hre from "hardhat";
import { Deployer } from "@matterlabs/hardhat-zksync-deploy";

const RICH_WALLET_PK = "0x7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110";

describe("Greeter", function () {
  it("Should return the new greeting once it's changed", async function () {
    const provider = Provider.getDefaultProvider();

    const wallet = new Wallet(RICH_WALLET_PK, provider);
    const deployer = new Deployer(hre, wallet);

    const artifact = await deployer.loadArtifact("Greeter");
    const greeter = await deployer.deploy(artifact, ["Hi"]);

    expect(await greeter.greet()).to.eq("Hi");

    const setGreetingTx = await greeter.setGreeting("Hola, mundo!");
    // wait until the transaction is mined
    await setGreetingTx.wait();

    expect(await greeter.greet()).to.equal("Hola, mundo!");
  });
});
```

To run the test file, execute:
```sh
yarn test
```

Well done! You've successfully run your first local tests with zkSync Era and `anvil-zksync`.

## CI/CD Testing with GitHub Actions
A GitHub Action is available for integrating anvil-zksync into your CI/CD environments. 
This action offers high configurability and streamlines the process of testing your applications in an automated way.

You can find this GitHub Action in the marketplace [here](https://github.com/marketplace/actions/anvil-zksync-action).

### Example Usage
Below is an example yaml configuration to use the anvil-zksync GitHub Action in your workflow:
```yml
name: Run anvil-zksync Action

on:
  push:
    branches: [main]

jobs:
  build:
    runs-on: ubuntu-latest

    steps:
      - name: Checkout code
        uses: actions/checkout@v2

      - name: Run anvil-zksync
        uses: dutterbutter/anvil-zksync-action@latest
```