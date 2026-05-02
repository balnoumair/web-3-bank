//! PostgreSQL implementation of [`PoolSnapshotRepository`].
//!
//! Records periodic `poolDepth()` snapshots from on-chain state
//! in `treasury.pool_snapshots` for monitoring and dashboards.

use alloy_primitives::U256;
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::error;

use crate::domain::newtypes::ChainId;
use crate::domain::repository::PoolSnapshotRepository;

pub struct PgPoolSnapshotRepository {
    pool: PgPool,
}

impl PgPoolSnapshotRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PoolSnapshotRepository for PgPoolSnapshotRepository {
    async fn record_snapshot(&self, chain_id: ChainId, depth: &U256) {
        let depth_str = depth.to_string();
        if let Err(e) = sqlx::query_unchecked!(
            "INSERT INTO treasury.pool_snapshots (chain_id, depth_wei) \
             VALUES ($1, $2::NUMERIC)",
            chain_id.0 as i64,
            &depth_str,
        )
        .execute(&self.pool)
        .await
        {
            error!(chain = chain_id.0, err = %e, "pool_snapshot_repo: failed to record snapshot");
        }
    }

    async fn get_latest_depth(&self, chain_id: ChainId) -> Option<String> {
        let row = sqlx::query!(
            r#"SELECT depth_wei::TEXT AS "depth_wei!" FROM treasury.pool_snapshots
               WHERE chain_id = $1
               ORDER BY recorded_at DESC
               LIMIT 1"#,
            chain_id.0 as i64,
        )
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()?;

        Some(row.depth_wei)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::newtypes::ChainId;
    use alloy_primitives::U256;
    use sqlx::PgPool;

    #[sqlx::test(migrations = "./migrations")]
    async fn record_and_retrieve_snapshot(pool: PgPool) {
        let repo = PgPoolSnapshotRepository::new(pool);
        let chain = ChainId(1);
        let depth = U256::from(500_000_000u64);

        repo.record_snapshot(chain, &depth).await;
        let result = repo.get_latest_depth(chain).await;
        assert!(result.is_some());
        assert_eq!(result.unwrap(), depth.to_string());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn get_latest_depth_returns_most_recent(pool: PgPool) {
        let repo = PgPoolSnapshotRepository::new(pool);
        let chain = ChainId(2);

        repo.record_snapshot(chain, &U256::from(100u64)).await;
        repo.record_snapshot(chain, &U256::from(200u64)).await;
        repo.record_snapshot(chain, &U256::from(300u64)).await;

        let result = repo.get_latest_depth(chain).await;
        assert_eq!(result.unwrap(), "300");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn get_latest_depth_missing_chain_returns_none(pool: PgPool) {
        let repo = PgPoolSnapshotRepository::new(pool);
        let result = repo.get_latest_depth(ChainId(9999)).await;
        assert!(result.is_none());
    }
}
