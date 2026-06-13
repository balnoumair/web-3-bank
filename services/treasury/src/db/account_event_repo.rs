//! PostgreSQL implementation of [`AccountEventRepository`].
//!
//! Persists indexed on-chain account events and per-chain block cursors in
//! `treasury.account_events` and `treasury.index_cursors`.

use async_trait::async_trait;
use sqlx::{PgPool, Row};
use tracing::error;

use crate::domain::newtypes::ChainId;
use crate::domain::repository::{AccountEventRepository, AccountEventRow, UpsertEventResult};

pub struct PgAccountEventRepository {
    pool: PgPool,
}

impl PgAccountEventRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AccountEventRepository for PgAccountEventRepository {
    async fn upsert_event(&self, row: &AccountEventRow) -> UpsertEventResult {
        let result = sqlx::query(
            r#"
            INSERT INTO treasury.account_events
                (chain_id, tx_hash, log_index, event_kind,
                 address_from, address_to, amount_wei,
                 block_number, block_time, correlation)
            VALUES ($1, $2, $3, $4, $5, $6, $7::NUMERIC, $8, to_timestamp($9), $10)
            ON CONFLICT (chain_id, tx_hash, log_index) DO NOTHING
            RETURNING id
            "#,
        )
        .bind(row.chain_id)
        .bind(&row.tx_hash)
        .bind(row.log_index)
        .bind(&row.event_kind)
        .bind(&row.address_from)
        .bind(&row.address_to)
        .bind(&row.amount_wei)
        .bind(row.block_number)
        .bind(row.block_time_unix.map(|ts| ts as f64))
        .bind(&row.correlation)
        .fetch_optional(&self.pool)
        .await;

        match result {
            Ok(Some(_)) => UpsertEventResult::Inserted,
            Ok(None) => UpsertEventResult::AlreadyExists,
            Err(e) => {
                error!(err = %e, "account_event_repo: upsert failed");
                UpsertEventResult::AlreadyExists
            }
        }
    }

