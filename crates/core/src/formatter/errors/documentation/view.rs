//! View components for displaying error documentation.
//!
//! This module provides various view wrappers for error documentation,
//! enabling the customizable display of different parts of error information
//! such as summary, causes, fixes, and detailed descriptions.

use std::fmt::Write;

use colored::Colorize as _;

use crate::formatter::PrettyFmt;

use super::{
    format_description, format_fixes, format_likely_causes, format_references, format_summary,
    AnvilErrorDocumentation,
};

/// Displays the complete documentation for an error.
///
/// This view wraps an error and provides a fully-formatted display of all
/// available documentation, including summary, causes, fixes, references,
/// and detailed description.
pub struct FullDocumentation<'a, E>(pub &'a E)
where
    E: AnvilErrorDocumentation;

impl<E> PrettyFmt for FullDocumentation<'_, E>
where
    E: AnvilErrorDocumentation,
{
    fn pretty_fmt(&self, w: &mut impl Write) -> std::fmt::Result {
        if let Some(doc) = self.0.get_documentation() {
            format_summary(doc, w)?;
            if !doc.likely_causes.is_empty() {
                writeln!(w, "    | ")?;
                format_likely_causes(doc, w)?;
                writeln!(w, "    | ")?;
                format_fixes(doc, w)?;
                writeln!(w, "    | ")?;
                format_references(doc, w)?;
                writeln!(w, "    |")?;
            }

            format_description(doc, w)?;
        }
        Ok(())
    }
}

impl<E> std::fmt::Display for FullDocumentation<'_, E>
where
    E: AnvilErrorDocumentation,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_fmt(f)
    }
}

/// Displays only the summary portion of error documentation.
///
/// This view provides a concise display showing just the error summary,
/// which is useful for brief error notifications.
pub struct SummaryView<'a, E>(pub &'a E)
where
    E: AnvilErrorDocumentation;

impl<E> PrettyFmt for SummaryView<'_, E>
where
    E: AnvilErrorDocumentation,
{
    fn pretty_fmt(&self, w: &mut impl Write) -> std::fmt::Result {
        if let Some(doc) = self.0.get_documentation() {
            format_summary(doc, w)?;
        } else {
            writeln!(
                w,
                "    = {error} An unknown error occurred",
                error = "error:".bright_red()
            )?;
        }
        Ok(())
    }
}

impl<E> std::fmt::Display for SummaryView<'_, E>
where
    E: AnvilErrorDocumentation,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_fmt(f)
    }
}

/// Displays the causes, fixes, and references sections of error documentation.
///
/// This view focuses on the actionable information about an error,
/// showing its likely causes, suggested fixes, and reference links.
pub struct CausesView<'a, E>(pub &'a E)
where
    E: AnvilErrorDocumentation;

impl<E> std::fmt::Display for CausesView<'_, E>
where
    E: AnvilErrorDocumentation,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_fmt(f)
    }
}

impl<E> PrettyFmt for CausesView<'_, E>
where
    E: AnvilErrorDocumentation,
{
    fn pretty_fmt(&self, w: &mut impl Write) -> std::fmt::Result {
        if let Some(doc) = self.0.get_documentation() {
            if !doc.likely_causes.is_empty() {
                writeln!(w, "    | ")?;
                format_likely_causes(doc, w)?;
                writeln!(w, "    | ")?;
                format_fixes(doc, w)?;
                writeln!(w, "    | ")?;
                format_references(doc, w)?;
                writeln!(w, "    |")?;
            }
        }
        Ok(())
    }
}

/// Displays only the detailed description of error documentation.
///
/// This view shows the full technical explanation of an error,
/// which is useful for advanced troubleshooting.
pub struct DescriptionView<'a, E>(pub &'a E)
where
    E: AnvilErrorDocumentation;

impl<E> PrettyFmt for DescriptionView<'_, E>
where
    E: AnvilErrorDocumentation,
{
    fn pretty_fmt(&self, w: &mut impl Write) -> std::fmt::Result {
        if let Some(doc) = self.0.get_documentation() {
            format_description(doc, w)?;
        }
        Ok(())
    }
}

impl<E> std::fmt::Display for DescriptionView<'_, E>
where
    E: AnvilErrorDocumentation,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_fmt(f)
    }
}
