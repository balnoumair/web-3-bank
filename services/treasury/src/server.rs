//! gRPC server bootstrap.
//!
//! Initialises the database pool, constructs all service components, and
//! starts the tonic gRPC server on the configured `GRPC_PORT`.

use std::fs;
use std::sync::Arc;

use alloy_primitives::keccak256;
use sqlx::PgPool;
use tokio::sync::Mutex;
use tonic::{Request, Response, Status};
use tracing::info;

use crate::account_activity::AccountActivityService;
use crate::account_balance::AccountBalanceService;
use crate::account_withdrawal_routing::AccountWithdrawalRoutingService;
use crate::config::Config;
use crate::decommission::{ChainStatePort, DecommissionOrchestrator, HolderIndexPort};
use crate::decommission_runtime::{
    build_drain_plan, make_drain_id, parse_drain_id, RuntimeBankDrain, RuntimeChainState,
    RuntimeHolderIndex,
};
use crate::domain::newtypes::ChainId;
use crate::domain::repository::DecommissionRepository;
use crate::hot_path::HotPath;
use crate::pool_manager::PoolManager;
use crate::proto::treasury::{
    health_check_response, treasury_service_server::TreasuryService, DecommissionStatusCount,
    GetAccountActivityRequest, GetAccountActivityResponse, GetBalanceRequest, GetBalanceResponse,
    GetDecommissionDrainStatusRequest, GetDecommissionDrainStatusResponse, GetPoolDepthRequest,
    GetPoolDepthResponse, GetRelayStatusRequest, GetRelayStatusResponse, GetWatcherAlertsRequest,
    GetWatcherAlertsResponse, GetWithdrawalRoutingRequest, GetWithdrawalRoutingResponse,
    HealthCheckRequest, HealthCheckResponse, IsChainActiveRequest, IsChainActiveResponse,
    IsChainDecommissionedRequest, IsChainDecommissionedResponse, StartDecommissionDrainRequest,
    StartDecommissionDrainResponse,
};
use crate::watcher::Watcher;

#[derive(Default)]
pub struct DrainRuntimeState {
    pub running_drain_id: Option<String>,
    pub resumable_drain_id: Option<String>,
}

pub struct TreasuryServer {
    pub cfg: Arc<Config>,
    pub http: reqwest::Client,
    pub pool: PgPool,
    pub hot_path: Arc<HotPath>,
    pub account_balance: Arc<AccountBalanceService>,
    pub account_withdrawal_routing: Arc<AccountWithdrawalRoutingService>,
    pub account_activity: Arc<AccountActivityService>,
    pub pool_manager: Arc<PoolManager>,
    pub watcher: Arc<Watcher>,
    /// Cached result of the startup relayer-key check.
    pub relayer_key_loaded: bool,
    /// Cached result of the startup RPC reachability check.
    pub rpc_reachable: bool,
    pub decommission_repo: Arc<dyn DecommissionRepository>,
    pub decommission_orchestrator: Arc<DecommissionOrchestrator>,
    pub holder_index_runtime: Arc<RuntimeHolderIndex>,
    pub chain_state_runtime: Arc<RuntimeChainState>,
    pub bank_drain_runtime: Arc<RuntimeBankDrain>,
    pub drain_runtime: Arc<Mutex<DrainRuntimeState>>,
}

#[tonic::async_trait]
impl TreasuryService for TreasuryServer {
    async fn health_check(
        &self,
        _req: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let db_connected = sqlx::query("SELECT 1").execute(&self.pool).await.is_ok();

        let status = if db_connected && self.rpc_reachable && self.relayer_key_loaded {
            health_check_response::Status::Serving
        } else {
            health_check_response::Status::NotServing
        };

        info!(
            db_connected,
            rpc_reachable = self.rpc_reachable,
            relayer_key_loaded = self.relayer_key_loaded,
            "health_check"
        );

        Ok(Response::new(HealthCheckResponse {
            status: status as i32,
            db_connected,
            rpc_reachable: self.rpc_reachable,
            relayer_key_loaded: self.relayer_key_loaded,
        }))
    }

    async fn get_relay_status(
        &self,
        req: Request<GetRelayStatusRequest>,
    ) -> Result<Response<GetRelayStatusResponse>, Status> {
        self.hot_path.get_relay_status(req).await
    }

    async fn get_pool_depth(
        &self,
        req: Request<GetPoolDepthRequest>,
    ) -> Result<Response<GetPoolDepthResponse>, Status> {
        self.pool_manager.get_pool_depth(req).await
    }

    async fn get_watcher_alerts(
        &self,
        req: Request<GetWatcherAlertsRequest>,
    ) -> Result<Response<GetWatcherAlertsResponse>, Status> {
        self.watcher.get_watcher_alerts(req).await
    }

    async fn get_balance(
        &self,
        req: Request<GetBalanceRequest>,
    ) -> Result<Response<GetBalanceResponse>, Status> {
        self.account_balance.get_balance(req).await
    }

    async fn get_account_activity(
        &self,
        req: Request<GetAccountActivityRequest>,
    ) -> Result<Response<GetAccountActivityResponse>, Status> {
        self.account_activity.get_account_activity(req).await
    }

