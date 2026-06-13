//! Indexes on-chain account-affecting events into `treasury.account_events`.
//!
//! Polls Bank Contract logs (`Deposited`, `Withdrawn`, `HotPathInitiated`,
//! `HotPathReleased`) and SyncUSD `Transfer` events per chain. Persists a
//! block cursor so indexing resumes after restart. Triggers `SetUserHomeChain`
//! on the first indexed deposit per user (replacing the standalone home-chain
//! poller).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{keccak256, B256, U256};
use tokio::sync::Mutex;
use tonic::Request;
use tracing::{info, warn};

use crate::config::Config;
use crate::domain::newtypes::ChainId;
use crate::domain::repository::{AccountEventRepository, AccountEventRow, UpsertEventResult};
use crate::eth;
use crate::user_pb::{user_service_client::UserServiceClient, SetUserHomeChainRequest};

const INDEX_POLL_INTERVAL: Duration = Duration::from_secs(3);
const MAX_BLOCK_RANGE: u64 = 2_000;

pub struct AccountIndexer {
    repo: Arc<dyn AccountEventRepository>,
    config: Arc<Config>,
    http: reqwest::Client,
    user_endpoint: Option<String>,
    bank_topics: BankEventTopics,
    transfer_topic: B256,
    sync_usd_selector: [u8; 4],
    sync_usd_cache: Mutex<HashMap<ChainId, String>>,
    block_time_cache: Mutex<HashMap<(ChainId, u64), i64>>,
}

struct BankEventTopics {
    deposited: String,
    withdrawn: String,
    hot_path_initiated: String,
    hot_path_released: String,
    all: Vec<String>,
}

impl BankEventTopics {
    fn new() -> Self {
        let deposited = format!("{}", keccak256(b"Deposited(address,address,uint256)"));
        let withdrawn = format!("{}", keccak256(b"Withdrawn(address,address,uint256)"));
        let hot_path_initiated = format!(
            "{}",
            keccak256(b"HotPathInitiated(address,address,uint256,uint256,bytes32,uint256)")
        );
        let hot_path_released =
            format!("{}", keccak256(b"HotPathReleased(address,uint256,bytes32)"));
        let all = vec![
            deposited.clone(),
            withdrawn.clone(),
            hot_path_initiated.clone(),
            hot_path_released.clone(),
        ];
        Self {
            deposited,
            withdrawn,
            hot_path_initiated,
            hot_path_released,
            all,
        }
    }
}

impl AccountIndexer {
    pub fn new(
        repo: Arc<dyn AccountEventRepository>,
        config: Arc<Config>,
        http: reqwest::Client,
        user_endpoint: Option<String>,
    ) -> Arc<Self> {
        let transfer_topic = keccak256(b"Transfer(address,address,uint256)");
        let sync_usd_hash = keccak256(b"syncUSD()");
        let mut sync_usd_selector = [0u8; 4];
        sync_usd_selector.copy_from_slice(&sync_usd_hash[..4]);

        Arc::new(Self {
            repo,
            config,
            http,
            user_endpoint,
            bank_topics: BankEventTopics::new(),
            transfer_topic,
            sync_usd_selector,
            sync_usd_cache: Mutex::new(HashMap::new()),
            block_time_cache: Mutex::new(HashMap::new()),
        })
    }

    pub fn spawn_background(self: Arc<Self>) {
        tokio::spawn(async move { self.poll_loop().await });
    }

    async fn poll_loop(self: Arc<Self>) {
        info!("account_index: indexer started");
        loop {
            self.block_time_cache.lock().await.clear();

            let chains: Vec<(ChainId, String, String)> = self
                .config
                .rpc_urls
                .iter()
                .filter_map(|(&chain_id, rpc_url)| {
                    self.config
                        .contract_addresses
                        .get(&chain_id)
                        .map(|addr| (ChainId(chain_id), rpc_url.clone(), addr.clone()))
                })
                .collect();

            for (chain_id, rpc_url, bank_addr) in chains {
                if let Err(e) = self.index_chain(chain_id, &rpc_url, &bank_addr).await {
                    warn!(chain_id = chain_id.0, err = %e, "account_index: chain poll failed");
                }
            }

            tokio::time::sleep(INDEX_POLL_INTERVAL).await;
        }
    }

