//! Database infrastructure layer for user-service.
//!
//! Provides PostgreSQL implementations of the repository traits and a shared
//! helper for mapping sqlx errors to [`DomainError`] variants.

pub mod credentials;
pub mod users;

use sqlx::PgPool;

use crate::domain::errors::DomainError;

pub use credentials::PgCredentialRepository;
pub use users::PgUserRepository;

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("SET search_path = users").execute(conn).await?;
                Ok(())
            })
        })
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Convert a sqlx error to a domain error.
///
/// Maps well-known Postgres error codes to semantic variants:
/// - `23505` (unique_violation) → [`DomainError::AlreadyExists`]
/// - `23503` (foreign_key_violation) → [`DomainError::UserNotFound`]
///   (e.g. inserting a credential for a non-existent user)
///
/// All other errors become [`DomainError::Infrastructure`].
pub(crate) fn sqlx_to_domain(e: sqlx::Error) -> DomainError {
    if let Some(db_err) = e.as_database_error() {
        match db_err.code().as_deref() {
            Some("23505") => return DomainError::AlreadyExists,
            Some("23503") => return DomainError::UserNotFound,
            _ => {}
        }
    }
    DomainError::Infrastructure(e.to_string())
}
