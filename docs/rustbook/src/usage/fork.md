# Forking Networks

To fork the `sepolia-testnet`, use the following command:
```sh
era_test_node fork sepolia-testnet
```

You can also fork `mainnet` with 
```sh
era_test_node fork mainnet
```

The expected output will be similar to the following:

```log
14:50:12  INFO Creating fork from "https://mainnet.era.zksync.io:443" L1 block: L1BatchNumber(356201) L2 block: 21979120 with timestamp 1703083811, L1 gas price 41757081846 and protocol version: Some(Version18)
14:50:12  INFO Starting network with chain id: L2ChainId(260)
14:50:12  INFO 
14:50:12  INFO Rich Accounts
14:50:12  INFO =============
14:50:16  INFO Account #0: 0xBC989fDe9e54cAd2aB4392Af6dF60f04873A033A (1_000_000_000_000 ETH)
14:50:16  INFO Private Key: 0x3d3cbc973389cb26f657686445bcc75662b415b656078503592ac8c1abb8810e
14:50:16  INFO Mnemonic: mass wild lava ripple clog cabbage witness shell unable tribe rubber enter
14:50:16  INFO 
14:50:16  INFO Account #1: 0x55bE1B079b53962746B2e86d12f158a41DF294A6 (1_000_000_000_000 ETH)
14:50:16  INFO Private Key: 0x509ca2e9e6acf0ba086477910950125e698d4ea70fa6f63e000c5a22bda9361c
14:50:16  INFO Mnemonic: crumble clutch mammal lecture lazy broken nominee visit gentle gather gym erupt

...

14:50:19  INFO Account #9: 0xe2b8Cb53a43a56d4d2AB6131C81Bd76B86D3AFe5 (1_000_000_000_000 ETH)
14:50:19  INFO Private Key: 0xb0680d66303a0163a19294f1ef8c95cd69a9d7902a4aca99c05f3e134e68a11a
14:50:19  INFO Mnemonic: increase pulp sing wood guilt cement satoshi tiny forum nuclear sudden thank
14:50:19  INFO 
14:50:19  INFO ========================================
14:50:19  INFO   Node is ready at 127.0.0.1:8011
14:50:19  INFO ========================================
```

This command starts the node, forking it from the latest block on the zkSync Sepolia testnet.

You also have the option to specify a custom http endpoint and a custom forking height, like so:

```sh
# Usage: era_test_node fork --fork-at <FORK_AT> <NETWORK>
era_test_node fork --fork-at 7000000 mainnet http://172.17.0.3:3060
```
