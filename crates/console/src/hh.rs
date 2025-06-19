//! Hardhat `console.sol` interface.
use alloy::sol;

sol!(
    #[sol(abi)]
    Console,
    "src/Console.json"
);

pub use Console::*;
