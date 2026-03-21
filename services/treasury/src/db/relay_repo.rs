use async_trait::async_trait;
use sqlx::PgPool;
use tracing::error;

use crate::domain::events::HotPathEvent;
use crate::domain::repository::RelayRepository;
use crate::domain::status::RelayStatus;
use crate::eth;

pub struct PgRelayRepository {
    pool: PgPool,
}

impl PgRelayRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RelayRepository for PgRelayRepository {
    async fn relay_already_recorded(&self, source_event_hash: &str) -> bool {
        sqlx::query("SELECT 1 FROM treasury.relay_logs WHERE source_event_hash = $1")
            .bind(source_event_hash)
            .fetch_optional(&self.pool)
            .await
            .map(|r| r.is_some())
            .unwrap_or(false)
    }

    async fn insert_relay_log(
        &self,
        event: &HotPathEvent,
        dest_tx_hash: Option<&str>,
        status: RelayStatus,
    ) {
        let amount_str = event.amount.to_string();
        let recipient_str = format!("0x{}", eth::bytes_to_hex(event.recipient.as_slice()));
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO treasury.relay_logs
                (source_event_hash, dest_tx_hash, source_chain_id, dest_chain_id,
                 recipient, amount_wei, status)
            VALUES ($1, $2, $3, $4, $5, $6::NUMERIC, $7)
            ON CONFLICT (source_event_hash) DO NOTHING
            "#,
        )
        .bind(&event.source_event_hash)
        .bind(dest_tx_hash)
        .bind(event.source_chain_id as i64)
        .bind(event.dest_chain_id as i64)
        .bind(&recipient_str)
        .bind(&amount_str)
        .bind(status.as_str())
        .execute(&self.pool)
        .await
        {
            error!(err = %e, "relay_repo: failed to insert relay log");
        }
    }

    async fn update_relay_log(
        &self,
        source_event_hash: &str,
        dest_tx_hash: &str,
        status: RelayStatus,
    ) {
        if let Err(e) = sqlx::query(
            "UPDATE treasury.relay_logs \
             SET dest_tx_hash = $1, status = $2, updated_at = now() \
             WHERE source_event_hash = $3",
        )
        .bind(dest_tx_hash)
        .bind(status.as_str())
        .bind(source_event_hash)
        .execute(&self.pool)
        .await
        {
            error!(err = %e, "relay_repo: failed to update relay log");
        }
    }

    async fn update_relay_log_failed(&self, source_event_hash: &str) {
        if let Err(e) = sqlx::query(
            "UPDATE treasury.relay_logs \
             SET status = 'failed', updated_at = now() \
             WHERE source_event_hash = $1",
        )
        .bind(source_event_hash)
        .execute(&self.pool)
        .await
        {
            error!(err = %e, "relay_repo: failed to mark relay log as failed");
        }
    }

    async fn get_relay_status(&self, source_event_hash: &str) -> Option<String> {
        let row =
            sqlx::query("SELECT status FROM treasury.relay_logs WHERE source_event_hash = $1")
                .bind(source_event_hash)
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten()?;

        use sqlx::Row;
        row.try_get("status").ok()
    }
}
