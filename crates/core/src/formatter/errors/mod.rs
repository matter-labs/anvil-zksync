//! Error formatting utilities for Anvil zksync.
//!
//! This module provides tools for formatting and displaying detailed error messages,
//! including both basic error messages and comprehensive error documentation.

pub mod documentation;
pub mod view;

use std::fmt::Write;

use colored::Colorize as _;
use zksync_error::CustomErrorMessage;

/// Formats a simple error message with standard styling.
///
/// Takes any type that implements `CustomErrorMessage` and writes a formatted
/// error message to the provided writer.
pub fn format_message(error: &impl CustomErrorMessage, w: &mut impl Write) -> std::fmt::Result {
    let error_msg = error.get_message();
    write!(w, "{}: {}", "error".red().bold(), error_msg.red())
}
