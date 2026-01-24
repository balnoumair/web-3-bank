# Event Indexing and Derived Projections

## When to use
Use this skill when building the Rust backend indexer/projections:
- listening to Base RPC events
- updating derived ledger tables
- handling confirmations and reorgs
- rebuilding state from chain

## Goal
Maintain a correct, rebuildable projection of system state from authoritative on-chain events.

## Inputs
- Contracts and events:
  - Vault events (Deposit/Withdrawal/Policy*/Transfer*)
  - USDC Transfer events (as needed, filtered)
- Confirmation target (e.g., N blocks)
- Storage choice for projections (DB)

## Procedure
1) Choose an ingestion strategy
- Subscribe via WS for near-real-time
- Always backstop with polling by block range

2) Store progress
- Persist last processed block (and optionally tx hash/log index checkpoints)

3) Idempotency
- Derive a stable event key: (chainId, blockHash/blockNumber, txHash, logIndex)
- Ensure upserts are idempotent

4) Reorg strategy (v1 practical)
- Process events only after N confirmations
- Keep a small rollback window (last K blocks) if you process optimistically
- On mismatch, rewind and replay

5) Projection rebuild
- From genesis or checkpoint:
  - replay events in order
  - recompute balances, policies, counters
- Do not “trust DB”; DB is a cache of derived state

## Outputs
- Indexer plan (ingest loop + storage schema)
- Rebuild procedure and operational runbook

## Constraints
- Never finalize payment status from submission alone.
- Do not write business rules in the indexer; domain layer should validate transitions.

