//! PostgreSQL implementation of [`ReserveRepository`].
//!
//! Persists USDC reserve-rebalance operations and tracks their lifecycle
//! (`pending` → `submitted` → `relayed` → `completed` | `failed`) in `treasury.reserve_ops`.

use alloy_primitives::U256;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use tracing::error;

use crate::db::reserve_ledger_repo::record_transfer_tx;
use crate::domain::ledger::{completion_transfer, initiation_transfer, reversal_transfer};
use crate::domain::newtypes::{ChainId, OperationId, TxHash};
use crate::domain::repository::{ReserveOpRow, ReserveRepository};

pub struct PgReserveRepository {
    pool: PgPool,
}

impl PgReserveRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Atomic `submitted` transition + ledger initiation transfer.
    /// Moves the op's amount from `reserve:<src>` into `in_transit`.
    async fn update_submitted_inner(
        &self,
        op_id: &OperationId,
        source_tx_hash: &TxHash,
        bridge_message_id: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            "UPDATE treasury.reserve_ops \
             SET status = 'submitted', source_tx_hash = $1, bridge_message_id = $2, updated_at = NOW() \
             WHERE op_id = $3",
            source_tx_hash.as_str(),
            bridge_message_id,
            op_id.as_str(),
        )
        .execute(&mut *tx)
        .await?;

        if let Some((source_chain, _dest_chain, amount)) =
            fetch_op_chains_amount(&mut tx, op_id).await?
        {
            let transfer = initiation_transfer(op_id.clone(), source_chain, amount);
            record_transfer_tx(&mut tx, &transfer).await?;
        }

        tx.commit().await
    }

    /// Atomic `completed` transition + ledger completion transfer.
    /// Moves the op's amount from `in_transit` into `reserve:<dst>`. Guarded so a
    /// completion is only recorded when its initiation leg exists, keeping the
    /// books balanced.
    async fn update_completed_inner(&self, op_id: &OperationId) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            "UPDATE treasury.reserve_ops \
             SET status = 'completed', completed_at = NOW(), updated_at = NOW() \
             WHERE op_id = $1",
            op_id.as_str(),
        )
        .execute(&mut *tx)
        .await?;

        if leg_exists(&mut tx, op_id, "initiation").await? {
            if let Some((_source_chain, dest_chain, amount)) =
                fetch_op_chains_amount(&mut tx, op_id).await?
            {
                let transfer = completion_transfer(op_id.clone(), dest_chain, amount);
                record_transfer_tx(&mut tx, &transfer).await?;
            }
        }

        tx.commit().await
    }

    /// Atomic `failed` transition + compensating ledger reversal.
    /// If the op was already in-transit (initiation recorded) and never
    /// completed, return its value from `in_transit` back to `reserve:<src>` so
    /// nothing leaks. Ops that failed before submission have no initiation leg
    /// and need no reversal.
    async fn update_failed_inner(&self, op_id: &OperationId) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        sqlx::query!(
            "UPDATE treasury.reserve_ops \
             SET status = 'failed', updated_at = NOW() \
             WHERE op_id = $1",
            op_id.as_str(),
        )
        .execute(&mut *tx)
        .await?;

        let needs_reversal = leg_exists(&mut tx, op_id, "initiation").await?
            && !leg_exists(&mut tx, op_id, "completion").await?;
        if needs_reversal {
            if let Some((source_chain, _dest_chain, amount)) =
                fetch_op_chains_amount(&mut tx, op_id).await?
            {
                let transfer = reversal_transfer(op_id.clone(), source_chain, amount);
                record_transfer_tx(&mut tx, &transfer).await?;
            }
        }

        tx.commit().await
    }
}

/// Read `(source_chain, dest_chain, amount)` for an op inside a transaction, so
/// the ledger transfer is derived from the same `reserve_ops` row it accompanies.
async fn fetch_op_chains_amount(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    op_id: &OperationId,
) -> Result<Option<(ChainId, ChainId, U256)>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT source_chain_id, dest_chain_id, amount_wei::TEXT AS amount_wei \
         FROM treasury.reserve_ops WHERE op_id = $1",
    )
    .bind(op_id.as_str())
    .fetch_optional(&mut **tx)
    .await?;

    let Some(row) = row else { return Ok(None) };
    let src: i64 = row.try_get("source_chain_id")?;
    let dst: i64 = row.try_get("dest_chain_id")?;
    let amount_str: String = row.try_get("amount_wei")?;
    let Ok(amount) = amount_str.parse::<U256>() else {
        return Ok(None);
    };
    Ok(Some((ChainId(src as u64), ChainId(dst as u64), amount)))
}

