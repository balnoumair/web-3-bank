//! Domain entities for user management.
//!
//! These are clean domain types free of infrastructure concerns
//! (no sqlx derives, no proto types). Infrastructure adapters convert
//! to/from these types at the boundary.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::errors::DomainError;
use crate::domain::validation::TempoAddress;

/// User account status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserStatus {
    Active,
    Inactive,
}

impl UserStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserStatus::Active => "active",
            UserStatus::Inactive => "inactive",
        }
    }
}

impl std::fmt::Display for UserStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for UserStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(UserStatus::Active),
            "inactive" => Ok(UserStatus::Inactive),
            other => Err(format!("unknown user status: {other}")),
        }
    }
}

/// Core user entity.
#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub display_name: String,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    /// Construct a new active `User` with the given ID and optional display name.
    ///
    /// Intended for in-memory construction (e.g., tests, application layer).
    /// The DB adapter uses [`From<UserRow>`] to reconstruct persisted users.
    pub fn new(id: Uuid, display_name: Option<String>, now: DateTime<Utc>) -> Self {
        User {
            id,
            display_name: display_name.unwrap_or_default(),
            status: UserStatus::Active,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a new [`Credential`] bound to this user, validating the tempo address.
    ///
    /// Credential operations are routed through the aggregate root so that
    /// domain invariants (e.g., valid address format) are enforced in one place
    /// rather than scattered across gRPC handlers.
    pub fn add_credential(
        &self,
        credential_id: Vec<u8>,
        public_key: Vec<u8>,
        tempo_address: String,
    ) -> Result<Credential, DomainError> {
        Credential::new(self.id, credential_id, public_key, tempo_address)
    }
}

/// A WebAuthn credential bound to a user.
#[derive(Debug, Clone)]
pub struct Credential {
    pub id: Uuid,
    pub user_id: Uuid,
    pub credential_id: Vec<u8>,
    pub public_key: Vec<u8>,
    pub tempo_address: TempoAddress,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl Credential {
    /// Construct a new active credential, validating the tempo address.
    ///
    /// Returns [`DomainError::InvalidTempoAddress`] if `tempo_address` is not a
    /// valid 0x-prefixed 40-char hex string.
    pub fn new(
        user_id: Uuid,
        credential_id: Vec<u8>,
        public_key: Vec<u8>,
        tempo_address: String,
    ) -> Result<Self, DomainError> {
        let tempo_address = TempoAddress::try_from(tempo_address)?;
        let now = Utc::now();
        Ok(Credential {
            id: Uuid::new_v4(),
            user_id,
            credential_id,
            public_key,
            tempo_address,
            revoked_at: None,
            created_at: now,
        })
    }

    /// Whether this credential is currently active (not revoked).
    pub fn is_active(&self) -> bool {
        self.revoked_at.is_none()
    }
}

/// A projection joining user and credential data, used for
/// address-based user lookups.
#[derive(Debug, Clone)]
pub struct UserWithCredential {
    pub user_id: Uuid,
    pub display_name: String,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
    pub tempo_address: TempoAddress,
}
