# Task 06: Treasury Watcher Module

**Service:** `treasury-service` (Rust)
**Depends on:** Task 04 (scaffold), Task 02 (Bank Contract for event ABI)
**Can parallelize with:** Task 05, Task 07, Task 08

## Goal

Implement the watcher module that independently verifies every hot path release against source chain events and can pause contracts on mismatch.

## Scope

### Event Monitoring
- Listen for `HotPathReleased` events on all destination chains
- For each release, fetch the corresponding `HotPathInitiated` event from the source chain using `sourceEventHash`

### Verification Logic
- Compare: amount, recipient address, source event existence
- Classify result: `Verified`, `Mismatch`, `SourceNotFound`
- Record all verification results in `watcher_alerts` table

### Pause Trigger
- On `Mismatch` or `SourceNotFound`: call `pause()` on the affected Bank Contract using `PAUSER_ROLE`
- Log the pause action with full context (which release, what mismatch, tx hash of pause)

### Independence
- Watcher must operate independently from the relayer — it should catch issues even if the relayer is compromised
- Reads from different RPC endpoints than the relayer where possible

## Acceptance Criteria
- Watcher verifies legitimate releases as `Verified`
- Watcher detects a simulated mismatch and triggers contract pause
- All verifications logged to PostgreSQL
- Watcher continues operating when relayer is stopped
