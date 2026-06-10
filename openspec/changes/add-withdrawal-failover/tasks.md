# Tasks — Withdrawal Failover

## 1. Prerequisite

- [ ] 1.1 Confirm `add-account-balance-and-activity` is implemented (per-chain `balanceOf` fan-out is reused here); rebase on it.

## 2. Treasury

- [ ] 2.1 Proto: `GetWithdrawalRouting(address)` returning per-chain entries `{chain_id, withdrawable_wei, available, reason}`; regenerate stubs.
- [ ] 2.2 Implement: compose per-chain `balanceOf`, `reserveDepth`, and RouteReceiver activation/decommission state; pure function for the min/availability logic with unit tests.
- [ ] 2.3 Tests: healthy chain full amount; inactive chain → unavailable with reason; reserve-capped amount; decommissioned chain excluded.

## 3. BFF

- [ ] 3.1 GraphQL query `withdrawalRouting` (requires auth) proxying the Treasury RPC, following the existing send-routing resolver pattern.
- [ ] 3.2 Port + adapter additions in `domain/ports/treasury-service.ts` and the gRPC adapter.

## 4. Frontend

- [ ] 4.1 `use-withdraw`: consult `withdrawalRouting` before building the transaction; target the returned chain; surface "temporarily unavailable" state per design.
- [ ] 4.2 Display decision from design.md open question (single figure with badge vs. separate line) — confirm with the architect before implementing.

## 5. Docs

- [ ] 5.1 Rewrite `docs/user-journey.md` §6 withdrawal text to the decided semantics (route, don't custodially fulfill; honest unavailability).
- [ ] 5.2 E2E: two-chain setup, deactivate one chain in RouteReceiver, verify routing response and client behavior for a user with balance split across both.
