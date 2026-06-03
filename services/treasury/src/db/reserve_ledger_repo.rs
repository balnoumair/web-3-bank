//! PostgreSQL implementation of the internal reserve-accounting ledger.
//!
//! Persists double-entry transfers in `treasury.reserve_ledger_transfers` and
//! derives account balances from them. The ledger is a SECONDARY mirror of
//! on-chain reserves — never the source of truth (see the `reserve-accounting`
//! spec).
//!
//! ## sqlx style
//!
//! Unlike the rest of `db::*`, this module uses sqlx's runtime query functions
//! (`sqlx::query`, `sqlx::query_scalar`) rather than the compile-time-checked
//! `query!` macros. The macros require a live database or a regenerated `.sqlx`
//! offline cache; these queries are new, so runtime queries keep the crate
//! buildable offline. Convert to macros later with `cargo sqlx prepare` against
//! a real database if compile-time checking is wanted here too.

use alloy_primitives::U256;
use async_trait::async_trait;
use sqlx::{PgConnection, PgPool};
use tracing::error;

use crate::domain::ledger::{opening_transfer, LedgerTransfer};
use crate::domain::newtypes::ChainId;
use crate::domain::repository::ReserveLedgerRepository;

/// Record one balanced transfer on an existing connection/transaction.
///
/// This is the shared write path: the reserve adapter calls it inside the same
/// transaction as the `reserve_ops` status update (atomic), and
/// [`PgReserveLedgerRepository`] calls it inside its own transaction for
/// bootstrap. Idempotent on `(op_id, leg)` — re-recording the same leg is a
/// no-op, so retries and restarts never double-count.
pub async fn record_transfer_tx(
    conn: &mut PgConnection,
    t: &LedgerTransfer,
) -> Result<(), sqlx::Error> {
    let debit = t.debit.key();
    let credit = t.credit.key();

    // Register both accounts (idempotent). Keeps the account registry populated
    // without an FK on the hot insert path.
    for (key, kind) in [(&debit, t.debit.kind()), (&credit, t.credit.kind())] {
        sqlx::query(
            "INSERT INTO treasury.reserve_ledger_accounts (account_key, kind) \
             VALUES ($1, $2) ON CONFLICT (account_key) DO NOTHING",
        )
        .bind(key)
        .bind(kind)
        .execute(&mut *conn)
        .await?;
    }

    // The balanced transfer itself. ON CONFLICT (op_id, leg) DO NOTHING gives
    // idempotency.
    sqlx::query(
        "INSERT INTO treasury.reserve_ledger_transfers \
             (op_id, leg, debit_account, credit_account, amount_wei) \
         VALUES ($1, $2, $3, $4, $5::NUMERIC) \
         ON CONFLICT (op_id, leg) DO NOTHING",
    )
    .bind(t.op_id.as_str())
    .bind(t.leg.as_str())
    .bind(&debit)
    .bind(&credit)
    .bind(t.amount.to_string())
    .execute(&mut *conn)
    .await?;

    Ok(())
}

pub struct PgReserveLedgerRepository {
    pool: PgPool,
}

impl PgReserveLedgerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Balance of an arbitrary account key: credits − debits, saturating at zero.
    async fn balance_of(&self, account_key: &str) -> U256 {
        let res: Result<Option<String>, sqlx::Error> = sqlx::query_scalar(
            "SELECT GREATEST( \
                 COALESCE(SUM(amount_wei) FILTER (WHERE credit_account = $1), 0) \
               - COALESCE(SUM(amount_wei) FILTER (WHERE debit_account  = $1), 0), \
               0)::TEXT \
             FROM treasury.reserve_ledger_transfers \
             WHERE debit_account = $1 OR credit_account = $1",
        )
        .bind(account_key)
        .fetch_one(&self.pool)
        .await;

        match res {
            Ok(Some(s)) => s.parse::<U256>().unwrap_or(U256::ZERO),
            Ok(None) => U256::ZERO,
            Err(e) => {
                error!(account = account_key, err = %e, "reserve_ledger: balance query failed");
                U256::ZERO
            }
        }
    }
}

#[async_trait]
impl ReserveLedgerRepository for PgReserveLedgerRepository {
    async fn account_balance(&self, chain: ChainId) -> U256 {
        self.balance_of(&format!("reserve:{}", chain.0)).await
    }

    async fn in_transit_balance(&self) -> U256 {
        self.balance_of("in_transit").await
    }

