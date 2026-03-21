//! Domain logic for relay eligibility decisions.
//!
//! Pure business rules: no I/O, no async, no infrastructure dependencies.
//! The caller is responsible for pre-fetching chain state (active set,
//! pool depth) and passing it in as plain values.

use alloy_primitives::U256;

use crate::domain::status::RelayStatus;

/// The outcome of evaluating whether a relay should be executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelayDecision {
    /// Relay should proceed.
    Approved,
    /// Destination chain is not currently in the active set.
    RejectedInactiveChain,
    /// Pool depth on the destination chain is insufficient for the requested amount.
    RejectedInsufficientDepth,
}

impl RelayDecision {
    /// Map a rejection to the corresponding [`RelayStatus`] for persistence.
    ///
    /// Returns `None` for `Approved` — callers should not log an approved relay
    /// as a rejection.
    pub fn as_relay_status(&self) -> Option<RelayStatus> {
        match self {
            Self::Approved => None,
            Self::RejectedInactiveChain => Some(RelayStatus::RejectedInactiveChain),
            Self::RejectedInsufficientDepth => Some(RelayStatus::RejectedInsufficientDepth),
        }
    }
}

/// Evaluate whether a relay should proceed given the current chain state.
///
/// - `chain_active`: whether the destination chain is in the active set.
/// - `pool_depth`: the current depth of the destination pool (pre-fetched by caller).
/// - `required_amount`: the amount the destination pool must cover.
///
/// Fetch failures (e.g., RPC errors) are an infrastructure concern; the caller
/// should return early before invoking this function when the depth cannot be
/// determined.
pub fn evaluate_relay_eligibility(
    chain_active: bool,
    pool_depth: U256,
    required_amount: U256,
) -> RelayDecision {
    if !chain_active {
        return RelayDecision::RejectedInactiveChain;
    }
    if pool_depth >= required_amount {
        RelayDecision::Approved
    } else {
        RelayDecision::RejectedInsufficientDepth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inactive_chain_rejected() {
        let decision =
            evaluate_relay_eligibility(false, U256::from(1000u64), U256::from(100u64));
        assert_eq!(decision, RelayDecision::RejectedInactiveChain);
    }

    #[test]
    fn insufficient_depth_rejected() {
        let decision =
            evaluate_relay_eligibility(true, U256::from(50u64), U256::from(100u64));
        assert_eq!(decision, RelayDecision::RejectedInsufficientDepth);
    }

    #[test]
    fn exact_depth_approved() {
        let decision =
            evaluate_relay_eligibility(true, U256::from(100u64), U256::from(100u64));
        assert_eq!(decision, RelayDecision::Approved);
    }

    #[test]
    fn surplus_depth_approved() {
        let decision =
            evaluate_relay_eligibility(true, U256::from(500u64), U256::from(100u64));
        assert_eq!(decision, RelayDecision::Approved);
    }

    #[test]
    fn inactive_chain_overrides_sufficient_depth() {
        // Even if pool depth is fine, inactive chain must be rejected.
        let decision =
            evaluate_relay_eligibility(false, U256::from(9999u64), U256::from(1u64));
        assert_eq!(decision, RelayDecision::RejectedInactiveChain);
    }

    #[test]
    fn as_relay_status_approved_is_none() {
        assert_eq!(RelayDecision::Approved.as_relay_status(), None);
    }

    #[test]
    fn as_relay_status_rejections_map_correctly() {
        assert_eq!(
            RelayDecision::RejectedInactiveChain.as_relay_status(),
            Some(RelayStatus::RejectedInactiveChain)
        );
        assert_eq!(
            RelayDecision::RejectedInsufficientDepth.as_relay_status(),
            Some(RelayStatus::RejectedInsufficientDepth)
        );
    }
}
