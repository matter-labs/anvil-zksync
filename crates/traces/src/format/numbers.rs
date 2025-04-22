use std::fmt::{self, Display};

use anvil_zksync_types::numbers::NumberExponentialRepr;
use colored::Colorize;

pub struct PrettyNumberExponentialRepr<'a>(pub &'a NumberExponentialRepr);

impl Display for PrettyNumberExponentialRepr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.value.inner.is_zero() {
            write!(f, "0")
        } else {
            f.write_fmt(format_args!("{}", self.0.value))?;
            if self.0.value.inner > 10_000.into() {
                let exponential = format!("[{}]", self.0).dimmed();
                f.write_fmt(format_args!(" {}", exponential))?;
            }
            Ok(())
        }
    }
}
