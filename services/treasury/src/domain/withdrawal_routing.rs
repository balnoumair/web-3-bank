//! Pure withdrawal-routing policy (no I/O).
//!
//! For each non-decommissioned chain, computes withdrawable amount as
//! `min(balance, reserve_depth)` when the chain is active. Inactive chains
//! with a non-zero balance are reported as temporarily unavailable.

use alloy_primitives::U256;

/// Reason returned when RouteReceiver marks the chain inactive.
pub const REASON_CHAIN_INACTIVE: &str = "chain_inactive";

/// Reason when reserve depth is zero on an otherwise active chain.
pub const REASON_INSUFFICIENT_RESERVE: &str = "insufficient_reserve";

/// Inputs for one chain's withdrawability computation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainWithdrawalInput {
    pub chain_id: u64,
    pub balance_wei: U256,
    pub reserve_depth_wei: U256,
    pub active: bool,
    pub decommissioned: bool,
}

/// One chain's entry in the withdrawal routing response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainWithdrawalEntry {
    pub chain_id: u64,
    pub balance_wei: U256,
    pub withdrawable_wei: U256,
    pub available: bool,
    pub reason: String,
}

/// Compute routing for a single chain. Returns `None` when the chain is
/// decommissioned or the user holds zero balance there.
pub fn compute_chain_withdrawal(input: &ChainWithdrawalInput) -> Option<ChainWithdrawalEntry> {
    if input.decommissioned || input.balance_wei.is_zero() {
        return None;
    }

    if !input.active {
        return Some(ChainWithdrawalEntry {
            chain_id: input.chain_id,
            balance_wei: input.balance_wei,
            withdrawable_wei: input.balance_wei,
            available: false,
            reason: REASON_CHAIN_INACTIVE.to_string(),
        });
    }

    let withdrawable = input.balance_wei.min(input.reserve_depth_wei);
    let available = !withdrawable.is_zero();
    let reason = if available {
        String::new()
    } else {
        REASON_INSUFFICIENT_RESERVE.to_string()
    };

    Some(ChainWithdrawalEntry {
        chain_id: input.chain_id,
        balance_wei: input.balance_wei,
        withdrawable_wei: withdrawable,
        available,
        reason,
    })
}

/// Fold per-chain inputs into routing entries, sorted by chain id.
pub fn compute_withdrawal_routing(inputs: &[ChainWithdrawalInput]) -> Vec<ChainWithdrawalEntry> {
    let mut entries: Vec<ChainWithdrawalEntry> = inputs
        .iter()
        .filter_map(compute_chain_withdrawal)
        .collect();
    entries.sort_by_key(|e| e.chain_id);
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(
        chain_id: u64,
        balance: u64,
        reserve: u64,
        active: bool,
        decommissioned: bool,
    ) -> ChainWithdrawalInput {
        ChainWithdrawalInput {
            chain_id,
            balance_wei: U256::from(balance),
            reserve_depth_wei: U256::from(reserve),
            active,
            decommissioned,
        }
    }

    #[test]
    fn healthy_chain_returns_full_withdrawable_amount() {
        let entry = compute_chain_withdrawal(&input(84532, 2_000_000, 5_000_000, true, false))
            .unwrap();
        assert_eq!(entry.withdrawable_wei, U256::from(2_000_000u64));
        assert!(entry.available);
        assert!(entry.reason.is_empty());
    }

    #[test]
    fn inactive_chain_is_unavailable_with_reason() {
        let entry = compute_chain_withdrawal(&input(42161, 1_000_000, 5_000_000, false, false))
            .unwrap();
        assert_eq!(entry.withdrawable_wei, U256::from(1_000_000u64));
        assert!(!entry.available);
        assert_eq!(entry.reason, REASON_CHAIN_INACTIVE);
    }

    #[test]
    fn reserve_depth_caps_withdrawable_amount() {
        let entry = compute_chain_withdrawal(&input(84532, 2_000_000, 500_000, true, false))
            .unwrap();
        assert_eq!(entry.withdrawable_wei, U256::from(500_000u64));
        assert!(entry.available);
    }

    #[test]
    fn decommissioned_chain_is_excluded() {
        assert!(compute_chain_withdrawal(&input(42161, 1_000_000, 5_000_000, true, true)).is_none());
    }

    #[test]
    fn zero_balance_chain_is_excluded() {
        assert!(compute_chain_withdrawal(&input(84532, 0, 5_000_000, true, false)).is_none());
    }

    #[test]
    fn zero_reserve_on_active_chain_is_unavailable() {
        let entry = compute_chain_withdrawal(&input(84532, 1_000_000, 0, true, false)).unwrap();
        assert_eq!(entry.withdrawable_wei, U256::ZERO);
        assert!(!entry.available);
        assert_eq!(entry.reason, REASON_INSUFFICIENT_RESERVE);
    }
}
