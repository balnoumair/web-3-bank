//! Pure rebalancing algorithm for cold-path operations.
//!
//! This module contains the core rebalancing logic that determines how
//! funds should be moved between chains to restore target pool ratios.
//! It is pure computation with no I/O dependencies.

use alloy_primitives::U256;

/// Greedily match surplus chains to deficit chains, emitting
/// `(source_chain_id, dest_chain_id, amount)` triples.
///
/// Both inputs are sorted by amount descending so the largest imbalances
/// are resolved first. Each triple is capped at `max_per_op` when set.
pub fn compute_rebalance_ops(
    surpluses: &[(u64, U256)],
    deficits: &[(u64, U256)],
    max_per_op: Option<U256>,
) -> Vec<(u64, u64, U256)> {
    let mut ops = Vec::new();

    // Sort descending so we drain the biggest imbalances first.
    let mut sur: Vec<(u64, U256)> = surpluses.to_vec();
    let mut def: Vec<(u64, U256)> = deficits.to_vec();
    sur.sort_by(|a, b| b.1.cmp(&a.1));
    def.sort_by(|a, b| b.1.cmp(&a.1));

    // Mutable remaining amounts.
    let mut sur_rem: Vec<U256> = sur.iter().map(|(_, a)| *a).collect();
    let mut def_rem: Vec<U256> = def.iter().map(|(_, a)| *a).collect();

    let mut si = 0;
    let mut di = 0;

    while si < sur.len() && di < def.len() {
        if sur_rem[si].is_zero() {
            si += 1;
            continue;
        }
        if def_rem[di].is_zero() {
            di += 1;
            continue;
        }

        let mut amount = sur_rem[si].min(def_rem[di]);
        if let Some(cap) = max_per_op {
            amount = amount.min(cap);
        }

        if !amount.is_zero() {
            ops.push((sur[si].0, def[di].0, amount));
        }

        sur_rem[si] = sur_rem[si].saturating_sub(amount);
        def_rem[di] = def_rem[di].saturating_sub(amount);

        // If the cap split the allocation, advance neither pointer — next
        // iteration will emit another op for the same pair.
        if sur_rem[si].is_zero() {
            si += 1;
        }
        if def_rem[di].is_zero() {
            di += 1;
        }
    }

    ops
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebalance_ops_basic() {
        let surpluses = vec![(1u64, U256::from(267u64))];
        let deficits = vec![(3u64, U256::from(233u64)), (2u64, U256::from(34u64))];

        let ops = compute_rebalance_ops(&surpluses, &deficits, None);
        assert_eq!(ops.len(), 2);
        let total_moved: U256 = ops
            .iter()
            .map(|(_, _, a)| *a)
            .fold(U256::ZERO, |acc, a| acc + a);
        assert_eq!(total_moved, U256::from(267u64));
    }

    #[test]
    fn rebalance_ops_capped() {
        let surpluses = vec![(1u64, U256::from(500u64))];
        let deficits = vec![(2u64, U256::from(500u64))];
        let cap = Some(U256::from(200u64));

        let ops = compute_rebalance_ops(&surpluses, &deficits, cap);
        assert_eq!(ops.len(), 3);
        let total_moved: U256 = ops
            .iter()
            .map(|(_, _, a)| *a)
            .fold(U256::ZERO, |acc, a| acc + a);
        assert_eq!(total_moved, U256::from(500u64));
        for (_, _, amount) in &ops {
            assert!(*amount <= U256::from(200u64));
        }
    }

    #[test]
    fn rebalance_ops_no_deficit() {
        let surpluses = vec![(1u64, U256::from(100u64))];
        let deficits: Vec<(u64, U256)> = vec![];
        let ops = compute_rebalance_ops(&surpluses, &deficits, None);
        assert!(ops.is_empty());
    }
}
