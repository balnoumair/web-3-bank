//! Governance-directed chain drain orchestration.
//!
//! The real CCIP and user-service calls are expressed as ports so the control
//! flow can be tested deterministically: skip completed holders on restart,
//! pause when the target chain is inactive, move holder balances, then drain
//! pool and reserve liquidity.

use std::collections::HashSet;
use std::sync::Arc;

use alloy_primitives::U256;
use async_trait::async_trait;
use tracing::warn;

use crate::domain::newtypes::{ChainId, OperationId, TxHash};
use crate::domain::repository::{DecommissionOpStatus, DecommissionRepository};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HolderBalance {
    pub address: String,
    pub amount: U256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrainPlan {
    pub source_chain: ChainId,
    pub target_chain: ChainId,
    pub holders: Vec<HolderBalance>,
    pub pool_amount: U256,
    pub reserve_amount: U256,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrainOutcome {
    Completed { holders_drained: usize },
    PausedTargetInactive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeReceipt {
    pub src_message_id: Option<String>,
    pub dst_tx_hash: Option<TxHash>,
}

#[async_trait]
pub trait ChainStatePort: Send + Sync {
    async fn is_chain_active(&self, chain: ChainId) -> bool;
}

#[async_trait]
pub trait HolderIndexPort: Send + Sync {
    async fn holders_for_chain(&self, chain: ChainId) -> Vec<String>;
    async fn balance_of(&self, chain: ChainId, holder: &str) -> U256;
}

#[async_trait]
pub trait BankDrainPort: Send + Sync {
    async fn bridge_holder(
        &self,
        source_chain: ChainId,
        target_chain: ChainId,
        holder: &str,
        amount: U256,
    ) -> Result<BridgeReceipt, String>;

    async fn drain_pool(
        &self,
        source_chain: ChainId,
        target_chain: ChainId,
        amount: U256,
    ) -> Result<(), String>;

    async fn drain_reserve(
        &self,
        source_chain: ChainId,
        target_chain: ChainId,
        amount: U256,
    ) -> Result<(), String>;
}

#[async_trait]
pub trait UserHomeChainPort: Send + Sync {
    async fn set_user_home_chain(&self, holder: &str, target_chain: ChainId) -> Result<(), String>;
}

#[async_trait]
pub trait OperatorAlertPort: Send + Sync {
    async fn alert(&self, message: &str);
}

pub struct DecommissionOrchestrator {
    repository: Arc<dyn DecommissionRepository>,
    chain_state: Arc<dyn ChainStatePort>,
    holder_index: Arc<dyn HolderIndexPort>,
    bank: Arc<dyn BankDrainPort>,
    user_home_chain: Arc<dyn UserHomeChainPort>,
    alerts: Arc<dyn OperatorAlertPort>,
}

impl DecommissionOrchestrator {
    pub fn new(
        repository: Arc<dyn DecommissionRepository>,
        chain_state: Arc<dyn ChainStatePort>,
        holder_index: Arc<dyn HolderIndexPort>,
        bank: Arc<dyn BankDrainPort>,
        user_home_chain: Arc<dyn UserHomeChainPort>,
        alerts: Arc<dyn OperatorAlertPort>,
    ) -> Self {
        Self {
            repository,
            chain_state,
            holder_index,
            bank,
            user_home_chain,
            alerts,
        }
    }

    /// Entry point for a governance `markDecommissioning` event.
    pub async fn handle_mark_decommissioning(
        &self,
        source_chain: ChainId,
        target_chain: ChainId,
        pool_amount: U256,
        reserve_amount: U256,
    ) -> DrainOutcome {
        let mut resolved = Vec::new();
        for address in self.holder_index.holders_for_chain(source_chain).await {
            let amount = self.holder_index.balance_of(source_chain, &address).await;
            if !amount.is_zero() {
                resolved.push(HolderBalance { address, amount });
            }
        }

        self.run_drain_plan(DrainPlan {
            source_chain,
            target_chain,
            holders: resolved,
            pool_amount,
            reserve_amount,
        })
        .await
    }

    pub async fn run_drain_plan(&self, plan: DrainPlan) -> DrainOutcome {
        if !self.chain_state.is_chain_active(plan.target_chain).await {
            self.alerts
                .alert("chain decommission drain paused: target chain inactive")
                .await;
            return DrainOutcome::PausedTargetInactive;
        }

        let completed: HashSet<String> = self
            .repository
            .completed_holders(plan.source_chain, plan.target_chain)
            .await
            .into_iter()
            .map(|address| address.to_lowercase())
            .collect();

        let mut holders_drained = 0;
        for holder in plan.holders {
            if completed.contains(&holder.address.to_lowercase()) {
                continue;
            }

            let op_id = OperationId(format!(
                "decom-{}-{}-{}",
                plan.source_chain, plan.target_chain, holder.address
            ));

            self.repository
                .insert_holder_op(
                    &op_id,
                    plan.source_chain,
                    plan.target_chain,
                    &holder.address,
                    &holder.amount,
                    DecommissionOpStatus::Pending,
                )
                .await;

            match self
                .bank
                .bridge_holder(
                    plan.source_chain,
                    plan.target_chain,
                    &holder.address,
                    holder.amount,
                )
                .await
            {
                Ok(receipt) => {
                    self.repository
                        .mark_holder_submitted(
                            &op_id,
                            receipt.src_message_id.as_deref(),
                            receipt.dst_tx_hash.as_ref(),
                        )
                        .await;
                    if let Err(err) = self
                        .user_home_chain
                        .set_user_home_chain(&holder.address, plan.target_chain)
                        .await
                    {
                        warn!(
                            holder = holder.address,
                            err, "decommission: home-chain update failed"
                        );
                        self.repository.mark_op_failed(&op_id, &err).await;
                        continue;
                    }
                    self.repository.mark_holder_completed(&op_id).await;
                    holders_drained += 1;
                }
                Err(err) => {
                    warn!(
                        holder = holder.address,
                        err, "decommission: holder bridge failed"
                    );
                    self.repository.mark_op_failed(&op_id, &err).await;
                }
            }
        }

        if !plan.pool_amount.is_zero() {
            let _ = self
                .bank
                .drain_pool(plan.source_chain, plan.target_chain, plan.pool_amount)
                .await;
        }
        if !plan.reserve_amount.is_zero() {
            let _ = self
                .bank
                .drain_reserve(plan.source_chain, plan.target_chain, plan.reserve_amount)
                .await;
        }

        self.alerts
            .alert("chain decommission drain complete: governance finalization ready")
            .await;
        DrainOutcome::Completed { holders_drained }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemoryRepo {
        completed: Mutex<HashSet<String>>,
        inserted: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl DecommissionRepository for MemoryRepo {
        async fn completed_holders(
            &self,
            _source_chain: ChainId,
            _target_chain: ChainId,
        ) -> Vec<String> {
            self.completed.lock().unwrap().iter().cloned().collect()
        }

        async fn insert_holder_op(
            &self,
            _op_id: &OperationId,
            _source_chain: ChainId,
            _target_chain: ChainId,
            holder_address: &str,
            _amount: &U256,
            _status: DecommissionOpStatus,
        ) {
            self.inserted
                .lock()
                .unwrap()
                .push(holder_address.to_string());
        }

        async fn mark_holder_submitted(
            &self,
            _op_id: &OperationId,
            _src_message_id: Option<&str>,
            _dst_tx_hash: Option<&TxHash>,
        ) {
        }

        async fn mark_holder_completed(&self, op_id: &OperationId) {
            let holder = op_id.as_str().rsplit('-').next().unwrap().to_string();
            self.completed.lock().unwrap().insert(holder);
        }

        async fn mark_op_failed(&self, _op_id: &OperationId, _failure_reason: &str) {}

        async fn has_incomplete_ops(&self) -> bool {
            false
        }

        async fn latest_incomplete_pair(&self) -> Option<(ChainId, ChainId)> {
            None
        }

        async fn status_counts(
            &self,
            _source_chain: ChainId,
            _target_chain: ChainId,
        ) -> Vec<(String, u64)> {
            Vec::new()
        }

        async fn drained_amount_wei(&self, _source_chain: ChainId, _target_chain: ChainId) -> String {
            "0".to_string()
        }

        async fn last_error(&self, _source_chain: ChainId, _target_chain: ChainId) -> Option<String> {
            None
        }
    }

    struct StaticChainState(bool);

    #[async_trait]
    impl ChainStatePort for StaticChainState {
        async fn is_chain_active(&self, _chain: ChainId) -> bool {
            self.0
        }
    }

    struct EmptyHolderIndex;

    #[async_trait]
    impl HolderIndexPort for EmptyHolderIndex {
        async fn holders_for_chain(&self, _chain: ChainId) -> Vec<String> {
            Vec::new()
        }

        async fn balance_of(&self, _chain: ChainId, _holder: &str) -> U256 {
            U256::ZERO
        }
    }

    #[derive(Default)]
    struct FakeBank {
        holder_bridges: Mutex<Vec<String>>,
        pool_drained: Mutex<bool>,
        reserve_drained: Mutex<bool>,
    }

    #[async_trait]
    impl BankDrainPort for FakeBank {
        async fn bridge_holder(
            &self,
            _source_chain: ChainId,
            _target_chain: ChainId,
            holder: &str,
            _amount: U256,
        ) -> Result<BridgeReceipt, String> {
            self.holder_bridges.lock().unwrap().push(holder.to_string());
            Ok(BridgeReceipt {
                src_message_id: Some("0xmsg".to_string()),
                dst_tx_hash: Some(TxHash("0xtx".to_string())),
            })
        }

        async fn drain_pool(
            &self,
            _source_chain: ChainId,
            _target_chain: ChainId,
            _amount: U256,
        ) -> Result<(), String> {
            *self.pool_drained.lock().unwrap() = true;
            Ok(())
        }

        async fn drain_reserve(
            &self,
            _source_chain: ChainId,
            _target_chain: ChainId,
            _amount: U256,
        ) -> Result<(), String> {
            *self.reserve_drained.lock().unwrap() = true;
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeHomeChain {
        updates: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl UserHomeChainPort for FakeHomeChain {
        async fn set_user_home_chain(
            &self,
            holder: &str,
            _target_chain: ChainId,
        ) -> Result<(), String> {
            self.updates.lock().unwrap().push(holder.to_string());
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeAlerts {
        messages: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl OperatorAlertPort for FakeAlerts {
        async fn alert(&self, message: &str) {
            self.messages.lock().unwrap().push(message.to_string());
        }
    }

    fn orchestrator(
        repo: Arc<MemoryRepo>,
        bank: Arc<FakeBank>,
        home: Arc<FakeHomeChain>,
        alerts: Arc<FakeAlerts>,
        active: bool,
    ) -> DecommissionOrchestrator {
        DecommissionOrchestrator::new(
            repo,
            Arc::new(StaticChainState(active)),
            Arc::new(EmptyHolderIndex),
            bank,
            home,
            alerts,
        )
    }

    #[tokio::test]
    async fn drains_holders_then_pool_and_reserve() {
        let repo = Arc::new(MemoryRepo::default());
        let bank = Arc::new(FakeBank::default());
        let home = Arc::new(FakeHomeChain::default());
        let alerts = Arc::new(FakeAlerts::default());
        let orch = orchestrator(
            Arc::clone(&repo),
            Arc::clone(&bank),
            Arc::clone(&home),
            Arc::clone(&alerts),
            true,
        );

        let outcome = orch
            .run_drain_plan(DrainPlan {
                source_chain: ChainId(1),
                target_chain: ChainId(2),
                holders: vec![
                    HolderBalance {
                        address: "0xaaa".to_string(),
                        amount: U256::from(10u64),
                    },
                    HolderBalance {
                        address: "0xbbb".to_string(),
                        amount: U256::from(20u64),
                    },
                ],
                pool_amount: U256::from(30u64),
                reserve_amount: U256::from(40u64),
            })
            .await;

        assert_eq!(outcome, DrainOutcome::Completed { holders_drained: 2 });
        assert_eq!(bank.holder_bridges.lock().unwrap().len(), 2);
        assert_eq!(home.updates.lock().unwrap().len(), 2);
        assert!(*bank.pool_drained.lock().unwrap());
        assert!(*bank.reserve_drained.lock().unwrap());
        assert!(alerts
            .messages
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .contains("finalization ready"));
    }

    #[tokio::test]
    async fn pauses_cleanly_when_target_chain_inactive() {
        let repo = Arc::new(MemoryRepo::default());
        let bank = Arc::new(FakeBank::default());
        let home = Arc::new(FakeHomeChain::default());
        let alerts = Arc::new(FakeAlerts::default());
        let orch = orchestrator(repo, Arc::clone(&bank), home, Arc::clone(&alerts), false);

        let outcome = orch
            .run_drain_plan(DrainPlan {
                source_chain: ChainId(1),
                target_chain: ChainId(2),
                holders: vec![HolderBalance {
                    address: "0xaaa".to_string(),
                    amount: U256::from(10u64),
                }],
                pool_amount: U256::from(30u64),
                reserve_amount: U256::from(40u64),
            })
            .await;

        assert_eq!(outcome, DrainOutcome::PausedTargetInactive);
        assert!(bank.holder_bridges.lock().unwrap().is_empty());
        assert!(alerts.messages.lock().unwrap()[0].contains("target chain inactive"));
    }

    #[tokio::test]
    async fn restart_skips_already_completed_holders() {
        let repo = Arc::new(MemoryRepo::default());
        repo.completed.lock().unwrap().insert("0xaaa".to_string());
        let bank = Arc::new(FakeBank::default());
        let home = Arc::new(FakeHomeChain::default());
        let alerts = Arc::new(FakeAlerts::default());
        let orch = orchestrator(repo, Arc::clone(&bank), home, alerts, true);

        let outcome = orch
            .run_drain_plan(DrainPlan {
                source_chain: ChainId(1),
                target_chain: ChainId(2),
                holders: vec![
                    HolderBalance {
                        address: "0xaaa".to_string(),
                        amount: U256::from(10u64),
                    },
                    HolderBalance {
                        address: "0xbbb".to_string(),
                        amount: U256::from(20u64),
                    },
                ],
                pool_amount: U256::ZERO,
                reserve_amount: U256::ZERO,
            })
            .await;

        assert_eq!(outcome, DrainOutcome::Completed { holders_drained: 1 });
        assert_eq!(bank.holder_bridges.lock().unwrap().as_slice(), ["0xbbb"]);
    }
}
