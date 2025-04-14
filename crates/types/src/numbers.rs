use zksync_types::U256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Sign {
    NonNegative,
    Negative,
}
impl std::fmt::Display for Sign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Sign::NonNegative => "+",
            Sign::Negative => "-",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedU256 {
    pub sign: Sign,
    pub inner: U256,
}
impl std::fmt::Display for SignedU256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { sign, inner } = self;
        if *sign == Sign::Negative {
            write!(f, "-")?;
        }
        write!(f, "{inner}")
    }
}
impl From<U256> for SignedU256 {
    fn from(val: U256) -> Self {
        SignedU256 {
            sign: Sign::NonNegative,
            inner: val,
        }
    }
}

impl From<SignedU256> for NumberExponentialRepr {
    /// Formats a U256 number to string, adding an exponential notation _hint_ if it
    /// is larger than `10_000`, with a precision of `4` figures, and trimming the
    /// trailing zeros.
    fn from(val: SignedU256) -> Self {
        to_exp_notation(val, 4, true)
    }
}

pub struct NumberExponentialRepr {
    pub value: SignedU256,
    pub mantissa: String,
    pub exponent: usize,
}

//////////////////////////////////////////////////////////////////////////////////////
// Attribution: Function `to_exp_notation`                                         //
// is adapted from the `foundry-common-fmt` crate.                                 //
//                                                                                  //
// Full credit goes to its authors. See the original implementation here:           //
// https://github.com/foundry-rs/foundry/blob/master/crates/common/fmt/src/exp.rs.  //
//                                                                                  //
// Note: These methods are used under the terms of the original project's license.  //
//////////////////////////////////////////////////////////////////////////////////////

/// Returns the number expressed as a string in exponential notation
/// with the given precision (number of significant figures),
/// optionally removing trailing zeros from the mantissa.
fn to_exp_notation(
    value: SignedU256,
    precision: usize,
    trim_end_zeros: bool,
) -> NumberExponentialRepr {
    let stringified = value.inner.to_string();
    let exponent = stringified.len() - 1;
    let mut mantissa = stringified.chars().take(precision).collect::<String>();

    // optionally remove trailing zeros
    if trim_end_zeros {
        mantissa = mantissa.trim_end_matches('0').to_string();
    }

    // Place a decimal point only if needed
    // e.g. 1234 -> 1.234e3 (needed)
    //      5 -> 5 (not needed)
    if mantissa.len() > 1 {
        mantissa.insert(1, '.');
    }
    NumberExponentialRepr {
        value,
        mantissa,
        exponent,
    }
}

impl std::fmt::Display for NumberExponentialRepr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let NumberExponentialRepr {
            value: SignedU256 { sign, .. },
            mantissa,
            exponent,
        } = self;
        f.write_fmt(format_args!("{sign}{mantissa}e{exponent}"))?;
        Ok(())
    }
}
