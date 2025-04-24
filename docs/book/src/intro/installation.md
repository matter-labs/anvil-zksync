# Installation

You have the following ways to get up and running with **anvil-zksync**:

1. **Homebrew** (recommended)
2. **Installation script** (bundles `forge`, `cast`, and `anvil-zksync` from `foundry-zksync`)
3. **Pre-built binaries** (download & extract)
4. **Docker**
5. **Build from source** (requires Rust/Cargo)

## 1. Homebrew (recommended)

If you're on macOS or Linux, the easiest and most up-to-date way is via Homebrew:

```bash
# Tap the official anvil-zksync formula
brew tap matter-labs/anvil-zksync https://github.com/matter-labs/anvil-zksync.git 
brew install anvil-zksync

# Install
brew install anvil-zksync

# To update later:
brew update
brew upgrade anvil-zksync
```

## 2. Installation script

The easiest path: grab and run the `foundryup-zksync` installer.

```bash
curl -L https://raw.githubusercontent.com/matter-labs/foundry-zksync/main/install-foundry-zksync \
  | bash
```

>  This installs `forge`, `cast`, **and** `anvil-zksync` into your `PATH`. **Note:** This will
>  override existing `foundry` installations.

Then start your local ZK chain node:

```bash
anvil-zksync
```

## 3. Pre-built binaries

1. Download the latest release for your platform from  
   [GitHub Releases](https://github.com/matter-labs/anvil-zksync/releases/latest).
2. Extract and install:

   ```bash
   tar xzf anvil-zksync-<version>-<os>.tar.gz -C /usr/local/bin/
   chmod +x /usr/local/bin/anvil-zksync
   ```

3. Verify and run:

   ```bash
   anvil-zksync --version
   anvil-zksync
   ```

### Convenience download links

These URLs always fetch the **latest** published build:

- [`anvil-zksync-aarch64-apple-darwin.tar.gz`](https://github.com/matter-labs/anvil-zksync/releases/latest/download/anvil-zksync-v0.5.1-aarch64-apple-darwin.tar.gz)
- [`anvil-zksync-aarch64-unknown-linux-gnu.tar.gz`](https://github.com/matter-labs/anvil-zksync/releases/latest/download/anvil-zksync-v0.5.1-aarch64-unknown-linux-gnu.tar.gz)
- [`anvil-zksync-x86_64-apple-darwin.tar.gz`](https://github.com/matter-labs/anvil-zksync/releases/latest/download/anvil-zksync-v0.5.1-x86_64-apple-darwin.tar.gz)
- [`anvil-zksync-x86_64-unknown-linux-gnu.tar.gz`](https://github.com/matter-labs/anvil-zksync/releases/latest/download/anvil-zksync-v0.5.1-x86_64-unknown-linux-gnu.tar.gz)

## 4. Docker

`anvil-zksync` is available as Docker image that supports Linux x64 and arm64.

```bash
docker pull ghcr.io/matter-labs/anvil-zksync:v0.5.1
```

**Run it (interactive):**

```bash
docker run --rm -it ghcr.io/matter-labs/anvil-zksync:v0.5.1 \
  anvil-zksync
```

## 5. Build from source

Requires Rust & Cargo installed.

```bash
gh repo clone matter-labs/anvil-zksync
cd anvil-zksync
cargo build --release
```

The `anvil-zksync` binary will be at `target/release/`. To make it globally available:

```bash
cp target/release/anvil-zksync /usr/local/bin/
```

## Checking installation

After installation, verify that **anvil-zksync** is available:

```bash
anvil-zksync --version
# 0.5.1
```

With any method above, you can immediately begin:

```bash
# Launch default (equivalent to `run`)
anvil-zksync

# Or fork with tracing enabled:
anvil-zksync -vv fork --fork-url mainnet
```

If you installed via script or binaries and see a “command not found” error, you may need to add the
install directory to your PATH.

## Adding to your `PATH`

First, determine what shell you're using:

```bash
echo $SHELL
```

Then add these lines below to bottom of your shell's configuration file.

```bash
export ANVIL_ZKSYNC_HOME="$HOME/.anvil-zksync"
export PATH="$ANVIL_ZKSYNC_HOME/bin:$PATH"
```

Save the file. You'll need to open a new shell/terminal window for the changes to take effect.

## Upgrading

- **Homebrew**: `brew upgrade anvil-zksync`
- **Installation script**: re-run the install script to pull the latest
- **Docker**: `docker pull ghcr.io/matter-labs/anvil-zksync:latest`
- **Source build**: pull latest, `cargo build --release`, then replace the binary

## Uninstall

### Homebrew

```bash
brew uninstall anvil-zksync
```

### Script

```bash
rm -f ~/.foundry/bin/anvil-zksync
```

### Docker

```bash
docker rmi ghcr.io/matter-labs/anvil-zksync
```

### Binaries

```bash
rm /usr/local/bin/anvil-zksync
```
