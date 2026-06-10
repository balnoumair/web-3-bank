# Add Server-Side Passkey Verification for Sessions

## Why

The BFF `authenticate` mutation issues a JWT for **any credentialId presented**, with no proof the caller actually holds the passkey — the schema even documents it: "The frontend must have already verified the passkey challenge before calling this." Client-side verification is not verification: anyone who learns a victim's credentialId (it is not a secret — it is returned by `ListCredentials`, stored in logs, visible to any past device) can mint a session for that user. Funds stay safe (fund movement requires an on-chain passkey signature, per the auth spec), but a forged session can read the victim's balance, full transfer history, and profile, and can hijack their username (`setUsername`) and attach rogue credentials to the account record (`addCredential`). `registerUser` has the same gap: it binds an arbitrary address/publicKey pair with no possession proof. The JWT secret also falls back to a hard-coded dev string when `JWT_SECRET` is unset.

## What Changes

- **Challenge-response login**: the BFF issues a single-use, expiring challenge (nonce); the client signs it via `navigator.credentials.get()`; the BFF verifies the WebAuthn assertion signature against the stored public key **before** issuing a JWT.
- **Registration attestation check**: `registerUser` verifies the WebAuthn registration response (challenge match, origin/rpId match) and verifies the supplied Tempo address is derived from the supplied public key, before creating the profile.
- **`addCredential` requires a fresh assertion** from an *existing* credential of the account, not just a bearer JWT.
- **JWT hardening**: the BFF SHALL refuse to start without a configured `JWT_SECRET` (no dev-secret fallback outside explicit dev mode).
- User Service stores public keys already (`users.credentials`) — it gains a lookup that returns the public key for verification (currently the proto comment says "public_key intentionally omitted: Tempo verifies WebAuthn on-chain", which is true for funds but insufficient for sessions).
- **BREAKING (API)**: `authenticate(credentialId)` becomes a two-step flow (`requestChallenge` → `authenticate(assertion)`); frontend login/registration must send full WebAuthn responses.

## Capabilities

### Modified Capabilities

- `auth`: the "Returning user logs in" and session requirements change — session issuance SHALL require server-verified proof of passkey possession; new requirements for challenge lifecycle and secret management.
- `user-identity`: credential lookup gains "return stored public key for assertion verification"; adding a credential requires possession proof of an existing one.

## Impact

- `services/bff`: challenge store (in-memory with TTL — single-instance BFF; no DB, preserving "no database" rule), WebAuthn assertion verification (P-256 signature over `authenticatorData || sha256(clientDataJSON)`), startup guard for `JWT_SECRET`.
- `packages/proto/user`: extend credential lookup response with `public_key`.
- `services/user-service`: return the stored public key; no schema change (column exists).
- `apps/bank-client`: login/registration flows pass the full WebAuthn assertion/attestation instead of bare IDs.
- No on-chain impact; on-chain signing path is untouched.
