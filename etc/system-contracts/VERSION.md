This directory was copied from https://github.com/matter-labs/era-contracts/tree/release-v20-1.4.1-short-term-fee-model/system-contracts.

The current repository commit: `2dfbc6bac84ecada93cab4a0dea113bc2aceba1c`.

The following directories/files were copied:
- [bootloader](bootloader)
- [contracts](contracts)
- [scripts](scripts)
- [hardhat.config.ts](hardhat.config.ts)
- [package.json](package.json)
- [SystemConfig.json](SystemConfig.json) **NOTE: was copied from the repository root.**

The next changes were introduced:
- [bootloader.yul](bootloader/bootloader.yul)
  - Debug data, marked as `DEBUG SUPPORT` blocks.
  - Impersonating preprocessing mode, blocks `<!-- @ifdef ACCOUNT_IMPERSONATING -->` and at some places added `<!-- @ifndef ACCOUNT_IMPERSONATING -->` condition.
- [preprocess-bootloader.ts](scripts/preprocess-bootloader.ts)
  - Impersonating preprocessing mode, "For impersonating" blocks.
  - System config path, "TEST NODE CHANGE" block.
- [DefaultAccount.sol](contracts/DefaultAccount.sol)
  - Return transaction data (empty), marked as `FOUNDRY SUPPORT` blocks.
- [DefaultAccountNoSecurity.sol](contracts/DefaultAccountNoSecurity.sol)
  - NEW smart contract, only for Hardhat/Forge testing.
- [IAccount.sol](contracts/interfaces/IAccount.sol)
  - Return transaction data (empty), marked as `FOUNDRY SUPPORT` blocks.
