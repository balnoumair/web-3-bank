# Banking Ledger — Delta

## ADDED Requirements

### Requirement: Bank Contract supports a freeze-for-decommission state

Bank Contracts SHALL expose an admin-only one-shot `freezeForDecommission()` function. Once called:

- `deposit`, `transferHotPath` (as source), and `releaseHotPath` (as destination) SHALL revert.
- `withdraw` SHALL remain available throughout the governance-defined grace period.
- `rebalance` and `bridgeReserve` SHALL remain available so the Treasury Service can drain pool and reserve.

Freeze SHALL NOT be reversible.

#### Scenario: Frozen Bank Contract rejects a deposit

- **WHEN** the Bank Contract is in the frozen state
- **AND** a user calls `deposit(USDC, 1000)`
- **THEN** the call SHALL revert
- **AND** the same contract SHALL still accept `withdraw` and Treasury-initiated drain operations

### Requirement: Bank Contract supports permanent pause

Bank Contracts SHALL expose an admin-only `pausePermanently()` function, called by governance after drain completes. The contract SHALL use the existing `Pausable` mechanism but SHALL NOT support unpause from the permanent state. After permanent pause, all operations SHALL revert.
