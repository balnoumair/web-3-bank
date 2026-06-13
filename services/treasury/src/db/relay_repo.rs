//! PostgreSQL implementation of [`RelayRepository`].
//!
//! Persists hot-path relay events and their lifecycle status
//! (`pending` → `completed` / `failed` / rejected variants) in
//! `treasury.relay_logs`.

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::error;

use crate::domain::events::HotPathEvent;
use crate::domain::newtypes::{EventHash, TxHash};
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
    async fn relay_already_recorded(&self, source_event_hash: &EventHash) -> bool {
        sqlx::query_scalar!(
            "SELECT EXISTS(SELECT 1 FROM treasury.relay_logs WHERE source_event_hash = $1)",
            source_event_hash.as_str()
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(Some(false))
        .unwrap_or(false)
    }

    async fn insert_relay_log(
        &self,
        event: &HotPathEvent,
        dest_tx_hash: Option<&TxHash>,
        status: RelayStatus,
    ) {
        let amount_str = event.amount.to_string();
        let recipient_str = format!("0x{}", eth::bytes_to_hex(event.recipient.as_slice()));
        if let Err(e) = sqlx::query_unchecked!(
            r#"
            INSERT INTO treasury.relay_logs
                (source_event_hash, dest_tx_hash, source_chain_id, dest_chain_id,
                 sender, recipient, amount_wei, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7::NUMERIC, $8)
            ON CONFLICT (source_event_hash) DO NOTHING
            "#,
            event.source_event_hash.as_str(),
            dest_tx_hash.map(|h| h.as_str()),
            event.source_chain_id.0 as i64,
            event.dest_chain_id.0 as i64,
            &event.sender,
            &recipient_str,
            &amount_str,
            status.as_str(),
        )
        .execute(&self.pool)
        .await
        {
            error!(err = %e, "relay_repo: failed to insert relay log");
        }
    }

    async fn update_relay_log(
        &self,
        source_event_hash: &EventHash,
        dest_tx_hash: &TxHash,
        status: RelayStatus,
    ) {
        if let Err(e) = sqlx::query!(
            "UPDATE treasury.relay_logs \
             SET dest_tx_hash = $1, status = $2, updated_at = now() \
             WHERE source_event_hash = $3",
            dest_tx_hash.as_str(),
            status.as_str(),
            source_event_hash.as_str(),
        )
        .execute(&self.pool)
        .await
        {
            error!(err = %e, "relay_repo: failed to update relay log");
        }
    }

    async fn update_relay_log_failed(&self, source_event_hash: &EventHash) {
        if let Err(e) = sqlx::query!(
            "UPDATE treasury.relay_logs \
             SET status = 'failed', updated_at = now() \
             WHERE source_event_hash = $1",
            source_event_hash.as_str(),
        )
        .execute(&self.pool)
        .await
        {
            error!(err = %e, "relay_repo: failed to mark relay log as failed");
        }
    }

    async fn get_relay_status(&self, source_event_hash: &EventHash) -> Option<String> {
        sqlx::query_scalar!(
            "SELECT status FROM treasury.relay_logs WHERE source_event_hash = $1",
            source_event_hash.as_str()
        )
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::events::HotPathEvent;
    use crate::domain::newtypes::{ChainId, EventHash, TxHash};
    use crate::domain::status::RelayStatus;
    use alloy_primitives::{Address, B256, U256};
    use sqlx::PgPool;

    fn make_event(hash: &str, src: u64, dst: u64, amount: u64) -> HotPathEvent {
        HotPathEvent {
            source_chain_id: ChainId(src),
            source_event_hash: EventHash(hash.to_string()),
            sender: "0xSender".to_string(),
            recipient: Address::ZERO,
            amount: U256::from(amount),
            dest_chain_id: ChainId(dst),
            event_id: B256::ZERO,
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn insert_and_query_relay_log(pool: PgPool) {
        let repo = PgRelayRepository::new(pool);
        let event = make_event("0xhash1", 1, 2, 1000);

        assert!(!repo.relay_already_recorded(&event.source_event_hash).await);
        repo.insert_relay_log(&event, None, RelayStatus::Pending)
            .await;
        assert!(repo.relay_already_recorded(&event.source_event_hash).await);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn insert_idempotent_on_conflict(pool: PgPool) {
        let repo = PgRelayRepository::new(pool);
        let event = make_event("0xhash2", 1, 2, 500);

        repo.insert_relay_log(&event, None, RelayStatus::Pending)
            .await;
        repo.insert_relay_log(&event, None, RelayStatus::Completed)
            .await; // should be ignored
        let status = repo.get_relay_status(&event.source_event_hash).await;
        assert_eq!(status.as_deref(), Some("pending")); // ON CONFLICT DO NOTHING
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn update_relay_log_to_completed(pool: PgPool) {
        let repo = PgRelayRepository::new(pool);
        let event = make_event("0xhash3", 1, 2, 200);
        let tx = TxHash("0xdestTx".to_string());

        repo.insert_relay_log(&event, None, RelayStatus::Pending)
            .await;
        repo.update_relay_log(&event.source_event_hash, &tx, RelayStatus::Completed)
            .await;

        let status = repo.get_relay_status(&event.source_event_hash).await;
        assert_eq!(status.as_deref(), Some("completed"));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn update_relay_log_failed(pool: PgPool) {
        let repo = PgRelayRepository::new(pool);
        let event = make_event("0xhash4", 1, 2, 100);

        repo.insert_relay_log(&event, None, RelayStatus::Pending)
            .await;
        repo.update_relay_log_failed(&event.source_event_hash).await;

        let status = repo.get_relay_status(&event.source_event_hash).await;
        assert_eq!(status.as_deref(), Some("failed"));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn get_relay_status_missing_returns_none(pool: PgPool) {
        let repo = PgRelayRepository::new(pool);
        let hash = EventHash("0xnonexistent".to_string());
        assert!(repo.get_relay_status(&hash).await.is_none());
    }
}
