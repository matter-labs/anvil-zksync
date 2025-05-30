//! Helper methods to display transaction data in more human readable way.
pub mod address;
pub mod errors;
pub mod log;
pub mod pubdata_bytes;
pub mod transaction;

use std::fmt::Write;

pub trait PrettyFmt {
    fn pretty_fmt(&self, writer: &mut impl Write) -> std::fmt::Result;
}

pub trait ToPrettyString {
    fn to_string_pretty(&self) -> Option<String>;
}

impl<T> ToPrettyString for T
where
    T: PrettyFmt + std::fmt::Debug,
{
    fn to_string_pretty(&self) -> Option<String> {
        let mut result = String::new();
        if let Err(e) = self.pretty_fmt(&mut result) {
            tracing::warn!(err = ?e, origin = ?self, "Error: Failed to pretty print.");
        }
        Some(result)
    }
}
