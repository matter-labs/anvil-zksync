use alloy::hex;

pub fn trimmed_hex(s: &[u8]) -> String {
    let n = 32;
    if s.len() <= n {
        hex::encode(s)
    } else {
        format!(
            "{}…{} ({} bytes)",
            &hex::encode(&s[..n / 2]),
            &hex::encode(&s[s.len() - n / 2..]),
            s.len(),
        )
    }
}

pub fn write_interspersed<T>(
    f: &mut std::fmt::Formatter<'_>,
    iter: impl Iterator<Item = T>,
    separator: &str,
) -> std::fmt::Result
where
    T: std::fmt::Display,
{
    for (i, value) in iter.enumerate() {
        if i > 0 {
            f.write_str(separator)?;
        }
        value.fmt(f)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use alloy::hex;

    use super::*;
    #[test]
    fn test_trimmed_hex() {
        assert_eq!(
            trimmed_hex(&hex::decode("1234567890").unwrap()),
            "1234567890"
        );
        assert_eq!(
            trimmed_hex(&hex::decode("492077697368207275737420737570706F72746564206869676865722D6B696E646564207479706573").unwrap()),
            "49207769736820727573742073757070…6865722d6b696e646564207479706573 (41 bytes)"
        );
    }
}
