# Chain Health Orchestration — Delta

## ADDED Requirements

### Requirement: Three chain states distinguished in RouteReceiver

`RouteReceiver.sol` SHALL distinguish three chain states: `active`, `inactive`, and `decommissioned`.

- `active` and `inactive` are set by the CRE Orchestrator based on real-time scoring (existing behavior).
- `decommissioned` is set only by governance via two admin functions: `markDecommissioning(chainId)` (intent / drain in progress) and `finalizeDecommission(chainId)` (terminal).
- Once a chain is `decommissioned`, it SHALL NOT transition back to any other state.

#### Scenario: CRE attempts to mark a decommissioned chain active

- **WHEN** the CRE Orchestrator publishes activation state for a chain that is already `decommissioned`
- **THEN** RouteReceiver SHALL reject the update for that chain
- **AND** the decommissioned status SHALL remain unchanged

### Requirement: CRE excludes decommissioned chains from scoring

The CRE Orchestrator SHALL skip decommissioned chains entirely. It SHALL NOT fetch metrics for them, SHALL NOT include them in published activation state, and SHALL NOT produce scores for them.