    async fn index_chain(
        &self,
        chain_id: ChainId,
        rpc_url: &str,
        bank_addr: &str,
    ) -> Result<(), String> {
        let Some(to_block) = eth::fetch_block_number(&self.http, rpc_url).await else {
            return Err("eth_blockNumber failed".into());
        };

        let scan_from = match self.repo.get_cursor(chain_id).await {
            Some(last) => last,
            None => self
                .config
                .index_from_blocks
                .as_ref()
                .and_then(|m| m.get(&chain_id.0).copied())
                .unwrap_or(0),
        };
        let scan_from = scan_from.min(to_block);
        let scan_from = scan_from.max(to_block.saturating_sub(MAX_BLOCK_RANGE));

        if scan_from > to_block {
            return Ok(());
        }

        let bank_logs = eth::fetch_logs_any(
            &self.http,
            rpc_url,
            bank_addr,
            &self.bank_topics.all,
            scan_from,
            to_block,
        )
        .await;

        for log in bank_logs {
            if let Some(row) = self.parse_bank_log(&log, chain_id, rpc_url).await {
                self.ingest_row(row).await;
            }
        }

        if let Some(token_addr) = self.resolve_sync_usd(chain_id, rpc_url, bank_addr).await {
            let transfer_topic = format!("{}", self.transfer_topic);
            let transfer_logs = eth::fetch_logs(
                &self.http,
                rpc_url,
                &token_addr,
                &transfer_topic,
                scan_from,
                to_block,
            )
            .await;

            for log in transfer_logs {
                if let Some(row) = self
                    .parse_transfer_log(&log, chain_id, rpc_url, bank_addr)
                    .await
                {
                    self.ingest_row(row).await;
                }
            }
        }

        self.repo.set_cursor(chain_id, to_block + 1).await;
        Ok(())
    }

    async fn resolve_sync_usd(
        &self,
        chain_id: ChainId,
        rpc_url: &str,
        bank_addr: &str,
    ) -> Option<String> {
        {
            let cache = self.sync_usd_cache.lock().await;
            if let Some(addr) = cache.get(&chain_id) {
                return Some(addr.clone());
            }
        }

        let addr = eth::fetch_address_view(&self.http, rpc_url, bank_addr, &self.sync_usd_selector)
            .await?;
        self.sync_usd_cache
            .lock()
            .await
            .insert(chain_id, addr.clone());
        Some(addr)
    }

    async fn block_time_unix(
        &self,
        chain_id: ChainId,
        rpc_url: &str,
        block_number: u64,
    ) -> Option<i64> {
        let key = (chain_id, block_number);
        {
            let cache = self.block_time_cache.lock().await;
            if let Some(ts) = cache.get(&key) {
                return Some(*ts);
            }
        }
        let ts = eth::fetch_block_timestamp(&self.http, rpc_url, block_number).await?;
        self.block_time_cache.lock().await.insert(key, ts);
        Some(ts)
    }

    async fn ingest_row(&self, row: AccountEventRow) {
        let user_for_home = if row.event_kind == "deposited" {
            row.address_to.clone()
        } else {
            None
        };

        let should_set_home = if let Some(ref user) = user_for_home {
            !self.repo.user_has_deposit(user).await
        } else {
            false
        };

        let chain_id = ChainId(row.chain_id as u64);
        let upsert = self.repo.upsert_event(&row).await;

        if should_push_home_chain(upsert, should_set_home) {
            if let Some(user) = user_for_home {
                self.push_home_chain(&user, chain_id).await;
            }
        }
    }

    async fn push_home_chain(&self, user: &str, chain_id: ChainId) {
        let Some(ref endpoint) = self.user_endpoint else {
            return;
        };

        let uri = if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
            endpoint.clone()
        } else {
            format!("http://{endpoint}")
        };

        let Ok(mut client) = UserServiceClient::connect(uri).await else {
            warn!(
                user,
                chain_id = chain_id.0,
                "account_index: user-service unreachable"
            );
            return;
        };

