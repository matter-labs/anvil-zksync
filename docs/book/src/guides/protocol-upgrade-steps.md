# Updating `anvil-zksync` to Support a New Protocol Version

This guide describes the step-by-step process for adding support for a new protocol version in `anvil-zksync`, using the [v28 support PR (#637)](https://github.com/matter-labs/anvil-zksync/pull/637) as a reference.

## 1. Prepare a Versioned Contract Branch with Anvil-ZKsync Debug Support

`anvil-zksync` relies on a customized version of the `era-contracts` repository to enable debugging, impersonation, and other test-specific features. This requires:

* Inserting special debug markers,
* Adding a no-security default account contract,
* Adjusting the preprocessing pipeline.

You can use the following diff as a reference:
[anvil-zksync-0.4.x-release-v28 vs release-v28](https://github.com/matter-labs/era-contracts/compare/release-v28...anvil-zksync-0.4.x-release-v28)

### Required Customizations

The following markers and blocks are used to encapsulate `anvil-zksync`-specific changes:

* `DEBUG SUPPORT START` / `DEBUG SUPPORT END`
* `FOUNDRY SUPPORT START` / `FOUNDRY SUPPORT END`
* `<!-- @ifndef ACCOUNT_IMPERSONATING -->` (Yul preprocessing)
* `For impersonating block start` / `For impersonating block end` in bootloader preprocessing

You must also include a special test contract:

* [`system-contracts/contracts/DefaultAccountNoSecurity.sol`](https://github.com/matter-labs/era-contracts/blob/anvil-zksync-0.4.x-release-v28/system-contracts/contracts/DefaultAccountNoSecurity.sol)

### Files to Modify

Apply the above debug customizations to the following files in your `era-contracts` release branch:

* `l1-contracts/contracts/state-transition/chain-deps/facets/Executor.sol`
* `system-contracts/bootloader/bootloader.yul`
* `system-contracts/contracts/DefaultAccount.sol`
* `system-contracts/contracts/interfaces/IAccount.sol`
* `system-contracts/contracts/DefaultAccountNoSecurity.sol` (new)
* `system-contracts/scripts/preprocess-bootloader.ts`

### Branch Naming Convention

Create a new branch from the official release branch (e.g. `release-v29`) using the format:

```bash
anvil-zksync-<anvil-version>-release-<protocol-version>
```

For example:

```bash
anvil-zksync-0.6.x-release-v29
```

This branch will contain the upstream contracts from the specified protocol version, augmented with the `anvil-zksync` debug and testing support described above.

Once the debug-ready contracts are prepared in this custom branch, they can be integrated into the `anvil-zksync` repository as part of the protocol support update.

## 2. Determine the Target Version and Affected Crates

Identify the new protocol version to support, such as `v29`, and locate the corresponding tag or commit in the [`zksync-era`](https://github.com/matter-labs/zksync-era) repository. This is typically a tag like `core-v29.0.0`.

Next, identify all crates in `anvil-zksync` that rely on upstream `zksync-era` components. The crates to be updated include:

* `zksync_mini_merkle_tree`
* `zksync_multivm`
* `zksync_contracts`
* `zksync_basic_types`
* `zksync_types`
* `zksync_vm_interface`
* `zksync_web3_decl`

## 2.1. Patch `zksync-era` Dependencies in Cargo

Update the zksync-era related dependencies section of the workspace `Cargo.toml` to pin all required crates to the appropriate tag. This ensures all crates across the workspace resolve to a consistent and correct version.

Example update for protocol version `v29`:

```toml
zksync_mini_merkle_tree = { git = "https://github.com/matter-labs/zksync-era", rev = "core-v29.0.0" }
zksync_multivm          = { git = "https://github.com/matter-labs/zksync-era", rev = "core-v29.0.0" }
zksync_contracts        = { git = "https://github.com/matter-labs/zksync-era", rev = "core-v29.0.0" }
zksync_basic_types      = { git = "https://github.com/matter-labs/zksync-era", rev = "core-v29.0.0" }
zksync_types            = { git = "https://github.com/matter-labs/zksync-era", rev = "core-v29.0.0" }
zksync_vm_interface     = { git = "https://github.com/matter-labs/zksync-era", rev = "core-v29.0.0" }
zksync_web3_decl        = { git = "https://github.com/matter-labs/zksync-era", rev = "core-v29.0.0" }
```

> Note: Although the protocol version is typically aligned with the crate tags (e.g. `core-v29.0.0`), this is not guaranteed. Always verify the correct tag by inspecting the upstream changelog or repository releases.

Once dependencies are patched, run:

```bash
cargo check --workspace
```

to confirm that all crates resolve successfully and that no incompatible API changes were introduced. Resolve breaking changes incrementally as needed.

## 4. Add the New Protocol Version and Contract Artifacts

Once your versioned contracts branch is prepared, the next step is to integrate it into `anvil-zksync` by updating the contract refresh pipeline and accounting for any new contracts introduced in the protocol upgrade.

### 4.1 Update the `refresh_contracts.sh` Script

Modify the `scripts/refresh_contracts.sh` script to register the new protocol version. This script downloads, compiles, and bundles contract artifacts for use by `anvil-zksync`.

1. Add a new `v29` entry to the `case` statement and set the corresponding `ERA_CONTRACTS_GIT_COMMIT`. This should point to the HEAD commit of your custom `anvil-zksync-<version>-release-v29` branch:

```bash
PROTOCOL_VERSION=${1:-v29}
case $PROTOCOL_VERSION in
  v26)
    ERA_CONTRACTS_GIT_COMMIT=50dc0669213366f5d3084a7a29a83541cf3c6435
    ;;
  v27)
    ERA_CONTRACTS_GIT_COMMIT=f0e17d700929e25292be971ea5196368bf120cea
    ;;
  v28)
    ERA_CONTRACTS_GIT_COMMIT=054a4745385119e7275dad801a2e830105f21e3e
    ;;
  v29)
    ERA_CONTRACTS_GIT_COMMIT=<COMMIT-HASH> # HEAD of anvil-zksync-0.6.x-release-v29
    ;;
  *)
    echo "Unrecognized/unsupported protocol version: $PROTOCOL_VERSION"
    exit 1
    ;;
esac
```

> Replace `<COMMIT-HASH>` with the actual commit hash of the latest commit in your `anvil-zksync-0.6.x-release-v29` branch.

### 4.2 Add New Contracts

If the protocol upgrade introduced any new contracts, append them to the relevant contract groups in the script.

For example, to include a new L1 contract named `ChainAssetHandler` added in v29:

```bash
if [[ $PROTOCOL_VERSION == v29 ]]; then
  l1_artifacts+=("ChainAssetHandler")
fi
```

Apply the same approach for L2 contracts, system contracts, or bootloader components.

For reference, see how this was handled for v28 [here](https://github.com/matter-labs/anvil-zksync/pull/637/files#diff-94249b93923d8c01fe26a91770ce4b7262fab4c16c89548e259f544e78eabd1d).

### 4.3 Generate the Contract Bundle

Run the updated script to fetch, compile, and package the contract artifacts for the new version:

```bash
./scripts/refresh_contracts.sh v29
```

This will produce a compressed archive at:

```bash
crates/core/src/deps/contracts/builtin-contracts-v29.tar.gz
```

This archive includes all compiled artifacts required for bootstrapping and executing system contracts under `--protocol-version 29`.

### 4.4 Refresh Other Artifacts

Depending on what changed in the protocol upgrade, you may also need to refresh:

* End-to-end contracts: `scripts/refresh_e2e_contracts.sh`
* L1 sidecar artifacts: `scripts/refresh_l1_sidecar_contracts.sh`
* Test contracts: `scripts/refresh_test_contracts.sh`

Each script behaves similarly—updating contracts for its respective domain.

> Tip: There is a `make` command to run the scripts: `make build-contracts`.

### 4.5 Add Protocol Version Support for `l1-setup`

In addition to refreshing contracts, you must also update the `l1-setup/setup.sh` script to support the new protocol version. This step generates the genesis state, upgrade transaction, and configuration files required for bootstrapping an L1 sidecar instance tied to a specific protocol version.

#### 1. Register the New Protocol in `setup.sh`

Locate the `case` block in `l1-setup/setup.sh` and add a new entry for the protocol version, pointing to the HEAD commit of your custom `anvil-zksync-<version>-release-<protocol-version>` branch and its matching `core-vXX.Y.Z` tag:

```bash
PROTOCOL_VERSION=${1:-v29}
case $PROTOCOL_VERSION in
  v26)
    ERA_CONTRACTS_GIT_COMMIT=50dc0669213366f5d3084a7a29a83541cf3c6435
    ERA_TAG=core-v26.8.1
    ;;
  v27)
    ERA_CONTRACTS_GIT_COMMIT=f0e17d700929e25292be971ea5196368bf120cea
    ERA_TAG=core-v27.0.0
    ;;
  v28)
    ERA_CONTRACTS_GIT_COMMIT=054a4745385119e7275dad801a2e830105f21e3e
    ERA_TAG=core-v28.0.0
    ;;
  v29)
    ERA_CONTRACTS_GIT_COMMIT=<COMMIT-HASH> # HEAD of anvil-zksync-0.6.x-release-v29
    ERA_TAG=core-v29.0.0
    ;;
  *)
    echo "Unrecognized/unsupported protocol version: $PROTOCOL_VERSION"
    exit 1
    ;;
esac
```

> Be sure to replace `<COMMIT-HASH>` with the actual commit hash of your branch and ensure the matching `ERA_TAG` exists upstream.

#### 2. Run the Setup Script

From the `l1-setup/` directory, invoke the setup script to generate the artifacts:

```bash
./setup.sh v29
```

This will:

* Spin up a temporary `anvil` instance
* Deploy and configure L1 contracts and L2 upgrade transaction
* Clone and patch the correct version of `zksync-era`
* Generate and persist the following:

#### Output Artifacts

All outputs are stored under `l1-setup/`:

* `configs/v29-genesis.yaml`: L2 genesis metadata
* `configs/v29-contracts.yaml`: Deployed contract addresses
* `state/v29-l2-upgrade-tx.json`: L1 upgrade transaction payload
* `state/v29-l1-state-payload.txt`: Compressed `anvil` state loadable via `anvil_loadState`
* `state/v29-l1-state.json`: Full uncompressed state JSON

#### Caveats

* The process uses a randomized salt when deploying via `CREATE2`, so running it multiple times will produce non-deterministic results.
* Do **not** commit changes to the `contracts` submodule that may appear after running the script.

#### 2. Register Protocol Version in `l1-sidecar`

After generating the files, register the new version in the following locations within the `l1-sidecar` and `l1-setup` crate.

##### `crates/l1_sidecar/src/zkstack_config/mod.rs`

Add a new mapping for the v29 configuration:

```rust
(
    ProtocolVersionId::Version29,
    ZkstackConfig {
        contracts: serde_yaml::from_slice(include_bytes!(
            "../../../../l1-setup/configs/v29-contracts.yaml"
        ))
        .unwrap(),
        genesis: serde_yaml::from_slice(include_bytes!(
            "../../../../l1-setup/configs/v29-genesis.yaml"
        ))
        .unwrap(),
        wallets,
    },
),
```

##### `crates/l1_sidecar/src/upgrade_tx.rs`

Include the upgrade transaction for v29:

```rust
(
    ProtocolVersionId::Version29,
    serde_json::from_slice::<UpgradeTx>(include_bytes!(
        "../../../l1-setup/state/v29-l2-upgrade-tx.json"
    ))
    .unwrap(),
),
```

##### `crates/l1_sidecar/src/anvil.rs`

Add the L1 state and payload for v29:

```rust
(
    ProtocolVersionId::Version29,
    include_bytes!("../../../l1-setup/state/v29-l1-state.json").as_slice(),
),
```

```rust
(
    ProtocolVersionId::Version29,
    include_str!("../../../l1-setup/state/v29-l1-state-payload.txt"),
),
```

## 5. Register the Protocol Version and New Contracts in `system_contracts.rs`

Once contract artifacts have been bundled via the refresh scripts, you must register the new protocol version and any newly introduced contracts in `crates/deps/system_contracts.rs`. This ensures `anvil-zksync` can load and deploy the correct system contracts when bootstrapping a chain at this protocol version.

### 5.1 Import New Addresses from `zksync_types`

Start by importing the relevant contract address constant from `zksync_types`. For example, if the protocol introduces a new `ChainAssetHandler` contract:

```rust
use zksync_types::L2_CHAIN_ASSET_HANDLER_ADDRESS;
```

### 5.2 Extend the `BUILTIN_CONTRACT_ARCHIVES` Array

Add the `v29` contract archive to the `BUILTIN_CONTRACT_ARCHIVES`:

```rust
static BUILTIN_CONTRACT_ARCHIVES: [(ProtocolVersionId, &[u8]); 4] = [
    (
        ProtocolVersionId::Version26,
        include_bytes!("contracts/builtin-contracts-v26.tar.gz"),
    ),
    (
        ProtocolVersionId::Version27,
        include_bytes!("contracts/builtin-contracts-v27.tar.gz"),
    ),
    (
        ProtocolVersionId::Version28,
        include_bytes!("contracts/builtin-contracts-v28.tar.gz"),
    ),
    (
        ProtocolVersionId::Version29,
        include_bytes!("contracts/builtin-contracts-v29.tar.gz"),
    ),
];
```

### 5.3 Add a Constant for V29

Declare a constant identifier for use in contract registration:

```rust
const V29: ProtocolVersionId = ProtocolVersionId::Version29;
```

### 5.4 Register the New Contract in `NON_KERNEL_CONTRACT_LOCATIONS` (or `BUILTIN_CONTRACT_LOCATIONS`)

If the new contract is a non-kernel system contract, add it to `NON_KERNEL_CONTRACT_LOCATIONS`:

```rust
pub static NON_KERNEL_CONTRACT_LOCATIONS: [(&str, Address, ProtocolVersionId); 9] = [
    // ... existing entries ...
    ("L2WrappedBaseToken", L2_WRAPPED_BASE_TOKEN_IMPL, V26),
    ("ChainAssetHandler", L2_CHAIN_ASSET_HANDLER_ADDRESS, V29),
];
```

> If the contract is kernel-level or categorized differently (e.g. a precompile), place it in the corresponding section within `BUILTIN_CONTRACT_LOCATIONS`.

### 5.5 Verify `get_deployed_contracts` Loads the Correct Contracts

The static `BUILTIN_CONTRACTS` map is automatically constructed from the entries in `BUILTIN_CONTRACT_LOCATIONS` and `NON_KERNEL_CONTRACT_LOCATIONS`. If the mappings above are correct, the logic will automatically pick up your new contract when bootstrapping a chain with `--protocol-version 29`.

### 5.6 Add a Unit Test

Consider duplicating an existing protocol-specific test to ensure the correct number of contracts are loaded for v29:

```rust
#[test]
fn load_v29_contracts() {
    let contracts = get_deployed_contracts(
        SystemContractsOptions::BuiltIn,
        ProtocolVersionId::Version29,
        None,
    );
    assert_eq!(
        contracts.len(),
        count_protocol_contracts(ProtocolVersionId::Version29)
    );
}
```

### 6 Register the New Protocol Version in `fork.rs`

To enable forking against chains using the new protocol version, update the list of supported protocol versions in `crates/core/src/node/inner/fork.rs`.

#### Extend `SUPPORTED_VERSIONS`

Locate the `SUPPORTED_VERSIONS` constant inside the `SupportedProtocolVersions` implementation and add your new entry:

```rust
impl SupportedProtocolVersions {
    const SUPPORTED_VERSIONS: [ProtocolVersionId; 4] = [
        ProtocolVersionId::Version26,
        ProtocolVersionId::Version27,
        ProtocolVersionId::Version28,
        ProtocolVersionId::Version29,
    ];
}
```

> Update the array length (`4` in this case) accordingly to reflect the number of supported versions.

## 7. Update Tests

With support for the new protocol version in place, add test coverage to ensure it is fully integrated and behaves as expected.

### 7.1 Add Protocol Version Test in `protocol.rs`

Navigate to `e2e-tests-rust/tests/protocol.rs` and add a dedicated test to verify that a node launched with the new protocol version (e.g. v29) reports it correctly via `eth_getBlockByNumber` or `zks_getBlockDetails`.

Example:

```rust
#[tokio::test]
async fn protocol_v29_on_demand() -> anyhow::Result<()> {
    let tester = AnvilZksyncTesterBuilder::default()
        .with_node_fn(&|node| node.args(["--protocol-version", "29"]))
        .build()
        .await?;

    let receipt = tester.tx().finalize().await?;
    let block_number = receipt.block_number().unwrap();

    let block_details = tester
        .l2_provider()
        .get_block_details(block_number)
        .await?
        .unwrap();

    assert_eq!(
        block_details.protocol_version,
        Some("Version29".to_string())
    );

    Ok(())
}
```

### 7.2 Update L1 Compatibility Test in `l1.rs`

In `e2e-tests-rust/tests/l1.rs`, bump the `#[test_casing]` macro to reflect the updated set of supported protocol versions:

```rust
#[test_casing(4, SUPPORTED_PROTOCOL_VERSIONS)]
#[tokio::test]
async fn l1_priority_tx(protocol_version: u16) -> anyhow::Result<()> {
    // ...
}
```

> Adjust the first argument (`4` in this case) to match the current number of supported versions.

This ensures your tests automatically run against all supported protocol configurations.

### 7.3 Run and Validate the Full Test Suite

After all test updates are complete, validate your changes across both unit and end-to-end tests.

```bash
# Run all Rust unit tests
make test

# Run Rust-based e2e tests (requires a running anvil-zksync instance)
cd e2e-tests-rust && cargo test

# Run full e2e suite (requires a running anvil-zksync instance)
make test-e2e
```

Confirm that all test suites pass successfully under the new protocol version to ensure correctness and regression safety.

## 8. Final Checklist

Use the following checklist to ensure all steps for adding a new protocol version (e.g. v29) have been completed:

* [ ] `era-contracts` fork updated with debug support
* [ ] Upstream dependencies patched
* [ ] New protocol feature added
* [ ] `refresh_contracts.sh` updated
* [ ] L1 setup (`l1-setup/setup.sh`) updated
* [ ] `l1-sidecar` integration completed
* [ ] `system_contracts.rs` updated
* [ ] Forking logic updated
* [ ] Tests written and updated
* [ ] Test suites executed and passing
* [ ] Documentation & Lint
