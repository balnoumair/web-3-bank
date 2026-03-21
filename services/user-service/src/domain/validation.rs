//! Domain validation rules.

/// Regex for a 0x-prefixed 40-character hex Ethereum address.
static TEMPO_ADDR_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

/// Validate that a string is a well-formed Tempo (Ethereum) address.
pub fn valid_tempo_address(addr: &str) -> bool {
    TEMPO_ADDR_RE
        .get_or_init(|| regex::Regex::new(r"^0x[0-9a-fA-F]{40}$").unwrap())
        .is_match(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_addresses() {
        assert!(valid_tempo_address(
            "0xaaaa111111111111111111111111111111111111"
        ));
        assert!(valid_tempo_address(
            "0xAbCdEf1234567890AbCdEf1234567890AbCdEf12"
        ));
    }

    #[test]
    fn invalid_addresses() {
        assert!(!valid_tempo_address("not-an-address"));
        assert!(!valid_tempo_address("0x1234")); // too short
        assert!(!valid_tempo_address(
            "aaaa111111111111111111111111111111111111"
        )); // missing 0x
        assert!(!valid_tempo_address(
            "0xgggg111111111111111111111111111111111111"
        )); // invalid hex
    }
}
