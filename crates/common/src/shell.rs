// shell.rs

use std::io::{self, Write};
use std::sync::{Mutex, OnceLock};

/// The output mode: either normal output or completely quiet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputMode {
    Normal,
    Quiet,
}

/// Choices for whether to use colored output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorChoice {
    Auto,
    Always,
    Never,
}

/// A simple shell abstraction.
///
/// In this simplified version we only track a verbosity level, an output mode,
/// and a color choice. In your application you might extend this with more
/// sophisticated buffering or logging features.
#[derive(Debug)]
pub struct Shell {
    /// Verbosity level (currently unused, but available for expansion)
    pub verbosity: u8,
    /// Whether to output anything at all.
    pub output_mode: OutputMode,
    /// Whether to use colors.
    pub color_choice: ColorChoice,
}

impl Shell {
    /// Create a new shell with default settings.
    pub fn new() -> Self {
        Self {
            verbosity: 0,
            output_mode: OutputMode::Normal,
            color_choice: ColorChoice::Auto,
        }
    }

    /// Print a string to stdout.
    pub fn print_out(&mut self, msg: &str) -> io::Result<()> {
        if self.output_mode == OutputMode::Quiet {
            return Ok(());
        }
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        write!(handle, "{}", msg)?;
        handle.flush()
    }

    /// Print a line (with a newline) to stdout.
    pub fn println_out(&mut self, msg: &str) -> io::Result<()> {
        if self.output_mode == OutputMode::Quiet {
            return Ok(());
        }
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        writeln!(handle, "{}", msg)?;
        handle.flush()
    }

    /// Print a string to stderr.
    pub fn print_err(&mut self, msg: &str) -> io::Result<()> {
        if self.output_mode == OutputMode::Quiet {
            return Ok(());
        }
        let stderr = io::stderr();
        let mut handle = stderr.lock();
        write!(handle, "{}", msg)?;
        handle.flush()
    }

    /// Print a line (with a newline) to stderr.
    pub fn println_err(&mut self, msg: &str) -> io::Result<()> {
        if self.output_mode == OutputMode::Quiet {
            return Ok(());
        }
        let stderr = io::stderr();
        let mut handle = stderr.lock();
        writeln!(handle, "{}", msg)?;
        handle.flush()
    }

    /// Print a warning message.
    ///
    /// If colors are enabled, the “Warning:” prefix is printed in yellow.
    pub fn warn(&mut self, msg: &str) -> io::Result<()> {
        let formatted = if self.should_color() {
            format!("\x1b[33mWarning:\x1b[0m {}", msg)
        } else {
            format!("Warning: {}", msg)
        };
        self.println_err(&formatted)
    }

    /// Print an error message.
    ///
    /// If colors are enabled, the “Error:” prefix is printed in red.
    pub fn error(&mut self, msg: &str) -> io::Result<()> {
        let formatted = if self.should_color() {
            format!("\x1b[31mError:\x1b[0m {}", msg)
        } else {
            format!("Error: {}", msg)
        };
        self.println_err(&formatted)
    }

    /// Should we output with ANSI colors?
    ///
    /// In the `Auto` case we use the [atty](https://crates.io/crates/atty) crate to detect
    /// whether stdout is a terminal.
    fn should_color(&self) -> bool {
        match self.color_choice {
            ColorChoice::Always => true,
            ColorChoice::Never => false,
            ColorChoice::Auto => atty::is(atty::Stream::Stdout),
        }
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
/// ```rust
/// sh_print!("Hello, {}!", "world");
/// ```
#[macro_export]
macro_rules! sh_print {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::shell::get_shell().print_out(&msg)
            .unwrap_or_else(|e| eprintln!("Error writing output: {}", e));
    }};
}

/// Macro to print a formatted message (with a newline) to stdout.
#[macro_export]
macro_rules! sh_println {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::shell::get_shell().println_out(&msg)
            .unwrap_or_else(|e| eprintln!("Error writing output: {}", e));
    }};
}

/// Macro to print a formatted message to stderr.
#[macro_export]
macro_rules! sh_eprint {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::shell::get_shell().print_err(&msg)
            .unwrap_or_else(|e| eprintln!("Error writing stderr: {}", e));
    }};
}

/// Macro to print a formatted message (with a newline) to stderr.
#[macro_export]
macro_rules! sh_eprintln {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::shell::get_shell().println_err(&msg)
            .unwrap_or_else(|e| eprintln!("Error writing stderr: {}", e));
    }};
}

/// Macro to print a warning message.
///
/// Usage:
/// ```rust
/// sh_warn!("This is a warning: {}", "be careful!");
/// ```
#[macro_export]
macro_rules! sh_warn {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::shell::get_shell().warn(&msg)
            .unwrap_or_else(|e| eprintln!("Error writing warning: {}", e));
    }};
}

/// Macro to print an error message.
///
/// Usage:
/// ```rust
/// sh_err!("Something went wrong: {}", "details");
/// ```
#[macro_export]
macro_rules! sh_err {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::shell::get_shell().error(&msg)
            .unwrap_or_else(|e| eprintln!("Error writing error: {}", e));
    }};
}

/// Macro to prompt the user for input.
///
/// This prints the prompt (using our shell abstraction), flushes stdout, reads
/// a line from stdin, and returns the trimmed input as a `String`.
///
/// Usage:
/// ```rust
/// let response = prompt!("Enter your name: ");
/// ```
#[macro_export]
macro_rules! prompt {
    ($($arg:tt)*) => {{
        $crate::sh_print!($($arg)*);
        let _ = std::io::stdout().flush();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)
            .expect("Failed to read input");
        input.trim().to_string()
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_macros() {
        // These calls use the global shell.
        sh_print!("Hello, ");
        sh_println!("world!");
        sh_eprint!("Error: ");
        sh_eprintln!("Something went wrong!");
        sh_warn!("This is a warning");
        sh_err!("This is an error");
    }

    // Uncomment the following test to try the prompt macro. Note that this requires interactive input.
    /*
    #[test]
    fn test_prompt_macro() {
        let response = prompt!("Type something: ");
        sh_println!("You typed: {}", response);
    }
    */
}