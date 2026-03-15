# Task 09: Frontend Rebuild with wagmi/viem

**Service:** `frontend` (bank-client)
**Depends on:** Task 08 (BFF for API), Task 01 (SyncUSD ABI for contract interaction)
**Can parallelize with:** Task 04, Task 05, Task 06

## Goal

Rebuild the frontend authentication and transaction layer to use wagmi/viem with Tempo native passkeys (EIP-2718), replacing the current SimpleWebAuthn prototype.

## Scope

### Replace Auth Layer
- Remove `@simplewebauthn/browser` dependency
- Add `@wagmi/solid`, `viem`, `@tanstack/solid-query`
- Configure wagmi with Tempo chain and WebAuthn connector
- Registration: `navigator.credentials.create()` → derive Tempo address → register via BFF GraphQL
- Login: `navigator.credentials.get()` → verify address → JWT session via BFF

### Transaction Signing
- Deposit: Sign `deposit()` call on Bank Contract via passkey (EIP-2718 tx)
- Withdraw: Sign `withdraw()` call via passkey
- Transfer (same-chain): Sign `transfer()` on SyncUSD via passkey
- Transfer (cross-chain): Sign `transferHotPath()` on Bank Contract via passkey
- All mutations trigger biometric prompt — no stored private keys

### Dashboard Updates
- Replace mock data with real GraphQL queries to BFF
- Balance: read from BFF `balance` query (which proxies to Treasury)
- Recent activity: read from BFF `recentTransfers` query
- Real-time balance updates after transactions

### Remove Prototype Code
- Remove `passkey-service.ts` (SimpleWebAuthn)
- Remove `graphql-client.ts` (replace with wagmi + tanstack query)
- Update `auth-context.tsx` to use wagmi hooks

## Acceptance Criteria
- Registration creates a passkey and registers the derived address via BFF
- Login recognizes existing passkey and establishes JWT session
- Deposit/withdraw/transfer trigger passkey signing and submit on-chain
- Dashboard shows real balance and activity data
