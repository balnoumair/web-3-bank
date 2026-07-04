# Design — Server-Side Passkey Verification

## Context

Sessions are pure trust today: `authenticate(credentialId)` → JWT. The browser does call `navigator.credentials.get()`, but the BFF never sees or checks the assertion, so the biometric ceremony protects nothing server-side. The auth spec already states the right invariant ("Session tokens SHALL NOT authorize fund movement") — this change makes the *session itself* trustworthy without weakening that invariant.

Constraints: the BFF must stay DB-less and thin; the User Service owns credentials and already stores `public_key` bytes; Tempo continues to verify WebAuthn for on-chain transactions natively (unchanged).

## Goals / Non-Goals

**Goals:**
- A JWT is only ever issued to a caller who just proved possession of a registered passkey.
- Registration binds address ⇄ public key verifiably.
- Credential addition cannot be done with a stolen JWT alone.
- BFF fails fast on missing `JWT_SECRET`.

**Non-Goals:**
- Account recovery flows (separate open item in README).
- Replacing Tempo's on-chain WebAuthn verification — untouched.
- Multi-instance BFF session/challenge replication (single instance today; noted as a future constraint).
- Full FIDO2 attestation chain validation (testnet scope: "none" attestation accepted; we verify challenge, origin, rpId, and the address↔key binding).

## Decisions

### 1. Two-step login: `requestChallenge` → `authenticate(assertion)`
BFF generates a 32-byte random nonce, stores it in an in-memory TTL map (60s, single-use), returns it. The client passes it as the WebAuthn challenge; `authenticate` receives `{credentialId, authenticatorData, clientDataJSON, signature}`. BFF checks: challenge matches an outstanding nonce (then burns it), `clientDataJSON.origin`/`rpId` match config, and the P-256 signature verifies against the public key fetched from the User Service.

*Alternative:* signing a server timestamp instead of a stored nonce (stateless). Rejected — replayable within the window; a burned nonce is strictly stronger and the in-memory map keeps the BFF DB-less.

### 2. Verification lives in the BFF, key comes from the User Service
The User Service stays off-chain CRUD; cryptographic verification is session logic, which is the BFF's only real job. The credential lookup response gains `public_key` (raw P-256 SEC1 bytes already stored at registration).

*Alternative:* verify inside the User Service (`VerifyAssertion` RPC). Rejected — it would push session semantics into the identity store and require shipping challenges across services.

### 3. Address binding at registration
`registerUser` recomputes the Tempo address from the submitted public key (same derivation the client uses) and rejects mismatches. This prevents registering someone else's address with your passkey, which today would poison credential→address lookups.

### 4. `addCredential` requires an assertion from an existing credential
Same challenge flow as login; the new credential's attestation is accepted only inside a verified session *plus* a fresh assertion. JWT alone is no longer sufficient for any credential mutation.

### 5. Startup guard for secrets
`JWT_SECRET` unset → process exits with a clear error unless `NODE_ENV=development`/`BFF_DEV_MODE=1`. Mirrors the treasury's startup checks pattern.

## Risks / Trade-offs

- [In-memory challenge store dies on restart] → users mid-login just retry; acceptable. Multi-instance deployment would need sticky routing or a shared store — flagged as a constraint, not solved here.
- [WebAuthn parsing bugs in hand-rolled verification] → use a maintained library (e.g. `@simplewebauthn/server` on Bun) rather than hand-rolling CBOR/COSE parsing; hand-roll only the address-derivation check.
- [Frontend breakage: login/registration payloads change] → frontend is not feature-complete yet (per project state); coordinate in the same PR series.
- [Clock skew / TTL too tight] → 60s challenge TTL is generous for a biometric prompt.

## Migration Plan

1. Add `public_key` to the User Service credential lookup (additive proto change).
2. Implement challenge store + verification in BFF behind the new mutations; keep old `authenticate` temporarily behind `BFF_DEV_MODE` only.
3. Update frontend login/registration to the two-step flow.
4. Remove the unverified `authenticate` path entirely.

Rollback: re-enable the legacy path via dev flag (testnet only).

## Open Questions

- Library choice on Bun: `@simplewebauthn/server` compatibility needs a spike (task 1.1).
- Should challenge issuance be rate-limited per IP now or deferred to the observability/ops work?
