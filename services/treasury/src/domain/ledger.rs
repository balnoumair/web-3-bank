//! Pure domain model for the internal reserve-accounting ledger.
//!
//! A double-entry mirror of USDC reserves: one account per chain plus a shared
//! `in_transit` account for value that has left a source chain but not yet
//! landed on a destination. Every entry is a [`LedgerTransfer`] whose debit and
//! credit are equal by construction, so the books always balance.
//!
//! This module is **pure**: no database, no I/O. The mapping functions take a
//! reserve-bridge lifecycle fact and return the transfer to record, which makes
//! the accounting logic unit-testable in isolation. Persistence lives in
//! `db::reserve_ledger_repo`.
//!
//! The ledger is a SECONDARY mirror, never the source of truth — see the
//! `reserve-accounting` spec.

use alloy_primitives::U256;

use crate::domain::newtypes::{ChainId, OperationId};

/// An account in the reserve ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerAccount {
    /// A chain's USDC reserve, mirroring its Bank Contract `reserveDepth()`.
    Reserve(ChainId),
    /// Value mid-bridge: debited from a source reserve, not yet credited to a
    /// destination reserve. Shared across all in-flight bridges.
    InTransit,
    /// Bootstrap counter-account for opening balances. Lets the ledger start
    /// reconciled with on-chain reserves without an asymmetric (unbalanced) entry.
    Genesis,
}

impl LedgerAccount {
    /// Stable string key used as the account identifier in storage.
    /// e.g. `reserve:8453`, `in_transit`, `genesis`.
    pub fn key(&self) -> String {
        match self {
            LedgerAccount::Reserve(chain) => format!("reserve:{}", chain.0),
            LedgerAccount::InTransit => "in_transit".to_string(),
            LedgerAccount::Genesis => "genesis".to_string(),
        }
    }

    /// Account kind, stored in the account registry.
    pub fn kind(&self) -> &'static str {
        match self {
            LedgerAccount::Reserve(_) => "reserve",
            LedgerAccount::InTransit => "in_transit",
            LedgerAccount::Genesis => "genesis",
        }
    }
}

/// Which leg of an operation a transfer represents. Combined with the op id it
/// forms the idempotency key, so each op contributes at most one transfer per leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferLeg {
    /// Seeds a reserve account from genesis at bootstrap.
    Opening,
    /// Bridge submitted: debit `reserve:<src>` → credit `in_transit`.
    Initiation,
    /// Bridge completed: debit `in_transit` → credit `reserve:<dst>`.
    Completion,
    /// Bridge failed after initiation: debit `in_transit` → credit `reserve:<src>`.
    Reversal,
}

impl TransferLeg {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransferLeg::Opening => "opening",
            TransferLeg::Initiation => "initiation",
            TransferLeg::Completion => "completion",
            TransferLeg::Reversal => "reversal",
        }
    }
}

/// A single balanced double-entry transfer. By construction the debited amount
/// equals the credited amount (one `amount`, applied as `-amount` to `debit` and
/// `+amount` to `credit`), so recording one can never unbalance the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedgerTransfer {
    pub op_id: OperationId,
    pub leg: TransferLeg,
    pub debit: LedgerAccount,
    pub credit: LedgerAccount,
    pub amount: U256,
}

/// Bridge initiation (`reserve_ops` → `submitted`): move `amount` out of the
/// source chain's reserve and into the shared in-transit account.
pub fn initiation_transfer(
    op_id: OperationId,
    source_chain: ChainId,
    amount: U256,
) -> LedgerTransfer {
    LedgerTransfer {
        op_id,
        leg: TransferLeg::Initiation,
        debit: LedgerAccount::Reserve(source_chain),
        credit: LedgerAccount::InTransit,
        amount,
    }
}

/// Bridge completion (`reserve_ops` → `completed`): move `amount` out of
/// in-transit and into the destination chain's reserve.
pub fn completion_transfer(
    op_id: OperationId,
    dest_chain: ChainId,
    amount: U256,
) -> LedgerTransfer {
    LedgerTransfer {
        op_id,
        leg: TransferLeg::Completion,
        debit: LedgerAccount::InTransit,
        credit: LedgerAccount::Reserve(dest_chain),
        amount,
    }
}