    async fn get_withdrawal_routing(
        &self,
        req: Request<GetWithdrawalRoutingRequest>,
    ) -> Result<Response<GetWithdrawalRoutingResponse>, Status> {
        self.account_withdrawal_routing
            .get_withdrawal_routing(req)
            .await
    }

    async fn is_chain_active(
        &self,
        req: Request<IsChainActiveRequest>,
    ) -> Result<Response<IsChainActiveResponse>, Status> {
        let chain_id = req.into_inner().chain_id;
        let active = self.hot_path.is_chain_active(chain_id).await;
        Ok(Response::new(IsChainActiveResponse { active }))
    }

    async fn is_chain_decommissioned(
        &self,
        req: Request<IsChainDecommissionedRequest>,
    ) -> Result<Response<IsChainDecommissionedResponse>, Status> {
        let chain_id = req.into_inner().chain_id;
        let decommissioned = self.hot_path.is_chain_decommissioned(chain_id).await;
        Ok(Response::new(IsChainDecommissionedResponse {
            decommissioned,
        }))
    }

    async fn start_decommission_drain(
        &self,
        req: Request<StartDecommissionDrainRequest>,
    ) -> Result<Response<StartDecommissionDrainResponse>, Status> {
        self.check_decommission_admin(&req)?;
        let payload = req.into_inner();
        let source_chain = ChainId(payload.source_chain);
        let target_chain = ChainId(payload.target_chain);
        let drain_id = make_drain_id(source_chain, target_chain);

        if !self
            .holder_index_runtime
            .index_fresh_enough(source_chain)
            .await
        {
            return Err(Status::failed_precondition(
                "account_events index cursor is too stale for source chain",
            ));
        }
        if !self
            .chain_state_runtime
            .is_source_draining(source_chain)
            .await
        {
            return Err(Status::failed_precondition(
                "source chain is not marked draining in RouteReceiver",
            ));
        }
        if !self.chain_state_runtime.is_chain_active(target_chain).await {
            return Err(Status::failed_precondition("target chain is not active"));
        }
        if !self
            .bank_drain_runtime
            .has_required_roles(source_chain)
            .await
        {
            return Err(Status::failed_precondition(
                "treasury signer missing required bank roles",
            ));
        }

        let mut runtime = self.drain_runtime.lock().await;
        if runtime.running_drain_id.is_some() {
            return Err(Status::already_exists("a drain is already running"));
        }
        if let Some(resumable) = runtime.resumable_drain_id.clone() {
            if resumable != drain_id {
                return Err(Status::already_exists(
                    "a different drain is resumable; resume it before starting another",
                ));
            }
        }

        let holders = self
            .holder_index_runtime
            .holders_for_chain(source_chain)
            .await;
        let mut balance_of_selector = [0u8; 4];
        balance_of_selector.copy_from_slice(&keccak256(b"balanceOf(address)")[..4]);
        let mut sync_usd_selector = [0u8; 4];
        sync_usd_selector.copy_from_slice(&keccak256(b"syncUSD()")[..4]);
        let plan = build_drain_plan(
            Arc::clone(&self.cfg),
            self.http.clone(),
            source_chain,
            target_chain,
            holders,
            balance_of_selector,
            sync_usd_selector,
        )
        .await;

        let dust_tolerance = self
            .cfg
            .decommission_dust_tolerance_wei
            .parse::<u128>()
            .unwrap_or(0);
        let holder_total = plan.holders.iter().fold(0u128, |acc, holder| {
            acc.saturating_add(holder.amount.to::<u128>())
        });
        if holder_total <= dust_tolerance
            && plan.pool_amount.is_zero()
            && plan.reserve_amount.is_zero()
        {
            return Err(Status::failed_precondition(
                "drain plan has no actionable balances above dust tolerance",
            ));
        }

        runtime.running_drain_id = Some(drain_id.clone());
        runtime.resumable_drain_id = Some(drain_id.clone());
        drop(runtime);

        let orch = Arc::clone(&self.decommission_orchestrator);
        let runtime_state = Arc::clone(&self.drain_runtime);
        let repo = Arc::clone(&self.decommission_repo);
        let drain_id_for_task = drain_id.clone();
        tokio::spawn(async move {
            let _ = orch.run_drain_plan(plan).await;
            let mut state = runtime_state.lock().await;
            state.running_drain_id = None;
            if repo.has_incomplete_ops().await {
                state.resumable_drain_id = Some(drain_id_for_task);
            } else {
                state.resumable_drain_id = None;
            }
        });

        Ok(Response::new(StartDecommissionDrainResponse { drain_id }))
    }

