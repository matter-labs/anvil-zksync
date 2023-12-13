This directory was copied from https://github.com/matter-labs/era-system-contracts.

The current commit: `a00ab9a11643f3a918ed95cdf8a04edff5499d92`.

The following directories/files were copied:
- [bootloader](bootloader)
- [contracts](contracts)
- [scripts](scripts)
- [hardhat.config.ts](hardhat.config.ts)
- [package.json](package.json)
- [SystemConfig.json](SystemConfig.json)
- [yarn.lock](yarn.lock)

The next changes were introduced:
- [bootloader.yul](bootloader/bootloader.yul)
  - Debug data, marked as `DEBUG SUPPORT` blocks.
  - Impersonating preprocessing mode, blocks `<!-- @ifdef ACCOUNT_IMPERSONATING -->` and at some places added `<!-- @ifndef ACCOUNT_IMPERSONATING -->` condition.
- [process.ts](scripts/process.ts)
  - Impersonating preprocessing mode, "For impersonating" blocks.
- [DefaultAccount.sol](contracts/DefaultAccount.sol)
  - Return transaction data (empty), marked as `FOUNDRY SUPPORT` blocks.
- [DefaultAccountNoSecurity.sol](contracts/DefaultAccountNoSecurity.sol)
  - NEW smart contract, only for Hardhat/Forge testing.
- [IAccount.sol](contracts/interfaces/IAccount.sol)
  - Return transaction data (empty), marked as `FOUNDRY SUPPORT` blocks.
