# Passkeys (WebAuthn) + Account Abstraction Integration

## When to use
Use this skill to design or implement:
- passkey registration and login
- session creation and verification
- “enable payments/subscriptions” delegation flows
- any on-chain authorization using WebAuthn signatures (ERC-4337)

## Goal
Implement a coherent model where the same passkey supports:
- off-chain authentication (login/session)
- on-chain authorization (UserOperations)
without collapsing security boundaries.

## Inputs
- Platform target (web first)
- Desired UX: when to re-prompt passkey, when to rely on existing policy
- Chosen AA approach (WebAuthn validator/module vs alternative)

## Procedure
1) Separate contexts explicitly
- Off-chain: login challenge → session (JWT/cookie)
- On-chain: UserOperation signature → smart account validation

2) Define identity binding
- Store passkey public key as part of user identity (backend)
- Ensure smart account/validator is bound to that passkey public key

3) Define what requires on-chain consent
- Any of:
  - enabling delegation (deposit + policy creation)
  - changing limits/policy
  - withdrawals (especially withdrawAll / emergency exit)
- Payments may be executed without re-prompt if policy permits.

4) Session-bound delegation pattern
- One user consent can create an on-chain policy:
  - maxPerTx, maxPerWindow, expiry, allowed targets
- After that, backend can trigger payments within policy without repeated prompts.

5) Security hardening
- Challenge freshness + replay prevention (off-chain)
- Domain separation (origin/rpId for WebAuthn)
- On-chain validation must be deterministic and policy-bound.

## Outputs
- A sequence diagram for:
  - register, login, enable policy, pay, withdraw
- A clear table of “requires passkey prompt?” per action

## Constraints
- Login session must never imply fund-movement authority.
- Backend must not sign UserOperations in place of the user.
- Any automation must be bounded by on-chain policy limits.

