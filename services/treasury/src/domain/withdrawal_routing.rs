//! Pure withdrawal-routing logic (no I/O).
//!
//! Composes per-chain balance, reserve depth, and activation state into
//! withdrawable amounts for the client.

use alloy_primitives::U256;

/// Inputs gathered per chain before routing is computed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainWithdrawalInput {
    pub chain_id: u64,
    pub balance_wei: U256,
    pub reserve_depth_wei: U256,
    pub active: bool,
    pub decommissioned: bool,
}

/// One chain's entry in the withdrawal-routing response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainWithdrawalEntry {
    pub chain_id: u64,
    pub balance_wei: U256,
    pub withdrawable_wei: U256,
    pub available: bool,
    pub reason: Option<String>,
}

/// Compute per-chain withdrawal routing, excluding decommissioned chains.
pub fn compute_withdrawal_routing(inputs: &[ChainWithdrawalInput]) -> Vec<ChainWithdrawalEntry> {
    let mut entries = Vec::new();

    for input in inputs {
        if input.decommissioned {
            continue;
        }

        if !input.active {
            entries.push(ChainWithdrawalEntry {
                chain_id: input.chain_id,
                balance_wei: input.balance_wei,
                withdrawable_wei: U256::ZERO,
                available: false,
                reason: Some("chain_inactive".to_string()),
            });
            continue;
        }

        if input.balance_wei.is_zero() {
            continue;
        }

        let withdrawable = input.balance_wei.min(input.reserve_depth_wei);
        if withdrawable.is_zero() {
            entries.push(ChainWithdrawalEntry {
                chain_id: input.chain_id,
                balance_wei: input.balance_wei,
                withdrawable_wei: U256::ZERO,
                available: false,
                reason: Some("insufficient_reserve".to_string()),
            });
        } else {
            entries.push(ChainWithdrawalEntry {
                chain_id: input.chain_id,
                balance_wei: input.balance_wei,
                withdrawable_wei: withdrawable,
                available: true,
                reason: None,
            });
        }
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wei(n: u64) -> U256 {
        U256::from(n)
    }

    #[test]
    fn healthy_chain_returns_full_balance() {
        let entries = compute_withdrawal_routing(&[ChainWithdrawalInput {
            chain_id: 1337,
            balance_wei: wei(2_000),
            reserve_depth_wei: wei(10_000),
            active: true,
            decommissioned: false,
        }]);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].withdrawable_wei, wei(2_000));
        assert!(entries[0].available);
        assert!(entries[0].reason.is_none());
    }

    #[test]
    fn inactive_chain_is_unavailable_with_reason() {
        let entries = compute_withdrawal_routing(&[ChainWithdrawalInput {
            chain_id: 42161,
            balance_wei: wei(1_000),
            reserve_depth_wei: wei(10_000),
            active: false,
            decommissioned: false,
        }]);

        assert_eq!(entries.len(), 1);
        assert!(!entries[0].available);
        assert_eq!(entries[0].reason.as_deref(), Some("chain_inactive"));
        assert_eq!(entries[0].withdrawable_wei, U256::ZERO);
    }

    #[test]
    fn reserve_depth_caps_withdrawable_amount() {
        let entries = compute_withdrawal_routing(&[ChainWithdrawalInput {
            chain_id: 1337,
            balance_wei: wei(5_000),
            reserve_depth_wei: wei(2_000),
            active: true,
            decommissioned: false,
        }]);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].withdrawable_wei, wei(2_000));
        assert!(entries[0].available);
    }

    #[test]
    fn decommissioned_chain_is_excluded() {
        let entries = compute_withdrawal_routing(&[ChainWithdrawalInput {
            chain_id: 42161,
            balance_wei: wei(1_000),
            reserve_depth_wei: wei(10_000),
            active: true,
            decommissioned: true,
        }]);

        assert!(entries.is_empty());
    }

    #[test]
    fn zero_balance_active_chain_is_omitted() {
        let entries = compute_withdrawal_routing(&[ChainWithdrawalInput {
            chain_id: 1337,
            balance_wei: U256::ZERO,
            reserve_depth_wei: wei(10_000),
            active: true,
            decommissioned: false,
        }]);

        assert!(entries.is_empty());
    }
}
