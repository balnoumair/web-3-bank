//! PostgreSQL implementation of [`DecommissionRepository`].

use alloy_primitives::U256;
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::error;

use crate::domain::newtypes::{ChainId, OperationId, TxHash};
use crate::domain::repository::{DecommissionOpStatus, DecommissionRepository};

pub struct PgDecommissionRepository {
    pool: PgPool,
}

impl PgDecommissionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DecommissionRepository for PgDecommissionRepository {
    async fn completed_holders(&self, source_chain: ChainId, target_chain: ChainId) -> Vec<String> {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT holder_address FROM treasury.decommission_ops
             WHERE chain_id = $1
               AND dst_chain_id = $2
               AND holder_address IS NOT NULL
               AND status = 'completed'",
        )
        .bind(source_chain.0 as i64)
        .bind(target_chain.0 as i64)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect()
    }

    async fn insert_holder_op(
        &self,
        op_id: &OperationId,
        source_chain: ChainId,
        target_chain: ChainId,
        holder_address: &str,
        amount: &U256,
        status: DecommissionOpStatus,
    ) {
        let amount = amount.to_string();
        if let Err(e) = sqlx::query(
            "INSERT INTO treasury.decommission_ops
                (op_id, chain_id, holder_address, amount, dst_chain_id, status)
             VALUES ($1, $2, $3, $4::NUMERIC, $5, $6)
             ON CONFLICT (op_id) DO NOTHING",
        )
        .bind(op_id.as_str())
        .bind(source_chain.0 as i64)
        .bind(holder_address)
        .bind(amount)
        .bind(target_chain.0 as i64)
        .bind(status.as_str())
        .execute(&self.pool)
        .await
        {
            error!(op_id = op_id.as_str(), err = %e, "decommission_repo: insert failed");
        }
    }

    async fn mark_holder_submitted(
        &self,
        op_id: &OperationId,
        src_message_id: Option<&str>,
        dst_tx_hash: Option<&TxHash>,
    ) {
        let dst = dst_tx_hash.map(TxHash::as_str);
        if let Err(e) = sqlx::query(
            "UPDATE treasury.decommission_ops
             SET status = 'submitted',
                 src_message_id = $1,
                 dst_tx_hash = $2
             WHERE op_id = $3",
        )
        .bind(src_message_id)
        .bind(dst)
        .bind(op_id.as_str())
        .execute(&self.pool)
        .await
        {
            error!(op_id = op_id.as_str(), err = %e, "decommission_repo: submit update failed");
        }
    }

    async fn mark_holder_completed(&self, op_id: &OperationId) {
        if let Err(e) = sqlx::query(
            "UPDATE treasury.decommission_ops
             SET status = 'completed', completed_at = NOW()
             WHERE op_id = $1",
        )
        .bind(op_id.as_str())
        .execute(&self.pool)
        .await
        {
            error!(op_id = op_id.as_str(), err = %e, "decommission_repo: complete update failed");
        }
    }

    async fn mark_op_failed(&self, op_id: &OperationId, failure_reason: &str) {
        if let Err(e) = sqlx::query(
            "UPDATE treasury.decommission_ops
             SET status = 'failed', failure_reason = $1
             WHERE op_id = $2",
        )
        .bind(failure_reason)
        .bind(op_id.as_str())
        .execute(&self.pool)
        .await
        {
            error!(op_id = op_id.as_str(), err = %e, "decommission_repo: failure update failed");
        }
    }

    async fn has_incomplete_ops(&self) -> bool {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(
                SELECT 1 FROM treasury.decommission_ops
                WHERE status IN ('pending', 'submitted', 'paused')
            )",
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(false)
    }

    async fn latest_incomplete_pair(&self) -> Option<(ChainId, ChainId)> {
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT chain_id, dst_chain_id
             FROM treasury.decommission_ops
             WHERE status IN ('pending', 'submitted', 'paused')
             ORDER BY started_at DESC
             LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .map(|(src, dst)| (ChainId(src as u64), ChainId(dst as u64)))
    }

    async fn status_counts(
        &self,
        source_chain: ChainId,
        target_chain: ChainId,
    ) -> Vec<(String, u64)> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT status, COUNT(*)::BIGINT AS count
             FROM treasury.decommission_ops
             WHERE chain_id = $1 AND dst_chain_id = $2
             GROUP BY status",
        )
        .bind(source_chain.0 as i64)
        .bind(target_chain.0 as i64)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();
        rows.into_iter()
            .map(|(s, c)| (s, c.max(0) as u64))
            .collect()
    }

    async fn drained_amount_wei(&self, source_chain: ChainId, target_chain: ChainId) -> String {
        sqlx::query_scalar::<_, String>(
            "SELECT COALESCE(SUM(amount), 0)::TEXT
             FROM treasury.decommission_ops
             WHERE chain_id = $1 AND dst_chain_id = $2 AND status = 'completed'",
        )
        .bind(source_chain.0 as i64)
        .bind(target_chain.0 as i64)
        .fetch_one(&self.pool)
        .await
        .unwrap_or_else(|_| "0".to_string())
    }

    async fn last_error(&self, source_chain: ChainId, target_chain: ChainId) -> Option<String> {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT failure_reason
             FROM treasury.decommission_ops
             WHERE chain_id = $1 AND dst_chain_id = $2 AND failure_reason IS NOT NULL
             ORDER BY started_at DESC
             LIMIT 1",
        )
        .bind(source_chain.0 as i64)
        .bind(target_chain.0 as i64)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()
        .flatten()
    }
}
