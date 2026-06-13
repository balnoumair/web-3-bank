//! Pure balance aggregation logic (no I/O).
//!
//! Treasury fans out live `balanceOf` reads per chain, then folds the
//! results here. Chains that fell back to the indexed estimate mark the
//! response as degraded.

use alloy_primitives::U256;

/// One chain's contribution to the user's total balance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainBalanceContribution {
    pub amount: U256,
    /// True when the live RPC read failed and the indexed estimate was used.
    pub used_fallback: bool,
}

/// Sum per-chain balances and report whether any chain used a fallback value.
pub fn aggregate_balances(contributions: &[ChainBalanceContribution]) -> (U256, bool) {
    let mut total = U256::ZERO;
    let mut degraded = false;
    for c in contributions {
        total = total.saturating_add(c.amount);
        if c.used_fallback {
            degraded = true;
        }
    }
    (total, degraded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_live_reads_without_degraded_flag() {
        let contributions = vec![
            ChainBalanceContribution {
                amount: U256::from(1_000u64),
                used_fallback: false,
            },
            ChainBalanceContribution {
                amount: U256::from(500u64),
                used_fallback: false,
            },
        ];
        let (total, degraded) = aggregate_balances(&contributions);
        assert_eq!(total, U256::from(1_500u64));
        assert!(!degraded);
    }

    #[test]
    fn marks_degraded_when_any_chain_used_fallback() {
        let contributions = vec![
            ChainBalanceContribution {
                amount: U256::from(1_000u64),
                used_fallback: false,
            },
            ChainBalanceContribution {
                amount: U256::from(200u64),
                used_fallback: true,
            },
        ];
        let (_, degraded) = aggregate_balances(&contributions);
        assert!(degraded);
    }

    #[test]
    fn empty_contributions_yield_zero() {
        let (total, degraded) = aggregate_balances(&[]);
        assert_eq!(total, U256::ZERO);
        assert!(!degraded);
    }
}
