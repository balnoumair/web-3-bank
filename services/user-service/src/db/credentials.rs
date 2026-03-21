//! PostgreSQL implementation of [`CredentialRepository`].
//!
//! Persists WebAuthn credentials bound to users in the `credentials` table.
//! Enforces the "last active credential" invariant at the database layer
//! using a transaction that counts active credentials before revoking.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::sqlx_to_domain;
use crate::domain::entities::{Credential, UserStatus, UserWithCredential};
use crate::domain::errors::DomainError;
use crate::domain::repository::CredentialRepository;
use crate::domain::validation::TempoAddress;

#[derive(Debug, sqlx::FromRow)]
struct CredentialRow {
    id: Uuid,
    user_id: Uuid,
    credential_id: Vec<u8>,
    public_key: Vec<u8>,
    tempo_address: String,
    revoked_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

impl From<CredentialRow> for Credential {
    fn from(row: CredentialRow) -> Self {
        Credential {
            id: row.id,
            user_id: row.user_id,
            credential_id: row.credential_id,
            public_key: row.public_key,
            tempo_address: TempoAddress(row.tempo_address), // DB data assumed valid
            revoked_at: row.revoked_at,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct UserWithCredentialRow {
    user_id: Uuid,
    display_name: String,
    status: String,
    created_at: DateTime<Utc>,
    tempo_address: String,
}

impl From<UserWithCredentialRow> for UserWithCredential {
    fn from(row: UserWithCredentialRow) -> Self {
        UserWithCredential {
            user_id: row.user_id,
            display_name: row.display_name,
            status: row.status.parse().unwrap_or(UserStatus::Active),
            created_at: row.created_at,
            tempo_address: TempoAddress(row.tempo_address), // DB data assumed valid
        }
    }
}

pub struct PgCredentialRepository {
    pool: PgPool,
}

impl PgCredentialRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CredentialRepository for PgCredentialRepository {
    async fn insert(
        &self,
        user_id: Uuid,
        credential_id: &[u8],
        public_key: &[u8],
        tempo_address: &TempoAddress,
    ) -> Result<Uuid, DomainError> {
        let row = sqlx::query!(
            "INSERT INTO users.credentials (user_id, credential_id, public_key, tempo_address)
         VALUES ($1, $2, $3, $4) RETURNING id",
            user_id,
            credential_id,
            public_key,
            tempo_address.as_str(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_to_domain)?;
        Ok(row.id)
    }

    async fn get_user_by_address(
        &self,
        tempo_address: &TempoAddress,
    ) -> Result<Option<UserWithCredential>, DomainError> {
        let row = sqlx::query_as!(
            UserWithCredentialRow,
            "SELECT u.id AS user_id, u.display_name, u.status, u.created_at, c.tempo_address
         FROM users.credentials c
         JOIN users.users u ON u.id = c.user_id
         WHERE c.tempo_address = $1 AND c.revoked_at IS NULL",
            tempo_address.as_str(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_to_domain)?;
        Ok(row.map(UserWithCredential::from))
    }

    async fn list(
        &self,
        user_id: Uuid,
        active_only: bool,
    ) -> Result<Vec<Credential>, DomainError> {
        let rows = if active_only {
            sqlx::query_as!(
                CredentialRow,
                "SELECT id, user_id, credential_id, public_key, tempo_address, revoked_at, created_at
             FROM users.credentials
             WHERE user_id = $1 AND revoked_at IS NULL
             ORDER BY created_at ASC",
                user_id,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(sqlx_to_domain)?
        } else {
            sqlx::query_as!(
                CredentialRow,
                "SELECT id, user_id, credential_id, public_key, tempo_address, revoked_at, created_at
             FROM users.credentials
             WHERE user_id = $1
             ORDER BY created_at ASC",
                user_id,
            )
            .fetch_all(&self.pool)
            .await
            .map_err(sqlx_to_domain)?
        };
        Ok(rows.into_iter().map(Credential::from).collect())
    }

    async fn revoke(&self, user_id: Uuid, credential_id: &[u8]) -> Result<(), DomainError> {
        let mut tx = self.pool.begin().await.map_err(sqlx_to_domain)?;

        let active = sqlx::query!(
            "SELECT id FROM users.credentials
         WHERE user_id = $1 AND revoked_at IS NULL
         FOR UPDATE",
            user_id,
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(sqlx_to_domain)?;

        if active.len() == 1 {
            drop(tx);
            return Err(DomainError::LastActiveCredential);
        }

        let updated = sqlx::query!(
            "UPDATE users.credentials
         SET revoked_at = now()
         WHERE user_id = $1 AND credential_id = $2 AND revoked_at IS NULL",
            user_id,
            credential_id,
        )
        .execute(&mut *tx)
        .await
        .map_err(sqlx_to_domain)?;

        if updated.rows_affected() == 0 {
            drop(tx);
            return Err(DomainError::CredentialNotFound);
        }

        tx.commit().await.map_err(sqlx_to_domain)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::users::PgUserRepository;
    use crate::domain::repository::UserRepository;
    use crate::domain::validation::TempoAddress;

    fn addr(s: &str) -> TempoAddress { TempoAddress::try_from(s).unwrap() }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_insert_and_get_by_address(pool: PgPool) {
        let user_repo = PgUserRepository::new(pool.clone());
        let cred_repo = PgCredentialRepository::new(pool);

        let user_id = user_repo.insert(Some("Alice")).await.unwrap();
        cred_repo
            .insert(user_id, b"cred-bytes", b"pk-bytes", &addr("0xabcdef1234567890abcdef1234567890abcdef12"))
            .await
            .unwrap();

        let row = cred_repo
            .get_user_by_address(&addr("0xabcdef1234567890abcdef1234567890abcdef12"))
            .await
            .unwrap()
            .expect("should find user");
        assert_eq!(row.user_id, user_id);
        assert_eq!(row.display_name, "Alice");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_get_user_by_address_not_found(pool: PgPool) {
        let cred_repo = PgCredentialRepository::new(pool);
        let result = cred_repo
            .get_user_by_address(&addr("0x0000000000000000000000000000000000000000"))
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_duplicate_address_rejected(pool: PgPool) {
        let user_repo = PgUserRepository::new(pool.clone());
        let cred_repo = PgCredentialRepository::new(pool);

        let user_id = user_repo.insert(None).await.unwrap();
        let a = addr("0xabcdef1234567890abcdef1234567890abcdef12");
        cred_repo.insert(user_id, b"cred-1", b"pk", &a).await.unwrap();
        let err = cred_repo.insert(user_id, b"cred-2", b"pk", &a).await.unwrap_err();
        assert!(matches!(err, DomainError::AlreadyExists));
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_list_credentials_active_only(pool: PgPool) {
        let user_repo = PgUserRepository::new(pool.clone());
        let cred_repo = PgCredentialRepository::new(pool);

        let user_id = user_repo.insert(None).await.unwrap();
        cred_repo.insert(user_id, b"cred1", b"pk1", &addr("0xaaaa111111111111111111111111111111111111")).await.unwrap();
        cred_repo.insert(user_id, b"cred2", b"pk2", &addr("0xbbbb222222222222222222222222222222222222")).await.unwrap();

        cred_repo.revoke(user_id, b"cred1").await.unwrap();

        let all = cred_repo.list(user_id, false).await.unwrap();
        assert_eq!(all.len(), 2);

        let active = cred_repo.list(user_id, true).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].credential_id, b"cred2");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_revoke_last_credential_fails(pool: PgPool) {
        let user_repo = PgUserRepository::new(pool.clone());
        let cred_repo = PgCredentialRepository::new(pool);

        let user_id = user_repo.insert(None).await.unwrap();
        cred_repo
            .insert(user_id, b"only-cred", b"pk", &addr("0xcccc333333333333333333333333333333333333"))
            .await
            .unwrap();

        let err = cred_repo.revoke(user_id, b"only-cred").await.unwrap_err();
        assert!(matches!(err, DomainError::LastActiveCredential));
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_revoke_nonexistent_credential_fails(pool: PgPool) {
        let user_repo = PgUserRepository::new(pool.clone());
        let cred_repo = PgCredentialRepository::new(pool);

        let user_id = user_repo.insert(None).await.unwrap();
        cred_repo.insert(user_id, b"cred1", b"pk1", &addr("0xaaaa111111111111111111111111111111111111")).await.unwrap();
        cred_repo.insert(user_id, b"cred2", b"pk2", &addr("0xbbbb222222222222222222222222222222222222")).await.unwrap();

        let result = cred_repo.revoke(user_id, b"nonexistent").await;
        assert!(matches!(result.unwrap_err(), DomainError::CredentialNotFound));
    }
}
