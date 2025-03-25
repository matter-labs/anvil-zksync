#!/bin/bash
set -xe

BUILTIN_CONTRACTS_OUTPUT_PATH=crates/core/src/deps/contracts/builtin-contracts-v27.tar.gz

# Forge JSON artifacts to be packed in the archive
L1_ARTIFACTS_SRC_DIR=contracts/l1-contracts/zkout
L2_ARTIFACTS_SRC_DIR=contracts/l2-contracts/zkout
SYSTEM_ARTIFACTS_SRC_DIR=contracts/system-contracts/zkout

l1_artifacts=("MessageRoot" "Bridgehub" "L2AssetRouter" "L2NativeTokenVault" "L2WrappedBaseToken")
l2_artifacts=("TimestampAsserter")
system_contracts_sol=(
  "AccountCodeStorage" "BootloaderUtilities" "Compressor" "ComplexUpgrader" "ContractDeployer" "DefaultAccount"
  "DefaultAccountNoSecurity" "EmptyContract" "ImmutableSimulator" "KnownCodesStorage" "L1Messenger" "L2BaseToken"
  "MsgValueSimulator" "NonceHolder" "SystemContext" "PubdataChunkPublisher" "Create2Factory" "L2GenesisUpgrade"
  "SloadContract"
)
system_contracts_yul=("EventWriter")
precompiles=("EcAdd" "EcMul" "Ecrecover" "Keccak256" "SHA256" "EcPairing" "CodeOracle" "P256Verify")
bootloaders=(
  "fee_estimate" "gas_test" "playground_batch" "proved_batch" "proved_batch_impersonating" "fee_estimate_impersonating"
)

for artifact in "${l1_artifacts[@]}"; do
  FILES="$FILES $L1_ARTIFACTS_SRC_DIR/$artifact.sol/$artifact.json"
done

for artifact in "${l2_artifacts[@]}"; do
  FILES="$FILES $L2_ARTIFACTS_SRC_DIR/$artifact.sol/$artifact.json"
done

for artifact in "${system_contracts_sol[@]}"; do
  FILES="$FILES $SYSTEM_ARTIFACTS_SRC_DIR/$artifact.sol/$artifact.json"
done

for artifact in "${system_contracts_yul[@]}"; do
  FILES="$FILES $SYSTEM_ARTIFACTS_SRC_DIR/$artifact.yul/$artifact.json"
done

for precompile in "${precompiles[@]}"; do
  FILES="$FILES $SYSTEM_ARTIFACTS_SRC_DIR/$precompile.yul/$precompile.json"
done

for bootloader in "${bootloaders[@]}"; do
  FILES="$FILES $SYSTEM_ARTIFACTS_SRC_DIR/$bootloader.yul/$bootloader.json"
done

# Make sure we are using GNU tar
case "$(uname -s)" in
    # Vast majority of Linux distributives have GNU tar installed
    Linux*)     GNU_TAR_BIN=tar;;
    # macOS comes with BSD tar but GNU tar is installable as gtar (available in Github runners by default)
    Darwin*)    GNU_TAR_BIN=gtar;;
    # Unknown, assuming `tar`
    *)          GNU_TAR_BIN=tar;;
esac

# Create reproducible GNU tar archives as per https://www.gnu.org/software/tar/manual/html_section/Reproducibility.html
cd contracts
SOURCE_EPOCH=$(TZ=UTC0 git log -1 \
  --format=tformat:%cd \
  --date=format:%Y-%m-%dT%H:%M:%SZ)
cd ..
TARFLAGS="
  --sort=name --format=posix
  --pax-option=exthdr.name=%d/PaxHeaders/%f
  --pax-option=delete=atime,delete=ctime
  --clamp-mtime --mtime=$SOURCE_EPOCH
  --numeric-owner --owner=0 --group=0
  --mode=go+u,go-w
"
GZIPFLAGS="--no-name --best"

LC_ALL=C $GNU_TAR_BIN $TARFLAGS -cvf - $FILES | gzip $GZIPFLAGS > $BUILTIN_CONTRACTS_OUTPUT_PATH