    async fn seed_opening_balance(&self, chain: ChainId, depth: U256) {
        let transfer = opening_transfer(chain, depth);
        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                error!(chain = chain.0, err = %e, "reserve_ledger: seed begin failed");
                return;
            }
        };
        if let Err(e) = record_transfer_tx(&mut tx, &transfer).await {
            error!(chain = chain.0, err = %e, "reserve_ledger: seed insert failed");
            return;
        }
        if let Err(e) = tx.commit().await {
            error!(chain = chain.0, err = %e, "reserve_ledger: seed commit failed");
        }
    }

    async fn has_opening_balance(&self, chain: ChainId) -> bool {
        let op_id = format!("opening:{}", chain.0);
        let res: Result<bool, sqlx::Error> = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM treasury.reserve_ledger_transfers \
             WHERE op_id = $1 AND leg = 'opening')",
        )
        .bind(&op_id)
        .fetch_one(&self.pool)
        .await;
        res.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ledger::{completion_transfer, initiation_transfer, reversal_transfer};
    use crate::domain::newtypes::OperationId;
    use sqlx::PgPool;

    async fn record(pool: &PgPool, t: &LedgerTransfer) {
        let mut conn = pool.acquire().await.unwrap();
        record_transfer_tx(&mut conn, t).await.unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn initiation_moves_reserve_into_in_transit(pool: PgPool) {
        let repo = PgReserveLedgerRepository::new(pool.clone());
        // Seed Tempo (chain 1) with 1000.
        repo.seed_opening_balance(ChainId(1), U256::from(1000u64))
            .await;
        assert_eq!(repo.account_balance(ChainId(1)).await, U256::from(1000u64));
        assert_eq!(repo.in_transit_balance().await, U256::ZERO);

        // Initiate a 100 bridge from chain 1.
        record(
            &pool,
            &initiation_transfer(OperationId("o1".into()), ChainId(1), U256::from(100u64)),
        )
        .await;
        assert_eq!(repo.account_balance(ChainId(1)).await, U256::from(900u64));
        assert_eq!(repo.in_transit_balance().await, U256::from(100u64));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn completion_nets_in_transit_to_zero(pool: PgPool) {
        let repo = PgReserveLedgerRepository::new(pool.clone());
        repo.seed_opening_balance(ChainId(1), U256::from(1000u64))
            .await;
        let op = OperationId("o1".into());
        record(
            &pool,
            &initiation_transfer(op.clone(), ChainId(1), U256::from(100u64)),
        )
        .await;
        record(
            &pool,
            &completion_transfer(op.clone(), ChainId(2), U256::from(100u64)),
        )
        .await;
        assert_eq!(repo.in_transit_balance().await, U256::ZERO);
        assert_eq!(repo.account_balance(ChainId(2)).await, U256::from(100u64));
        assert_eq!(repo.account_balance(ChainId(1)).await, U256::from(900u64));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn reversal_drains_in_transit_back_to_source(pool: PgPool) {
        let repo = PgReserveLedgerRepository::new(pool.clone());
        repo.seed_opening_balance(ChainId(1), U256::from(1000u64))
            .await;
        let op = OperationId("o1".into());
        record(
            &pool,
            &initiation_transfer(op.clone(), ChainId(1), U256::from(100u64)),
        )
        .await;
        record(
            &pool,
            &reversal_transfer(op.clone(), ChainId(1), U256::from(100u64)),
        )
        .await;
        assert_eq!(repo.in_transit_balance().await, U256::ZERO);
        // Funds returned to source: balance back to the opening 1000.
        assert_eq!(repo.account_balance(ChainId(1)).await, U256::from(1000u64));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn recording_same_leg_twice_is_idempotent(pool: PgPool) {
        let repo = PgReserveLedgerRepository::new(pool.clone());
        repo.seed_opening_balance(ChainId(1), U256::from(1000u64))
            .await;
        let init =
            initiation_transfer(OperationId("o1".into()), ChainId(1), U256::from(100u64));
        record(&pool, &init).await;
        record(&pool, &init).await; // duplicate
        // in_transit reflects a single 100, not 200.
        assert_eq!(repo.in_transit_balance().await, U256::from(100u64));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn seed_opening_balance_is_idempotent(pool: PgPool) {
        let repo = PgReserveLedgerRepository::new(pool.clone());
        assert!(!repo.has_opening_balance(ChainId(1)).await);
        repo.seed_opening_balance(ChainId(1), U256::from(1000u64))
            .await;
        repo.seed_opening_balance(ChainId(1), U256::from(9999u64))
            .await; // ignored
        assert!(repo.has_opening_balance(ChainId(1)).await);
        assert_eq!(repo.account_balance(ChainId(1)).await, U256::from(1000u64));
    }
}