/// Whether a given ledger leg has been recorded for an op, inside a transaction.
async fn leg_exists(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    op_id: &OperationId,
    leg: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM treasury.reserve_ledger_transfers \
         WHERE op_id = $1 AND leg = $2)",
    )
    .bind(op_id.as_str())
    .bind(leg)
    .fetch_one(&mut **tx)
    .await
}

#[async_trait]
impl ReserveRepository for PgReserveRepository {
    async fn op_in_flight(&self, source_chain: ChainId, dest_chain: ChainId) -> bool {
        sqlx::query_scalar!(
            "SELECT EXISTS(\
               SELECT 1 FROM treasury.reserve_ops \
               WHERE source_chain_id = $1 \
                 AND dest_chain_id   = $2 \
                 AND status IN ('pending', 'submitted', 'relayed') \
                 AND created_at > NOW() - INTERVAL '24 hours'\
             )",
            source_chain.0 as i64,
            dest_chain.0 as i64,
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(Some(false))
        .unwrap_or(false)
    }

    async fn insert_reserve_op(
        &self,
        op_id: &OperationId,
        source_chain: ChainId,
        dest_chain: ChainId,
        amount: &U256,
        bridge_type: &str,
    ) {
        let amount_str = amount.to_string();
        if let Err(e) = sqlx::query_unchecked!(
            r#"
            INSERT INTO treasury.reserve_ops
                (op_id, source_chain_id, dest_chain_id, amount_wei, bridge_type, status)
            VALUES ($1, $2, $3, $4::NUMERIC, $5, 'pending')
            ON CONFLICT (op_id) DO NOTHING
            "#,
            op_id.as_str(),
            source_chain.0 as i64,
            dest_chain.0 as i64,
            &amount_str,
            bridge_type,
        )
        .execute(&self.pool)
        .await
        {
            error!(op_id = op_id.as_str(), err = %e, "reserve_repo: insert failed");
        }
    }

    async fn update_submitted(
        &self,
        op_id: &OperationId,
        source_tx_hash: &TxHash,
        bridge_message_id: Option<&str>,
    ) {
        if let Err(e) = self
            .update_submitted_inner(op_id, source_tx_hash, bridge_message_id)
            .await
        {
            error!(op_id = op_id.as_str(), err = %e, "reserve_repo: update_submitted failed");
        }
    }

    async fn update_attestation(
        &self,
        op_id: &OperationId,
        cctp_message_bytes: &str,
        cctp_attestation: &str,
    ) {
        if let Err(e) = sqlx::query!(
            "UPDATE treasury.reserve_ops \
             SET cctp_message_bytes = $1, cctp_attestation = $2, updated_at = NOW() \
             WHERE op_id = $3",
            cctp_message_bytes,
            cctp_attestation,
            op_id.as_str(),
        )
        .execute(&self.pool)
        .await
        {
            error!(op_id = op_id.as_str(), err = %e, "reserve_repo: update_attestation failed");
        }
    }

    async fn update_relayed(&self, op_id: &OperationId, dest_tx_hash: &TxHash) {
        if let Err(e) = sqlx::query!(
            "UPDATE treasury.reserve_ops \
             SET status = 'relayed', dest_tx_hash = $1, updated_at = NOW() \
             WHERE op_id = $2",
            dest_tx_hash.as_str(),
            op_id.as_str(),
        )
        .execute(&self.pool)
        .await
        {
            error!(op_id = op_id.as_str(), err = %e, "reserve_repo: update_relayed failed");
        }
    }

    async fn update_completed(&self, op_id: &OperationId) {
        if let Err(e) = self.update_completed_inner(op_id).await {
            error!(op_id = op_id.as_str(), err = %e, "reserve_repo: update_completed failed");
        }
    }

    async fn update_failed(&self, op_id: &OperationId) {
        if let Err(e) = self.update_failed_inner(op_id).await {
            error!(op_id = op_id.as_str(), err = %e, "reserve_repo: update_failed failed");
        }
    }

    async fn list_awaiting_attestation(&self) -> Vec<ReserveOpRow> {
        match sqlx::query!(
            r#"
            SELECT op_id, source_chain_id, dest_chain_id, amount_wei::TEXT AS amount_wei,
                   bridge_type, bridge_message_id, source_tx_hash, cctp_message_bytes,
                   cctp_attestation, dest_tx_hash, status
            FROM treasury.reserve_ops
            WHERE status = 'submitted'
              AND bridge_message_id IS NOT NULL
              AND cctp_attestation IS NULL
            ORDER BY updated_at ASC
            LIMIT 50
            "#,
        )
        .fetch_all(&self.pool)
        .await
        {
            Ok(rows) => rows
                .into_iter()
                .map(|r| ReserveOpRow {
                    op_id: r.op_id,
                    source_chain_id: r.source_chain_id,
                    dest_chain_id: r.dest_chain_id,
                    amount_wei: r.amount_wei.unwrap_or_default(),
                    bridge_type: r.bridge_type,
                    bridge_message_id: r.bridge_message_id,
                    source_tx_hash: r.source_tx_hash,
                    cctp_message_bytes: r.cctp_message_bytes,
                    cctp_attestation: r.cctp_attestation,
                    dest_tx_hash: r.dest_tx_hash,
                    status: r.status,
                })
                .collect(),
            Err(e) => {
                error!(err = %e, "reserve_repo: list_awaiting_attestation failed");
                vec![]
            }
        }
    }

    async fn list_awaiting_completion(&self) -> Vec<ReserveOpRow> {
        match sqlx::query!(
            r#"
            SELECT op_id, source_chain_id, dest_chain_id, amount_wei::TEXT AS amount_wei,
                   bridge_type, bridge_message_id, source_tx_hash, cctp_message_bytes,
                   cctp_attestation, dest_tx_hash, status
            FROM treasury.reserve_ops
            WHERE status = 'relayed'
            ORDER BY updated_at ASC
            LIMIT 50
            "#,
        )
        .fetch_all(&self.pool)
        .await
        {
            Ok(rows) => rows
                .into_iter()
                .map(|r| ReserveOpRow {
                    op_id: r.op_id,
                    source_chain_id: r.source_chain_id,
                    dest_chain_id: r.dest_chain_id,
                    amount_wei: r.amount_wei.unwrap_or_default(),
                    bridge_type: r.bridge_type,
                    bridge_message_id: r.bridge_message_id,
                    source_tx_hash: r.source_tx_hash,
                    cctp_message_bytes: r.cctp_message_bytes,
                    cctp_attestation: r.cctp_attestation,
                    dest_tx_hash: r.dest_tx_hash,
                    status: r.status,
                })
                .collect(),
            Err(e) => {
                error!(err = %e, "reserve_repo: list_awaiting_completion failed");
                vec![]
            }
        }
    }

    async fn find_by_dest_and_message_id(
        &self,
        dest_chain: ChainId,
        bridge_message_id: &str,
    ) -> Option<ReserveOpRow> {
        match sqlx::query!(
            r#"
            SELECT op_id, source_chain_id, dest_chain_id, amount_wei::TEXT AS amount_wei,
                   bridge_type, bridge_message_id, source_tx_hash, cctp_message_bytes,
                   cctp_attestation, dest_tx_hash, status
            FROM treasury.reserve_ops
            WHERE dest_chain_id = $1 AND bridge_message_id = $2
            LIMIT 1
            "#,
            dest_chain.0 as i64,
            bridge_message_id,
        )
        .fetch_optional(&self.pool)
        .await
        {
            Ok(Some(r)) => Some(ReserveOpRow {
                op_id: r.op_id,
                source_chain_id: r.source_chain_id,
                dest_chain_id: r.dest_chain_id,
                amount_wei: r.amount_wei.unwrap_or_default(),
                bridge_type: r.bridge_type,
                bridge_message_id: r.bridge_message_id,
                source_tx_hash: r.source_tx_hash,
                cctp_message_bytes: r.cctp_message_bytes,
                cctp_attestation: r.cctp_attestation,
                dest_tx_hash: r.dest_tx_hash,
                status: r.status,
            }),
            Ok(None) => None,
            Err(e) => {
                error!(err = %e, "reserve_repo: find_by_dest_and_message_id failed");
                None
            }
        }
    }

    async fn list_stuck(&self, stuck_after_secs: i64) -> Vec<ReserveOpRow> {
        match sqlx::query!(
            r#"
            SELECT op_id, source_chain_id, dest_chain_id, amount_wei::TEXT AS amount_wei,
                   bridge_type, bridge_message_id, source_tx_hash, cctp_message_bytes,
                   cctp_attestation, dest_tx_hash, status
            FROM treasury.reserve_ops
            WHERE status IN ('pending', 'submitted', 'relayed')
              AND updated_at < NOW() - make_interval(secs => $1::DOUBLE PRECISION)
            ORDER BY updated_at ASC
            LIMIT 50
            "#,
            stuck_after_secs as f64,
        )
        .fetch_all(&self.pool)
        .await
        {
            Ok(rows) => rows
                .into_iter()
                .map(|r| ReserveOpRow {
                    op_id: r.op_id,
                    source_chain_id: r.source_chain_id,
                    dest_chain_id: r.dest_chain_id,
                    amount_wei: r.amount_wei.unwrap_or_default(),
                    bridge_type: r.bridge_type,
                    bridge_message_id: r.bridge_message_id,
                    source_tx_hash: r.source_tx_hash,
                    cctp_message_bytes: r.cctp_message_bytes,
                    cctp_attestation: r.cctp_attestation,
                    dest_tx_hash: r.dest_tx_hash,
                    status: r.status,
                })
                .collect(),
            Err(e) => {
                error!(err = %e, "reserve_repo: list_stuck failed");
                vec![]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    fn op(id: &str) -> OperationId {
        OperationId(id.to_string())
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn insert_then_op_in_flight(pool: PgPool) {
        let repo = PgReserveRepository::new(pool);
        let src = ChainId(8453);
        let dst = ChainId(42161);
        assert!(!repo.op_in_flight(src, dst).await);
        repo.insert_reserve_op(&op("o1"), src, dst, &U256::from(100u64), "CCTP")
            .await;
        assert!(repo.op_in_flight(src, dst).await);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn lifecycle_transitions(pool: PgPool) {
        let repo = PgReserveRepository::new(pool);
        let id = op("lifecycle");
        repo.insert_reserve_op(&id, ChainId(1), ChainId(2), &U256::from(7u64), "CCTP")
            .await;

        repo.update_submitted(&id, &TxHash("0xsrc".into()), Some("0xmid"))
            .await;
        assert_eq!(repo.list_awaiting_attestation().await.len(), 1);

        repo.update_attestation(&id, "0xmsg", "0xatt").await;
        // Still 'submitted' until relay; attestation columns are populated.
        let row = repo
            .find_by_dest_and_message_id(ChainId(2), "0xmid")
            .await
            .unwrap();
        assert_eq!(row.cctp_attestation.as_deref(), Some("0xatt"));
        // Attestation now present, so it should no longer appear in list_awaiting_attestation.
        assert!(repo.list_awaiting_attestation().await.is_empty());

        repo.update_relayed(&id, &TxHash("0xdest".into())).await;
        assert_eq!(repo.list_awaiting_completion().await.len(), 1);

        repo.update_completed(&id).await;
        assert!(repo.list_awaiting_completion().await.is_empty());
        assert!(!repo.op_in_flight(ChainId(1), ChainId(2)).await);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn failed_ops_drop_out_of_in_flight(pool: PgPool) {
        let repo = PgReserveRepository::new(pool);
        let id = op("failedop");
        repo.insert_reserve_op(&id, ChainId(3), ChainId(4), &U256::from(50u64), "CCTP")
            .await;
        assert!(repo.op_in_flight(ChainId(3), ChainId(4)).await);
        repo.update_failed(&id).await;
        assert!(!repo.op_in_flight(ChainId(3), ChainId(4)).await);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn list_stuck_returns_old_in_flight_rows(pool: PgPool) {
        let repo = PgReserveRepository::new(pool);
        let id = op("stuck");
        repo.insert_reserve_op(&id, ChainId(5), ChainId(6), &U256::from(1u64), "CCTP")
            .await;
        // Backdate updated_at past the threshold.
        sqlx::query(
            "UPDATE treasury.reserve_ops SET updated_at = NOW() - INTERVAL '2 hours' WHERE op_id = $1",
        )
        .bind(id.as_str())
        .execute(&repo.pool)
        .await
        .unwrap();
        let stuck = repo.list_stuck(60 * 60).await;
        assert_eq!(stuck.len(), 1);
        assert_eq!(stuck[0].op_id, "stuck");
    }
}
