//! PostgreSQL implementation of [`WatcherRepository`].
//!
//! Persists watcher verification alerts in `treasury.watcher_alerts`.
//! Each alert is keyed by the transfer ID hex and stores the alert type
//! and a JSON detail blob.

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::error;

use crate::domain::repository::WatcherRepository;
use crate::domain::status::AlertType;

pub struct PgWatcherRepository {
    pool: PgPool,
}

impl PgWatcherRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WatcherRepository for PgWatcherRepository {
    async fn already_verified(&self, transfer_id_hex: &str) -> bool {
        sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM treasury.watcher_alerts WHERE source_event_hash = $1)",
            transfer_id_hex
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(Some(false))
        .unwrap_or(false)
    }

    async fn insert_alert(&self, transfer_id_hex: &str, alert_type: AlertType, detail: &str) {
        if let Err(e) = sqlx::query!(
            r#"
            INSERT INTO treasury.watcher_alerts (source_event_hash, alert_type, detail)
            VALUES ($1, $2, $3)
            ON CONFLICT (source_event_hash) DO NOTHING
            "#,
            transfer_id_hex,
            alert_type.as_str(),
            Some(detail) as Option<&str>,
        )
        .execute(&self.pool)
        .await
        {
            error!(err = %e, "watcher_repo: failed to insert alert");
        }
    }

    async fn get_alert_ids(&self, limit: i64) -> Result<Vec<String>, String> {
        let ids = sqlx::query_scalar!(
            "SELECT id FROM treasury.watcher_alerts ORDER BY created_at DESC LIMIT $1",
            limit
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        Ok(ids.iter().map(|id| id.to_string()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::status::AlertType;
    use sqlx::PgPool;

    #[sqlx::test(migrations = "migrations")]
    async fn insert_and_check_alert(pool: PgPool) {
        let repo = PgWatcherRepository::new(pool);
        let transfer_id = "0xdeadbeef";

        assert!(!repo.already_verified(transfer_id).await);
        repo.insert_alert(transfer_id, AlertType::Verified, r#"{"ok": true}"#)
            .await;
        assert!(repo.already_verified(transfer_id).await);
    }

    #[sqlx::test(migrations = "migrations")]
    async fn insert_idempotent_on_conflict(pool: PgPool) {
        let repo = PgWatcherRepository::new(pool);
        let transfer_id = "0xdeadbeef2";

        repo.insert_alert(transfer_id, AlertType::Verified, "{}")
            .await;
        repo.insert_alert(transfer_id, AlertType::Mismatch, "{}")
            .await; // ON CONFLICT DO NOTHING
        let ids = repo.get_alert_ids(10).await.unwrap();
        // only one alert inserted
        assert_eq!(ids.len(), 1);
    }

    #[sqlx::test(migrations = "migrations")]
    async fn get_alert_ids_respects_limit(pool: PgPool) {
        let repo = PgWatcherRepository::new(pool);
        for i in 0..5 {
            let id = format!("0xtx{i}");
            repo.insert_alert(&id, AlertType::Verified, "{}").await;
        }
        let ids = repo.get_alert_ids(3).await.unwrap();
        assert_eq!(ids.len(), 3);
    }

    #[sqlx::test(migrations = "migrations")]
    async fn mismatch_alert_persisted(pool: PgPool) {
        let repo = PgWatcherRepository::new(pool);
        repo.insert_alert(
            "0xmismatch",
            AlertType::Mismatch,
            r#"{"reason":"amount_mismatch"}"#,
        )
        .await;
        assert!(repo.already_verified("0xmismatch").await);
    }
}
