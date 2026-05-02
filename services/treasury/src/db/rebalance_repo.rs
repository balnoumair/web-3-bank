//! PostgreSQL implementation of [`RebalanceRepository`].
//!
//! Persists cold-path rebalance operations and tracks their lifecycle
//! (`pending` → `submitted` / `failed`) in `treasury.rebalance_ops`.

use alloy_primitives::U256;
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::error;

use crate::domain::newtypes::{ChainId, OperationId, TxHash};
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
    async fn op_in_flight(&self, source_chain: ChainId, dest_chain: ChainId) -> bool {
        sqlx::query_scalar!(
            "SELECT EXISTS(\
               SELECT 1 FROM treasury.rebalance_ops \
               WHERE source_chain_id = $1 \
                 AND dest_chain_id   = $2 \
                 AND status IN ('pending', 'submitted') \
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

    async fn insert_rebalance_op(
        &self,
        op_id: &OperationId,
        source_chain: ChainId,
        dest_chain: ChainId,
        amount: &U256,
    ) {
        let amount_str = amount.to_string();
        if let Err(e) = sqlx::query_unchecked!(
            r#"
            INSERT INTO treasury.rebalance_ops
                (op_id, source_chain_id, dest_chain_id, amount_wei, status)
            VALUES ($1, $2, $3, $4::NUMERIC, 'pending')
            ON CONFLICT (op_id) DO NOTHING
            "#,
            op_id.as_str(),
            source_chain.0 as i64,
            dest_chain.0 as i64,
            &amount_str,
        )
        .execute(&self.pool)
        .await
        {
            error!(op_id = op_id.as_str(), err = %e, "rebalance_repo: failed to insert rebalance_op");
        }
    }

    async fn update_rebalance_op_submitted(
        &self,
        op_id: &OperationId,
        tx_hash: &TxHash,
        ccip_message_id: Option<&str>,
    ) {
        if let Err(e) = sqlx::query!(
            "UPDATE treasury.rebalance_ops \
             SET status = 'submitted', source_tx_hash = $1, \
                 ccip_message_id = $2, updated_at = NOW() \
             WHERE op_id = $3",
            tx_hash.as_str(),
            ccip_message_id,
            op_id.as_str(),
        )
        .execute(&self.pool)
        .await
        {
            error!(op_id = op_id.as_str(), err = %e, "rebalance_repo: failed to update rebalance_op to submitted");
        }
    }

    async fn update_rebalance_op_failed(&self, op_id: &OperationId) {
        if let Err(e) = sqlx::query!(
            "UPDATE treasury.rebalance_ops \
             SET status = 'failed', updated_at = NOW() \
             WHERE op_id = $1",
            op_id.as_str(),
        )
        .execute(&self.pool)
        .await
        {
            error!(op_id = op_id.as_str(), err = %e, "rebalance_repo: failed to mark rebalance_op as failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::newtypes::{ChainId, OperationId, TxHash};
    use alloy_primitives::U256;
    use sqlx::PgPool;

    #[sqlx::test(migrations = "./migrations")]
    async fn insert_and_op_in_flight(pool: PgPool) {
        let repo = PgRebalanceRepository::new(pool);
        let op_id = OperationId("chain1-chain2-000".to_string());
        let src = ChainId(1);
        let dst = ChainId(2);
        let amount = U256::from(1_000_000u64);

        assert!(!repo.op_in_flight(src, dst).await);
        repo.insert_rebalance_op(&op_id, src, dst, &amount).await;
        assert!(repo.op_in_flight(src, dst).await);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn insert_idempotent_on_conflict(pool: PgPool) {
        let repo = PgRebalanceRepository::new(pool);
        let op_id = OperationId("chain1-chain2-001".to_string());
        let src = ChainId(1);
        let dst = ChainId(2);
        let amount = U256::from(500u64);

        repo.insert_rebalance_op(&op_id, src, dst, &amount).await;
        repo.insert_rebalance_op(&op_id, src, dst, &amount).await; // ON CONFLICT DO NOTHING
        assert!(repo.op_in_flight(src, dst).await);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn update_submitted(pool: PgPool) {
        let repo = PgRebalanceRepository::new(pool);
        let op_id = OperationId("chain3-chain4-002".to_string());
        let tx = TxHash("0xcciptx".to_string());

        repo.insert_rebalance_op(&op_id, ChainId(3), ChainId(4), &U256::from(100u64))
            .await;
        repo.update_rebalance_op_submitted(&op_id, &tx, Some("0xmsgid"))
            .await;
        // op is still "in flight" after submitted (within 24h window)
        assert!(repo.op_in_flight(ChainId(3), ChainId(4)).await);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn update_failed_removes_in_flight(pool: PgPool) {
        let repo = PgRebalanceRepository::new(pool);
        let op_id = OperationId("chain5-chain6-003".to_string());

        repo.insert_rebalance_op(&op_id, ChainId(5), ChainId(6), &U256::from(200u64))
            .await;
        assert!(repo.op_in_flight(ChainId(5), ChainId(6)).await);
        repo.update_rebalance_op_failed(&op_id).await;
        // failed ops are NOT in_flight
        assert!(!repo.op_in_flight(ChainId(5), ChainId(6)).await);
    }
}
