# Era Test Node E2E Tests project

This project is used for running e2e tests against `era_test_node`

## Project structure

- `/contracts`: smart contracts.
- `/test`: test files
- `hardhat.config.ts`: configuration file.

## Commands

- `yarn build` will compile the contracts.
- `yarn test`: will run all e2e tests.
- `yarn fmt:fix`: will format the code using `Prettier` default formatting rules.
- `yarn dev:start` will launch a locally built node
- `yarn dev:kill` will kill all `era_test_node` on machine.

> [!WARNING]\
> Not safe to run `dev:kill` on CI workflows in case the runner has poor separation between workers.