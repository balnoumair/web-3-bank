# Task 10: Treasury Cold Path Rebalancing

**Service:** `treasury-service` (Rust)
**Depends on:** Task 04 (scaffold), Task 03 (deployed contracts with CCIP)
**Can parallelize with:** Task 07, Task 08, Task 09

## Goal

Implement the cold path rebalancing module: monitor pool depths, detect imbalances, and execute batched CCIP burn-and-mint operations.

## Scope

### Pool Depth Monitoring
- Periodically query SyncUSD balances held by each Bank Contract across all chains
- Record snapshots in `pool_snapshots` table
- Compare against target ratios from `RouteReceiver.sol` activation state

### Imbalance Detection
- Define configurable thresholds per chain (minimum pool ratio, target pool ratio)
- Detect when a pool drops below minimum threshold
- Calculate optimal rebalancing amounts to restore target ratios

### CCIP Rebalance Execution
- Build CCIP burn-and-mint transaction: burn SyncUSD on surplus chain, mint on deficit chain
- Batch multiple rebalancing operations for gas efficiency
- Submit transactions and track completion (CCIP message lifecycle)
- Record rebalance operations in database

### Safety Checks
- Never rebalance more than a configurable maximum per operation
- Verify source pool has sufficient surplus before burning
- Read `RouteReceiver.sol` to skip inactive chains

## Acceptance Criteria
- Pool monitoring detects simulated imbalance on Anvil multi-fork
- Rebalancing transaction is constructed and submitted correctly
- Pool snapshots recorded in PostgreSQL
- Safety limits prevent over-rebalancing
