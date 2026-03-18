// Pool manager module — implemented in Tasks 05 / 10.
//
// Responsibilities (future):
//   - Track liquidity pool depth per chain
//   - Trigger rebalancing when depth falls below threshold
//   - Record snapshots in treasury.pool_snapshots

use tonic::{Request, Response, Status};

use crate::proto::treasury::{GetPoolDepthRequest, GetPoolDepthResponse};

pub struct PoolManager;

impl PoolManager {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_pool_depth(
        &self,
        _req: Request<GetPoolDepthRequest>,
    ) -> Result<Response<GetPoolDepthResponse>, Status> {
        Err(Status::unimplemented("get_pool_depth not yet implemented"))
    }
}
