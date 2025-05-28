//! View components for displaying formatted error reports.
//!
//! This module provides specialized view structs for rendering different types
//! of error reports with appropriate formatting and context information.

use std::fmt::Write;
use std::fmt::Debug;

use colored::Colorize as _;
use zksync_error::CustomErrorMessage;
use zksync_types::Transaction;

use crate::formatter::transaction::view::PrettyTransactionEstimationView;
use crate::formatter::{transaction::view::PrettyTransaction, PrettyFmt};

use super::documentation::{
    view::{CausesView, DescriptionView, SummaryView},
    AnvilErrorDocumentation,
};

/// Displays a basic error message with standard styling.
///
/// This view wraps any type that implements `CustomErrorMessage` and 
/// renders its error message with appropriate styling.
pub struct ErrorMessageView<'a, E>(pub &'a E)
where
    E: CustomErrorMessage;

impl<'a, E> PrettyFmt for ErrorMessageView<'a, E>
where
    E: CustomErrorMessage,
{
    fn pretty_fmt(&self, w: &mut impl Write) -> std::fmt::Result {
        writeln!(
            w,
            "{}: {}",
            "error".red().bold(),
            self.0.get_message().red()
        )?;
        Ok(())
    }
}

impl<'a, E> std::fmt::Display for ErrorMessageView<'a, E>
where
    E: CustomErrorMessage,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_fmt(f)
    }
}

/// Comprehensive error report for transaction execution failures.
///
/// Combines error details with transaction context to provide a complete
/// picture of what went wrong during transaction execution.
pub struct ExecutionErrorReport<'a, E>
where
    E: AnvilErrorDocumentation + CustomErrorMessage + Debug,
{
    /// The error that occurred during execution
    pub error: &'a E,
    /// The transaction that failed
    pub tx: &'a Transaction,
}

impl<'a, E> ExecutionErrorReport<'a, E>
where
    E: AnvilErrorDocumentation + CustomErrorMessage + Debug,
{
    /// Creates a new execution error report with the given error and transaction.
    pub fn new(error: &'a E, tx: &'a Transaction) -> Self {
        Self { error, tx }
    }
}

impl<'a, E> PrettyFmt for ExecutionErrorReport<'a, E>
where
    E: AnvilErrorDocumentation + CustomErrorMessage + Debug,
{
    fn pretty_fmt(&self, w: &mut impl Write) -> std::fmt::Result {
        write!(w, "{}", ErrorMessageView(self.error))?;
        write!(w, "{}", SummaryView(self.error))?;
        write!(w, "{}", PrettyTransaction(self.tx))?;
        write!(w, "{}", CausesView(self.error))?;
        write!(w, "{}", DescriptionView(self.error))?;
        write!(
            w,
            "{} transaction execution halted due to the above error",
            "error:".red()
        )?;
        Ok(())
    }
}

impl<E> std::fmt::Display for ExecutionErrorReport<'_, E>
where
    E: AnvilErrorDocumentation + CustomErrorMessage + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_fmt(f)
    }
}

/// Comprehensive error report for transaction estimation failures.
///
/// Similar to `ExecutionErrorReport`, but tailored for errors that occur
/// during transaction gas estimation.
pub struct EstimationErrorReport<'a, E>
where
    E: AnvilErrorDocumentation + CustomErrorMessage + Debug,
{
    /// The error that occurred during estimation
    pub error: &'a E,
    /// The transaction that was being estimated
    pub tx: &'a Transaction,
}

impl<'a, E> EstimationErrorReport<'a, E>
where
    E: AnvilErrorDocumentation + CustomErrorMessage + Debug,
{
    /// Creates a new estimation error report with the given error and transaction.
    pub fn new(error: &'a E, tx: &'a Transaction) -> Self {
        Self { error, tx }
    }
}

impl<'a, E> PrettyFmt for EstimationErrorReport<'a, E>
where
    E: AnvilErrorDocumentation + CustomErrorMessage + Debug,
{
    fn pretty_fmt(&self, w: &mut impl Write) -> std::fmt::Result {
        dbg!(self.error);
        dbg!(self.tx);
        write!(w, "{}", ErrorMessageView(self.error))?;
        write!(w, "{}", SummaryView(self.error))?;
        write!(w, "{}", PrettyTransactionEstimationView(self.tx))?;
        write!(w, "{}", CausesView(self.error))?;
        write!(w, "{}", DescriptionView(self.error))?;
        Ok(())
    }
}

impl<E> std::fmt::Display for EstimationErrorReport<'_, E>
where
    E: AnvilErrorDocumentation + CustomErrorMessage + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_fmt(f)
    }
}
