//! Repository traits (driven ports) for user-service persistence.
//!
//! These define the storage contracts that the domain layer requires.
//! Implementations live in the infrastructure layer (currently `db/`).

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entities::{Credential, User, UserWithCredential};
use crate::domain::errors::DomainError;
use crate::domain::validation::TempoAddress;

#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Create a new user with an optional display name.
    async fn insert(&self, display_name: Option<&str>) -> Result<Uuid, DomainError>;

    /// Get a user by ID. Returns `None` if not found.
    async fn get_by_id(&self, id: Uuid) -> Result<Option<User>, DomainError>;

    /// Update the display name for a user.
    async fn update_display_name(&self, id: Uuid, name: &str) -> Result<(), DomainError>;
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

    /// List credentials for a user. When `active_only` is true, excludes
    /// revoked credentials.
    async fn list(
        &self,
        user_id: Uuid,
        active_only: bool,
    ) -> Result<Vec<Credential>, DomainError>;

    /// Revoke a credential. Enforces the business rule that a user must
    /// retain at least one active credential.
    async fn revoke(&self, user_id: Uuid, credential_id: &[u8]) -> Result<(), DomainError>;
}
