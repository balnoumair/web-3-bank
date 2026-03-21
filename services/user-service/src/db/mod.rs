pub mod credentials;
pub mod users;

use sqlx::PgPool;

use crate::domain::errors::DomainError;

pub use credentials::PgCredentialRepository;
pub use users::PgUserRepository;

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Convert a sqlx error to a domain error, mapping unique constraint
/// violations to `AlreadyExists`.
pub(crate) fn sqlx_to_domain(e: sqlx::Error) -> DomainError {
    if let Some(db_err) = e.as_database_error() {
        if db_err.code().as_deref() == Some("23505") {
            return DomainError::AlreadyExists;
        }
    }
    DomainError::Infrastructure(e.to_string())
}
