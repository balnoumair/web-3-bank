use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub display_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn insert_user(pool: &PgPool, display_name: Option<&str>) -> Result<Uuid, sqlx::Error> {
    let name = display_name.unwrap_or("");
    let row = sqlx::query!(
        "INSERT INTO users.users (display_name) VALUES ($1) RETURNING id",
        name
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn get_user_by_id(pool: &PgPool, id: Uuid) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as!(
        UserRow,
        "SELECT id, display_name, status, created_at, updated_at
         FROM users.users WHERE id = $1",
        id
    )
    .fetch_optional(pool)
    .await
}

pub async fn update_display_name(
    pool: &PgPool,
    id: Uuid,
    display_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE users.users SET display_name = $1, updated_at = now() WHERE id = $2",
        display_name,
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_insert_user_default_display_name(pool: PgPool) {
        let id = insert_user(&pool, None).await.unwrap();
        let row = get_user_by_id(&pool, id).await.unwrap().expect("user should exist");
        assert_eq!(row.display_name, "");
        assert_eq!(row.status, "active");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_insert_user_with_display_name(pool: PgPool) {
        let id = insert_user(&pool, Some("Alice")).await.unwrap();
        let row = get_user_by_id(&pool, id).await.unwrap().expect("user should exist");
        assert_eq!(row.display_name, "Alice");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_get_user_by_id_not_found(pool: PgPool) {
        let result = get_user_by_id(&pool, Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_update_display_name(pool: PgPool) {
        let id = insert_user(&pool, Some("Old Name")).await.unwrap();
        update_display_name(&pool, id, "New Name").await.unwrap();
        let row = get_user_by_id(&pool, id).await.unwrap().expect("user should exist");
        assert_eq!(row.display_name, "New Name");
    }
}
