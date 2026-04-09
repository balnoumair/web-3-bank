//! Domain validation rules and validated value types.
//!
//! [`valid_tempo_address`] provides the raw predicate, while [`TempoAddress`]
//! is a validated newtype that bakes the check into construction via
//! [`TryFrom`] so invalid addresses can never reach the repository layer.
//!
//! [`Username`] follows the same pattern for user-chosen display handles.

use crate::domain::errors::DomainError;

/// A validated Tempo (Ethereum-style) address.
///
/// Guaranteed to be a 0x-prefixed 40-character hex string.
/// Use [`TryFrom<String>`] or [`TryFrom<&str>`] to construct from untrusted
/// input.  Trusted sources (e.g. DB rows that were already validated on write)
/// may construct directly via the public inner field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempoAddress(pub String);

impl TempoAddress {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TempoAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::ops::Deref for TempoAddress {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TempoAddress {
    type Error = DomainError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        if valid_tempo_address(&s) {
            Ok(TempoAddress(s))
        } else {
            Err(DomainError::InvalidTempoAddress)
        }
    }
}

impl TryFrom<&str> for TempoAddress {
    type Error = DomainError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if valid_tempo_address(s) {
            Ok(TempoAddress(s.to_string()))
        } else {
            Err(DomainError::InvalidTempoAddress)
        }
    }
}

/// Regex for a 0x-prefixed 40-character hex Ethereum address.
static TEMPO_ADDR_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

/// Validate that a string is a well-formed Tempo (Ethereum) address.
pub fn valid_tempo_address(addr: &str) -> bool {
    TEMPO_ADDR_RE
        .get_or_init(|| regex::Regex::new(r"^0x[0-9a-fA-F]{40}$").unwrap())
        .is_match(addr)
}

// ---------------------------------------------------------------------------
// Username
// ---------------------------------------------------------------------------

/// A validated username handle.
///
/// Rules: 3-20 characters, starts with a letter, remaining chars are
/// alphanumeric or underscore.  Stored and compared case-insensitively in the
/// database, but the original casing is preserved here for display.
///
/// Trusted sources (e.g. DB rows that were already validated on write) may
/// construct directly via `Username(s)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Username(pub String);

impl Username {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Username {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::ops::Deref for Username {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Username {
    type Error = crate::domain::errors::DomainError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        if valid_username(&s) {
            Ok(Username(s))
        } else {
            Err(crate::domain::errors::DomainError::InvalidUsername)
        }
    }
}

impl TryFrom<&str> for Username {
    type Error = crate::domain::errors::DomainError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if valid_username(s) {
            Ok(Username(s.to_string()))
        } else {
            Err(crate::domain::errors::DomainError::InvalidUsername)
        }
    }
}

/// Regex: starts with a letter, followed by 2-19 alphanumeric/underscore chars
/// (total 3-20 characters).
static USERNAME_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

/// Validate that a string meets username format requirements.
pub fn valid_username(s: &str) -> bool {
    USERNAME_RE
        .get_or_init(|| regex::Regex::new(r"^[a-zA-Z][a-zA-Z0-9_]{2,19}$").unwrap())
        .is_match(s)
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

    #[test]
    fn valid_usernames() {
        assert!(valid_username("alice"));
        assert!(valid_username("Bob_123"));
        assert!(valid_username("abc")); // min length 3
        assert!(valid_username("abcdefghijklmnopqrst")); // max length 20
        assert!(valid_username("A1_b2_C3"));
    }

    #[test]
    fn invalid_usernames() {
        assert!(!valid_username("")); // empty
        assert!(!valid_username("ab")); // too short (2 chars)
        assert!(!valid_username("1abc")); // starts with digit
        assert!(!valid_username("_abc")); // starts with underscore
        assert!(!valid_username("ab@cd")); // special char
        assert!(!valid_username("ab-cd")); // hyphen not allowed
        assert!(!valid_username("abcdefghijklmnopqrstu")); // 21 chars — too long
    }
}
