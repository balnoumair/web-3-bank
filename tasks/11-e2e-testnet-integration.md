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
