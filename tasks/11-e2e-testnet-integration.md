# Task 11: End-to-End Testnet Integration

**Service:** All services
**Depends on:** Task 03, Task 05, Task 06, Task 07, Task 08, Task 09, Task 10
**Can parallelize with:** None (this is the final integration task)

## Goal

Wire all services together on testnet and validate the complete user journey: registration → deposit → same-chain transfer → cross-chain transfer → background rebalancing.

## Scope

### Deployment
- Deploy SyncUSD + Bank Contract to 2 testnets (e.g., Base Sepolia + Arbitrum Sepolia)
- Deploy or connect to existing `RouteReceiver.sol` from CRE project
- Run Treasury Service pointed at testnet contracts
- Run User Service with PostgreSQL
- Run BFF proxying to both services
- Run frontend pointed at BFF

### Test Scenarios
1. **Registration:** Create a new user via passkey, verify address appears in User Service
2. **Deposit:** Deposit test USDC, verify SyncUSD minted to user's address
3. **Same-chain transfer:** Transfer SyncUSD between two addresses on same chain
4. **Cross-chain transfer (hot path):** Initiate hot path, verify Treasury relays to destination chain
5. **Watcher verification:** Confirm watcher logs all releases as `Verified`
6. **Pool depletion:** Exhaust destination pool, verify hot path rejection
7. **Cold path rebalance:** Trigger rebalance, verify pool depths restore
8. **Pause/failover:** Simulate a mismatch, verify watcher pauses contract

### Documentation
- Testnet deployment guide (step-by-step)
- Environment variable reference
- Known limitations and workarounds

## Acceptance Criteria
- All 8 test scenarios pass on testnet
- No service crashes during the full flow
- Deployment is reproducible from documentation

---

## Progress

### Contract Deployment

| Step | Script | Chain | Status |
|---|---|---|---|
| 1 | DeploySyncUSD | Arbitrum Sepolia | ✅ Done |
| 2 | DeployBank | Arbitrum Sepolia | ✅ Done |
| 3 | AssignRoles | Arbitrum Sepolia | ✅ Done |
| 4 | DeploySyncUSD | Base Sepolia | ⏳ Waiting for testnet ETH |
| 5 | DeployBank | Base Sepolia | ⏳ |
| 6 | AssignRoles | Base Sepolia | ⏳ |
| 7 | DeploySyncUSD | Tempo Moderato | ✅ Done |
| 8 | DeployBank | Tempo Moderato | ✅ Done |
| 9 | AssignRoles | Tempo Moderato | ✅ Done |

Deployed addresses are tracked in `deployments.md` at the repo root.

### Services

| Step | Task | Status |
|---|---|---|
| 10 | Fill treasury `.env` with deployed contract addresses | ⏳ |
| 11 | Complete `docker-compose.yml` (add BFF + treasury) | ⏳ |
| 12 | Run all services locally against testnet | ⏳ |
| 13 | Run frontend pointed at BFF | ⏳ |

### Test Scenarios

| # | Scenario | Status |
|---|---|---|
| 1 | Registration via passkey | ⏳ |
| 2 | Deposit USDC → mint SyncUSD | ⏳ |
| 3 | Same-chain transfer | ⏳ |
| 4 | Cross-chain transfer (hot path) | ⏳ |
| 5 | Watcher verification | ⏳ |
| 6 | Pool depletion | ⏳ |
| 7 | Cold path rebalance | ⏳ |
| 8 | Pause/failover | ⏳ |

## Known Limitations & Decisions

- **SyncUSD on Tempo is ERC-20, not TIP-20:** TIP-20 (Tempo's native token standard with USD gas, payment lanes, etc.) was deferred. The UUPS proxy allows upgrading to TIP-20 later without changing the contract address. The TIP-20 migration will be proposed as an OpenSpec change against `openspec/specs/banking-ledger/`.
- **Tempo deployment requires Tempo Foundry fork:** Standard Foundry does not support Tempo's USD gas system. Install with `foundryup -n tempo` and use `~/.foundry/bin/forge` with the `--tempo.fee-token` flag.
- **Testnet faucets:** Base/Arbitrum Sepolia ETH can be obtained via Alchemy or by bridging from Ethereum Sepolia. Tempo Moderato gas (PathUSD) is obtained via `cast rpc tempo_fundAddress <address> --rpc-url https://rpc.moderato.tempo.xyz`.
