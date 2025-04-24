//!
//! Routines to help with node diagnostics, parsing, extracting information from
//! different subsystems. The `[crate::formatter]` module is used to output it.
//!

use zksync_multivm::interface::storage::ReadStorage;
use zksync_types::Address;

pub mod transaction;
pub mod vm;

pub fn account_has_code(address: Address, fork_storage: &mut impl ReadStorage) -> bool {
    let code_key = zksync_types::get_code_key(&address);
    let bytecode_hash =
        zksync_multivm::interface::storage::ReadStorage::read_value(fork_storage, &code_key);

    !bytecode_hash.is_zero()
}
