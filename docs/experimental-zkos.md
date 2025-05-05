# ZKOS (experimental)

ZKOS is the new backend for proving. The features below are still experimental and might break without warning.


Currently the code resides in `zkos-dev` branch (due to dependencies on private crates).


## Usage

```
cargo run  -- --use-zkos 
```

Afterwards, any regular forge script should work:

```
forge script script/Counter.s.sol --rpc-url http://localhost:8011 --private-key 0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6 --broadcast --slow -g 400
```

## Witness & proving

Anvil-zksync can also generate witnesses for batches, which can be then passed to the proving system.

To generate witness, you have to also provide the path to 'zk_os' app.bin file (which contains RISC_v binary).
It can be computed with the https://github.com/matter-labs/zk_ee/blob/main/zk_os/dump_bin.sh

Then you can run the anvil-zksync:

```
cargo run  -- --use-zkos --zkos-bin-path=../zk_ee/zk_os/app.bin 
```

And after sending some transactions, you can get the witness for the batch, by calling a new method:

```
http POST http://127.0.0.1:8011 \
    Content-Type:application/json \
    id:=1 jsonrpc="2.0" method="zkos_getWitness" params:='[1]'
```

The resulting data can be passed to tools like the prover cli: https://github.com/matter-labs/air_compiler/tree/main/tools/cli to generate the proof.

## Caveats

There is still no gas equivalency -- that's why this `-g` option that is increasing the gas limits.

Many things will not work yet, but basic things like deploying contracts & calling them should work.