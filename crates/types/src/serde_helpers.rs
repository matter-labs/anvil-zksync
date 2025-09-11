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

/// 0x-hex <-> u64 (accepts hex string or JSON number)
pub mod u64_hex {
    use super::*;
    pub fn serialize<S>(val: &u64, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // serialize as 0x-prefixed hex to be consistent
        s.serialize_str(&format!("0x{:x}", val))
    }
    pub fn deserialize<'de, D>(d: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Num {
            Str(String),
            Num(u64),
        }
        match Num::deserialize(d)? {
            Num::Num(n) => Ok(n),
            Num::Str(s) => {
                let s = s.strip_prefix("0x").unwrap_or(&s);
                u64::from_str_radix(s, 16)
                    .map_err(|_| D::Error::invalid_value(Unexpected::Str(&s), &"0x.. hex u64"))
            }
        }
    }
}

/// 0x-hex <-> H160
pub mod h160_hex {
    use zksync_types::H160;

    use super::*;
    pub fn serialize<S>(val: &H160, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(&format!("0x{}", hex::encode(val.as_bytes())))
    }
    pub fn deserialize<'de, D>(d: D) -> Result<H160, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(d)?;
        let s = s.strip_prefix("0x").unwrap_or(&s);
        let bytes = hex::decode(s).map_err(D::Error::custom)?;
        if bytes.len() != 20 {
            return Err(D::Error::invalid_length(bytes.len(), &"20-byte address"));
        }
        Ok(H160::from_slice(&bytes))
    }
}

/// 0x-hex <-> U256 (string or number)
pub mod u256_hex {
    use zksync_types::U256;

    use super::*;
    pub fn serialize<S>(val: &U256, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut buf = [0u8; 32];
        val.to_big_endian(&mut buf);
        s.serialize_str(&format!("0x{}", hex::encode(buf)))
    }
    pub fn deserialize<'de, D>(d: D) -> Result<U256, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Num {
            Str(String),
            Num(u64),
        } // accept small numeric literal too
        match Num::deserialize(d)? {
            Num::Num(n) => Ok(U256::from(n)),
            Num::Str(s) => {
                let s = s.strip_prefix("0x").unwrap_or(&s);
                U256::from_str_radix(s, 16).map_err(D::Error::custom)
            }
        }
    }
}

/// 0x-hex <-> Vec<u8> (accepts "" or "0x" as empty)
pub mod bytes_hex {
    use super::*;
    pub fn serialize<S>(val: &Vec<u8>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if val.is_empty() {
            return s.serialize_str("0x");
        }
        s.serialize_str(&format!("0x{}", hex::encode(val)))
    }
    pub fn deserialize<'de, D>(d: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(d)?;
        if s.is_empty() || s == "0x" {
            return Ok(vec![]);
        }
        let s = s.strip_prefix("0x").unwrap_or(&s);
        hex::decode(s).map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{bytes_hex, h160_hex, u64_hex, u256_hex};
    use crate::serde_helpers::Numeric;
    use serde::{Deserialize, Serialize};
    use serde_json as json;
    use zksync_types::{H160, U256};

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

    // Small wrappers so we can test #[serde(with = "...")] easily.
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct U64W(#[serde(with = "u64_hex")] u64);

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct H160W(#[serde(with = "h160_hex")] H160);

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct U256W(#[serde(with = "u256_hex")] U256);

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct BytesW(#[serde(with = "bytes_hex")] Vec<u8>);

    // --- u64_hex ---

    #[test]
    fn u64_hex_deser_from_number() {
        let v: U64W = json::from_str("12345").unwrap();
        assert_eq!(v.0, 12345);
    }

    #[test]
    fn u64_hex_deser_from_hex_str() {
        let v: U64W = json::from_str(r#""0x3039""#).unwrap(); // 0x3039 == 12345
        assert_eq!(v.0, 12345);
    }

    #[test]
    fn u64_hex_serde_roundtrip() {
        let orig = U64W(0xdead_beef);
        let s = json::to_string(&orig).unwrap();
        assert_eq!(s, r#""0xdeadbeef""#); // serialized as hex string
        let back: U64W = json::from_str(&s).unwrap();
        assert_eq!(back, orig);
    }

    #[test]
    fn u64_hex_rejects_invalid() {
        let err = json::from_str::<U64W>(r#""not-hex""#).unwrap_err();
        assert!(err.is_data());
    }

    // --- h160_hex ---

    #[test]
    fn h160_hex_deser() {
        let addr = "0x000000000000000000000000000000000000dead";
        let w: H160W = json::from_str(&format!(r#""{addr}""#)).unwrap();
        assert_eq!(format!("{:#x}", w.0), addr);
    }

    #[test]
    fn h160_hex_serde_roundtrip() {
        let a = H160::from_low_u64_be(0xdead_beef);
        let w = H160W(a);
        let s = json::to_string(&w).unwrap();
        let back: H160W = json::from_str(&s).unwrap();
        assert_eq!(back, w);
    }

    #[test]
    fn h160_hex_wrong_length_errors() {
        // 19 bytes -> invalid
        let err = json::from_str::<H160W>(r#""0x11aa22bb33cc44dd55ee66ff77""#).unwrap_err();
        assert!(err.is_data());
    }

    // --- u256_hex ---

    #[test]
    fn u256_hex_deser_from_number() {
        let w: U256W = json::from_str("42").unwrap();
        assert_eq!(w.0, U256::from(42u64));
    }

    #[test]
    fn u256_hex_deser_from_hex_str() {
        let w: U256W = json::from_str(r#""0x2a""#).unwrap();
        assert_eq!(w.0, U256::from(42u64));
    }

    #[test]
    fn u256_hex_serde_roundtrip() {
        let mut big = U256::from(1u64);
        big = (big << 200) + U256::from(0xdead_beefu64);
        let w = U256W(big);
        let s = json::to_string(&w).unwrap();
        // round-trip back
        let back: U256W = json::from_str(&s).unwrap();
        assert_eq!(back, w);
    }

    // --- bytes_hex ---

    #[test]
    fn bytes_hex_deser_empty_variants() {
        let a: BytesW = json::from_str(r#""""#).unwrap();
        assert!(a.0.is_empty());
        let b: BytesW = json::from_str(r#""0x""#).unwrap();
        assert!(b.0.is_empty());
    }

    #[test]
    fn bytes_hex_deser_non_empty() {
        let w: BytesW = json::from_str(r#""0xdeadbeef""#).unwrap();
        assert_eq!(w.0, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn bytes_hex_serde_roundtrip() {
        let w = BytesW(vec![0, 1, 2, 0xfe, 0xff]);
        let s = json::to_string(&w).unwrap();
        assert_eq!(s, r#""0x000102feff""#);
        let back: BytesW = json::from_str(&s).unwrap();
        assert_eq!(back, w);
    }

    #[test]
    fn bytes_hex_deser_invalid_hex_errors() {
        let err = json::from_str::<BytesW>(r#""0xzzzz""#).unwrap_err();
        assert!(err.is_data());
    }
}
