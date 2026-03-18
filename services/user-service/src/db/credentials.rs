use sqlx::PgPool;
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub struct Credential {
    pub id: Uuid,
    pub user_id: Uuid,
    pub credential_id: Vec<u8>,
    pub tempo_address: String,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub async fn insert_credential(
    pool: &PgPool,
    user_id: Uuid,
    credential_id: &[u8],
    public_key: &[u8],
    tempo_address: &str,
) -> Result<Credential, sqlx::Error> {
    todo!()
}

pub async fn get_user_by_address(
    pool: &PgPool,
    tempo_address: &str,
) -> Result<Option<(crate::db::users::UserRow, Credential)>, sqlx::Error> {
    todo!()
}

pub async fn list_credentials(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<Credential>, sqlx::Error> {
    todo!()
}

pub async fn revoke_credential(
    pool: &PgPool,
    user_id: Uuid,
    credential_id: &[u8],
) -> Result<(), sqlx::Error> {
    todo!()
}
