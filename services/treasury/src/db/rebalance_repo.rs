use alloy_primitives::U256;
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::error;

use crate::domain::repository::RebalanceRepository;

pub struct PgRebalanceRepository {
    pool: PgPool,
}

impl PgRebalanceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RebalanceRepository for PgRebalanceRepository {
    async fn op_in_flight(&self, source_chain: u64, dest_chain: u64) -> bool {
        sqlx::query(
            "SELECT 1 FROM treasury.rebalance_ops \
             WHERE source_chain_id = $1 \
               AND dest_chain_id   = $2 \
               AND status IN ('pending', 'submitted') \
               AND created_at > NOW() - INTERVAL '24 hours'",
        )
        .bind(source_chain as i64)
        .bind(dest_chain as i64)
        .fetch_optional(&self.pool)
        .await
        .map(|r| r.is_some())
        .unwrap_or(false)
    }

    async fn insert_rebalance_op(
        &self,
        op_id: &str,
        source_chain: u64,
        dest_chain: u64,
        amount: &U256,
    ) {
        let amount_str = amount.to_string();
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO treasury.rebalance_ops
                (op_id, source_chain_id, dest_chain_id, amount_wei, status)
            VALUES ($1, $2, $3, $4::NUMERIC, 'pending')
            ON CONFLICT (op_id) DO NOTHING
            "#,
        )
        .bind(op_id)
        .bind(source_chain as i64)
        .bind(dest_chain as i64)
        .bind(&amount_str)
        .execute(&self.pool)
        .await
        {
            error!(op_id, err = %e, "rebalance_repo: failed to insert rebalance_op");
        }
    }

    async fn update_rebalance_op_submitted(
        &self,
        op_id: &str,
        tx_hash: &str,
        ccip_message_id: Option<&str>,
    ) {
        if let Err(e) = sqlx::query(
            "UPDATE treasury.rebalance_ops \
             SET status = 'submitted', source_tx_hash = $1, \
                 ccip_message_id = $2, updated_at = NOW() \
             WHERE op_id = $3",
        )
        .bind(tx_hash)
        .bind(ccip_message_id)
        .bind(op_id)
        .execute(&self.pool)
        .await
        {
            error!(op_id, err = %e, "rebalance_repo: failed to update rebalance_op to submitted");
        }
    }

    async fn update_rebalance_op_failed(&self, op_id: &str) {
        if let Err(e) = sqlx::query(
            "UPDATE treasury.rebalance_ops \
             SET status = 'failed', updated_at = NOW() \
             WHERE op_id = $1",
        )
        .bind(op_id)
        .execute(&self.pool)
        .await
        {
            error!(op_id, err = %e, "rebalance_repo: failed to mark rebalance_op as failed");
        }
    }
}
