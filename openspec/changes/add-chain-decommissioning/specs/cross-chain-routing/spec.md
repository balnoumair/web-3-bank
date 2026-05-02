# Cross-Chain Routing — Delta

## ADDED Requirements

### Requirement: Decommissioned chains are never used as source or destination

The Treasury Service and BFF SHALL treat any chain in `decommissioned` state as terminal. No hot-path transfer, no cold-path rebalance, no reserve bridge SHALL target a decommissioned chain as source or destination. Existing inflight operations targeting a chain that becomes decommissioned mid-flight SHALL be allowed to complete if the destination is the dying chain itself **only** if the operation is part of the drain procedure.

#### Scenario: BFF rejects a transfer to a decommissioned home chain

- **WHEN** a sender initiates a transfer to a recipient whose `home_chain` is decommissioned
- **THEN** the BFF SHALL fall back to same-chain delivery (sender's chain)
- **AND** SHALL NOT update `home_chain` (only the decommission orchestrator may change it)

### Requirement: Treasury executes the chain drain procedure

When a chain is marked for decommissioning, the Treasury Service SHALL execute a drain procedure that, in order:

1. Enumerates SyncUSD holders on the dying chain via its event index.
2. For each holder, bridges the holder's SyncUSD to a governance-chosen target healthy chain via CCIP burn-and-mint, and updates the holder's `home_chain` via the User Service.
3. Drains the SyncUSD pool via `rebalance` to the target chain.
4. Drains the USDC reserve via `bridgeReserve` to the target chain.
5. Records every step in `treasury.decommission_ops` keyed on per-step `messageId` or correlation id.

The drain procedure SHALL be resumable: restart MUST skip already-completed holder bridges based on the audit table.

### Requirement: Drain respects activation gate of the target chain

If the drain target chain becomes `inactive` during a drain, Treasury SHALL pause the drain and alert operators. The drain SHALL NOT proceed against an inactive target.
