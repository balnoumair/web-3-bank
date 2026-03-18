// Watcher / verifier module — implemented in Task 06.
//
// Responsibilities (future):
//   - Monitor relay_logs for mismatches between expected and actual outcomes
//   - Raise alerts in treasury.watcher_alerts
//   - Expose alert stream via GetWatcherAlerts RPC

use tonic::{Request, Response, Status};

use crate::proto::treasury::{
    GetWatcherAlertsRequest, GetWatcherAlertsResponse,
};

pub struct Watcher;

impl Watcher {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_watcher_alerts(
        &self,
        _req: Request<GetWatcherAlertsRequest>,
    ) -> Result<Response<GetWatcherAlertsResponse>, Status> {
        Err(Status::unimplemented("get_watcher_alerts not yet implemented"))
    }
}
