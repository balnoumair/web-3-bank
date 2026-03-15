# User Journey — End-to-End

> How a user experiences web3Bank, and what happens behind the scenes at every step.

## 1. Bob Opens web3Bank for the First Time

**What Bob sees:** A clean landing page with a single "Create Account" button.

**What Bob does:** Taps "Create Account".

**What happens internally:**
```
Bob's Browser                    Tempo Chain
    │                                │
    ├─ navigator.credentials.create()│
    ├─ FaceID prompt appears         │
    ├─ Bob scans face ✓              │
    │                                │
    │  Secure Enclave generates      │
    │  P-256 keypair. Private key    │
    │  never leaves the device.      │
    │                                │
    ├─ Derives Tempo address from    │
    │  public key                    │
    │                                │
    ├─ GraphQL → BFF ──→ User Service│
    │  "Create user: address 0xBob,  │
    │   credentialId: abc123"        │
    │                                │
    │  User Service stores profile   │
    │  in Postgres (users schema)    │
    │                                │
    ├─ BFF returns JWT session token │
    │                                │
    │  NOTE: Nothing on-chain yet.   │
    │  Bob's account will be         │
    │  "activated" on first tx.      │
```

**What Bob sees:** Dashboard. Balance: $0.00. No complexity, no seed phrase backup screen, no "write down these 12 words".

---

## 2. Bob Deposits $5,000

**What Bob sees:** Clicks "Deposit", selects USDC, enters $5,000, confirms.

**What Bob does:** FaceID prompt → scans face.

**What happens internally:**
```
Bob's Browser                    Tempo Chain                    Treasury Service
    │                                │                              │
    ├─ wagmi builds deposit() tx     │                              │
    │  for Bank Contract on Tempo    │                              │
    │                                │                              │
    ├─ EIP-2718 passkey tx payload   │                              │
    │  signed by Secure Enclave      │                              │
    │                                │                              │
    ├─ Broadcasts signed tx ────────►│                              │
    │                                ├─ Bank Contract executes:     │
    │                                │  1. Pulls 5,000 USDC from    │
    │                                │     Bob's wallet             │
    │                                │  2. Escrows USDC in reserve  │
    │                                │  3. Mints 5,000 SyncUSD      │
    │                                │     to Bob's address         │
    │                                │  4. Emits Deposited event    │
    │                                │                              │
    │                                │  Gas fee: ~$0.001            │
    │                                │  (sponsored by bank via      │
    │                                │   Tempo fee sponsorship)     │
    │                                │                              │
    │  Tx confirmed (~0.5s) ◄────────┤                              │
    │                                │                              │
    ├─ wagmi detects balance change  │                              │
    │  via tanstack-query refetch    │                              │
```

**What Bob sees:** Balance updates to **$5,000.00**. A small "Deposit confirmed" toast. The whole thing took ~2 seconds.

**What Bob doesn't know:** His $5,000 is now SyncUSD tokens on the Tempo blockchain. He doesn't see "Tempo", "SyncUSD", "TIP-20", or any of that. Just dollars.

---

## 3. Bob Sends $500 to Alice (Same Chain)

**What Bob sees:** Clicks "Send", types "Alice" (or pastes her address), enters $500, confirms.

**What Bob does:** FaceID prompt → scans face.

**What happens internally:**
```
Bob's Browser                    Tempo Chain
    │                                │
    ├─ wagmi builds transfer() tx    │
    │  on SyncUSD token contract     │
    │  (standard TIP-20 transfer)    │
    │                                │
    ├─ Passkey signs tx ────────────►│
    │                                ├─ SyncUSD.transfer(Alice, 500)
    │                                │  Standard token transfer.
    │                                │  No Bank Contract involved.
    │                                │
    │  Confirmed (~0.5s) ◄───────────┤
```

**What Bob sees:** Balance: $4,500.00. "Sent $500 to Alice" in activity feed.

**What Alice sees:** Balance increases by $500 instantly. She gets a notification.

**Why this is fast:** Both are on Tempo. It's a plain token transfer — the simplest possible operation. No cross-chain anything.

---

