//! Utility functions for writing to stdout and stderr.
//!
//! Simplified adaptation from
//! https://github.com/foundry-rs/foundry/blob/master/crates/common/src/io/macros.rs.

use anstream::{AutoStream, ColorChoice as AnstreamColorChoice};
use anstyle::{AnsiColor, Effects, Reset, Style};
use std::fmt::Arguments;
use std::io::{self, IsTerminal, Write};
use std::sync::{Mutex, OnceLock};

/// Default styles for WARNING and ERROR messages.
pub const ERROR: Style = AnsiColor::Red.on_default().effects(Effects::BOLD);
pub const WARN: Style = AnsiColor::Yellow.on_default().effects(Effects::BOLD);

/// Choices for whether to use colored output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

impl ColorChoice {
    #[inline]
    fn to_anstream(self) -> AnstreamColorChoice {
        match self {
            ColorChoice::Always => AnstreamColorChoice::Always,
            ColorChoice::Never => AnstreamColorChoice::Never,
            ColorChoice::Auto => AnstreamColorChoice::Auto,
        }
    }
}

/// The output mode: either normal output or completely quiet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputMode {
    Normal,
    Quiet,
}

/// A shell abstraction that tracks verbosity level, an output mode,
/// and a color choice.
/// It uses anstream’s AutoStream for stdout and stderr.
#[derive(Debug)]
pub struct Shell {
    /// Verbosity level.
    pub verbosity: u8,
    /// Whether to output anything at all.
    pub output_mode: OutputMode,
    /// Whether to use colors.
    pub color_choice: ColorChoice,
    /// Adaptive stdout.
    stdout: AutoStream<std::io::Stdout>,
    /// Adaptive stderr.
    stderr: AutoStream<std::io::Stderr>,
}

impl Shell {
    /// Creates a new shell with default settings:
    /// - verbosity 0,
    /// - Normal output mode,
    /// - Auto color detection.
    pub fn new() -> Self {
        let color = ColorChoice::Auto;
        Self {
            verbosity: 0,
            output_mode: OutputMode::Normal,
            color_choice: color,
            stdout: AutoStream::new(std::io::stdout(), color.to_anstream()),
            stderr: AutoStream::new(std::io::stderr(), color.to_anstream()),
        }
    }

    /// Print a string to stdout.
    pub fn print_out(&mut self, args: Arguments) -> io::Result<()> {
        if self.output_mode == OutputMode::Quiet {
            return Ok(());
        }

        self.stdout.write_fmt(args)?;
        self.stdout.flush()
    }

    /// Print a line (with a newline) to stdout.
    pub fn println_out(&mut self, args: Arguments) -> io::Result<()> {
        if self.output_mode == OutputMode::Quiet {
            return Ok(());
        }

        self.stdout.write_fmt(args)?;
        writeln!(self.stdout)?;
        self.stdout.flush()
    }

    /// Print a string to stderr.
    pub fn print_err(&mut self, args: Arguments) -> io::Result<()> {
        if self.output_mode == OutputMode::Quiet {
            return Ok(());
        }
        self.stderr.write_fmt(args)?;
        self.stderr.flush()
    }

    /// Print a line (with a newline) to stderr.
    pub fn println_err(&mut self, args: Arguments) -> io::Result<()> {
        if self.output_mode == OutputMode::Quiet {
            return Ok(());
        }
        self.stderr.write_fmt(args)?;
        writeln!(self.stderr)?;
        self.stderr.flush()
    }

    /// Print a warning message.
    ///
    /// If colors are enabled, the “Warning:” prefix is printed in yellow.
    pub fn warn(&mut self, args: Arguments) -> io::Result<()> {
        if self.output_mode == OutputMode::Quiet {
            return Ok(());
        }
        if self.should_color() {
            write!(self.stderr, "{}Warning:{} ", WARN, Reset)?;
        } else {
            write!(self.stderr, "Warning: ")?;
        }
        self.stderr.write_fmt(args)?;
        writeln!(self.stderr)?;
        self.stderr.flush()
    }

