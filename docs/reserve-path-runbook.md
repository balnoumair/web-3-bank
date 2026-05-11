# Reserve Path Rebalance Runbook

The reserve path keeps USDC reserves balanced across Bank Contracts so withdrawals continue to work on every active chain. It is a parallel concern to the SyncUSD pool cold path — different ledger, different role, different transport.

This document covers operational procedures only. Architecture is in the OpenSpec change `add-usdc-reserve-rebalance`.

---

## 1. Pause the reserve path

Two graduated options, depending on intent:

- **Pause Bank entirely** — `Bank.pause()` from the pauser address. While paused, `bridgeReserve` reverts alongside deposits, withdrawals, and every other state-changing entrypoint. Use this when there is a real incident.
- **Stop Treasury from submitting only** — disable the Treasury reserve-path worker (`RESERVE_PATH_POLL_SECS=0` or stop the binary), or revoke the Treasury reserve-ops signer's `RESERVE_REBALANCER_ROLE` on each Bank. This lets user-facing Bank operations continue but freezes outbound reserve bridges.

For destination-side freezing during an incident: revoke the per-chain adapter via `Bank.setReserveBridge(0)` to make `bridgeReserve` fail closed; `completeReserveBridge` will still accept inbound from the previously registered adapter only until reset (the check is `msg.sender == reserveBridge`).

---

## 2. Adjust per-chain limits

```text
Bank.setMaxReserveRebalanceAmount(amount_in_usdc_base_units)
```

Spec default: **5% of total USDC reserves** per chain. A zero cap disables outbound reserve bridges from that source chain. Treasury also respects a local `RESERVE_PATH_MAX_WEI` for planning, but the on-chain cap is authoritative.

```text
Bank.setReserveDestination(destChainId, destBankAddress)
Bank.setAllowlistedDestChain(destChainId, true)
```

Both must be set before `bridgeReserve` will accept a destination. `bridgeReserve` reads `reserveDestinations[destChainId]` and passes the result as `destReserve` to the adapter — this is the address CCTP mints to / the Tempo multisig releases to.

---

## 3. Treasury role grants

The spec requires that the reserve-ops signer hold a **distinct** key from the cold-path signer.

```text
Bank.grantRole(RESERVE_REBALANCER_ROLE, reserveOpsAddress)
```

Treasury config:

```text
RESERVE_RELAYER_KEY_PATH=/path/to/reserve_key.hex   # optional; falls back to RELAYER_KEY_PATH
```

If `RESERVE_RELAYER_KEY_PATH` is unset, Treasury uses the existing cold-path relayer key. This is fine for dev. Production deploys MUST set a separate path.

---

## 4. Per-adapter configuration

### CCTPReserveBridge (CCTP-supported chains)

Run **on each adapter** after deploy:

```text
CCTPReserveBridge.setBank(bankProxy)
CCTPReserveBridge.setChainDomain(chainId, cctpDomain)        # one call per chain pair
CCTPReserveBridge.setRemoteAdapter(remoteDomain, remoteAdapterAddress)
```

CCTP domain reference (subset): Ethereum=0, Avalanche=1, OP=2, Arbitrum=3, Noble=4, Solana=5, Base=6, Polygon=7. Treasury also needs `CCTP_DOMAINS` env var so it can query the right Circle endpoint.

### TempoReserveBridge (chains without CCTP)

```text
TempoReserveBridge.setBank(bankProxy)
TempoReserveBridge.setChainEid(chainId, lzEid)
TempoReserveBridge.setRemoteAdapter(remoteEid, bytes32(remoteAdapterAddress))   # bytes32, LZ-style
TempoReserveBridge.setSigners([signer1, signer2, ...], threshold)
```

After deploy, **fund the adapter with native gas** (ETH on EVM, TIP on Tempo) so it can pay LayerZero fees. Use `TempoReserveBridge.quoteBridgeOut(destChainId, amount, destReserve)` to estimate fee per bridge.

The reserve-ops signer set must be the **same multisig keys** that will sign destination releases. Compromising any threshold-minus-one signer is tolerable; compromising threshold or more is a key-rotation incident — call `addSigner`/`removeSigner`/`setThreshold` to rotate.