        let req = Request::new(SetUserHomeChainRequest {
            tempo_address: user.to_string(),
            chain_id: chain_id.0,
            decommission_override: false,
            operator: String::new(),
        });
        if let Err(e) = client.set_user_home_chain(req).await {
            warn!(
                err = %e,
                user,
                chain_id = chain_id.0,
                "account_index: SetUserHomeChain failed"
            );
        } else {
            info!(
                user,
                chain_id = chain_id.0,
                "account_index: home chain set from deposit"
            );
        }
    }

    async fn parse_bank_log(
        &self,
        log: &eth::RpcLog,
        chain_id: ChainId,
        rpc_url: &str,
    ) -> Option<AccountEventRow> {
        let topic0 = log.topics.first()?;
        let block_number = eth::parse_hex_u64(&log.block_number)?;
        let log_index = eth::parse_hex_u64(&log.log_index)? as i32;
        let block_time_unix = self.block_time_unix(chain_id, rpc_url, block_number).await;

        if topic0.eq_ignore_ascii_case(&self.bank_topics.deposited) {
            return parse_deposited(log, chain_id, block_number, log_index, block_time_unix);
        }
        if topic0.eq_ignore_ascii_case(&self.bank_topics.withdrawn) {
            return parse_withdrawn(log, chain_id, block_number, log_index, block_time_unix);
        }
        if topic0.eq_ignore_ascii_case(&self.bank_topics.hot_path_initiated) {
            return parse_hot_path_initiated(
                log,
                chain_id,
                block_number,
                log_index,
                block_time_unix,
            );
        }
        if topic0.eq_ignore_ascii_case(&self.bank_topics.hot_path_released) {
            return parse_hot_path_released(
                log,
                chain_id,
                block_number,
                log_index,
                block_time_unix,
            );
        }
        None
    }

    async fn parse_transfer_log(
        &self,
        log: &eth::RpcLog,
        chain_id: ChainId,
        rpc_url: &str,
        bank_addr: &str,
    ) -> Option<AccountEventRow> {
        if log.topics.len() < 3 {
            return None;
        }
        let from = topic_to_address(&log.topics[1])?;
        let to = topic_to_address(&log.topics[2])?;
        let bank_lower = bank_addr.to_lowercase();

        if from.eq_ignore_ascii_case(&bank_lower) || to.eq_ignore_ascii_case(&bank_lower) {
            return None;
        }

        let data = eth::decode_hex(&log.data)?;
        if data.len() < 32 {
            return None;
        }
        let amount_bytes: [u8; 32] = data[0..32].try_into().ok()?;
        let amount = U256::from_be_bytes(amount_bytes);

        let block_number = eth::parse_hex_u64(&log.block_number)?;
        let log_index = eth::parse_hex_u64(&log.log_index)? as i32;
        let block_time_unix = self.block_time_unix(chain_id, rpc_url, block_number).await;

        Some(AccountEventRow {
            chain_id: chain_id.0 as i64,
            tx_hash: log.transaction_hash.clone(),
            log_index,
            event_kind: "transfer".to_string(),
            address_from: Some(from),
            address_to: Some(to),
            amount_wei: amount.to_string(),
            block_number: block_number as i64,
            block_time_unix,
            correlation: None,
        })
    }
}

/// Home-chain notification fires only on a newly inserted first deposit.
pub(crate) fn should_push_home_chain(upsert: UpsertEventResult, is_first_deposit: bool) -> bool {
    upsert == UpsertEventResult::Inserted && is_first_deposit
}

fn topic_to_address(topic: &str) -> Option<String> {
    let raw = eth::decode_hex(topic)?;
    if raw.len() < 32 {
        return None;
    }
    Some(format!("0x{}", eth::bytes_to_hex(&raw[12..32])))
}

fn parse_u256_from_data(data: &[u8], slot: usize) -> Option<U256> {
    let start = slot * 32;
    let end = start + 32;
    if data.len() < end {
        return None;
    }
    let bytes: [u8; 32] = data[start..end].try_into().ok()?;
    Some(U256::from_be_bytes(bytes))
}

fn parse_deposited(
    log: &eth::RpcLog,
    chain_id: ChainId,
    block_number: u64,
    log_index: i32,
    block_time_unix: Option<i64>,
) -> Option<AccountEventRow> {
    let user = topic_to_address(log.topics.get(1)?)?;
    let data = eth::decode_hex(&log.data)?;
    let amount = parse_u256_from_data(&data, 0)?;

    Some(AccountEventRow {
        chain_id: chain_id.0 as i64,
        tx_hash: log.transaction_hash.clone(),
        log_index,
        event_kind: "deposited".to_string(),
        address_from: None,
        address_to: Some(user),
        amount_wei: amount.to_string(),
        block_number: block_number as i64,
        block_time_unix,
        correlation: None,
    })
}

fn parse_withdrawn(
    log: &eth::RpcLog,
    chain_id: ChainId,
    block_number: u64,
    log_index: i32,
    block_time_unix: Option<i64>,
) -> Option<AccountEventRow> {
    let user = topic_to_address(log.topics.get(1)?)?;
    let data = eth::decode_hex(&log.data)?;
    let amount = parse_u256_from_data(&data, 0)?;

    Some(AccountEventRow {
        chain_id: chain_id.0 as i64,
        tx_hash: log.transaction_hash.clone(),
        log_index,
        event_kind: "withdrawn".to_string(),
        address_from: Some(user),
        address_to: None,
        amount_wei: amount.to_string(),
        block_number: block_number as i64,
        block_time_unix,
        correlation: None,
    })
}

