# User Identity Specification

## Purpose

Manage user profiles and the mapping between WebAuthn credentials and Tempo addresses. The User Service is the sole owner of this data; no other service SHALL read or write the `users` schema directly.

## Requirements

### Requirement: User Service ownership

The User Service SHALL be the sole owner of user-profile and credential data, stored in the `users.*` PostgreSQL schema. All access SHALL go through its gRPC API. No other service SHALL query the `users` schema directly.

#### Scenario: Service needs profile data

- **WHEN** another service needs user-profile or credential data
- **THEN** it SHALL request the data through the User Service gRPC API
- **AND** it SHALL NOT query the `users` schema directly

### Requirement: Profile creation on sign-up

The User Service SHALL accept a create-user request containing the derived Tempo address, WebAuthn credential ID, and public key, and SHALL persist a user profile record.

#### Scenario: BFF forwards a registration

- **WHEN** the BFF receives a registration request from the frontend
- **THEN** it SHALL forward the request to the User Service via gRPC
- **AND** the User Service SHALL store the profile and return the created user

### Requirement: Profile and credential lookup

The User Service SHALL expose gRPC methods to fetch a user profile by Tempo address, including the credential-to-address mapping needed to identify returning users.

#### Scenario: Returning user is identified by Tempo address

- **WHEN** a caller looks up a profile by Tempo address
- **THEN** the User Service SHALL return the matching user profile
- **AND** it SHALL include the credential mapping needed to identify the returning user

### Requirement: No on-chain interaction

The User Service SHALL NOT interact with any blockchain. It is purely an off-chain identity store. Any on-chain data (e.g. balances) SHALL be fetched by the Treasury Service.

#### Scenario: Caller needs on-chain balance data

- **WHEN** a caller needs on-chain data for a user
- **THEN** the User Service SHALL NOT fetch the data from a blockchain
- **AND** the caller SHALL use the Treasury Service for that data

### Requirement: User profile carries a home chain

The User Service SHALL store a `home_chain` value per user representing the chain on which incoming cross-chain transfers are preferentially delivered. `home_chain` SHALL be set automatically on the user's first observed `Deposited` event and SHALL NOT change thereafter except via the chain-decommissioning procedure.

`home_chain` SHALL NOT be exposed to end users and SHALL NOT be settable via any user-facing API.

#### Scenario: Bob deposits on Tempo for the first time

- **WHEN** the User Service observes the first `Deposited` event for Bob's address, on Tempo
- **THEN** `users.profiles.home_chain` for Bob SHALL be set to Tempo's chain id
- **AND** subsequent deposits by Bob on any chain SHALL NOT modify `home_chain`

### Requirement: Home chain is queryable by address

The User Service SHALL expose `GetUserHomeChain(address)` returning the stored `home_chain` or a `not_found` indication. The method SHALL NOT implement fallback policy; callers (e.g., the BFF) are responsible for handling `not_found` and inactive-chain cases.

#### Scenario: Caller queries a known user's home chain

- **WHEN** a caller requests `GetUserHomeChain` for an address with a stored `home_chain`
- **THEN** the User Service SHALL return the stored chain id
- **AND** it SHALL indicate that a home chain was found

#### Scenario: Caller queries an unknown address

- **WHEN** a caller requests `GetUserHomeChain` for an address without a matching user profile
- **THEN** the User Service SHALL return a `not_found` indication
