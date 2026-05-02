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
use crate::domain::validation::Username;

#[derive(Debug, sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    display_name: String,
    username: Option<String>,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        User {
            id: row.id,
            display_name: row.display_name,
            username: row.username.map(Username), // DB data assumed valid
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
            "SELECT id, display_name, username, status, created_at, updated_at
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

    async fn set_username(&self, id: Uuid, username: &Username) -> Result<(), DomainError> {
        sqlx::query!(
            "UPDATE users.users SET username = $1, updated_at = now() WHERE id = $2",
            username.as_str(),
            id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            // The UNIQUE index on LOWER(username) fires a 23505 here, but it
            // means "username taken" — not a generic AlreadyExists.
            if let Some(db_err) = e.as_database_error() {
                if db_err.code().as_deref() == Some("23505") {
                    return DomainError::UsernameTaken;
                }
            }
            sqlx_to_domain(e)
        })?;
        Ok(())
    }

    async fn get_by_username(&self, username: &Username) -> Result<Option<User>, DomainError> {
        let row = sqlx::query_as!(
            UserRow,
            "SELECT id, display_name, username, status, created_at, updated_at
             FROM users.users WHERE LOWER(username) = LOWER($1)",
            username.as_str()
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(sqlx_to_domain)?;
        Ok(row.map(User::from))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
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
        assert!(user.username.is_none());
    }

    #[sqlx::test]
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

    #[sqlx::test]
    async fn test_get_user_by_id_not_found(pool: PgPool) {
        let repo = PgUserRepository::new(pool);
        let result = repo.get_by_id(Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test]
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

    #[sqlx::test]
    async fn test_set_and_get_username(pool: PgPool) {
        let repo = PgUserRepository::new(pool);
        let id = repo.insert(Some("Alice")).await.unwrap();
        let uname = Username::try_from("alice123").unwrap();
        repo.set_username(id, &uname).await.unwrap();

        let user = repo
            .get_by_id(id)
            .await
            .unwrap()
            .expect("user should exist");
        assert_eq!(user.username.unwrap().as_str(), "alice123");
    }

    #[sqlx::test]
    async fn test_get_by_username_case_insensitive(pool: PgPool) {
        let repo = PgUserRepository::new(pool);
        let id = repo.insert(Some("Alice")).await.unwrap();
        repo.set_username(id, &Username::try_from("Alice123").unwrap())
            .await
            .unwrap();

        let found = repo
            .get_by_username(&Username::try_from("alice123").unwrap())
            .await
            .unwrap()
            .expect("should find user");
        assert_eq!(found.id, id);
    }

    #[sqlx::test]
    async fn test_get_by_username_not_found(pool: PgPool) {
        let repo = PgUserRepository::new(pool);
        let result = repo
            .get_by_username(&Username::try_from("nobody123").unwrap())
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test]
    async fn test_username_taken_case_insensitive(pool: PgPool) {
        let repo = PgUserRepository::new(pool);
        let id1 = repo.insert(None).await.unwrap();
        let id2 = repo.insert(None).await.unwrap();

        repo.set_username(id1, &Username::try_from("Alice123").unwrap())
            .await
            .unwrap();

        let err = repo
            .set_username(id2, &Username::try_from("alice123").unwrap())
            .await
            .unwrap_err();
        assert!(matches!(err, DomainError::UsernameTaken));
    }
}
