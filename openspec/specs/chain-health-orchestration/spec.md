# Chain Health Orchestration Specification

## Purpose

Continuously score the health of every supported chain and publish those scores and an active/inactive set on-chain to `RouteReceiver.sol`. The Treasury Service consumes this state to make routing decisions. There is **no** direct service-to-service communication between the CRE Orchestrator and the Treasury Service — they are decoupled through on-chain state.

## Requirements

### Requirement: Weighted multi-metric scoring

The CRE Route Orchestrator SHALL score each chain on four metrics with the following fixed weights:

| Metric | Weight |
|---|---|
| Fee | 35% |
| Latency | 30% |
| Reliability | 25% |
| Liquidity | 10% |

#### Scenario: Composite score is computed with fixed weights

- **WHEN** an evaluation run produces per-metric scores for a chain
- **THEN** the overall score SHALL be the weighted sum using exactly the weights above
- **AND** no other metric SHALL contribute to the score

### Requirement: Data sourced via Chainlink DON consensus

Score inputs SHALL be fetched via Chainlink DON consensus: gas prices and block freshness via RPC, TVL via DeFiLlama.

#### Scenario: Metric inputs are gathered for a run

- **WHEN** an evaluation run gathers inputs for a chain
- **THEN** gas prices and block freshness SHALL be fetched via DON-consensus RPC reads and TVL via DeFiLlama
- **AND** no single-node, non-consensus value SHALL feed the score

### Requirement: Scheduled and on-demand evaluation

The orchestrator SHALL run on a 5-minute cron and SHALL also expose an HTTP trigger for on-demand evaluation.

#### Scenario: Cron and manual triggers both evaluate

- **WHEN** the 5-minute cron fires, or an operator calls the HTTP trigger
- **THEN** a full evaluation run SHALL execute and (subject to simulation) publish its result

### Requirement: Pre-publication simulation

Before publishing on-chain, the orchestrator SHALL simulate the publication transaction via Tenderly.

#### Scenario: Simulation failure blocks publication

- **WHEN** the Tenderly simulation of the publication transaction fails
- **THEN** the orchestrator SHALL NOT submit the transaction on-chain

### Requirement: Activation threshold

A chain SHALL be marked active when its overall score is greater than or equal to the activation threshold (default `0.7`) and inactive otherwise. Activation state SHALL be published to `RouteReceiver.sol`.

#### Scenario: A chain degrades and is deactivated

- **WHEN** a chain's score drops below the activation threshold (e.g. due to outage, stale blocks, or degraded reliability)
- **THEN** the orchestrator SHALL publish updated activation state marking the chain inactive
- **AND** the Treasury Service, on its next read of `RouteReceiver.sol`, SHALL stop routing hot path transfers to that chain

### Requirement: On-chain publication with replay protection

`RouteReceiver.sol` SHALL expose `publishRoute()` and `publishActivationState()`, and SHALL guard against replay via a `_publishedRuns` mapping. Consumers SHALL read the latest route via `getLatestRoute()`.

#### Scenario: Replayed publication is rejected

- **WHEN** a publication with an already-recorded run id is submitted to `RouteReceiver.sol`
- **THEN** the transaction SHALL revert and the stored route SHALL remain unchanged

### Requirement: Decoupling between CRE and Treasury

The CRE Orchestrator SHALL NOT call the Treasury Service directly, and the Treasury Service SHALL NOT call the CRE Orchestrator directly. All coordination SHALL flow through `RouteReceiver.sol`.

#### Scenario: Treasury learns of a deactivation

- **WHEN** the CRE Orchestrator deactivates a chain
- **THEN** the Treasury Service SHALL learn of it only by reading `RouteReceiver.sol`
- **AND** no direct request SHALL flow between the two services in either direction

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

#### Scenario: Decommissioned chain is omitted from activation output

- **WHEN** CRE is given a chain list that includes a `decommissioned` chain
- **THEN** CRE SHALL omit that chain from scoring
- **AND** CRE SHALL NOT include that chain in either the active or inactive published activation sets
