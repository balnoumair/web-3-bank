# User Identity — Delta

## ADDED Requirements

### Requirement: Stored public keys are available for assertion verification

The User Service SHALL return the stored P-256 public key as part of credential lookup, so the BFF can verify WebAuthn assertions server-side. Public keys are not secrets; revoked credentials SHALL be excluded from verification lookups.

#### Scenario: BFF verifies a login assertion

- **WHEN** the BFF looks up a credential by credential ID during login
- **THEN** the User Service SHALL include the stored public key for that credential in the response

#### Scenario: Revoked credential cannot authenticate

- **WHEN** the BFF looks up a credential that has been revoked
- **THEN** the User Service SHALL indicate the credential is not usable for authentication
- **AND** the BFF SHALL NOT issue a session for it
