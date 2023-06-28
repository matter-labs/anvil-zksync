# In memory node, with fork support

This crate provides an in-memory node that supports forking the state from other networks.

The goal of this crate is to offer a fast solution for integration testing, bootloader and system contract testing, and
prototyping.

Please note that this crate is still in the alpha stage, and not all functionality is fully supported. For final
testing, it is highly recommended to use the 'local-node' or a testnet.

Current limitations:

- No communication between Layer 1 and Layer 2 (the local node operates only on Layer 2).
- Many APIs are not yet implemented, but the basic set of APIs is supported.
- No support for accessing historical data, such as the storage state at a specific block.
- Only one transaction is allowed per Layer 1 batch.
- Fixed values are returned for zk Gas estimation.

Current features:

- Can fork the state of the mainnet, testnet, or a custom network at any given height.
- Can replay the existing mainnet, testnet transaaction.
- Uses local bootloader and system contracts, making it suitable for testing new changes.
- When running in non-fork mode, it operates deterministically (only one transaction per block, etc.), which simplifies
  testing.
- Starts up quickly and comes pre-configured with a few 'rich' accounts.
- Supports hardhat's console.log debugging.
- Can resolve the names of ABI functions and Events (using openchain)

## Installation

The easiest way is to install from source:
```
cargo install --git https://github.com/matter-labs/era-test-node.git
```

Rust should install it in ``~/.cargo/bin`` directory.

If you get compile errors due to rocksDB, you might also want to install:

```
apt-get install -y cmake pkg-config libssl-dev clang
```

## How to

To start a node:

```shell
zksync_test_node run
```

This will run a node (with an empty state) and make it available on port 8011

To fork mainnet:

```shell
zksync_test_node fork mainnet
```

This will run the node, forked at current head of mainnet

You can also specify the custom http endpoint and custom forking height:

```shell
zksync_test_node fork --fork-at 7000000 http://172.17.0.3:3060
```

Or replay locally a remote transaction (for example to see more debug
information).

```shell
zksync_test_node replay_tx testnet 0x7f039bcbb1490b855be37e74cf2400503ad57f51c84856362f99b0cbf1ef478a
```


## Seeing more details of the transactions

By default, the tool is just printing the basic information about the executed transactions (like status, gas used etc).

But with `--show-calls` flag, it can print more detailed call traces, and with --resolve-hashes, it will ask openchain for ABI names.

```shell
$ zksync_test_node --show-calls=user --resolve-hashes replay_tx testnet 0x7f039bcbb1490b855be37e74cf2400503ad57f51c84856362f99b0cbf1ef478a


Executing 0x7f039bcbb1490b855be37e74cf2400503ad57f51c84856362f99b0cbf1ef478a
Transaction: SUCCESS
Initiator: 0x55362182242a4de20ea8a0ec055b2134bb24e23d Payer: 0x55362182242a4de20ea8a0ec055b2134bb24e23d
Gas Limit: 797128 used: 399148 refunded: 397980
18 call traces. Use --show-calls flag to display more info.
Call(Normal) 0x55362182242a4de20ea8a0ec055b2134bb24e23d 0x202bcce7   729918
  Call(Normal) 0x0000000000000000000000000000000000000001 0xbb1e83e6   688275
Call(Normal) 0x55362182242a4de20ea8a0ec055b2134bb24e23d 0xe2f318e3   693630
Call(Normal) 0x55362182242a4de20ea8a0ec055b2134bb24e23d 0xdf9c1589   624834
    Call(Mimic) 0x6eef3310e09df3aa819cc2aa364d4f3ad2e6ffe3 swapExactETHForTokens(uint256,address[],address,uint256)   562275
      Call(Normal) 0x053f26a020de152a947b8ba7d8974c85c5fc5b81 getPair(address,address)   544068

```


## Forking network & sending calls

You can use your favorite development tool (or tools like `curl`) or zksync-foundry:

Check testnet LINK balance

```shell
$ zksync_test_node fork testnet

$ zkcast call 0x40609141Db628BeEE3BfAB8034Fc2D8278D0Cc78 "name()(string)" --rpc-url http://localhost:8011

> ChainLink Token (goerli)


$ zkcast call 0x40609141Db628BeEE3BfAB8034Fc2D8278D0Cc78 "balanceOf(address)(uint256)"  0x40609141Db628BeEE3BfAB8034Fc2D8278D0Cc78  --rpc-url http://localhost:8011
> 28762283719732275444443116625665
```

Or Mainnet USDT:

```shell
zksync_test_node fork mainnet

zkcast call 0x493257fD37EDB34451f62EDf8D2a0C418852bA4C "name()(string)" --rpc-url http://localhost:8011

> Tether USD
```

And you can also build & deploy your own contracts:

```shell
zkforge zkc src/Greeter.sol:Greeter --constructor-args "ZkSync and Foundry" --private-key 7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110 --rpc-url http://localhost:8011 --chain 270

```

## Testing bootloader & system contracts

In-memory node is taking the currently compiled bootloader & system contracts - therefore easily allowing to test
changes (and together with fork, allows to see the effects of the changes on the already deployed contracts).

You can see the bootloader logs, by setting the proper log level. In the example below, we recompile the bootloader, and
run it with mainnet fork.

```shell

cd etc/system-contracts
yarn preprocess && yarn hardhat run ./scripts/compile-yul.ts
cd -
RUST_LOG=vm=trace cargo run -p zksync_test_node fork --dev_use_local_contracts --fork-at 70000000 testnet
```


### How does it work

It utilizes an in-memory database to store the state information and employs simplified hashmaps to track blocks and
transactions.

In fork mode, it attempts to retrieve missing storage data from a remote source when it's not available locally.

Moreover it also uses the remote server (openchain) to resolve the ABI and topics to human readable names.
