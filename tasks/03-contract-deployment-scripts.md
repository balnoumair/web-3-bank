# Task 03: Contract Deployment & Integration Tests

**Service:** `contracts`
**Depends on:** Task 01, Task 02
**Can parallelize with:** Task 05, Task 06, Task 07, Task 08

## Goal

Create deployment scripts for SyncUSD and Bank Contract to testnets, and write integration tests that simulate the full deposit → hot path → release → rebalance lifecycle.

## Scope

### Deployment Scripts
- Foundry deploy scripts for SyncUSD (UUPS proxy + implementation)
- Foundry deploy scripts for Bank Contract (UUPS proxy + implementation)
- Role assignment script (grant `MINTER_ROLE`, `RELAYER_ROLE`, `PAUSER_ROLE`)
- Deploy to at least 2 testnets (e.g., Base Sepolia + Arbitrum Sepolia)
- Document deployed addresses in a config file

### Integration Tests (Anvil Multi-Fork)
- Full lifecycle: deposit USDC → receive SyncUSD → hot path transfer → release on destination
- Pool depletion scenario: hot path revert when pool is empty
- Pause scenario: watcher pauses contract, all mutations revert
- Cross-reference with `RouteReceiver.sol` (mock or fork from CRE's deployed instance)

### Configuration
- Environment variable template for RPC URLs, deployer keys
- Network config file mapping chain IDs to contract addresses

## Acceptance Criteria
- Contracts deployed to 2 testnets with verified addresses
- Integration test suite passes on Anvil multi-fork
- Deployment is reproducible from scripts