## 4. Bob Sends $1,000 to Charlie (Cross-Chain — Hot Path)

Charlie is on Base. Bob doesn't know this. Bob doesn't care.

**What Bob sees:** Clicks "Send", types "Charlie", enters $1,000, confirms.

**What Bob does:** FaceID prompt → scans face.

**What happens internally:**
```
Bob's Browser          Tempo Chain              Treasury Service           Base Chain
    │                      │                         │                        │
    ├─ wagmi builds        │                         │                        │
    │  transferHotPath()   │                         │                        │
    │  tx on Bank Contract │                         │                        │
    │                      │                         │                        │
    ├─ Passkey signs ─────►│                         │                        │
    │                      ├─ Bank Contract:         │                        │
    │                      │  1. Pulls 1,000 SyncUSD │                        │
    │                      │     from Bob             │                        │
    │                      │  2. Locks it in Tempo    │                        │
    │                      │     liquidity pool       │                        │
    │                      │  3. Emits                │                        │
    │                      │     HotPathInitiated     │                        │
    │                      │     event                │                        │
    │                      │                         │                        │
    │  Tx confirmed ◄──────┤                         │                        │
    │  Bob sees -$1,000    │                         │                        │
    │                      │    Event detected ──────►│                        │
    │                      │                         ├─ Validates:            │
    │                      │                         │  1. Reads              │
    │                      │                         │     RouteReceiver.sol  │
    │                      │                         │     → Base is ACTIVE   │
    │                      │                         │     (score: 0.85)      │
    │                      │                         │  2. Checks Base pool   │
    │                      │                         │     depth: $200k       │
    │                      │                         │     → sufficient       │
    │                      │                         │  3. Builds             │
    │                      │                         │     releaseHotPath()   │
    │                      │                         │     tx                 │
    │                      │                         │                        │
    │                      │                         ├─ Submits release ─────►│
    │                      │                         │                        ├─ Bank Contract:
    │                      │                         │                        │  1. Verifies caller
    │                      │                         │                        │     has RELAYER_ROLE
    │                      │                         │                        │  2. Releases 1,000
    │                      │                         │                        │     SyncUSD from Base
    │                      │                         │                        │     pool to Charlie
    │                      │                         │                        │  3. Emits
    │                      │                         │                        │     HotPathReleased
    │                      │                         │                        │
    │                      │                         ├─ Logs relay to         │
    │                      │                         │  Postgres              │
    │                      │                         │                        │
    │                      │         WATCHER MODULE  │                        │
    │                      │                         ├─ Independently reads   │
    │                      │                         │  HotPathReleased on    │
    │                      │                         │  Base                  │
    │                      │                         ├─ Fetches matching      │
    │                      │                         │  HotPathInitiated on   │
    │                      │                         │  Tempo                 │
    │                      │                         ├─ Compares: amount ✓    │
    │                      │                         │  recipient ✓           │
    │                      │                         │  source exists ✓       │
    │                      │                         ├─ Result: VERIFIED      │
    │                      │                         ├─ Logs to Postgres     │
```

**What Bob sees:** Balance: $3,500.00. "Sent $1,000 to Charlie" — done in ~2-3 seconds.

**What Charlie sees:** Balance increases by $1,000. He doesn't know it came from Tempo. He just sees +$1,000.

**What neither of them knows:** No CCIP was used. No 15-minute wait. The Treasury Service moved pool liquidity in the background. The Tempo pool now has +$1,000 surplus, the Base pool has -$1,000 deficit. This imbalance will be corrected later by the cold path.

---

## 5. Hours Later — Background Rebalancing (Cold Path)

Nobody triggers this. Nobody sees this. It just happens.

