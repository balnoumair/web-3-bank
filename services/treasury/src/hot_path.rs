// Hot-path relay module — implemented in Task 05.
//
// Responsibilities (future):
//   - Listen for DepositReceived events on source chains
//   - Execute cross-chain relay via RouteReceiver.sol
//   - Record relay attempts in treasury.relay_logs

use tonic::{Request, Response, Status};

use crate::proto::treasury::{
    GetRelayStatusRequest, GetRelayStatusResponse,
};

pub struct HotPath;

impl HotPath {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_relay_status(
        &self,
        _req: Request<GetRelayStatusRequest>,
    ) -> Result<Response<GetRelayStatusResponse>, Status> {
        Err(Status::unimplemented("get_relay_status not yet implemented"))
    }
}
