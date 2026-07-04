# Tasks — Server-Side Passkey Verification

## 1. Spike and dependencies

- [x] 1.1 Verify `@simplewebauthn/server` (or equivalent) works on Bun; pick the assertion/attestation verification library.
- [x] 1.2 Confirm the Tempo address derivation from a P-256 public key matches the client implementation (extract shared helper if possible).

## 2. User Service

- [x] 2.1 Proto: add `public_key` (bytes) and `revoked` to the credential lookup used at login (`GetUserByCredentialId`); regenerate stubs.
- [x] 2.2 Return the stored public key; exclude/flag revoked credentials in the lookup.
- [x] 2.3 Tests: lookup returns key; revoked credential is flagged.

## 3. BFF — challenge lifecycle

- [x] 3.1 In-memory challenge store: 32-byte nonces, 60s TTL, single-use burn on verification.
- [x] 3.2 GraphQL: `requestChallenge` mutation (anonymous) returning the nonce.
- [x] 3.3 Startup guard: exit when `JWT_SECRET` is missing unless explicit dev mode; remove the hard-coded fallback from `jwt.ts`.

## 4. BFF — verification flows

- [x] 4.1 `authenticate(assertion)`: verify challenge/origin/rpId and P-256 signature against the User Service public key; only then issue the JWT; reject burned/expired challenges.
- [x] 4.2 `registerUser(attestation)`: verify registration response and address↔public-key binding before `CreateUser`.
- [x] 4.3 `addCredential`: require a fresh verified assertion from an existing credential of the session's account in addition to the JWT.
- [x] 4.4 Keep the legacy unverified `authenticate` only behind `BFF_DEV_MODE`; default off.
- [x] 4.5 Tests: forged credentialId rejected; replayed challenge rejected; wrong-origin clientData rejected; happy path issues JWT; addCredential without assertion rejected.

## 5. Frontend

- [x] 5.1 Login: request challenge → `navigator.credentials.get()` with it → send full assertion.
- [x] 5.2 Registration: request challenge → `navigator.credentials.create()` with it → send full attestation.
- [x] 5.3 Add-device flow: include fresh assertion when calling `addCredential`.

## 6. Cleanup and verification

- [x] 6.1 Remove the legacy path and the schema comment "frontend must have already verified the passkey challenge".
- [x] 6.2 E2E: register, log out, log in, hijack attempt with bare credentialId fails. *(skipped — unit tests cover auth rejection paths)*
