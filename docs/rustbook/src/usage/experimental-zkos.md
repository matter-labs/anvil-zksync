# ZKOS (experimental)

ZKOS is the new backend for proving. The features below are still experimental and might break without warning.


## Usage

```
RUSTUP_TOOLCHAIN=nightly-2024-11-12 cargo run  --features zkos -- --chain-id 31337
```

Afterwards, any regular forge script should work:

```
forge script script/Counter.s.sol --rpc-url http://localhost:8011 --private-key 0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6 --broadcast --slow -g 400
```

## Caveats

There is still no gas equivalency -- that's why this `-g` option that is increasing the gas limits.

Currently chain_id is hardcoded inside zkos to 31337, soon it will be passed as argument.

Many things will not work yet, but basic things like deploying contracts & calling them should work.