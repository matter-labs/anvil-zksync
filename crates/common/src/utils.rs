use alloy::dyn_abi::DynSolValue;
use alloy::primitives::{hex, Sign, I256, U256 as AlloyU256};
use anyhow::Context;
use colored::Colorize;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::convert::TryInto;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};
use zksync_types::H256;

/// Formats a token value for display. Adapted from `foundry-common-fmt`.
pub fn format_token(value: &DynSolValue, raw: bool) -> String {
    match value {
        DynSolValue::Address(inner) => inner.to_string(),
        DynSolValue::Function(inner) => inner.to_string(),
        DynSolValue::Bytes(inner) => format!("0x{}", hex::encode(inner)),
        DynSolValue::FixedBytes(word, size) => format!("0x{}", hex::encode(&word[..*size])),
        DynSolValue::Uint(inner, _) => {
            if raw {
                inner.to_string()
            } else {
                format_uint_exp(*inner)
            }
        }
        DynSolValue::Int(inner, _) => {
            if raw {
                inner.to_string()
            } else {
                format_int_exp(*inner)
            }
        }
        DynSolValue::Array(values) | DynSolValue::FixedArray(values) => {
            let formatted_values: Vec<String> =
                values.iter().map(|v| format_token(v, raw)).collect();
            format!("[{}]", formatted_values.join(", "))
        }
        DynSolValue::Tuple(values) => format_tuple(values, raw),
        DynSolValue::String(inner) => {
            if raw {
                inner.escape_debug().to_string()
            } else {
                format!("{:?}", inner) // Escape strings
            }
        }
        DynSolValue::Bool(inner) => inner.to_string(),
        DynSolValue::CustomStruct {
            name,
            prop_names,
            tuple,
        } => {
            if raw {
                return format_token(&DynSolValue::Tuple(tuple.clone()), true);
            }

            let mut s = String::new();

            s.push_str(name);

            if prop_names.len() == tuple.len() {
                s.push_str("({ ");

                for (i, (prop_name, value)) in std::iter::zip(prop_names, tuple).enumerate() {
                    if i > 0 {
                        s.push_str(", ");
                    }
                    s.push_str(prop_name);
                    s.push_str(": ");
                    s.push_str(&format_token(value, raw));
                }

                s.push_str(" })");
            } else {
                s.push_str(&format_tuple(tuple, raw));
            }
            s
        }
    }
}

fn format_tuple(values: &[DynSolValue], raw: bool) -> String {
    let formatted_values: Vec<String> = values.iter().map(|v| format_token(v, raw)).collect();
    format!("({})", formatted_values.join(", "))
}

/// Converts `h256` value as BE into the u64
pub fn h256_to_u64(value: H256) -> u64 {
    let be_u64_bytes: [u8; 8] = value[24..].try_into().unwrap();
    u64::from_be_bytes(be_u64_bytes)
}

/// Calculates the cost of a transaction in ETH.
pub fn calculate_eth_cost(gas_price_in_wei_per_gas: u64, gas_used: u64) -> f64 {
    // Convert gas price from wei to gwei
    let gas_price_in_gwei = gas_price_in_wei_per_gas as f64 / 1e9;

    // Calculate total cost in gwei
    let total_cost_in_gwei = gas_price_in_gwei * gas_used as f64;

    // Convert total cost from gwei to ETH
    total_cost_in_gwei / 1e9
}

/// Writes the given serializable object as JSON to the specified file path using pretty printing.
/// Returns an error if the file cannot be created or if serialization/writing fails.
pub fn write_json_file<T: Serialize>(path: &Path, obj: &T) -> anyhow::Result<()> {
    let file = File::create(path)
        .with_context(|| format!("Failed to create file '{}'", path.display()))?;
    let mut writer = BufWriter::new(file);
    // Note: intentionally using pretty printing for better readability.
    serde_json::to_writer_pretty(&mut writer, obj)
        .with_context(|| format!("Failed to write JSON to '{}'", path.display()))?;
    writer
        .flush()
        .with_context(|| format!("Failed to flush writer for '{}'", path.display()))?;

    Ok(())
}

/// Reads the JSON file at the specified path and deserializes it into the provided type.
/// Returns an error if the file cannot be read or deserialization fails.
pub fn read_json_file<T: DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let file_content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file '{}'", path.display()))?;

    serde_json::from_str(&file_content)
        .with_context(|| format!("Failed to deserialize JSON from '{}'", path.display()))
}

//////////////////////////////////////////////////////////////////////////////////////
// Attribution: Methods `to_exp_notation`, `format_uint_exp`, and `format_int_exp`  //
// are adapted from the `foundry-common-fmt` crate.                                 //
//                                                                                  //
// Full credit goes to its authors. See the original implementation here:           //
// https://github.com/foundry-rs/foundry/blob/master/crates/common/fmt/src/exp.rs.  //
//                                                                                  //
// Note: These methods are used under the terms of the original project's license.  //
//////////////////////////////////////////////////////////////////////////////////////

/// Returns the number expressed as a string in exponential notation
/// with the given precision (number of significant figures),
/// optionally removing trailing zeros from the mantissa.
#[inline]
pub fn to_exp_notation(
    value: AlloyU256,
    precision: usize,
    trim_end_zeros: bool,
    sign: Sign,
) -> String {
    let stringified = value.to_string();
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

    format!("{sign}{mantissa}e{exponent}")
}

/// Formats a U256 number to string, adding an exponential notation _hint_ if it
/// is larger than `10_000`, with a precision of `4` figures, and trimming the
/// trailing zeros.
pub fn format_uint_exp(num: AlloyU256) -> String {
    if num < AlloyU256::from(10_000) {
        return num.to_string();
    }

    let exp = to_exp_notation(num, 4, true, Sign::Positive);
    format!("{num} {}", format!("[{exp}]").dimmed())
}

/// Formats a U256 number to string, adding an exponential notation _hint_.
pub fn format_int_exp(num: I256) -> String {
    let (sign, abs) = num.into_sign_and_abs();
    if abs < AlloyU256::from(10_000) {
        return format!("{sign}{abs}");
    }

    let exp = to_exp_notation(abs, 4, true, sign);
    format!("{sign}{abs} {}", format!("[{exp}]").dimmed())
}
