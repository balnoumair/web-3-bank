# Auth — Delta

## MODIFIED Requirements

### Requirement: Passkey-only authentication

The system SHALL authenticate users exclusively via WebAuthn passkeys bound to the `web3Bank` domain. No password or seed-phrase flows SHALL exist. Possession of a passkey SHALL be proven to the BFF cryptographically — the BFF SHALL NOT trust client-side claims of a completed passkey ceremony.

#### Scenario: New user signs up

- **WHEN** a new user clicks "Create Account"
- **THEN** the browser SHALL call `navigator.credentials.create()` against a BFF-issued challenge to generate a P-256 keypair in the device Secure Enclave
- **AND** the public key SHALL be used to derive the user's Tempo address
- **AND** the full WebAuthn registration response SHALL be sent to the BFF
- **AND** the BFF SHALL verify the challenge, origin, and rpId of the registration response and SHALL verify that the submitted Tempo address derives from the submitted public key, rejecting the registration on any mismatch
- **AND** no on-chain transaction SHALL be required at sign-up

#### Scenario: Returning user logs in

- **WHEN** a returning user clicks "Log In"
- **THEN** the BFF SHALL issue a single-use challenge and the browser SHALL call `navigator.credentials.get()` over it
- **AND** the BFF SHALL verify the returned WebAuthn assertion signature against the public key stored for that credential before issuing any session token
- **AND** the BFF SHALL fetch the user's profile using the verified credential's address and direct the user to the dashboard

## ADDED Requirements

### Requirement: Session issuance requires server-verified passkey possession

The BFF SHALL issue a session JWT only after verifying a WebAuthn assertion: the assertion's challenge SHALL match an outstanding BFF-issued challenge, the challenge SHALL be single-use and SHALL expire within 60 seconds, the origin and rpId SHALL match configuration, and the P-256 signature SHALL verify against the stored public key for the presented credential. A bare credential identifier SHALL NOT be sufficient to obtain a session.

#### Scenario: Forged login with a known credentialId

- **WHEN** a caller submits a credentialId without a valid signed assertion (or with a signature that fails verification)
- **THEN** the BFF SHALL reject the request and SHALL NOT issue a JWT

#### Scenario: Replayed assertion

- **WHEN** a caller resubmits an assertion whose challenge was already consumed or has expired
- **THEN** the BFF SHALL reject the request and SHALL NOT issue a JWT

### Requirement: Credential addition requires a fresh assertion

Adding a passkey credential to an account SHALL require, in addition to an authenticated session, a fresh server-verified assertion from a credential already registered to that account. A session token alone SHALL NOT authorize credential addition.

#### Scenario: Stolen JWT cannot attach a rogue credential

- **WHEN** a caller presents a valid session JWT but no fresh assertion from an existing credential of the account
- **THEN** the `addCredential` request SHALL be rejected

### Requirement: Session signing secret must be configured

The BFF SHALL refuse to start when no JWT signing secret is configured, unless an explicit development mode is enabled. A hard-coded fallback secret SHALL NOT be used outside development mode.

#### Scenario: Missing secret in production

- **WHEN** the BFF starts without `JWT_SECRET` and development mode is not explicitly enabled
- **THEN** the process SHALL exit with a configuration error before serving any request
