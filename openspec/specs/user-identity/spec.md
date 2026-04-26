# User Identity Specification

## Purpose

Manage user profiles and the mapping between WebAuthn credentials and Tempo addresses. The User Service is the sole owner of this data; no other service SHALL read or write the `users` schema directly.

## Requirements

### Requirement: User Service ownership

The User Service SHALL be the sole owner of user-profile and credential data, stored in the `users.*` PostgreSQL schema. All access SHALL go through its gRPC API. No other service SHALL query the `users` schema directly.

### Requirement: Profile creation on sign-up

The User Service SHALL accept a create-user request containing the derived Tempo address, WebAuthn credential ID, and public key, and SHALL persist a user profile record.

#### Scenario: BFF forwards a registration

- **WHEN** the BFF receives a registration request from the frontend
- **THEN** it SHALL forward the request to the User Service via gRPC
- **AND** the User Service SHALL store the profile and return the created user

### Requirement: Profile and credential lookup

The User Service SHALL expose gRPC methods to fetch a user profile by Tempo address, including the credential-to-address mapping needed to identify returning users.

### Requirement: No on-chain interaction

The User Service SHALL NOT interact with any blockchain. It is purely an off-chain identity store. Any on-chain data (e.g. balances) SHALL be fetched by the Treasury Service.
