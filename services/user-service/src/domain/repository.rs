//! Repository traits (driven ports) for user-service persistence.
//!
//! These define the storage contracts that the domain layer requires.
//! Implementations live in the infrastructure layer (currently `db/`).

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entities::{Credential, User, UserWithCredential};
use crate::domain::errors::DomainError;
use crate::domain::validation::{TempoAddress, Username};

#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Create a new user with an optional display name.
    async fn insert(&self, display_name: Option<&str>) -> Result<Uuid, DomainError>;

    /// Get a user by ID. Returns `None` if not found.
    async fn get_by_id(&self, id: Uuid) -> Result<Option<User>, DomainError>;

    /// Update the display name for a user.
    async fn update_display_name(&self, id: Uuid, name: &str) -> Result<(), DomainError>;

    /// Set or update the username for a user (case-insensitive uniqueness enforced
    /// at the DB level). Returns `UsernameTaken` if the username is already in use.
    async fn set_username(&self, id: Uuid, username: &Username) -> Result<(), DomainError>;

    /// Look up a user by username (case-insensitive). Returns `None` if not found.
    async fn get_by_username(&self, username: &Username) -> Result<Option<User>, DomainError>;

    /// Stored `home_chain` for a Tempo address (via active credential), if set.
    async fn get_home_chain_by_tempo_address(
        &self,
        tempo_address: &TempoAddress,
    ) -> Result<Option<i64>, DomainError>;

    /// Sets `home_chain` once (first deposit). No-op if unknown user or already set.
    async fn set_home_chain_if_unset(
        &self,
        tempo_address: &TempoAddress,
        chain_id: i64,
    ) -> Result<(), DomainError>;

    /// Governance decommission override. This is the only path that may mutate an existing home_chain.
    async fn set_home_chain_for_decommission(
        &self,
        tempo_address: &TempoAddress,
        chain_id: i64,
        operator: &str,
    ) -> Result<(), DomainError>;
}

#[async_trait]
pub trait CredentialRepository: Send + Sync {
    /// Insert a new credential for a user.
    async fn insert(
        &self,
        user_id: Uuid,
        credential_id: &[u8],
        public_key: &[u8],
        tempo_address: &TempoAddress,
    ) -> Result<Uuid, DomainError>;

    /// Look up a user by their Tempo address via the credentials table.
    async fn get_user_by_address(
        &self,
        tempo_address: &TempoAddress,
    ) -> Result<Option<UserWithCredential>, DomainError>;

    /// Look up a user by their WebAuthn credential ID (raw bytes).
    async fn get_user_by_credential_id(
        &self,
        credential_id: &[u8],
    ) -> Result<Option<UserWithCredential>, DomainError>;

    /// List credentials for a user. When `active_only` is true, excludes
    /// revoked credentials.
    async fn list(&self, user_id: Uuid, active_only: bool) -> Result<Vec<Credential>, DomainError>;

    /// Revoke a credential. Enforces the business rule that a user must
    /// retain at least one active credential.
    async fn revoke(&self, user_id: Uuid, credential_id: &[u8]) -> Result<(), DomainError>;

    /// Look up a user (with their active credential address) by username
    /// (case-insensitive). Returns `None` if not found.
    async fn get_user_by_username(
        &self,
        username: &Username,
    ) -> Result<Option<UserWithCredential>, DomainError>;
}
