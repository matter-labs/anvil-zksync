# Forking Networks

To fork the `testnet`, use the following command:
```sh
era_test_node fork testnet
```

You can also fork `mainnet` with 
```sh
era_test_node fork mainnet
```

The expected output will be as follows:

```log
12:34:56 [INFO] Creating fork from "https://testnet.era.zksync.dev:443" L1 block: L1BatchNumber(128846) L2 block: 12088718 with timestamp 1695822231, L1 gas price 2500000014 and protocol version: Some(Version15)
12:34:56 [INFO] Starting network with chain id: L2ChainId(260)
12:34:56 [INFO] Rich Accounts
12:34:56 [INFO] =============
12:34:56 [INFO] Account #0: 0x36615Cf349d7F6344891B1e7CA7C72883F5dc049 (1_000_000_000_000 ETH)
12:34:56 [INFO] Private Key: 0x7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110
12:34:56 [INFO]
12:34:56 [INFO] Account #1: 0xa61464658AfeAf65CccaaFD3a512b69A83B77618 (1_000_000_000_000 ETH)
12:34:56 [INFO] Private Key: 0xac1e735be8536c6534bb4f17f06f6afc73b2b5ba84ac2cfb12f7461b20c0bbe3

...

12:34:56 [INFO] Account #9: 0xE90E12261CCb0F3F7976Ae611A29e84a6A85f424 (1_000_000_000_000 ETH)
12:34:56 [INFO] Private Key: 0x3eb15da85647edd9a1159a4a13b9e7c56877c4eb33f614546d4db06a51868b1c
12:34:56 [INFO]
12:34:56 [INFO] ========================================
12:34:56 [INFO]   Node is ready at 127.0.0.1:8011
12:34:56 [INFO] ========================================
```

This command starts the node, forked at the current head of the testnet.

You also have the option to specify a custom http endpoint and a custom forking height, like so:

```sh
# Usage: era_test_node fork --fork-at <FORK_AT> <NETWORK>
era_test_node fork --fork-at 7000000 mainnet http://172.17.0.3:3060
```

## Sending network calls

You can send network calls against a running `era-test-node`. You can check the testnet LINK balance or mainnet USDT using `curl` or [foundry-zksync](https://github.com/matter-labs/foundry-zksync).

To get started, launch the local in-memory node:
```sh
era_test_node fork testnet
```

Next, use curl to send a network call:
```sh
curl -X POST -H "Content-Type: application/json" --data '{"jsonrpc":"2.0","method":"eth_call","params":[{"to":"0x40609141Db628BeEE3BfAB8034Fc2D8278D0Cc78", "data":"0x06fdde03"}, "latest"],"id":1}' http://localhost:8011
```

Here's an example of what you should expect to see:
```log
{"jsonrpc":"2.0","result":"0x00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000018436861696e4c696e6b20546f6b656e2028676f65726c69290000000000000000","id":1}
```

Or, if you prefer, use [foundry-zksync](https://github.com/matter-labs/foundry-zksync). Make sure to install and configure `foundry-zksync` before proceeding 
(for installation instructions, please see this [link](https://github.com/matter-labs/foundry-zksync/tree/main#foundry-with-zksync-era-v01)):
```sh
zkcast call 0x40609141Db628BeEE3BfAB8034Fc2D8278D0Cc78 "name()(string)" --rpc-url http://localhost:8011
```

Here's an example of what you should expect to see:
```log
ChainLink Token (goerli)
```

Retrieve the balance of a particular contract:
```sh
zkcast call 0x40609141Db628BeEE3BfAB8034Fc2D8278D0Cc78 "balanceOf(address)(uint256)"  0x40609141Db628BeEE3BfAB8034Fc2D8278D0Cc78  --rpc-url http://localhost:8011
```

Here's an example of what you should expect to see:
```log
28762283719941475444443116625665
```