    async fn get_decommission_drain_status(
        &self,
        req: Request<GetDecommissionDrainStatusRequest>,
    ) -> Result<Response<GetDecommissionDrainStatusResponse>, Status> {
        self.check_decommission_admin(&req)?;
        let requested = req.into_inner().drain_id;
        let drain_id = if requested.trim().is_empty() {
            let runtime = self.drain_runtime.lock().await;
            runtime
                .running_drain_id
                .clone()
                .or(runtime.resumable_drain_id.clone())
                .ok_or_else(|| Status::not_found("no running or resumable drain found"))?
        } else {
            requested
        };
        let (source_chain, target_chain) = parse_drain_id(&drain_id)
            .ok_or_else(|| Status::invalid_argument("drain_id must be '<source>-<target>'"))?;

        let counts = self
            .decommission_repo
            .status_counts(source_chain, target_chain)
            .await;
        let drained_amount_wei = self
            .decommission_repo
            .drained_amount_wei(source_chain, target_chain)
            .await;
        let last_error = self
            .decommission_repo
            .last_error(source_chain, target_chain)
            .await
            .unwrap_or_default();

        let runtime = self.drain_runtime.lock().await;
        let state = derive_drain_state(runtime.running_drain_id.as_deref(), &drain_id, &counts);
        let status_counts = counts
            .into_iter()
            .map(|(status, count)| DecommissionStatusCount { status, count })
            .collect();
        Ok(Response::new(GetDecommissionDrainStatusResponse {
            drain_id,
            state,
            status_counts,
            drained_amount_wei,
            last_error,
        }))
    }
}

fn derive_drain_state(
    running_drain_id: Option<&str>,
    requested_drain_id: &str,
    counts: &[(String, u64)],
) -> String {
    if running_drain_id == Some(requested_drain_id) {
        return "running".to_string();
    }
    if counts
        .iter()
        .any(|(status, _)| status == "paused" || status == "failed")
    {
        return "paused".to_string();
    }
    if counts
        .iter()
        .any(|(status, _)| status == "pending" || status == "submitted")
    {
        return "resumable".to_string();
    }
    "completed".to_string()
}

// ── Startup checks ────────────────────────────────────────────────────────────

/// Check that the relayer key file exists and contains a valid 32-byte hex key.
pub fn check_relayer_key(path: &str) -> bool {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let hex = contents.trim().trim_start_matches("0x");
            hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())
        }
        Err(_) => false,
    }
}

/// Fire an `eth_blockNumber` JSON-RPC call to verify reachability.
pub async fn check_rpc_reachable(
    rpc_urls: &std::collections::HashMap<u64, String>,
    http: &reqwest::Client,
) -> bool {
    if rpc_urls.is_empty() {
        return false;
    }
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_blockNumber",
        "params": [],
        "id": 1
    });
    for url in rpc_urls.values() {
        match http.post(url).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => continue,
            _ => return false,
        }
    }
    true
}

impl TreasuryServer {
    #[allow(clippy::result_large_err)]
    fn check_decommission_admin<T>(&self, req: &Request<T>) -> Result<(), Status> {
        let expected = self.cfg.decommission_admin_token.as_ref().ok_or_else(|| {
            Status::failed_precondition("DECOMMISSION_ADMIN_TOKEN is not configured")
        })?;

        if let Some(raw) = req
            .metadata()
            .get("x-decommission-admin-token")
            .and_then(|v| v.to_str().ok())
        {
            if raw == expected {
                return Ok(());
            }
        }
        if let Some(raw_auth) = req
            .metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
        {
            if raw_auth.strip_prefix("Bearer ").unwrap_or(raw_auth) == expected {
                return Ok(());
            }
        }
        Err(Status::permission_denied(
            "missing or invalid decommission admin token",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::derive_drain_state;
    use crate::decommission_runtime::{make_drain_id, parse_drain_id};
    use crate::domain::newtypes::ChainId;

    #[test]
    fn parse_and_make_drain_id_round_trip() {
        let id = make_drain_id(ChainId(84532), ChainId(421614));
        let parsed = parse_drain_id(&id).expect("valid drain id");
        assert_eq!(parsed.0 .0, 84532);
        assert_eq!(parsed.1 .0, 421614);
    }

    #[test]
    fn parse_drain_id_rejects_invalid_format() {
        assert!(parse_drain_id("84532").is_none());
        assert!(parse_drain_id("a-b").is_none());
        assert!(parse_drain_id("84532-").is_none());
    }

    #[test]
    fn derive_state_running_wins() {
        let counts = vec![("failed".to_string(), 1)];
        let state = derive_drain_state(Some("84532-421614"), "84532-421614", &counts);
        assert_eq!(state, "running");
    }

    #[test]
    fn derive_state_paused_when_failed_or_paused_present() {
        let counts = vec![("failed".to_string(), 1)];
        let state = derive_drain_state(None, "84532-421614", &counts);
        assert_eq!(state, "paused");
    }

    #[test]
    fn derive_state_resumable_when_pending_or_submitted_present() {
        let counts = vec![("pending".to_string(), 2)];
        let state = derive_drain_state(None, "84532-421614", &counts);
        assert_eq!(state, "resumable");
    }

    #[test]
    fn derive_state_completed_when_no_active_counts() {
        let counts = vec![("completed".to_string(), 5)];
        let state = derive_drain_state(None, "84532-421614", &counts);
        assert_eq!(state, "completed");
    }
}
