//! View components for displaying formatted error reports.
//!
//! This module provides specialized view structs for rendering different types
//! of error reports with appropriate formatting and context information.

use colored::Colorize;
use std::fmt::Debug;
use std::fmt::Write;
use zksync_error::CustomErrorMessage;
use zksync_error::anvil_zksync::gas_estim::GasEstimationError;
use zksync_types::Transaction;

use crate::formatter::transaction::view::PrettyTransactionEstimationView;
use crate::formatter::util::indenting_writer::IndentingWriter;
use crate::formatter::{PrettyFmt, transaction::view::PrettyTransaction};

use super::documentation::{
    AnvilErrorDocumentation,
    view::{CausesView, DescriptionView, SummaryView},
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
        write!(
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
        let mut wr = IndentingWriter::new(w, Some("│ "));

        writeln!(wr, "{}", ErrorMessageView(self.error))?;
        wr.indent();
        writeln!(wr, "{}", SummaryView(self.error))?;
        writeln!(wr, "{}", PrettyTransaction(self.tx))?;
        writeln!(wr, "{}", CausesView(self.error))?;
        writeln!(wr, "{}", DescriptionView(self.error))?;
        writeln!(
            wr,
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
pub struct EstimationErrorReport<'a> {
    /// The error that occurred during estimation
    pub error: &'a GasEstimationError,
    /// The transaction that was being estimated
    pub tx: &'a Transaction,
}

impl<'a> EstimationErrorReport<'a> {
    /// Creates a new estimation error report with the given error and transaction.
    pub fn new(error: &'a GasEstimationError, tx: &'a Transaction) -> Self {
        Self { error, tx }
    }
}

impl PrettyFmt for EstimationErrorReport<'_> {
    fn pretty_fmt(&self, w: &mut impl Write) -> std::fmt::Result {
        writeln!(w)?;
        match self.error {
            GasEstimationError::TransactionHalt { inner }
            | GasEstimationError::TransactionAlwaysHalts { inner } => {
                let halt = inner.as_ref();
                writeln!(w, "{}", ErrorMessageView(self.error))?;
                let mut w = IndentingWriter::new(w, Some("│ "));
                writeln!(w, "{}", SummaryView(self.error))?;
                w.indent();
                w.push_prefix("= ");
                writeln!(w, "{}", ErrorMessageView(halt))?;
                w.pop_prefix();
                if let zksync_error::anvil_zksync::halt::HaltError::GenericError { .. } = **inner {
                } else {
                    writeln!(w, "{}", SummaryView(halt))?;
                }

                writeln!(w, "{}", CausesView(halt))?;
                writeln!(w, "{}", PrettyTransactionEstimationView(self.tx))?;
                if let zksync_error::anvil_zksync::halt::HaltError::GenericError { .. } = **inner {
                } else {
                    writeln!(w, "{}", DescriptionView(halt))?;
                }
                w.dedent();
                writeln!(w, "{}", DescriptionView(self.error))?;
            }

            GasEstimationError::TransactionRevert { inner, data: _ }
            | GasEstimationError::TransactionAlwaysReverts { inner, data: _ } => {
                let revert = inner.as_ref();
                writeln!(w, "{}", ErrorMessageView(self.error))?;
                let mut w = IndentingWriter::new(w, Some("│ "));
                writeln!(w, "{}", SummaryView(self.error))?;
                w.indent();
                w.push_prefix("= ");
                writeln!(w, "{}", ErrorMessageView(revert))?;
                w.pop_prefix();

                if let zksync_error::anvil_zksync::revert::RevertError::GenericError { .. } =
                    **inner
                {
                } else {
                    writeln!(w, "{}", SummaryView(revert))?;
                }
                writeln!(w, "{}", CausesView(revert))?;
                writeln!(w, "{}", PrettyTransactionEstimationView(self.tx))?;

                if let zksync_error::anvil_zksync::revert::RevertError::GenericError { .. } =
                    **inner
                {
                } else {
                    writeln!(w, "{}", DescriptionView(revert))?;
                }
                w.dedent();
                writeln!(w, "{}", DescriptionView(self.error))?;
            }
            other => {
                let mut w = IndentingWriter::new(w, Some("│ "));
                w.push_prefix("= ");
                writeln!(w, "{}", ErrorMessageView(other))?;
                w.indent();
                w.pop_prefix();
                writeln!(w, "{}", SummaryView(other))?;
                writeln!(w, "{}", CausesView(other))?;
                writeln!(w, "{}", PrettyTransactionEstimationView(self.tx))?;
                writeln!(w, "{}", DescriptionView(other))?;
            }
        };
        Ok(())
    }
}

impl std::fmt::Display for EstimationErrorReport<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.pretty_fmt(f)
    }
}
