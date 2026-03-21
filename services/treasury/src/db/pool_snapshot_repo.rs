use alloy_primitives::U256;
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::error;

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
    async fn record_snapshot(&self, chain_id: u64, depth: &U256) {
        let depth_str = depth.to_string();
        if let Err(e) = sqlx::query(
            "INSERT INTO treasury.pool_snapshots (chain_id, depth_wei) \
             VALUES ($1, $2::NUMERIC)",
        )
        .bind(chain_id as i64)
        .bind(&depth_str)
        .execute(&self.pool)
        .await
        {
            error!(chain = chain_id, err = %e, "pool_snapshot_repo: failed to record snapshot");
        }
    }

    async fn get_latest_depth(&self, chain_id: i64) -> Option<String> {
        let row = sqlx::query(
            "SELECT depth_wei::TEXT FROM treasury.pool_snapshots \
             WHERE chain_id = $1 \
             ORDER BY recorded_at DESC \
             LIMIT 1",
        )
        .bind(chain_id)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten()?;

        use sqlx::Row;
        row.try_get("depth_wei").ok()
    }
}
