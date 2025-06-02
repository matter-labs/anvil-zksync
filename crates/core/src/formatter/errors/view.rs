//! View components for displaying formatted error reports.
//!
//! This module provides specialized view structs for rendering different types
//! of error reports with appropriate formatting and context information.

use std::fmt::Debug;
use std::fmt::Write;
use colored::Colorize;
use zksync_error::anvil_zksync::gas_estim::GasEstimationError;
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
#[derive(Debug)]
pub struct ErrorMessageView<'a, E>(pub &'a E)
where
    E: CustomErrorMessage + Debug;

impl<E> PrettyFmt for ErrorMessageView<'_, E>
where
    E: CustomErrorMessage + Debug,
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

impl<E> std::fmt::Display for ErrorMessageView<'_, E>
where
    E: CustomErrorMessage + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_fmt(f)
    }
}

/// Comprehensive error report for transaction execution failures.
///
/// Combines error details with transaction context to provide a complete
/// picture of what went wrong during transaction execution.
#[derive(Debug)]
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

impl<E> PrettyFmt for ExecutionErrorReport<'_, E>
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
#[derive(Debug)]
pub struct EstimationErrorReport<'a>
{
    /// The error that occurred during estimation
    pub error: &'a GasEstimationError,
    /// The transaction that was being estimated
    pub tx: &'a Transaction,
}

impl<'a> EstimationErrorReport<'a>

{
    /// Creates a new estimation error report with the given error and transaction.
    pub fn new(error: &'a GasEstimationError, tx: &'a Transaction) -> Self {
        Self { error, tx }
    }
}

impl PrettyFmt for EstimationErrorReport<'_>
{
    fn pretty_fmt(&self, w: &mut impl Write) -> std::fmt::Result {


        match self.error {
            GasEstimationError::TransactionHalt { inner } |
            GasEstimationError::TransactionAlwaysHalts { inner } => {
                write!(w, "{}", ErrorMessageView(self.error))?;
                let halt = inner.as_ref();
                write!(w, "{}", SummaryView(halt))?;
                write!(w, "{}", InnerErrorMessageView(halt))?;
                write!(w, "{}", CausesView(halt))?;
                write!(w, "{}", PrettyTransactionEstimationView(self.tx))?;
                write!(w, "{}", DescriptionView(halt))?;
            },

            GasEstimationError::TransactionRevert { inner } |
            GasEstimationError::TransactionAlwaysReverts { inner } => {
                write!(w, "{}", ErrorMessageView(self.error))?;
                let revert = inner.as_ref();
                write!(w, "{}", SummaryView(revert))?;
                write!(w, "{}", InnerErrorMessageView(revert))?;
                write!(w, "{}", CausesView(revert))?;
                write!(w, "{}", PrettyTransactionEstimationView(self.tx))?;
                write!(w, "{}", DescriptionView(revert))?;
            }
            other =>  {
                write!(w, "{}", ErrorMessageView(other))?;
                write!(w, "{}", SummaryView(other))?;
                write!(w, "{}", CausesView(other))?;
                write!(w, "{}", PrettyTransactionEstimationView(self.tx))?;
                write!(w, "{}", DescriptionView(other))?;
            }
        };
        Ok(())
    }
}

impl std::fmt::Display for EstimationErrorReport<'_>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_fmt(f)
    }
}

/// Displays a basic error message with standard styling and an offset.
///
/// This view wraps any type that implements `CustomErrorMessage` and
/// renders its error message with appropriate styling.
#[derive(Debug)]
pub struct InnerErrorMessageView<'a, E>(pub &'a E)
where
    E: CustomErrorMessage + Debug;

impl<E> PrettyFmt for InnerErrorMessageView<'_, E>
where
    E: CustomErrorMessage + Debug,
{
    fn pretty_fmt(&self, w: &mut impl Write) -> std::fmt::Result {
        writeln!(
            w,
            "    | {}: {}",
            "error".red().bold(),
            self.0.get_message().red()
        )?;
        Ok(())
    }
}

impl<E> std::fmt::Display for InnerErrorMessageView<'_, E>
where
    E: CustomErrorMessage + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_fmt(f)
    }
}
