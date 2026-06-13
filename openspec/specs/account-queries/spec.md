# account-queries Specification

## Purpose
TBD - created by archiving change add-account-balance-and-activity. Update Purpose after archive.
## Requirements
### Requirement: Balance is the aggregated on-chain SyncUSD balance

A user's balance SHALL be the sum of on-chain SyncUSD `balanceOf(address)` across all chains that are not decommissioned. The Treasury Service SHALL compute this aggregation; the BFF SHALL only proxy it. No service SHALL derive the displayed balance from relay or audit logs.

#### Scenario: Bob's first deposit is reflected in his balance

- **WHEN** Bob deposits 5,000 USDC on Tempo and the deposit transaction confirms
- **THEN** the next `GetBalance(bob)` response SHALL equal 5,000 SyncUSD (in wei)
- **AND** the value SHALL come from on-chain `balanceOf` reads, not from `treasury.relay_logs`

#### Scenario: Balance spans multiple chains

- **WHEN** Bob holds 1,000 SyncUSD on Tempo and 500 SyncUSD on Base
- **THEN** `GetBalance(bob)` SHALL return 1,500 SyncUSD (in wei)

#### Scenario: One chain's RPC is unreachable

- **WHEN** the Base RPC endpoint fails while Tempo is reachable
- **THEN** Treasury SHALL substitute Base's most recently indexed balance for the live read
- **AND** the response SHALL indicate degraded freshness

#### Scenario: Decommissioned chains are excluded

- **WHEN** a chain is marked decommissioned in RouteReceiver
- **THEN** `GetBalance` SHALL NOT read `balanceOf` on that chain

### Requirement: Treasury indexes account-affecting events per chain

The Treasury Service SHALL maintain a persistent index (`treasury.account_events`) of `Deposited`, `Withdrawn`, SyncUSD `Transfer`, `HotPathInitiated`, and `HotPathReleased` events for every non-decommissioned chain. Ingestion SHALL be idempotent (keyed on chain id, tx hash, and log index) and SHALL resume from a persisted block cursor after restart.

#### Scenario: Indexer restarts mid-stream

- **WHEN** the Treasury Service restarts after indexing up to block N on Tempo
- **THEN** indexing SHALL resume from block N+1 using the persisted cursor
- **AND** re-delivered events SHALL NOT create duplicate index rows

#### Scenario: Home-chain assignment is driven by the shared index

- **WHEN** the index ingests the first `Deposited` event for an address
- **THEN** Treasury SHALL push `SetUserHomeChain` for that address exactly as the home-chain requirement specifies
- **AND** a restart SHALL NOT re-trigger the push for already-indexed deposits

### Requirement: Activity feed covers every user-visible flow in both directions

The account activity feed SHALL include deposits, withdrawals, same-chain SyncUSD transfers, and hot-path transfers, in both directions — entries where the user is sender as well as entries where the user is recipient. Each entry SHALL carry a kind, direction, counterparty address, chain id, amount, status, transaction hash, and timestamp. Internal liquidity movements (pool rebalances, reserve bridges, transfers from/to the Bank Contract itself) SHALL NOT appear in the user feed.

#### Scenario: Outgoing hot-path send appears for the sender

- **WHEN** Bob on Tempo sends 1,000 SyncUSD to Charlie on Base via `transferHotPath`
- **THEN** Bob's activity SHALL contain an outgoing entry of kind `transfer` for 1,000 with Charlie as counterparty
- **AND** Charlie's activity SHALL contain an incoming entry for the same logical transfer

#### Scenario: Same-chain transfer appears for both parties

- **WHEN** Bob sends 500 SyncUSD to Alice on the same chain via a plain token transfer
- **THEN** Bob's activity SHALL show an outgoing 500 entry and Alice's SHALL show an incoming 500 entry

#### Scenario: Deposit and withdrawal appear in the feed

- **WHEN** Bob deposits 5,000 USDC and later withdraws 2,000
- **THEN** Bob's activity SHALL contain a `deposit` entry of 5,000 and a `withdrawal` entry of 2,000

#### Scenario: Cold-path rebalance does not appear

- **WHEN** Treasury rebalances 50,000 SyncUSD pool liquidity from Tempo to Base
- **THEN** no user's activity feed SHALL contain an entry for that movement

### Requirement: Account queries are served by Treasury and proxied by the BFF

The BFF SHALL expose `balance` and account-activity queries to authenticated users by forwarding to the Treasury Service gRPC API. The BFF SHALL NOT compute balances, query chains, or read any database. The User Service SHALL NOT be involved in serving balances or activity.

#### Scenario: Authenticated balance query

- **WHEN** an authenticated user requests `balance` via GraphQL
- **THEN** the BFF SHALL call Treasury's `GetBalance` with the session's address and return the result unmodified

#### Scenario: Unauthenticated query is rejected

- **WHEN** a request without a valid session token queries `balance` or activity
- **THEN** the BFF SHALL reject the request without calling the Treasury Service

