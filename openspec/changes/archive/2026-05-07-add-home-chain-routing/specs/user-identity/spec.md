# User Identity — Delta

## ADDED Requirements

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