/// Bridge failure after initiation (`reserve_ops` → `failed`): return the
/// in-transit value to the source chain's reserve so nothing leaks. Assumes the
/// underlying funds return to source per the bridge's failure semantics.
pub fn reversal_transfer(
    op_id: OperationId,
    source_chain: ChainId,
    amount: U256,
) -> LedgerTransfer {
    LedgerTransfer {
        op_id,
        leg: TransferLeg::Reversal,
        debit: LedgerAccount::InTransit,
        credit: LedgerAccount::Reserve(source_chain),
        amount,
    }
}

/// Opening balance at bootstrap: seed a chain's reserve account from genesis so
/// the ledger starts reconciled with the chain's current `reserveDepth()`.
/// Keyed per chain (op id `opening:<chain>`) so it is recorded at most once.
pub fn opening_transfer(chain: ChainId, amount: U256) -> LedgerTransfer {
    LedgerTransfer {
        op_id: OperationId(format!("opening:{}", chain.0)),
        leg: TransferLeg::Opening,
        debit: LedgerAccount::Genesis,
        credit: LedgerAccount::Reserve(chain),
        amount,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(id: &str) -> OperationId {
        OperationId(id.to_string())
    }

    #[test]
    fn account_keys_are_stable() {
        assert_eq!(LedgerAccount::Reserve(ChainId(8453)).key(), "reserve:8453");
        assert_eq!(LedgerAccount::InTransit.key(), "in_transit");
        assert_eq!(LedgerAccount::Genesis.key(), "genesis");
    }

    #[test]
    fn initiation_debits_source_credits_in_transit() {
        let t = initiation_transfer(op("o1"), ChainId(1), U256::from(100u64));
        assert_eq!(t.leg, TransferLeg::Initiation);
        assert_eq!(t.debit, LedgerAccount::Reserve(ChainId(1)));
        assert_eq!(t.credit, LedgerAccount::InTransit);
        assert_eq!(t.amount, U256::from(100u64));
    }

    #[test]
    fn completion_debits_in_transit_credits_dest() {
        let t = completion_transfer(op("o1"), ChainId(2), U256::from(100u64));
        assert_eq!(t.leg, TransferLeg::Completion);
        assert_eq!(t.debit, LedgerAccount::InTransit);
        assert_eq!(t.credit, LedgerAccount::Reserve(ChainId(2)));
    }

    #[test]
    fn reversal_returns_in_transit_to_source() {
        let t = reversal_transfer(op("o1"), ChainId(1), U256::from(100u64));
        assert_eq!(t.leg, TransferLeg::Reversal);
        assert_eq!(t.debit, LedgerAccount::InTransit);
        assert_eq!(t.credit, LedgerAccount::Reserve(ChainId(1)));
    }

    #[test]
    fn opening_is_keyed_per_chain() {
        let t = opening_transfer(ChainId(42), U256::from(5u64));
        assert_eq!(t.op_id.as_str(), "opening:42");
        assert_eq!(t.debit, LedgerAccount::Genesis);
        assert_eq!(t.credit, LedgerAccount::Reserve(ChainId(42)));
    }

    /// An initiation followed by its completion nets in-transit back to zero and
    /// moves the amount from source to destination — the core invariant.
    #[test]
    fn initiation_then_completion_nets_in_transit_to_zero() {
        let amount = U256::from(100u64);
        let init = initiation_transfer(op("o1"), ChainId(1), amount);
        let comp = completion_transfer(op("o1"), ChainId(2), amount);

        // Net effect on in_transit: +amount (init credit) then -amount (comp debit) = 0.
        assert_eq!(init.credit, LedgerAccount::InTransit);
        assert_eq!(comp.debit, LedgerAccount::InTransit);
        assert_eq!(init.amount, comp.amount);
    }
}
