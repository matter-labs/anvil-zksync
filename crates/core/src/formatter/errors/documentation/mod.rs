//! Error documentation handling system for Anvil zksync.
//!
//! This module provides the framework for accessing, formatting, and displaying
//! comprehensive error documentation for various error types in the system.
//! It enables detailed error reporting with likely causes, fixes, and references.

pub mod view;

use std::fmt::Write;

use colored::Colorize as _;
use zksync_error::documentation::Documented;
use zksync_error_description::ErrorDocumentation;

/// Provides access to error documentation for any type.
///
/// This trait is a core part of the error handling system, allowing errors
/// to expose their associated documentation for improved error reporting.
pub trait AnvilErrorDocumentation: std::fmt::Debug {
    /// Retrieves the error documentation if available.
    ///
    /// Returns `None` if documentation isn't available for this error.
    fn get_documentation(&self) -> Option<&'static ErrorDocumentation>;
}

impl<T> AnvilErrorDocumentation for T
where
    T: Documented<Documentation = &'static ErrorDocumentation> + std::fmt::Debug,
{
    fn get_documentation(&self) -> Option<&'static ErrorDocumentation> {
        match Documented::get_documentation(self) {
            Ok(res) => res,
            Err(e) => {
                tracing::warn!(
                    error = ?e, "Failed to get error documentation. The documentation may be malformed."
                );
                None
            }
        }
    }
}

/// Formats the likely causes section of error documentation.
///
/// Writes a formatted list of likely causes for the error, if any exist.
fn format_likely_causes(doc: &ErrorDocumentation, w: &mut impl Write) -> std::fmt::Result {
    if !doc.likely_causes.is_empty() {
        writeln!(w, "    | {}", "Likely causes:".cyan())?;
        for cause in doc.likely_causes.iter().map(|descr| &descr.cause) {
            writeln!(w, "    |   - {cause}")?;
        }
    }
    Ok(())
}

/// Formats the possible fixes section of error documentation.
///
/// Writes a formatted list of possible fixes for the error, if any exist.
fn format_fixes(doc: &ErrorDocumentation, w: &mut impl Write) -> std::fmt::Result {
    let has_fixes = doc
        .likely_causes
        .iter()
        .any(|cause| !cause.fixes.is_empty());
    if has_fixes {
        writeln!(w, "    | ")?;
        writeln!(w, "    | {}", "Possible fixes:".green().bold())?;
        for fix in doc.likely_causes.iter().flat_map(|cause| &cause.fixes) {
            writeln!(w, "    |   - {fix}")?;
        }
    }
    Ok(())
}

/// Formats the references section of error documentation.
///
/// Writes a formatted list of reference links for further information.
fn format_references(doc: &ErrorDocumentation, w: &mut impl Write) -> std::fmt::Result {
    let has_references = doc
        .likely_causes
        .iter()
        .any(|cause| !cause.references.is_empty());
    if has_references {
        writeln!(
            w,
            "\n{} ",
            "For more information about this error, visit:"
                .cyan()
                .bold()
        )?;
        for reference in doc.likely_causes.iter().flat_map(|cause| &cause.references) {
            writeln!(w, "  - {}", reference.underline())?;
        }
    }
    Ok(())
}

/// Formats the error summary section.
///
/// Writes a concise summary of the error with appropriate styling.
fn format_summary(doc: &ErrorDocumentation, w: &mut impl Write) -> std::fmt::Result {
    writeln!(w, "    = {} {}", "error:".bright_red(), &doc.summary)
}

/// Formats the detailed error description.
///
/// Writes the full description of the error with appropriate styling.
fn format_description(doc: &ErrorDocumentation, w: &mut impl Write) -> std::fmt::Result {
    writeln!(w, "{} {}", "note:".blue(), doc.description)
}
