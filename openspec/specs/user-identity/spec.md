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

### Requirement: Username handle for human-friendly addressing

A user MAY set a username handle on their profile. Usernames SHALL be 3–20 characters, SHALL start with a letter, and SHALL contain only alphanumeric characters or underscores. Uniqueness SHALL be case-insensitive; the original casing SHALL be preserved for display. Usernames are optional — a profile without one is valid. Only the authenticated owner of a profile SHALL be able to set or change its username (the User Service SHALL receive the user id from the BFF session, never from the client).

#### Scenario: User sets a valid username

- **WHEN** an authenticated user sets the username `Bob_42`
- **THEN** the User Service SHALL store it on the user's profile
- **AND** a later attempt by any user to claim `bob_42` SHALL be rejected as taken

#### Scenario: Invalid username is rejected

- **WHEN** a user submits a username that is too short, too long, starts with a non-letter, or contains other characters
- **THEN** the User Service SHALL reject it without modifying the profile

### Requirement: Username resolves to a profile for sending

The User Service SHALL expose lookup of a user profile by username (case-insensitive). This lookup powers the send flow: the BFF resolves the recipient's username to a Tempo address (and then resolves routing separately, per cross-chain-routing).

#### Scenario: Sender types a recipient's username

- **WHEN** the BFF looks up `alice` and a profile with username `Alice` exists
- **THEN** the User Service SHALL return that profile including its Tempo address

#### Scenario: Unknown username

- **WHEN** the BFF looks up a username with no matching profile
- **THEN** the User Service SHALL return a not-found error

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

### Requirement: Decommission is the only path that mutates home_chain after creation

The User Service SHALL expose an admin-only `SetUserHomeChain(address, chain_id)` gRPC method, restricted to the Treasury Service's decommission orchestrator (authenticated via service identity). Every call SHALL be recorded in an audit log.

This is the only mutation path for `home_chain` after first-deposit creation. Regular user actions, BFF flows, and CRE chain-health changes SHALL NOT modify `home_chain`.

#### Scenario: Decommission orchestrator reassigns home_chain

- **WHEN** the Treasury Service drains a holder during chain decommissioning
- **AND** the holder's `home_chain` was the decommissioned chain
- **THEN** the Treasury Service SHALL call `SetUserHomeChain(holder_address, target_chain_id)`
- **AND** the User Service SHALL update the record and append an audit entry

### Requirement: Stored public keys are available for assertion verification

The User Service SHALL return the stored P-256 public key as part of credential lookup, so the BFF can verify WebAuthn assertions server-side. Public keys are not secrets; revoked credentials SHALL be excluded from verification lookups.

#### Scenario: BFF verifies a login assertion

- **WHEN** the BFF looks up a credential by credential ID during login
- **THEN** the User Service SHALL include the stored public key for that credential in the response

#### Scenario: Revoked credential cannot authenticate

- **WHEN** the BFF looks up a credential that has been revoked
- **THEN** the User Service SHALL indicate the credential is not usable for authentication
- **AND** the BFF SHALL NOT issue a session for it
