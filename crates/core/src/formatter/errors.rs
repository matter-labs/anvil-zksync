use std::fmt::Write;

use colored::Colorize as _;
use zksync_error::documentation::Documented;
use zksync_error_description::ErrorDocumentation;



/// Formats the likely causes of an error with a styled header and bullet points.
///
/// # Arguments
///
/// * `doc` - The error documentation containing likely causes
/// * `f` - A mutable formatter to write the formatted output
///
/// # Returns
///
/// A `std::fmt::Result` indicating success or failure of the formatting operation
///
/// # Example Output
///
/// ```text
///     | Likely causes:
///     |   - Insufficient funds to cover transaction costs
///     |   - Incorrect transaction fee configuration
/// ```
fn format_likely_causes(doc: &ErrorDocumentation, f: &mut impl Write) -> std::fmt::Result {
    if !doc.likely_causes.is_empty() {
        writeln!(f, "    | {}", "Likely causes:".cyan())?;
        for cause in doc.likely_causes.iter().map(|descr| &descr.cause) {
            writeln!(f, "    |   - {cause}")?;
        }
    }
    Ok(())
}

/// Formats possible fixes for an error with a styled header and bullet points.
///
/// This function only produces output if at least one fix exists among all causes.
///
/// # Arguments
///
/// * `doc` - The error documentation containing possible fixes
/// * `f` - A mutable formatter to write the formatted output
///
/// # Returns
///
/// A `std::fmt::Result` indicating success or failure of the formatting operation
///
/// # Example Output
///
/// ```text
///     |
///     | Possible fixes:
///     |   - Ensure the account balance is sufficient to cover the fee
///     |   - Verify that the maxFeePerGas and gasLimit values are correctly set
/// ```
fn format_fixes(doc: &ErrorDocumentation, f: &mut impl Write) -> std::fmt::Result {
    let has_fixes = doc
        .likely_causes
        .iter()
        .any(|cause| !cause.fixes.is_empty());
    if has_fixes {
        writeln!(f, "    | ")?;
        writeln!(f, "    | {}", "Possible fixes:".green().bold())?;
        for fix in doc.likely_causes.iter().flat_map(|cause| &cause.fixes) {
            writeln!(f, "    |   - {fix}")?;
        }
    }
    Ok(())
}

/// Formats reference links related to an error with a styled header and underlined links.
///
/// This function only produces output if at least one reference exists among all causes.
///
/// # Arguments
///
/// * `doc` - The error documentation containing reference links
/// * `f` - A mutable formatter to write the formatted output
///
/// # Returns
///
/// A `std::fmt::Result` indicating success or failure of the formatting operation
///
/// # Example Output
///
/// ```text
///
/// For more information about this error, visit:
///   - https://docs.zksync.io/error/ANVIL-05-01
/// ```
fn format_references(doc: &ErrorDocumentation, f: &mut impl Write) -> std::fmt::Result {
    let has_references = doc
        .likely_causes
        .iter()
        .any(|cause| !cause.references.is_empty());
    if has_references {
        writeln!(
            f,
            "\n{} ",
            "For more information about this error, visit:"
                .cyan()
                .bold()
        )?;
        for reference in doc.likely_causes.iter().flat_map(|cause| &cause.references) {
            writeln!(f, "  - {}", reference.underline())?;
        }
    }
    Ok(())
}

/// Formats the error summary with a styled error prefix.
///
/// # Arguments
///
/// * `doc` - The error documentation containing the summary
/// * `f` - A mutable formatter to write the formatted output
///
/// # Returns
///
/// A `std::fmt::Result` indicating success or failure of the formatting operation
///
/// # Example Output
///
/// ```text
///     = error: Transaction has published 1024 bytes which exceeds limit for published pubdata (500)
/// ```
pub fn format_summary(doc: &ErrorDocumentation, f: &mut impl Write) -> std::fmt::Result {
    writeln!(f, "    = {} {}", "error:".bright_red(), &doc.summary)?;
    Ok(())
}

/// Formats all additional error information, including likely causes, possible fixes, and references.
///
/// This function combines multiple formatters to create a comprehensive error description
/// with appropriate spacing and styling.
///
/// # Arguments
///
/// * `doc` - The error documentation containing all error details
/// * `f` - A mutable formatter to write the formatted output
///
/// # Returns
///
/// A `std::fmt::Result` indicating success or failure of the formatting operation
pub fn format_additional(doc: &ErrorDocumentation, f: &mut impl Write) -> std::fmt::Result {
    if !doc.likely_causes.is_empty() {
        writeln!(f, "    | ")?;
        format_likely_causes(doc, f)?;
        writeln!(f, "    | ")?;
        format_fixes(doc, f)?;
        writeln!(f, "    | ")?;
        format_references(doc, f)?;
        writeln!(f, "    |")?;
    }
    writeln!(f, "{} {}", "note:".blue(), doc.description)?;

    Ok(())
}

/// Formats a complete error documentation for any error type that implements the `Documented` trait.
///
/// This function attempts to retrieve the error documentation and format it using
/// `format_summary` and `format_additional`. If documentation retrieval fails,
/// the error is logged but no output is produced.
///
/// # Arguments
///
/// * `error` - Any error type that implements the `Documented` trait
/// * `f` - A mutable formatter to write the formatted output
///
/// # Returns
///
/// A `std::fmt::Result` indicating success or failure of the formatting operation
///
/// # Type Parameters
///
/// * `E` - The error type, which must implement `Documented<Documentation = &'static ErrorDocumentation>`
pub fn format_complete_docs<E>(error: &E, f: &mut impl Write) -> std::fmt::Result
where
    E: Documented<Documentation = &'static ErrorDocumentation>,
{
    let doc = match error.get_documentation() {
        Ok(opt) => opt,
        Err(e) => {
            tracing::info!("Failed to get error documentation: {}", e);
            None
        }
    };

    if let Some(doc) = doc {
        format_summary(doc, f)?;
        format_additional(doc, f)?;
    }
    Ok(())
}
