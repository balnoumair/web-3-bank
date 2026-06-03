## ADDED Requirements

### Requirement: Reserves modeled as double-entry accounts

The Treasury Service SHALL maintain an internal double-entry ledger of USDC reserves in its own `treasury.*` schema. The ledger SHALL define one **reserve account per chain** (`reserve:<chain_id>`), each mirroring that chain's Bank Contract USDC reserve, and exactly one shared **in-transit account** holding value that has left a source chain but not yet arrived on a destination chain.

Every ledger entry SHALL be recorded as one or more balanced transfers in which total debits equal total credits. Amounts SHALL be stored as integer wei (`NUMERIC(78, 0)`); floating-point representations SHALL NOT be used.

#### Scenario: Ledger defines an account per active chain plus one in-transit account

- **WHEN** the reserve ledger is initialized for an active chain set of {Tempo, Base}
- **THEN** the ledger SHALL contain reserve accounts `reserve:Tempo` and `reserve:Base`
- **AND** the ledger SHALL contain exactly one `in_transit` account
- **AND** every recorded transfer SHALL have equal total debits and credits

### Requirement: Ledger is a mirror, not the source of truth

The reserve ledger SHALL be a secondary, observational mirror of on-chain reserves. The chain SHALL remain authoritative: the value returned by a Bank Contract's `reserveDepth()` is the source of truth for that chain's reserve. The ledger SHALL NOT move, hold, or custody any funds, and SHALL NOT gate, block, authorize, or alter any reserve-bridge operation. No reserve-bridge decision SHALL depend on ledger state.

#### Scenario: Ledger never blocks a bridge operation

- **WHEN** the Treasury reserve planner decides to execute `bridgeReserve(destChainId, amount)`
- **THEN** the decision SHALL be made solely from on-chain `reserveDepth()` and operational state
- **AND** the ledger SHALL NOT be consulted as a precondition for the bridge

#### Scenario: Chain wins when ledger and chain disagree

- **WHEN** the ledger's balance for a chain's reserve account disagrees with that chain's `reserveDepth()`
- **THEN** the on-chain value SHALL be treated as correct
- **AND** the ledger SHALL NOT silently overwrite itself to match

### Requirement: Bridge initiation debits source reserve and credits in-transit

When a reserve-bridge operation transitions to `submitted` (the source-chain `bridgeReserve` is confirmed and a `messageId` is captured), the Treasury Service SHALL record a balanced transfer that debits the source chain's reserve account and credits the in-transit account by the bridged amount. This transfer SHALL be recorded in the same database transaction as the `reserve_ops` status update that triggers it.

#### Scenario: 100,000 USDC bridge from Tempo to Base is initiated

- **WHEN** a reserve op bridging 100,000 USDC from Tempo to Base transitions to `submitted`
- **THEN** the ledger SHALL record a transfer debiting `reserve:Tempo` 100,000 and crediting `in_transit` 100,000
- **AND** `balance(in_transit)` SHALL increase by 100,000
- **AND** the transfer SHALL be committed atomically with the `reserve_ops` row update

### Requirement: Bridge completion debits in-transit and credits destination reserve

When a reserve-bridge operation transitions to `completed` (the destination `ReserveBridgeCompleted` event is observed), the Treasury Service SHALL record a balanced transfer that debits the in-transit account and credits the destination chain's reserve account by the same amount. This transfer SHALL be recorded in the same database transaction as the `reserve_ops` status update that triggers it.

#### Scenario: The Tempo-to-Base bridge completes

- **WHEN** the same reserve op transitions to `completed`
- **THEN** the ledger SHALL record a transfer debiting `in_transit` 100,000 and crediting `reserve:Base` 100,000
- **AND** `balance(in_transit)` attributable to this op SHALL return to zero
- **AND** `balance(reserve:Base)` SHALL increase by 100,000

### Requirement: Failed bridges SHALL NOT leak value in the in-transit account

When a reserve-bridge operation transitions to `failed` after its initiation transfer has been recorded, the Treasury Service SHALL record a compensating balanced transfer so that no value remains stranded in the in-transit account for that operation. If the underlying funds return to the source chain (per the bridge's failure semantics), the compensating transfer SHALL debit the in-transit account and credit the source chain's reserve account.

#### Scenario: A bridge fails after the source debit

- **WHEN** a reserve op that already recorded its initiation transfer transitions to `failed`
- **AND** the bridged funds return to the source chain
- **THEN** the ledger SHALL record a transfer debiting `in_transit` and crediting `reserve:<source>` for the op's amount
- **AND** no residual in-transit balance SHALL remain attributable to that op

### Requirement: Ledger transfers are immutable and idempotent

Ledger transfers SHALL be append-only and immutable once written. Each transfer SHALL be uniquely keyed by its operation id and leg (`initiation`, `completion`, or `reversal`). Re-processing a lifecycle transition (due to retry, loop re-entry, or restart) SHALL NOT produce a duplicate transfer.

#### Scenario: Re-processing an initiation does not double-count

- **WHEN** the reserve path attempts to record the initiation transfer for an op whose `(op_id, initiation)` transfer already exists
- **THEN** no new transfer SHALL be written
- **AND** `balance(in_transit)` SHALL be unchanged by the retry

### Requirement: The books always balance

At every committed state of the ledger, the sum of all reserve account balances plus the in-transit account balance SHALL equal the total reserve value recorded by the ledger. No reserve account balance SHALL be negative.

#### Scenario: Total is conserved across an in-flight bridge

- **WHEN** a bridge of amount A has been initiated but not yet completed
- **THEN** `Σ balance(reserve:<chain>) + balance(in_transit)` SHALL equal the same total as before initiation
- **AND** `balance(in_transit)` SHALL be at least A

### Requirement: Reconciliation against on-chain reserve depth

The Treasury Service SHALL periodically reconcile each chain's ledger reserve-account balance against that chain's on-chain `reserveDepth()`. When the difference exceeds a configured tolerance (accounting for value legitimately in transit and expected completion latency), the Treasury Service SHALL raise an alert via its existing watcher alert mechanism. Reconciliation SHALL NOT auto-correct the ledger to match the chain.

#### Scenario: Divergence beyond tolerance raises an alert

- **WHEN** reconciliation finds `| balance(reserve:Base) − reserveDepth(Base) |` exceeds the configured tolerance and the gap is not explained by an in-flight bridge
- **THEN** the Treasury Service SHALL raise a watcher alert identifying the chain and the discrepancy
- **AND** the ledger balance SHALL be left unchanged for human investigation

#### Scenario: Reconciliation passes within tolerance

- **WHEN** reconciliation finds every chain's ledger reserve balance within tolerance of its `reserveDepth()`
- **THEN** no alert SHALL be raised
