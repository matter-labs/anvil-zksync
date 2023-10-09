# Basic

Start the node:

```sh
era_test_node run
```

The expected output will be as follows:


```log
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

> *Note:* When utilizing `era-test-node` with MetaMask, it's essential to note that any restart of the in-memory node will necessitate a reset of MetaMask's 
cached account data (nonce, etc). To do this, navigate to `Settings`, then `Advanced`, and finally, select `Clear activity tab data`.
