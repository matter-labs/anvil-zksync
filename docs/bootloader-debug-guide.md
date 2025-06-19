# Bootloader Debug Guide

Let's assume you have a mainnet transaction that you would like to investigate in-depth. It could be halting, reverting without apparent reason, taking too much gas etc. You have tried looking at the trace, but it is not enough to figure out what the problem is. With `anvil-zksync` you can replay that transaction locally and reliably* reproduce the execution flow.

\* with some caveats, like slight mismatch in gas consumption due to the difference in touched slots  

Replaying a transaction locally means we have access to low-level ZK EVM and hence can in theory debug it step by step. That being said, this is a time-consuming process and is overkill for most scenarios. Instead, you can debug bootloader by observing (and adding your own) debug prints. Bootloader exposes a system hook that allows to log arbitrary strings:

```
/// @dev This method accepts the message and some 1-word data associated with it
/// It triggers a VM hook that allows the server to observe the behavior of the system.
function debugLog(msg, data) {
    storeVmHookParam(0, msg)
    storeVmHookParam(1, data)
    setHook(VM_HOOK_DEBUG_LOG())
}
```

This hook is used by `zksync_multivm` crate to propagate the logs into `tracing`:

```rust
/// Accepts a vm hook and, if it requires to output some debug log, outputs it.
pub(crate) fn print_debug_if_needed<H: HistoryMode>(
    hook: &VmHook,
    state: &VmLocalStateData<'_>,
    memory: &SimpleMemory<H>,
) {
    let log = match hook {
        VmHook::DebugLog => get_debug_log(state, memory),
        VmHook::DebugReturnData => get_debug_returndata(memory),
        _ => return,
    };

    tracing::trace!("{}", log);
}
```

If you run `anvil-zksync` with `RUST_LOG=zksync_multivm=trace` you will see these logs being printed to your console:

```
$ RUST_LOG=zksync_multivm=trace anvil-zksync -vv replay_tx --fork-url https://genlayer-testnet.rpc.caldera.xyz/http 0x3ef55f45c8ad0580604aaff5371fc669aaef3b518b4b70599bf4c4222fd86aa6
...
TRACE Bootloader transaction 0: txPtr: 52056224
TRACE Bootloader transaction 0: execute: 1
TRACE Bootloader transaction 0: ethCall: 0
TRACE Bootloader transaction 0: Setting new L2 block: : 20520
TRACE Bootloader transaction 0: Setting new L2 block: : 1743956487
TRACE Bootloader transaction 0: Setting new L2 block: : 0x9f60ca85c6a78283453674a4809f59d40c4bd6584cf54c91e80b1cfe8e7f039a
TRACE Bootloader transaction 0: Setting new L2 block: : 1
...
```

Now, lets assume this is not enough, and you want to instrument some part of bootloader yourself. To do this you can open `contracts/system-contracts/bootloader/bootloader.yul` and modify it as you see fit. For the purposes of this demonstration I will add a new log at the very start of the transaction processing:

```
////////////////////////////////////////////////////////////////////////////
//                      Main Transaction Processing
////////////////////////////////////////////////////////////////////////////

debugLog("hello world")
```

Now rebuild bootloader:

```
$ cd contracts/system-contracts
$ yarn install --frozen-lockfile
$ yarn build:foundry
```

Finally, run `anvil-zksync` with your local version of contracts: 

```
$ RUST_LOG=zksync_multivm=trace anvil-zksync --dev-system-contracts local --system-contracts-path $YOUR_PATH/contracts/system-contracts -vv replay_tx --fork-url https://genlayer-testnet.rpc.caldera.xyz/http 0x3ef55f45c8ad0580604aaff5371fc669aaef3b518b4b70599bf4c4222fd86aa6
...
TRACE Bootloader transaction 0: hello world
...
```
