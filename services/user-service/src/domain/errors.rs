//! Domain errors for user and credential operations.
//!
//! These represent business rule violations independent of
//! infrastructure. Infrastructure-specific errors (e.g. database)
//! are mapped at the adapter layer.

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

    #[error("{0}")]
    Infrastructure(String),
}
