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

### Call
We don't have support for 'call' in the bootloader:
* should ignore gas price
* should not check signature
* should not check if can pay.

had to do a lot of hacks with not-requiring payments (and refunds) during 'call' execution.
[TODO] - Solution from Anton



### Calling the contract..
[FIXED] Must know the preimage source - how do we add stuff to pre-image source??





## Testing

cast send -r http://localhost:8011 0x8B31b1F39Cc7dD799405E232327dcf0e71909020 --value 1 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 --gas-limit 10000000 && cast balance -r http://localhost:8011 0x8B31b1F39Cc7dD799405E232327dcf0e71909020


orge create --gas-limit 30000000 --private-key 0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6 --rpc-url http://localhost:8011 Counter
('fails' - but stuff actually gets deployed).


cast call -r http://localhost:8011 0x700b6a60ce7eaaea56f065753d8dcb9653dbad35 "number()"

cast send -r http://localhost:8011 0x700b6a60ce7eaaea56f065753d8dcb9653dbad35 "setNumber(uint256)" 11 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 --gas-limit 10000000 

-- without setting gas limit fails -- so estimation fails. FIX.