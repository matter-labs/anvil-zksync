# Deploying Contracts

For the deployment of your contracts, you have the flexibility to choose between two preferred methods: either by using Hardhat 
with the `hardhat-zksync-deploy` and `hardhat-zksync-solc` plugins, or via `foundry-zksync`. 
The following example will detail the process using `foundry-zksync`.

Before proceeding, ensure that you've compiled your contracts using `zkforge zk-build`. 
For instructions on how to do this, please refer to this [link](https://github.com/matter-labs/foundry-zksync#compile-with-zkforge-zk-build).

```sh
zkforge zkc contracts/Greeter.sol:Greeter --constructor-args "ZkSync and Foundry" --private-key 7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110 --rpc-url http://localhost:8011 --chain 260
```

Here's an example of what you should expect to see:
```log
Deploying contract...
+-------------------------------------------------+
Contract successfully deployed to address: 0x0a40ecde17dc16c4001bf0e4f5d5ff1818219b3b
Transaction Hash: 0x9d59bea38ca6f3cef365c23f339547bcc8ce28abb8344999ffffa5fa62c9ff8e
Gas used: 2570407
Effective gas price: 500
Block Number: 8072361
+-------------------------------------------------+
```