fn parse_hot_path_initiated(
    log: &eth::RpcLog,
    chain_id: ChainId,
    block_number: u64,
    log_index: i32,
    block_time_unix: Option<i64>,
) -> Option<AccountEventRow> {
    if log.topics.len() < 3 {
        return None;
    }
    let sender = topic_to_address(&log.topics[1])?;
    let recipient = topic_to_address(&log.topics[2])?;
    let data = eth::decode_hex(&log.data)?;
    if data.len() < 96 {
        return None;
    }
    let amount = parse_u256_from_data(&data, 0)?;
    let event_hash = format!("0x{}", eth::bytes_to_hex(&data[64..96]));

    Some(AccountEventRow {
        chain_id: chain_id.0 as i64,
        tx_hash: log.transaction_hash.clone(),
        log_index,
        event_kind: "hot_path_initiated".to_string(),
        address_from: Some(sender),
        address_to: Some(recipient),
        amount_wei: amount.to_string(),
        block_number: block_number as i64,
        block_time_unix,
        correlation: Some(event_hash),
    })
}

fn parse_hot_path_released(
    log: &eth::RpcLog,
    chain_id: ChainId,
    block_number: u64,
    log_index: i32,
    block_time_unix: Option<i64>,
) -> Option<AccountEventRow> {
    if log.topics.len() < 3 {
        return None;
    }
    let recipient = topic_to_address(&log.topics[1])?;
    let correlation = log.topics.get(2).cloned();
    let data = eth::decode_hex(&log.data)?;
    let amount = parse_u256_from_data(&data, 0)?;

    Some(AccountEventRow {
        chain_id: chain_id.0 as i64,
        tx_hash: log.transaction_hash.clone(),
        log_index,
        event_kind: "hot_path_released".to_string(),
        address_from: None,
        address_to: Some(recipient),
        amount_wei: amount.to_string(),
        block_number: block_number as i64,
        block_time_unix,
        correlation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_deposited_log(user_topic: &str) -> eth::RpcLog {
        let deposited = format!("{}", keccak256(b"Deposited(address,address,uint256)"));
        eth::RpcLog {
            transaction_hash: "0xabc".to_string(),
            block_number: "0x2a".to_string(),
            log_index: "0x0".to_string(),
            topics: vec![deposited, user_topic.to_string()],
            data: format!(
                "0x{}",
                eth::bytes_to_hex(&U256::from(5000u64).to_be_bytes::<32>())
            ),
        }
    }

    #[test]
    fn parse_deposited_extracts_user_and_amount() {
        let user = "0x0000000000000000000000001111111111111111111111111111111111111111";
        let log = sample_deposited_log(user);
        let row = parse_deposited(&log, ChainId(84532), 42, 0, Some(1_700_000_000)).unwrap();
        assert_eq!(row.event_kind, "deposited");
        assert_eq!(
            row.address_to.as_deref(),
            Some("0x1111111111111111111111111111111111111111")
        );
        assert_eq!(row.amount_wei, "5000");
    }

    #[test]
    fn parse_withdrawn_extracts_user() {
        let withdrawn = format!("{}", keccak256(b"Withdrawn(address,address,uint256)"));
        let user = "0x0000000000000000000000002222222222222222222222222222222222222222";
        let log = eth::RpcLog {
            transaction_hash: "0xdef".to_string(),
            block_number: "0x10".to_string(),
            log_index: "0x1".to_string(),
            topics: vec![withdrawn, user.to_string()],
            data: format!(
                "0x{}",
                eth::bytes_to_hex(&U256::from(1000u64).to_be_bytes::<32>())
            ),
        };
        let row = parse_withdrawn(&log, ChainId(1), 16, 1, None).unwrap();
        assert_eq!(row.event_kind, "withdrawn");
        assert_eq!(
            row.address_from.as_deref(),
            Some("0x2222222222222222222222222222222222222222")
        );
    }

    #[test]
    fn home_chain_push_only_on_first_insert() {
        use crate::domain::repository::UpsertEventResult;

        assert!(should_push_home_chain(UpsertEventResult::Inserted, true));
        assert!(!should_push_home_chain(
            UpsertEventResult::AlreadyExists,
            true
        ));
        assert!(!should_push_home_chain(UpsertEventResult::Inserted, false));
    }
}
