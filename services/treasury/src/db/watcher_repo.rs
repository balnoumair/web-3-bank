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
        sqlx::query("SELECT 1 FROM treasury.watcher_alerts WHERE source_event_hash = $1")
            .bind(transfer_id_hex)
            .fetch_optional(&self.pool)
            .await
            .map(|r| r.is_some())
            .unwrap_or(false)
    }

    async fn insert_alert(&self, transfer_id_hex: &str, alert_type: AlertType, detail: &str) {
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO treasury.watcher_alerts (source_event_hash, alert_type, detail)
            VALUES ($1, $2, $3)
            ON CONFLICT (source_event_hash) DO NOTHING
            "#,
        )
        .bind(transfer_id_hex)
        .bind(alert_type.as_str())
        .bind(detail)
        .execute(&self.pool)
        .await
        {
            error!(err = %e, "watcher_repo: failed to insert alert");
        }
    }

    async fn get_alert_ids(&self, limit: i64) -> Result<Vec<String>, String> {
        let rows = sqlx::query(
            "SELECT id FROM treasury.watcher_alerts \
             ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| e.to_string())?;

        use sqlx::Row;
        Ok(rows
            .iter()
            .map(|r| r.try_get::<i64, _>("id").unwrap_or(0).to_string())
            .collect())
    }
}
