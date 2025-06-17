#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(allocator_api)]
#![feature(array_chunks)]
#![feature(get_mut_unchecked)]
#![feature(const_type_id)]
#![feature(vec_push_within_capacity)]
#![feature(ptr_alignment_type)]
#![feature(btreemap_alloc)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(ptr_metadata)]
#![feature(alloc_layout_extra)]
#![feature(array_windows)]
#![feature(btree_cursors)]
#![feature(slice_from_ptr_range)]
#![feature(const_trait_impl)]
#![allow(clippy::new_without_default)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::double_must_use)]
#![allow(clippy::explicit_auto_deref)]

pub mod api;
mod l2_tx_builder;
mod log;
pub mod numbers;
mod serde_helpers;
mod show_details;
pub mod traces;
mod transaction_order;

pub use self::{
    l2_tx_builder::L2TxBuilder,
    log::LogLevel,
    serde_helpers::Numeric,
    show_details::{ShowGasDetails, ShowStorageLogs, ShowVMDetails},
    transaction_order::{TransactionOrder, TransactionPriority},
};
