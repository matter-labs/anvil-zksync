//! Format various types of transaction data in a human-readable format for
//! displaying in the anvil-zksync output.

pub mod address;
pub mod errors;
pub mod log;
pub mod pubdata_bytes;
pub mod transaction;
pub mod util;

use std::fmt::Write;

/// Trait for types that can be formatted in a human-readable way in the
/// anvil-zksync output log.
pub trait PrettyFmt: std::fmt::Debug {
    /// Formats the value in a pretty, colorful, and human-readable way, writing
    /// to the provided writer.
    fn pretty_fmt(&self, writer: &mut impl Write) -> std::fmt::Result;
}

/// Extension trait that provides a convenient method to get a pretty-formatted string.
///
/// This trait is automatically implemented for all types that implement `PrettyFmt`.
pub trait ToPrettyString {
    /// Converts the value to a pretty-formatted string.
    fn to_string_pretty(&self) -> String;
}

impl<T> ToPrettyString for T
where
    T: PrettyFmt,
{
    fn to_string_pretty(&self) -> String {
        let mut result = String::new();
        if let Err(e) = self.pretty_fmt(&mut result) {
            tracing::warn!(err = ?e, origin = ?self, "Error: Failed to pretty print.");
        }
        result
    }
}
