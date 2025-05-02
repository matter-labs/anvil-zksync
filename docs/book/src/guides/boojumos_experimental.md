# ZKOS (experimental)

ZKOS is the new backend for proving in the Elastic chain ecosystem.  
This feature is experimental and may change or break without warning.

Currently, support resides on the `zkos-dev` branch due to dependencies on private crates.

## Usage

Start `anvil-zksync` with ZKOS enabled:

```bash
anvil-zksync --use-zkos
```

After the node is running, any standard Forge script should work:

```bash
forge script script/Counter.s.sol \
  --rpc-url http://localhost:8011 \
  --private-key 0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6 \
  --broadcast --slow -g 400
```

The `-g` flag is currently needed to raise gas limits, as ZKOS mode does not yet use calibrated gas costs.

## Witness Generation and Proving

`anvil-zksync` can generate execution witnesses for individual batches, which can then be passed to a proving system.

### 1. Build the zkOS RISC-V binary

Obtain the binary file `app.bin` using the build script from the [`zk_ee`](https://github.com/matter-labs/zk_ee) repository:

```bash
./zk_os/dump_bin.sh
```

This will output a RISC-V binary located at `zk_os/app.bin`.

### 2. Run anvil-zksync with zkOS binary

Pass the path to `app.bin` when launching:

```bash
anvil-zksync --use-zkos --zkos-bin-path=../zk_ee/zk_os/app.bin
```

### 3. Generate a witness

After sending one or more transactions, you can request the execution witness for a specific batch using the `zkos_getWitness` RPC method:

```bash
curl -X POST http://127.0.0.1:8011 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"zkos_getWitness","params":[1]}'
```

This returns witness data for batch number `1`, which can be passed into downstream proving pipelines.

### 4. Prove the batch

The resulting witness can be used with the [`air_compiler`](https://github.com/matter-labs/air_compiler/tree/main/tools/cli) CLI to generate a zkSNARK proof.

## Caveats

- Gas equivalence is not yet implemented. Use the `-g` flag with Forge to raise gas limits when deploying or interacting with contracts.
- This feature is in early development; many components of the stack may not yet function as expected.
- Basic interactions such as deploying and calling contracts are expected to work.
