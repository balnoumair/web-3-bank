//! User-facing account activity feed (`GetAccountActivity`).

use std::sync::Arc;

use tonic::{Request, Response, Status};

use crate::domain::activity::map_event_to_activity;
use crate::domain::newtypes::EventHash;
use crate::domain::repository::{AccountEventRepository, RelayRepository};
use crate::proto::treasury::{
    ActivityEntry, GetAccountActivityRequest, GetAccountActivityResponse,
};

pub struct AccountActivityService {
    account_events: Arc<dyn AccountEventRepository>,
    relay_repo: Arc<dyn RelayRepository>,
}

impl AccountActivityService {
    pub fn new(
        account_events: Arc<dyn AccountEventRepository>,
        relay_repo: Arc<dyn RelayRepository>,
    ) -> Arc<Self> {
        Arc::new(Self {
            account_events,
            relay_repo,
        })
    }

    pub async fn get_account_activity(
        &self,
        req: Request<GetAccountActivityRequest>,
    ) -> Result<Response<GetAccountActivityResponse>, Status> {
        let inner = req.into_inner();
        let limit = inner.limit.max(1).min(100) as i64;
        let rows = self
            .account_events
            .list_activity_for_user(&inner.address, limit)
            .await;

        let mut entries = Vec::new();
        for row in rows {
            let relay_status = if row.event_kind == "hot_path_initiated" {
                self.relay_repo
                    .get_relay_status(&EventHash(row.tx_hash.clone()))
                    .await
            } else {
                None
            };

            if let Some(view) =
                map_event_to_activity(&row, &inner.address, relay_status.as_deref())
            {
                entries.push(ActivityEntry {
                    kind: view.kind,
                    direction: view.direction,
                    counterparty: view.counterparty,
                    chain_id: view.chain_id,
                    amount_wei: view.amount_wei,
                    status: view.status,
                    tx_hash: view.tx_hash,
                    occurred_at: view.occurred_at,
                });
            }
        }

        Ok(Response::new(GetAccountActivityResponse { entries }))
    }
}
