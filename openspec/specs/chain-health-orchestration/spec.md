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

### Requirement: Data sourced via Chainlink DON consensus

Score inputs SHALL be fetched via Chainlink DON consensus: gas prices and block freshness via RPC, TVL via DeFiLlama.

### Requirement: Scheduled and on-demand evaluation

The orchestrator SHALL run on a 5-minute cron and SHALL also expose an HTTP trigger for on-demand evaluation.

### Requirement: Pre-publication simulation

Before publishing on-chain, the orchestrator SHALL simulate the publication transaction via Tenderly.

### Requirement: Activation threshold

A chain SHALL be marked active when its overall score is greater than or equal to the activation threshold (default `0.7`) and inactive otherwise. Activation state SHALL be published to `RouteReceiver.sol`.

#### Scenario: A chain degrades and is deactivated

- **WHEN** a chain's score drops below the activation threshold (e.g. due to outage, stale blocks, or degraded reliability)
- **THEN** the orchestrator SHALL publish updated activation state marking the chain inactive
- **AND** the Treasury Service, on its next read of `RouteReceiver.sol`, SHALL stop routing hot path transfers to that chain

### Requirement: On-chain publication with replay protection

`RouteReceiver.sol` SHALL expose `publishRoute()` and `publishActivationState()`, and SHALL guard against replay via a `_publishedRuns` mapping. Consumers SHALL read the latest route via `getLatestRoute()`.

### Requirement: Decoupling between CRE and Treasury

The CRE Orchestrator SHALL NOT call the Treasury Service directly, and the Treasury Service SHALL NOT call the CRE Orchestrator directly. All coordination SHALL flow through `RouteReceiver.sol`.
