use serde::de::{Error as DeError, Unexpected};
use serde::{Deserialize, Deserializer, Serializer};
use zksync_types::U64;

/// Helper type to be able to parse both integers (as `u64`) and hex strings (as `U64`) depending on
/// the user input.
#[derive(Copy, Clone, Deserialize)]
#[serde(untagged)]
pub enum Numeric {
    /// A [U64] value.
    U64(U64),
    /// A `u64` value.
    Num(u64),
}

impl From<u64> for Numeric {
    fn from(value: u64) -> Self {
        Numeric::Num(value)
    }
}

impl From<Numeric> for u64 {
    fn from(value: Numeric) -> Self {
        match value {
            Numeric::U64(value) => value.as_u64(),
            Numeric::Num(value) => value,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::serde_helpers::Numeric;

    #[test]
    fn test_deserialize() {
        let tests = [
            // Hex strings
            ("\"0x0\"", 0u64),
            ("\"0x1\"", 1u64),
            ("\"0x2\"", 2u64),
            ("\"0xa\"", 10u64),
            ("\"0xf\"", 15u64),
            ("\"0x10\"", 16u64),
            ("\"0\"", 0u64),
            ("\"1\"", 1u64),
            ("\"2\"", 2u64),
            ("\"a\"", 10u64),
            ("\"f\"", 15u64),
            ("\"10\"", 16u64),
            // Numbers
            ("0", 0u64),
            ("1", 1u64),
            ("2", 2u64),
            ("10", 10u64),
            ("15", 15u64),
            ("16", 16u64),
        ];
        for (serialized, expected_value) in tests {
            let actual_value: Numeric = serde_json::from_str(serialized).unwrap();
            assert_eq!(u64::from(actual_value), expected_value);
        }
    }
}
