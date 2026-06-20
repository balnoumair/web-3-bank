# Chain Decommissioning Runbook

This runbook covers governance-directed retirement of a non-Tempo chain. The procedure is manual, auditable, and one-way.

## Pre-Flight

1. Select one healthy target chain for the drain.
2. Confirm the target chain is active in RouteReceiver and has enough reserve and pool headroom for the incoming balances.
3. Confirm Treasury has the required Bank roles on the source chain: `REBALANCER_ROLE` and `RESERVE_REBALANCER_ROLE`.
4. Confirm User Service has `DECOMMISSION_ORCHESTRATOR_TOKEN` configured and Treasury has the matching service credential.
5. Confirm the Grafana dashboard below can read `treasury.decommission_ops`.

## Step 1: Freeze And Mark Draining

Governance multisig calls, in this order:

1. Source chain Bank: `freezeForDecommission()`.
2. RouteReceiver: `markDecommissioning(chainId)`.

The 7-day grace period begins when both transactions are confirmed. During the grace period, deposits and hot-path operations are blocked on the source chain, but direct `withdraw` remains available for users who can still access the chain.

## Step 2: Run And Monitor Drain

Start the drain with Treasury's admin RPC (token-gated via `DECOMMISSION_ADMIN_TOKEN`):

```bash
grpcurl -plaintext \
  -H "x-decommission-admin-token: ${DECOMMISSION_ADMIN_TOKEN}" \
  -d '{"source_chain":84532,"target_chain":421614}' \
  localhost:50051 treasury.TreasuryService/StartDecommissionDrain
```

Poll status with the returned `drain_id` (format: `source-target`, example `84532-421614`):

```bash
grpcurl -plaintext \
  -H "x-decommission-admin-token: ${DECOMMISSION_ADMIN_TOKEN}" \
  -d '{"drain_id":"84532-421614"}' \
  localhost:50051 treasury.TreasuryService/GetDecommissionDrainStatus
```

The orchestrator:

1. Enumerates SyncUSD holders from Treasury's indexed transfer/deposit history.
2. Cross-checks every holder balance with on-chain `balanceOf`.
3. Bridges each holder balance to the target chain.
4. Calls User Service `SetUserHomeChain` with `decommission_override=true`.
5. Records every holder operation in `treasury.decommission_ops`.
6. Drains the remaining SyncUSD pool with `rebalance`.
7. Drains the USDC reserve with `bridgeReserve`.

If the target chain becomes inactive during the drain, Treasury pauses and alerts operators. Governance decides whether to wait for target recovery or start a separate migration plan.

If Treasury is redeployed or restarted mid-drain, run `StartDecommissionDrain` again with the same source/target pair to resume from `treasury.decommission_ops` progress.

## Drain Progress Dashboard

Create a Grafana table panel backed by:

```sql
SELECT
  chain_id,
  dst_chain_id,
  status,
  COUNT(*) AS ops,
  COALESCE(SUM(amount), 0) AS amount_wei,
  MIN(started_at) AS first_started_at,
  MAX(completed_at) AS last_completed_at
FROM treasury.decommission_ops
GROUP BY chain_id, dst_chain_id, status
ORDER BY chain_id, dst_chain_id, status;
```

Add a single-stat panel for completion:

```sql
SELECT
  COUNT(*) FILTER (WHERE status = 'completed') AS completed_ops,
  COUNT(*) AS total_ops
FROM treasury.decommission_ops
WHERE chain_id = $source_chain_id
  AND dst_chain_id = $target_chain_id;
```

Alert when any row is `failed` or when pending/submitted work has not changed for 30 minutes:

```sql
SELECT *
FROM treasury.decommission_ops
WHERE status IN ('pending', 'submitted', 'failed')
  AND started_at < NOW() - INTERVAL '30 minutes';
```

## Step 3: Finalize

After the 7-day grace period elapses and every holder, pool, and reserve operation is complete:

1. Source chain Bank: `pausePermanently()`.
2. RouteReceiver: `finalizeDecommission(chainId)`.
3. Verify BFF and Treasury report the chain as decommissioned and no longer route to it.
4. Export `treasury.decommission_ops` and User Service `users.home_chain_audit` for governance records.

## Non-Coverage

Tempo cannot be decommissioned through this procedure. Tempo is the anchor chain for multiple account and home-chain flows; retiring it requires a separate system migration plan.
