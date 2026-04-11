//! Domain errors for user and credential operations.
//!
//! These represent business rule violations independent of
//! infrastructure. Infrastructure-specific errors (e.g. database)
//! are mapped at the adapter layer.
//!
//! # Unified error hierarchy
//!
//! All credential-specific errors (e.g. [`DomainError::LastActiveCredential`],
//! [`DomainError::CredentialNotFound`]) are variants here rather than a
//! separate `CredentialError` type, keeping the hierarchy flat and avoiding
//! the need for `From` conversions between hierarchies.

/// Unified domain error type for user and credential operations.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("last active credential — cannot revoke")]
    LastActiveCredential,

    #[error("credential not found")]
    CredentialNotFound,

    #[error("user not found")]
    UserNotFound,

    #[error("address or credential already registered")]
    AlreadyExists,

    #[error("invalid tempo address: must be 0x-prefixed 40-char hex")]
    InvalidTempoAddress,

    #[error("invalid username: must be 3-20 chars, start with a letter, alphanumeric/underscore only")]
    InvalidUsername,

    #[error("username already taken")]
    UsernameTaken,

    #[error("infrastructure: {0}")]
    Infrastructure(String),
}
