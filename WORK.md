[x] - Apply stuff to storage
[x] - actually udpate the storage slot for load_last_l2_block (as currently it crashes)
[x] - update rich accounts correctly.
[x] - basic transfers
[ ] - deploying & calling a solidity contract
[ ] - cross contract calls
[ ] - deploying WASM contract




## Issues:

### Balances address
zk_ee balance is in 0x8009, in 'direct' account key.
in era -it is in 0x800a - in 'hashed' account key..


### Failing when keys updated < 3

Fails in verify & apply batch


### Nonce addressing
Same issue as balances address -- hashing in key.

### 'null' address
had to switch the bool - to say it is enabled.

Also seems that we're treating 'null' and all-zeros in the same way - seems that deployment doens't really work.



When contract is deployed - how do we pass the address to the output??

* not great info when we run out of gas.
