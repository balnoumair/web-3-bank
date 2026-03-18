use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct CredentialRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub credential_id: Vec<u8>,
    pub public_key: Vec<u8>,
    pub tempo_address: String,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct UserWithCredential {
    pub user_id: Uuid,
    pub display_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("last active credential — cannot revoke")]
    LastActiveCredential,
    #[error("credential not found")]
    NotFound,
}

/// Insert a credential. Caller must verify user existence before calling.
pub async fn insert_credential(
    pool: &PgPool,
    user_id: Uuid,
    credential_id: &[u8],
    public_key: &[u8],
    tempo_address: &str,
) -> Result<Uuid, CredentialError> {
    let row = sqlx::query!(
        "INSERT INTO users.credentials (user_id, credential_id, public_key, tempo_address)
         VALUES ($1, $2, $3, $4) RETURNING id",
        user_id,
        credential_id,
        public_key,
        tempo_address,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

/// Resolve a Tempo address to user info via credentials → users JOIN.
pub async fn get_user_by_address(
    pool: &PgPool,
    tempo_address: &str,
) -> Result<Option<UserWithCredential>, sqlx::Error> {
    sqlx::query_as!(
        UserWithCredential,
        "SELECT u.id AS user_id, u.display_name, u.status, u.created_at
         FROM users.credentials c
         JOIN users.users u ON u.id = c.user_id
         WHERE c.tempo_address = $1 AND c.revoked_at IS NULL",
        tempo_address,
    )
    .fetch_optional(pool)
    .await
}

/// List credentials for a user. Pass `active_only = true` to exclude revoked ones.
pub async fn list_credentials(
    pool: &PgPool,
    user_id: Uuid,
    active_only: bool,
) -> Result<Vec<CredentialRow>, sqlx::Error> {
    if active_only {
        sqlx::query_as!(
            CredentialRow,
            "SELECT id, user_id, credential_id, public_key, tempo_address, revoked_at, created_at
             FROM users.credentials
             WHERE user_id = $1 AND revoked_at IS NULL
             ORDER BY created_at ASC",
            user_id,
        )
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as!(
            CredentialRow,
            "SELECT id, user_id, credential_id, public_key, tempo_address, revoked_at, created_at
             FROM users.credentials
             WHERE user_id = $1
             ORDER BY created_at ASC",
            user_id,
        )
        .fetch_all(pool)
        .await
    }
}

/// Revoke a credential.
/// - `LastActiveCredential` if this would leave the user with zero active credentials.
/// - `NotFound` if the credential doesn't exist / already revoked for this user.
/// Uses SELECT ... FOR UPDATE inside a transaction to prevent concurrent races.
pub async fn revoke_credential(
    pool: &PgPool,
    user_id: Uuid,
    credential_id: &[u8],
) -> Result<(), CredentialError> {
    let mut tx = pool.begin().await?;

    // Lock all active credentials for this user to prevent concurrent revocations.
    let active = sqlx::query!(
        "SELECT id FROM users.credentials
         WHERE user_id = $1 AND revoked_at IS NULL
         FOR UPDATE",
        user_id,
    )
    .fetch_all(&mut *tx)
    .await?;

    if active.len() == 1 {
        drop(tx);
        return Err(CredentialError::LastActiveCredential);
    }

    let updated = sqlx::query!(
        "UPDATE users.credentials
         SET revoked_at = now()
         WHERE user_id = $1 AND credential_id = $2 AND revoked_at IS NULL",
        user_id,
        credential_id,
    )
    .execute(&mut *tx)
    .await?;

    if updated.rows_affected() == 0 {
        drop(tx);
        return Err(CredentialError::NotFound);
    }

    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::users::insert_user;

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_insert_and_get_by_address(pool: PgPool) {
        let user_id = insert_user(&pool, Some("Alice")).await.unwrap();
        insert_credential(&pool, user_id, b"cred-bytes", b"pk-bytes",
            "0xabcdef1234567890abcdef1234567890abcdef12").await.unwrap();

        let row = get_user_by_address(&pool, "0xabcdef1234567890abcdef1234567890abcdef12")
            .await.unwrap().expect("should find user");
        assert_eq!(row.user_id, user_id);
        assert_eq!(row.display_name, "Alice");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_get_user_by_address_not_found(pool: PgPool) {
        let result = get_user_by_address(&pool, "0x0000000000000000000000000000000000000000")
            .await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_duplicate_address_rejected(pool: PgPool) {
        let user_id = insert_user(&pool, None).await.unwrap();
        let addr = "0xabcdef1234567890abcdef1234567890abcdef12";
        insert_credential(&pool, user_id, b"cred-1", b"pk", addr).await.unwrap();
        let err = insert_credential(&pool, user_id, b"cred-2", b"pk", addr).await.unwrap_err();
        // Postgres unique constraint violation (error code 23505)
        assert!(matches!(err, CredentialError::Db(_)));
        let db_err = match err { CredentialError::Db(e) => e, _ => panic!("expected Db error") };
        let pg_err = db_err.as_database_error().expect("expected database error");
        assert_eq!(pg_err.code().as_deref(), Some("23505"), "expected unique violation");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_list_credentials_active_only(pool: PgPool) {
        let user_id = insert_user(&pool, None).await.unwrap();
        let addr1 = "0xaaaa111111111111111111111111111111111111";
        let addr2 = "0xbbbb222222222222222222222222222222222222";
        insert_credential(&pool, user_id, b"cred1", b"pk1", addr1).await.unwrap();
        insert_credential(&pool, user_id, b"cred2", b"pk2", addr2).await.unwrap();

        revoke_credential(&pool, user_id, b"cred1").await.unwrap();

        let all = list_credentials(&pool, user_id, false).await.unwrap();
        assert_eq!(all.len(), 2);

        let active = list_credentials(&pool, user_id, true).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].credential_id, b"cred2");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_revoke_last_credential_fails(pool: PgPool) {
        let user_id = insert_user(&pool, None).await.unwrap();
        insert_credential(&pool, user_id, b"only-cred", b"pk",
            "0xcccc333333333333333333333333333333333333").await.unwrap();

        let err = revoke_credential(&pool, user_id, b"only-cred").await.unwrap_err();
        assert!(matches!(err, CredentialError::LastActiveCredential));
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_revoke_nonexistent_credential_fails(pool: PgPool) {
        let user_id = insert_user(&pool, None).await.unwrap();
        insert_credential(&pool, user_id, b"cred1", b"pk1",
            "0xaaaa111111111111111111111111111111111111").await.unwrap();
        insert_credential(&pool, user_id, b"cred2", b"pk2",
            "0xbbbb222222222222222222222222222222222222").await.unwrap();

        let result = revoke_credential(&pool, user_id, b"nonexistent").await;
        assert!(matches!(result.unwrap_err(), CredentialError::NotFound));
    }
}
