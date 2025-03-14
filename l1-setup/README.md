# Setting up [anvil](https://github.com/foundry-rs/foundry) as an L1

## Prerequisites

You will need:
* `zkstack` matching protocol version support by anvil-zksync
  * To install: take a look at the root workspace `Cargo.toml` which should contain `zksync_contracts = "=<version>-non-semver-compat"`. Now clone [zksync-era](https://github.com/matter-labs/zksync-era) and checkout tag `core-v<version>`. Install zkstackup if you have not done so already and then run `zkstackup --local`. Confirm that build timestamp is recent when you run `zkstack --version`.
* A local postgres instance as expected by `zkstack`
  * To initialize a fresh one just run `zkstack dev clean all && zkstack containers -o false` in the `zksync-era` root directory. We do not actually need the database but zkstack will fail on one of the steps as it expects it to exist.
  * NOTE: `zkstack` will also spin up a `reth` instance that occupies port 8545 which we need. Please stop it before proceeding by running `docker stop zksync-era-reth-1`.
* `yq` for YAML config manipulation. Follow https://github.com/mikefarah/yq to install.

## Generate state file

⚠️ Note: all commands below assume you are running them from the `l1-setup` directory.

Run foundry anvil (make sure you are using version >=1.0.0):

```bash
$ anvil --port 8545  --dump-state state/l1-state.json
```

Once it is up just run the setup script:

```bash
$ ./setup.sh
```

Finally, if everything went smoothly, stop anvil (thus dumping the state) and then verify that the state is loadable like this:

```bash
$ anvil --port 8545 --load-state state/l1-state.json
```

Additionally, `state/l1-state-payload.txt` contains the payload version of anvil's state. In other words, you can inject state into a running anvil instance by submitting `anvil_loadState` JSON-RPC request with the contents of this file as the payload.

Done!

Note: Re-running this flow with the same input parameters will result in different contract configuration and L1 state. This is expected as `create2_factory_salt` is random each time.