    /// Print an error message.
    ///
    /// If colors are enabled, the “Error:” prefix is printed in red.
    pub fn error(&mut self, args: Arguments) -> io::Result<()> {
        if self.should_color() {
            write!(self.stderr, "{}Error:{} ", ERROR, Reset)?;
        } else {
            write!(self.stderr, "Error: ")?;
        }
        self.stderr.write_fmt(args)?;
        writeln!(self.stderr)?;
        self.stderr.flush()
    }

    fn should_color(&self) -> bool {
        match self.color_choice {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => std::io::stdout().is_terminal(),
        }
    }
}

impl Default for Shell {
    fn default() -> Self {
        Self::new()
    }
}

/// The global shell instance.
///
/// This uses a [`OnceLock`](https://doc.rust-lang.org/std/sync/struct.OnceLock.html)
/// to initialize the shell once and then a [`Mutex`](std::sync::Mutex) to allow
/// mutable access to it from anywhere.
static GLOBAL_SHELL: OnceLock<Mutex<Shell>> = OnceLock::new();

/// Get a lock to the global shell.
///
/// This will initialize the shell with default values if it has not been set yet.
pub fn get_shell() -> std::sync::MutexGuard<'static, Shell> {
    GLOBAL_SHELL
        .get_or_init(|| Mutex::new(Shell::new()))
        .lock()
        .expect("global shell mutex is poisoned")
}

/// (Optional) Set the global shell with a custom configuration.
///
/// Note that this will fail if the shell has already been set.
pub fn set_shell(shell: Shell) {
    let _ = GLOBAL_SHELL.set(Mutex::new(shell));
}

/// Macro to print a formatted message to stdout.
///
/// Usage:
/// ```
/// use anvil_zksync_common::sh_print;
///
/// sh_print!("Hello, {}!", "world");
/// ```
#[macro_export]
macro_rules! sh_print {
    ($($arg:tt)*) => {{
        $crate::shell::get_shell().print_out(format_args!($($arg)*))
            .unwrap_or_else(|e| eprintln!("Error writing output: {}", e));
    }};
}

/// Macro to print a formatted message (with a newline) to stdout.
#[macro_export]
macro_rules! sh_println {
    ($($arg:tt)*) => {{
        $crate::shell::get_shell().println_out(format_args!($($arg)*))
            .unwrap_or_else(|e| eprintln!("Error writing output: {}", e));
    }};
}

/// Macro to print a formatted message to stderr.
#[macro_export]
macro_rules! sh_eprint {
    ($($arg:tt)*) => {{
        $crate::shell::get_shell().print_err(format_args!($($arg)*))
            .unwrap_or_else(|e| eprintln!("Error writing stderr: {}", e));
    }};
}

/// Macro to print a formatted message (with newline) to stderr.
#[macro_export]
macro_rules! sh_eprintln {
    ($($arg:tt)*) => {{
        $crate::shell::get_shell().println_err(format_args!($($arg)*))
            .unwrap_or_else(|e| eprintln!("Error writing stderr: {}", e));
    }};
}

/// Macro to print a warning message.
///
/// Usage:
/// ```
/// use anvil_zksync_common::sh_warn;
///
/// sh_warn!("This is a warning: {}", "be careful!");
/// ```
#[macro_export]
macro_rules! sh_warn {
    ($($arg:tt)*) => {{
        $crate::shell::get_shell().warn(format_args!($($arg)*))
            .unwrap_or_else(|e| eprintln!("Error writing warning: {}", e));
    }};
}

/// Macro to print an error message.
///
/// Usage:
/// ```
/// use anvil_zksync_common::sh_err;
///
/// sh_err!("Something went wrong: {}", "details");
/// ```
#[macro_export]
macro_rules! sh_err {
    ($($arg:tt)*) => {{
        $crate::shell::get_shell().error(format_args!($($arg)*))
            .unwrap_or_else(|e| eprintln!("Error writing error: {}", e));
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_shell_macros() {
        sh_print!("Hello, ");
        sh_println!("world!");
        sh_eprint!("Error: ");
        sh_eprintln!("Something went wrong!");
        sh_warn!("This is a warning");
        sh_err!("This is an error");
    }
}
