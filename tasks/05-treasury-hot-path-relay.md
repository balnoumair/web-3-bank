# Task 05: Treasury Hot Path Relay

**Service:** `treasury-service` (Rust)
**Depends on:** Task 04 (scaffold), Task 02 (Bank Contract for event ABI)
**Can parallelize with:** Task 06, Task 07, Task 08

## Goal

Implement the hot path relay module: listen for `HotPathInitiated` events, validate against `RouteReceiver.sol`, check pool depth, and submit `releaseHotPath` transactions.

## Scope

### Event Listener
- Subscribe to `HotPathInitiated` events on all active chains via WebSocket/polling
- Parse event data: sender, recipient, amount, destination chain ID, event hash
- Handle RPC reconnection and missed blocks

### Validation
- Read `RouteReceiver.sol` to confirm destination chain is in the active set
- Query destination Bank Contract for available pool liquidity
- Reject relay if pool depth is insufficient (log the rejection)

### Relay Execution
- Build and sign `releaseHotPath` transaction using the relayer key
- Submit to destination chain RPC
- Wait for confirmation
- Record in `relay_logs` table (source event hash, dest tx hash, status)

### Error Handling
- Retry logic for transient RPC failures (with backoff)
- Nonce management for the relayer account
- Alert on persistent failures

## Acceptance Criteria
- Relay picks up events from Anvil local fork and submits release transactions
- Rejects relay when destination chain is inactive (RouteReceiver check)
- Rejects relay when pool depth is insufficient
- All relay actions logged to PostgreSQL
