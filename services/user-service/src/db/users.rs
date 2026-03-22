//! PostgreSQL implementation of [`UserRepository`].
//!
//! Creates and retrieves user accounts from the `users` table.
//! Display names default to an empty string when not provided.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::sqlx_to_domain;
use crate::domain::entities::{User, UserStatus};
use crate::domain::errors::DomainError;
use crate::domain::repository::UserRepository;

#[derive(Debug, sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    display_name: String,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        User {
            id: row.id,
            display_name: row.display_name,
            status: row.status.parse().unwrap_or(UserStatus::Active),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

pub struct PgUserRepository {
    pool: PgPool,
}

impl PgUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PgUserRepository {
    async fn insert(&self, display_name: Option<&str>) -> Result<Uuid, DomainError> {
        let name = display_name.unwrap_or("");
        let row = sqlx::query!(
            "INSERT INTO users.users (display_name) VALUES ($1) RETURNING id",
            name
        )
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx_to_domain)?;
        Ok(row.id)
    }

    async fn get_by_id(&self, id: Uuid) -> Result<Option<User>, DomainError> {
        let row = sqlx::query_as!(
            UserRow,
            "SELECT id, display_name, status, created_at, updated_at
         FROM users.users WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_to_domain)?;
        Ok(row.map(User::from))
    }

    async fn update_display_name(&self, id: Uuid, name: &str) -> Result<(), DomainError> {
        sqlx::query!(
            "UPDATE users.users SET display_name = $1, updated_at = now() WHERE id = $2",
            name,
            id
        )
        .execute(&self.pool)
        .await
        .map_err(sqlx_to_domain)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_insert_user_default_display_name(pool: PgPool) {
        let repo = PgUserRepository::new(pool);
        let id = repo.insert(None).await.unwrap();
        let user = repo
            .get_by_id(id)
            .await
            .unwrap()
            .expect("user should exist");
        assert_eq!(user.display_name, "");
        assert_eq!(user.status, UserStatus::Active);
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_insert_user_with_display_name(pool: PgPool) {
        let repo = PgUserRepository::new(pool);
        let id = repo.insert(Some("Alice")).await.unwrap();
        let user = repo
            .get_by_id(id)
            .await
            .unwrap()
            .expect("user should exist");
        assert_eq!(user.display_name, "Alice");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_get_user_by_id_not_found(pool: PgPool) {
        let repo = PgUserRepository::new(pool);
        let result = repo.get_by_id(Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_update_display_name(pool: PgPool) {
        let repo = PgUserRepository::new(pool);
        let id = repo.insert(Some("Old Name")).await.unwrap();
        repo.update_display_name(id, "New Name").await.unwrap();
        let user = repo
            .get_by_id(id)
            .await
            .unwrap()
            .expect("user should exist");
        assert_eq!(user.display_name, "New Name");
    }
}