    async fn set_cursor(&self, chain_id: ChainId, last_block: u64) {
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO treasury.index_cursors (chain_id, last_block)
            VALUES ($1, $2)
            ON CONFLICT (chain_id) DO UPDATE
                SET last_block = EXCLUDED.last_block,
                    updated_at = now()
            "#,
        )
        .bind(chain_id.0 as i64)
        .bind(last_block as i64)
        .execute(&self.pool)
        .await
        {
            error!(err = %e, chain_id = chain_id.0, "account_event_repo: set_cursor failed");
        }
    }

    async fn get_cursor(&self, chain_id: ChainId) -> Option<u64> {
        sqlx::query("SELECT last_block FROM treasury.index_cursors WHERE chain_id = $1")
            .bind(chain_id.0 as i64)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
            .and_then(|row| row.try_get::<i64, _>("last_block").ok())
            .map(|b| b as u64)
    }

    async fn user_has_deposit(&self, user_address: &str) -> bool {
        sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM treasury.account_events
                WHERE event_kind = 'deposited'
                  AND address_to = $1
            )
            "#,
        )
        .bind(user_address)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(false)
    }

    async fn indexed_balance_on_chain(&self, chain_id: ChainId, address: &str) -> String {
        let addr = address.to_lowercase();
        sqlx::query_scalar::<_, String>(
            r#"
            SELECT COALESCE(SUM(delta), 0)::TEXT
            FROM (
                SELECT amount_wei AS delta
                FROM treasury.account_events
                WHERE chain_id = $1 AND event_kind = 'deposited'
                  AND lower(address_to) = $2
                UNION ALL
                SELECT -amount_wei
                FROM treasury.account_events
                WHERE chain_id = $1 AND event_kind = 'withdrawn'
                  AND lower(address_from) = $2
                UNION ALL
                SELECT amount_wei
                FROM treasury.account_events
                WHERE chain_id = $1 AND event_kind = 'transfer'
                  AND lower(address_to) = $2
                UNION ALL
                SELECT -amount_wei
                FROM treasury.account_events
                WHERE chain_id = $1 AND event_kind = 'transfer'
                  AND lower(address_from) = $2
                UNION ALL
                SELECT -amount_wei
                FROM treasury.account_events
                WHERE chain_id = $1 AND event_kind = 'hot_path_initiated'
                  AND lower(address_from) = $2
                UNION ALL
                SELECT amount_wei
                FROM treasury.account_events
                WHERE chain_id = $1 AND event_kind = 'hot_path_released'
                  AND lower(address_to) = $2
            ) AS deltas
            "#,
        )
        .bind(chain_id.0 as i64)
        .bind(&addr)
        .fetch_one(&self.pool)
        .await
        .unwrap_or_else(|_| "0".to_string())
    }

    async fn list_activity_for_user(&self, address: &str, limit: i64) -> Vec<AccountEventRow> {
        let addr = address.to_lowercase();
        let rows = sqlx::query(
            r#"
            SELECT chain_id, tx_hash, log_index, event_kind,
                   address_from, address_to, amount_wei::TEXT AS amount_wei,
                   block_number,
                   EXTRACT(EPOCH FROM block_time)::BIGINT AS block_time_unix,
                   correlation
            FROM treasury.account_events
            WHERE lower(address_from) = $1 OR lower(address_to) = $1
            ORDER BY block_time DESC NULLS LAST, block_number DESC, log_index DESC
            LIMIT $2
            "#,
        )
        .bind(&addr)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        rows.into_iter()
            .filter_map(|row| {
                Some(AccountEventRow {
                    chain_id: row.try_get("chain_id").ok()?,
                    tx_hash: row.try_get("tx_hash").ok()?,
                    log_index: row.try_get("log_index").ok()?,
                    event_kind: row.try_get("event_kind").ok()?,
                    address_from: row.try_get("address_from").ok()?,
                    address_to: row.try_get("address_to").ok()?,
                    amount_wei: row.try_get("amount_wei").ok()?,
                    block_number: row.try_get("block_number").ok()?,
                    block_time_unix: row.try_get("block_time_unix").ok()?,
                    correlation: row.try_get("correlation").ok()?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_row(chain_id: i64, tx_hash: &str, log_index: i32) -> AccountEventRow {
        AccountEventRow {
            chain_id,
            tx_hash: tx_hash.to_string(),
            log_index,
            event_kind: "deposited".to_string(),
            address_from: None,
            address_to: Some("0xabc".to_string()),
            amount_wei: "1000".to_string(),
            block_number: 42,
            block_time_unix: None,
            correlation: None,
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn upsert_is_idempotent(pool: PgPool) {
        let repo = PgAccountEventRepository::new(pool);
        let row = sample_row(84532, "0xdead", 0);

        assert_eq!(repo.upsert_event(&row).await, UpsertEventResult::Inserted);
        assert_eq!(
            repo.upsert_event(&row).await,
            UpsertEventResult::AlreadyExists
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn cursor_round_trip(pool: PgPool) {
        let repo = PgAccountEventRepository::new(pool);
        let chain = ChainId(84532);

        assert_eq!(repo.get_cursor(chain).await, None);
        repo.set_cursor(chain, 100).await;
        assert_eq!(repo.get_cursor(chain).await, Some(100));
        repo.set_cursor(chain, 200).await;
        assert_eq!(repo.get_cursor(chain).await, Some(200));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn user_has_deposit_tracks_deposited_events(pool: PgPool) {
        let repo = PgAccountEventRepository::new(pool);
        let user = "0xuser123";

        assert!(!repo.user_has_deposit(user).await);

        let mut row = sample_row(1, "0xtx1", 0);
        row.address_to = Some(user.to_string());
        assert_eq!(repo.upsert_event(&row).await, UpsertEventResult::Inserted);
        assert!(repo.user_has_deposit(user).await);
    }
}
