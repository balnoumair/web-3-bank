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
}
