use sqlx::PgPool;
use uuid::Uuid;

pub struct User {
    pub id: Uuid,
    pub display_name: String,
    pub status: String,
}

pub async fn insert_user(
    pool: &PgPool,
    display_name: &str,
) -> Result<User, sqlx::Error> {
    todo!()
}

pub async fn get_user_by_id(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<User>, sqlx::Error> {
    todo!()
}

pub async fn update_display_name(
    pool: &PgPool,
    user_id: Uuid,
    display_name: &str,
) -> Result<User, sqlx::Error> {
    todo!()
}
