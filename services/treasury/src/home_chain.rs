//! Observes `Deposited` events on every Bank Contract and pushes
//! `SetUserHomeChain` to user-service (first deposit wins; idempotent).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use alloy_primitives::{keccak256, Address, B256};
use tonic::Request;
use tracing::{info, warn};

use crate::config::Config;
use crate::domain::newtypes::ChainId;
use crate::eth;
use crate::user_pb::{user_service_client::UserServiceClient, SetUserHomeChainRequest};

const DEPOSIT_POLL_INTERVAL: Duration = Duration::from_secs(3);
const MAX_BLOCK_RANGE: u64 = 2_000;

pub struct HomeChainIndexer {
    config: Arc<Config>,
    http: reqwest::Client,
    user_endpoint: String,
    deposited_topic: B256,
}

impl HomeChainIndexer {
    pub fn new(config: Arc<Config>, http: reqwest::Client, user_endpoint: String) -> Arc<Self> {
        let deposited_topic = keccak256(b"Deposited(address,address,uint256)");
        Arc::new(Self {
            config,
            http,
            user_endpoint,
            deposited_topic,
        })
    }

    pub fn spawn_background(self: Arc<Self>) {
        tokio::spawn(async move { self.poll_loop().await });
    }

    async fn poll_loop(self: Arc<Self>) {
        info!(endpoint = %self.user_endpoint, "home_chain: indexer started");
        let mut last_block: HashMap<ChainId, u64> = HashMap::new();

        loop {
            let uri = if self.user_endpoint.starts_with("http://")
                || self.user_endpoint.starts_with("https://")
            {
                self.user_endpoint.clone()
            } else {
                format!("http://{}", self.user_endpoint)
            };

            let mut client = match UserServiceClient::connect(uri).await {
                Ok(c) => c,
                Err(e) => {
                    warn!(err = %e, "home_chain: cannot connect to user-service — retrying");
                    tokio::time::sleep(DEPOSIT_POLL_INTERVAL).await;
                    continue;
                }
            };

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
                let Some(to_block) = eth::fetch_block_number(&self.http, &rpc_url).await else {
                    continue;
                };

                let scan_from = match last_block.get(&chain_id) {
                    Some(&last) => last,
                    None => to_block,
                };
                let scan_from = scan_from.max(to_block.saturating_sub(MAX_BLOCK_RANGE));

                let topic = format!("{}", self.deposited_topic);
                let logs = eth::fetch_logs(
                    &self.http, &rpc_url, &bank_addr, &topic, scan_from, to_block,
                )
                .await;

                for log in logs {
                    if let Some(user) = parse_deposited_user(&log) {
                        let tempo = format!("{:#x}", user);
                        let req = Request::new(SetUserHomeChainRequest {
                            tempo_address: tempo,
                            chain_id: chain_id.0,
                        });
                        if let Err(e) = client.set_user_home_chain(req).await {
                            warn!(
                                err = %e,
                                chain_id = chain_id.0,
                                "home_chain: SetUserHomeChain failed"
                            );
                        }
                    }
                }

                last_block.insert(chain_id, to_block + 1);
            }

            tokio::time::sleep(DEPOSIT_POLL_INTERVAL).await;
        }
    }
}

fn parse_deposited_user(log: &eth::RpcLog) -> Option<Address> {
    if log.topics.len() < 2 {
        return None;
    }
    let user_raw = eth::decode_hex(&log.topics[1])?;
    if user_raw.len() < 32 {
        return None;
    }
    Some(Address::from_slice(&user_raw[12..32]))
}
