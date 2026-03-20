pub mod credentials;
pub mod users;

use sqlx::PgPool;

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;
    Ok(pool)
}