**What happens internally:**
```
CRE Orchestrator              RouteReceiver.sol          Treasury Service            CCIP Network
    │                              │                          │                          │
    ├─ Cron fires (every 5min)     │                          │                          │
    ├─ Fetches gas prices,         │                          │                          │
    │  block freshness, TVL        │                          │                          │
    │  from all chains via DON     │                          │                          │
    ├─ Scores chains:              │                          │                          │
    │  Tempo: 0.92                 │                          │                          │
    │  Base: 0.85                  │                          │                          │
    │  Arbitrum: 0.78              │                          │                          │
    ├─ Publishes to ──────────────►│                          │                          │
    │  RouteReceiver.sol           │                          │                          │
    │                              │                          │                          │
    │                              │    Treasury reads ◄──────┤                          │
    │                              │    pool depths:          │                          │
    │                              │                          ├─ Tempo pool: $250k       │
    │                              │                          │  (surplus: +$50k)        │
    │                              │                          ├─ Base pool: $150k        │
    │                              │                          │  (deficit: -$50k)        │
    │                              │                          │  Below target threshold! │
    │                              │                          │                          │
    │                              │                          ├─ Triggers rebalance:     │
    │                              │                          │  Burn 50k SyncUSD        │
    │                              │                          │  on Tempo ───────────────►│
    │                              │                          │                          ├─ CCIP burns on Tempo
    │                              │                          │                          ├─ Transmits proof
    │                              │                          │                          │  (~15 minutes)
    │                              │                          │                          ├─ CCIP mints 50k
    │                              │                          │                          │  SyncUSD on Base
    │                              │                          │                          │
    │                              │                          ├─ Confirms rebalance      │
    │                              │                          ├─ Records in Postgres     │
    │                              │                          ├─ Tempo pool: $200k ✓     │
    │                              │                          ├─ Base pool: $200k ✓      │
```

**What users see:** Absolutely nothing. Their balances don't change. The pool liquidity behind the scenes is restored.

---

## 6. Arbitrum Goes Down — Failover

A real scenario: Arbitrum's sequencer goes offline for 3 hours.

**What happens internally:**
```
CRE Orchestrator              RouteReceiver.sol          Treasury Service
    │                              │                          │
    ├─ Cron detects:               │                          │
    │  Arbitrum block age > 120s   │                          │
    │  Reliability score → 0       │                          │
    │  Overall score → 0.12        │                          │
    │  Below activation threshold  │                          │
    │                              │                          │
    ├─ Publishes updated state ───►│                          │
    │  Active: [Tempo, Base]       │                          │
    │  Inactive: [Arbitrum]        │                          │
    │                              │                          │
    │                              │    Treasury reads ◄──────┤
    │                              │                          ├─ Removes Arbitrum from
    │                              │                          │  hot path routing
    │                              │                          ├─ Any hot path to Arbitrum
    │                              │                          │  → REJECTED
    │                              │                          │
    │                              │                          │  If a user on Arbitrum
    │                              │                          │  wants to withdraw:
    │                              │                          ├─ Treasury fulfills from
    │                              │                          │  Tempo or Base pool
    │                              │                          │  instead
```

**What users see:** Nothing. The bank works. Deposits go to Tempo or Base. Withdrawals are served from healthy pools. Nobody even knows Arbitrum is down.

---

## 7. Bob Withdraws $2,000

**What Bob sees:** Clicks "Withdraw", enters $2,000, selects USDC, confirms.

**What Bob does:** FaceID prompt → scans face.

**What happens internally:**
```
Bob's Browser                    Tempo Chain
    │                                │
    ├─ wagmi builds withdraw() tx    │
    │  on Bank Contract              │
    │                                │
    ├─ Passkey signs ───────────────►│
    │                                ├─ Bank Contract:
    │                                │  1. Burns 2,000 SyncUSD
    │                                │     from Bob
    │                                │  2. Releases 2,000 USDC
    │                                │     from reserve pool
    │                                │     to Bob's wallet
    │                                │  3. Emits Withdrawn event
    │                                │
    │  Confirmed (~0.5s) ◄───────────┤
```

**What Bob sees:** Balance: $1,500.00. 2,000 USDC back in his wallet. Done.

---

## The Experience In One Sentence

**Bob creates an account with his face, deposits dollars, sends money to people, and withdraws — exactly like a banking app.** He never sees a chain name, a gas fee, a transaction hash, a token symbol, or a seed phrase. Under the hood, 4 services, 3+ blockchains, CCIP, a scoring engine, and a relayer network are making it all work.

---
*Last updated: March 2026*
