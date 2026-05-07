# Design — USDC Reserve Rebalance

## Goals

- Withdrawals on every active chain continue to work in steady state, regardless of the deposit/withdraw geographic distribution.
- USDC reserves drift no more than a configurable target band before being corrected.
- The mechanism is bridge-agnostic: CCTP on supported chains, custom adapters elsewhere.

## Non-goals

- Real-time reserve rebalance. CCTP and most bridges have minute-to-hours settlement; reserves are managed proactively before depletion, not synchronously on withdraw.
- Reactive withdraw failover ("user wants to withdraw on Base but Base is empty → pay from Arbitrum"). Different problem; addressed separately if needed.
- Defining the Tempo-specific bridge. Out of scope here; Tempo's adapter is a separate engineering task, written against the interface defined in this proposal.

## Why a separate role and adapter from the SyncUSD cold path

| Concern | SyncUSD pool (cold path) | USDC reserve (this change) |
|---|---|---|
| Asset moved | SyncUSD (our token) | USDC (Circle's token) |
| Mechanism | CCIP burn/mint via `BurnMintTokenPool` | Per-chain bridge (CCTP / custom) |
| Settlement time | ~15 min (CCIP) | ~15 min (CCTP); other bridges vary |
| Failure mode | Stuck CCIP message | Stuck bridge message; possibly different recovery |
| Authority | `REBALANCER_ROLE` | `RESERVE_REBALANCER_ROLE` |

Two different assets, two different bridges, two different operational concerns. A single role compromised would otherwise drain both ledgers. Separation is defense-in-depth.

## Key design decisions

### 1. Pluggable bridge adapter interface

The Bank Contract holds an `IReserveBridge` reference, settable by governance per chain. Adapter responsibilities:

- `bridgeOut(uint64 destChainId, uint256 amount, address destReserve) → bytes32 messageId` — burns/locks USDC from the calling Bank Contract and initiates the cross-chain transfer.
- `bridgeIn(...)` — handler for inbound bridge messages, mints/releases USDC into the calling Bank Contract.

Why pluggable: Tempo is a custom chain unlikely to be on Circle's CCTP list. Hard-coding CCTP would block reserve rebalance to/from Tempo entirely. The adapter pattern lets each chain plug in whatever native bridge it supports.

Trade-off: governance must trust each adapter implementation. The adapter is a privileged contract that can move the reserve. Mitigations: per-call cap (same as cold path), `Pausable`, audited adapter implementations.

### 2. Per-rebalance cap on-chain

Same rationale as cold path: bound blast radius if `RESERVE_REBALANCER_ROLE` or the adapter is compromised. `maxReserveRebalanceAmount` is configurable via governance, suggested initial 5% of total system USDC reserves.

### 3. Triggering policy in Treasury

Bank Contract exposes `reserveDepth()` view (mirrors `poolDepth()`). Treasury monitors per-chain reserve depths, computes target deviation, and triggers bridges when below threshold. Policy lives in Treasury; contract just exposes the function.

### 4. Audit on `treasury.reserve_ops` keyed by bridge `messageId`

Each bridge produces a `messageId` (CCTP attestation id, custom adapter id, etc.). Treasury logs every attempt regardless of outcome, keyed on `messageId`. If the bridge call reverts before producing an id, a Treasury-side correlation id is used instead.

### 5. Activation gate

Reserve rebalance respects `RouteReceiver` chain activation. Cannot bridge to or from an inactive chain. Decommissioned chains (separate change) drain via this mechanism but in a one-shot, governance-triggered mode.

### 6. Reserve token and destination registry

`bridgeReserve(uint64 destChainId, uint256 amount)` intentionally does not accept a token address. The Bank Contract therefore stores a canonical `reserveToken` for the USDC reserve; governance may update it only to an already-allowed token. The first allowed token initializes `reserveToken` for backwards compatibility with existing deployment flow.

Outbound reserve bridging also needs the destination Bank/reserve address. The Bank Contract stores a governance-managed `reserveDestinations[destChainId]` registry and requires the destination chain to be allowlisted before bridging. Treasury continues to use `RouteReceiver` activation state when planning operations; the on-chain allowlist/registry is the local execution gate.

## Decisions

- **Tempo bridge: custom adapter using LayerZero or Wormhole, gated by a multisig.** No public CCTP support on Tempo. The custom adapter implements `IReserveBridge` against the chosen messaging protocol; a multisig signs each release on the destination side as the trust anchor. Concrete protocol choice (LayerZero vs. Wormhole) and adapter implementation are tracked as a sub-task — they require their own design pass, but the path is decided.
- **`maxReserveRebalanceAmount`: 5% of total USDC reserves**, set per chain by governance multisig. Mirrors the cold-path cap for consistency.
- **Reserve target threshold: equal share with 80% floor.** Each active chain's target USDC reserve is `total_reserve / num_active_chains`. Treasury triggers a bridge *from* a chain whose reserve exceeds 100% of target *to* any chain whose reserve falls below 80% of target. Mirrors the cold-path policy.
- **Adapter governance: multisig.** Adapter address per Bank Contract is set only via governance multisig, not a single admin. Privilege level demands it — a malicious adapter can drain the reserve.
- **Stuck-message handling: manual review after timeout.** Treasury times out an in-flight bridge after a configurable window (initial: 30 minutes for CCTP, 60 minutes for the Tempo custom bridge given multisig latency), marks `failed`, alerts operators. Auto-retry not in scope.

## Open questions

- **LayerZero vs. Wormhole for the Tempo adapter.** Both support arbitrary messaging; trade-offs are around fees, multisig posture, and existing project relationships. To be decided during the Tempo adapter sub-task before that contract is built.