---

## 5. Investigate a stuck reserve bridge

1. **Find the row.** `SELECT * FROM treasury.reserve_ops WHERE op_id = $1 OR bridge_message_id = $2;`
2. **Status check:**
   - `pending` → bridgeReserve tx not yet confirmed on source. Check `source_tx_hash`. If absent, Treasury crashed before submission — re-plan after operator review.
   - `submitted` → confirmed on source, but no Circle attestation yet (CCTP) or no LZ delivery yet (Tempo). For CCTP, query `${CIRCLE_ATTESTATION_API_URL}/v1/messages/{srcDomain}/{txHash}` directly. Attestations typically take 10–20 min.
   - `relayed` → bridgeIn (CCTP) or `executeRelease` (Tempo) was submitted on destination; awaiting `ReserveBridgeCompleted` event from dest Bank.
   - `failed` → operator review required. Read `cctp_message_bytes` and `cctp_attestation` columns (CCTP rows only) and decide whether to retry the dispatch manually.
3. **On the destination Bank**, check `processedReserveMessages(messageId)` — if `true`, the bridge completed and Treasury just missed the event. Update the row to `completed` manually.
4. **Spec timeout: 30 minutes for CCTP, 60 minutes for Tempo.** Treasury auto-marks `failed` after `RESERVE_PATH_STUCK_TIMEOUT_SECS`. Failed rows are surfaced in logs at WARN with the chain pair and amount.

### CCTP-specific failure modes

- **Attestation API 404 for too long**: Circle's iris-api may be slow during high-traffic windows; this is benign up to ~30 min. Beyond that, query the burn tx receipt directly to confirm the `MessageSent` event from Circle's MessageTransmitter, and contact Circle support if the message bytes are present but no attestation appears.
- **Wrong CCTP domain**: if `CCTP_DOMAINS` is mis-configured in Treasury, the relayer queries the wrong source domain and gets 404 forever. Verify against `https://developers.circle.com/cctp/supported-domains`.

### Tempo-specific failure modes

- **Adapter ETH balance exhausted**: `bridgeOut` reverts with `InsufficientNativeBalance`. Operator must top up via direct transfer.
- **Signature collection stuck**: destination side has the `PendingRelease` queued (visible via `pendingReleases(messageId)`) but the multisig hasn't reached threshold. This is a coordination issue, not a code issue — escalate to multisig signers.
- **LZ message not delivered**: check the LZ scan (`https://layerzeroscan.com`) by source tx hash. If LZ retried and failed, governance can manually deliver via the LZ executor as documented in LZ's runbook.

---

## 6. Cross-chain operator playbook

A **complete chain pair onboarding** (e.g. add Optimism to the active set with CCTP) requires the following ordered actions:

1. Deploy `CCTPReserveBridge` on the new chain (`DeployCCTPReserveBridge.s.sol`).
2. Deploy/verify Bank on the new chain (existing `DeployBank.s.sol`).
3. On **each existing chain's Bank**: register the new chain in `reserveDestinations` and allowlist it.
4. On **each existing chain's adapter**: register the new chain's CCTP domain and the new adapter's address as a remote.
5. On **the new chain's adapter**: register each existing chain's domain and adapter as remotes; call `setBank`.
6. On **the new chain's Bank**: call `setReserveBridge(adapter)`, `setMaxReserveRebalanceAmount(cap)`, allowlist every other active chain, grant `RESERVE_REBALANCER_ROLE` to Treasury's reserve-ops signer.
7. Add the new chain id to Treasury's `RPC_URLS`, `CONTRACT_ADDRESSES`, `RESERVE_BRIDGE_ADDRESSES`, `CCTP_DOMAINS`.
8. Wait for `RouteReceiver` to publish the new chain as active; verify Treasury picks it up via the `reserve_path: activation set updated` log line.

For a **Tempo chain pair**, replace step 1 with `DeployTempoReserveBridge.s.sol`, step 5 also includes `setSigners` and ETH funding, and the Treasury config additions are `RESERVE_BRIDGE_ADDRESSES` only (CCTP-domain map and Circle URL are not used by Tempo flows).
