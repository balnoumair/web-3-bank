# Auth Specification

## Purpose

Authenticate users via Tempo Native Passkeys (WebAuthn over EIP-2718). No passwords, no seed phrases. Authentication is fundamentally key generation and transaction signing — every fund-mutating action is an on-chain WebAuthn signature verified natively by Tempo.

## Requirements

### Requirement: Passkey-only authentication

The system SHALL authenticate users exclusively via WebAuthn passkeys bound to the `web3Bank` domain. No password or seed-phrase flows SHALL exist.

#### Scenario: New user signs up

- **WHEN** a new user clicks "Create Account"
- **THEN** the browser SHALL call `navigator.credentials.create()` to generate a P-256 keypair in the device Secure Enclave
- **AND** the public key SHALL be used to derive the user's Tempo address
- **AND** the address, credential ID, and public key SHALL be sent to the BFF for user-profile creation
- **AND** no on-chain transaction SHALL be required at sign-up

#### Scenario: Returning user logs in

- **WHEN** a returning user clicks "Log In"
- **THEN** the browser SHALL call `navigator.credentials.get()` to verify possession of the existing credential
- **AND** the BFF SHALL fetch the user's profile and balance using the verified address
- **AND** the user SHALL be directed to the dashboard

### Requirement: On-chain transaction signing

Every fund-mutating user action SHALL be signed by the device passkey and submitted as a native Tempo EIP-2718 passkey transaction. Tempo SHALL verify the WebAuthn signature at the protocol level.

#### Scenario: User initiates a transfer

- **WHEN** a user confirms a transfer in the UI
- **THEN** the browser SHALL build the EIP-2718 passkey transaction payload
- **AND** SHALL prompt the user for biometric verification (FaceID / TouchID / PIN)
- **AND** SHALL package the WebAuthn signature into the transaction and broadcast it via RPC to Tempo

### Requirement: Session state distinct from signing

The frontend MAY maintain a session (JWT) for visual logged-in state, but ANY mutation of funds SHALL trigger a fresh biometric passkey prompt to sign the on-chain transaction. Session tokens SHALL NOT authorize fund movement.

#### Scenario: Stolen session token cannot move funds

- **WHEN** an attacker presents a valid session JWT but cannot complete a biometric passkey prompt
- **THEN** no fund-mutating transaction SHALL be produced
- **AND** read-only UI state is the most the session token grants

### Requirement: Recovery via platform passkey sync

Recovery SHALL rely on platform passkey sync (Apple iCloud Keychain, Google Password Manager). If a user loses access to their cloud account, the funds bound to that passkey SHALL be unrecoverable through the bank's systems.

#### Scenario: User replaces their phone

- **WHEN** a user signs into a new device with the same platform account (e.g. iCloud)
- **THEN** the synced passkey SHALL be available and the user SHALL be able to log in and transact

#### Scenario: User loses their platform account

- **WHEN** a user permanently loses access to the cloud account holding the passkey
- **THEN** the bank SHALL have no mechanism to recover access to the funds bound to that passkey

### Requirement: Device portability follows passkey availability

Users SHALL only be able to transact on devices that have access to the relevant passkey via platform sync.

#### Scenario: Device without the synced passkey

- **WHEN** a user attempts to transact on a device where the passkey is not available via platform sync
- **THEN** the biometric prompt SHALL fail to produce a valid signature and no transaction SHALL be signed
