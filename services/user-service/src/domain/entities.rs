//! Domain entities for user management.
//!
//! These are clean domain types free of infrastructure concerns
//! (no sqlx derives, no proto types). Infrastructure adapters convert
//! to/from these types at the boundary.

use chrono::{DateTime, Utc};
use uuid::Uuid;

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

/// A WebAuthn credential bound to a user.
#[derive(Debug, Clone)]
pub struct Credential {
    pub id: Uuid,
    pub user_id: Uuid,
    pub credential_id: Vec<u8>,
    pub public_key: Vec<u8>,
    pub tempo_address: String,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl Credential {
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
    pub tempo_address: String,
}
