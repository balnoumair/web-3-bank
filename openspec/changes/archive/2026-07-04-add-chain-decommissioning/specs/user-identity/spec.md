# User Identity — Delta

## ADDED Requirements

### Requirement: Decommission is the only path that mutates home_chain after creation

The User Service SHALL expose an admin-only `SetUserHomeChain(address, chain_id)` gRPC method, restricted to the Treasury Service's decommission orchestrator (authenticated via service identity). Every call SHALL be recorded in an audit log.

This is the only mutation path for `home_chain` after first-deposit creation. Regular user actions, BFF flows, and CRE chain-health changes SHALL NOT modify `home_chain`.

#### Scenario: Decommission orchestrator reassigns home_chain

- **WHEN** the Treasury Service drains a holder during chain decommissioning
- **AND** the holder's `home_chain` was the decommissioned chain
- **THEN** the Treasury Service SHALL call `SetUserHomeChain(holder_address, target_chain_id)`
- **AND** the User Service SHALL update the record and append an audit entry
