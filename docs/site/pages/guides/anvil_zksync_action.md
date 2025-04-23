---
title: anvil-zksync-action
description: GitHub Action for running anvil-zksync in your CI workflows
layout: docs
---

# anvil-zksync Action 🚀

A lightweight GitHub Action that starts an
[anvil-zksync](https://github.com/matter-labs/anvil-zksync) node inside your workflow, so you can
run tests or scripts against a local ZK chain.

## Quickstart

```yaml
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Start anvil-zksync
        uses: dutterbutter/anvil-zksync-action@v1.1
        with:
          mode: run

      - name: Run tests
        run: |
          # e.g. point your tests at http://localhost:8011
          yarn test:contracts
```

## Example: Forking Mainnet

```yaml
- name: Start anvil-zksync in fork mode
  uses: dutterbutter/anvil-zksync-action@v1.1
  with:
    mode: fork
    forkUrl: mainnet
    forkBlockNumber: 59_485_781
    port: 8015
```

## Inputs

| Input             | Description                                      | Default     |
| ----------------- | ------------------------------------------------ | ----------- |
| `mode`            | Operation mode: `run` or `fork`                  | `run`       |
| `forkUrl`         | JSON-RPC endpoint to fork from (requires `fork`) | —           |
| `forkBlockNumber` | Block number to snapshot (fork mode only)        | —           |
| `port`            | Port for the node’s RPC server                   | `8011`      |
| `releaseTag`      | Release tag to download from                     | `latest`    |
| `host`            | Host address to bind                             | `127.0.0.1` |
| `chainId`         | Chain ID exposed by the node                     | `260`       |
| `logLevel`        | Verbosity: `debug`, `info`, `warn`, `error`      | `info`      |
| `logFile`         | File path to write logs (if set, action waits)   | —           |

For an exhausted list of available inputs, see the
[anvil-zksync-action](https://github.com/dutterbutter/anvil-zksync-action?tab=readme-ov-file#inputs-)
readme.

## Uploading Logs

```yaml
- name: Upload anvil logs
  uses: actions/upload-artifact@v4
  with:
    name: anvil-zksync-log
    path: anvil-zksync.log
```

For more options and detailed CLI flags, see the [anvil-zksync CLI reference](/docs/cli/index.mdx).
