# Task 08: BFF GraphQL Proxy

**Service:** `bff`
**Depends on:** Task 07 (User Service API to proxy to)
**Can parallelize with:** Task 01, Task 02, Task 04, Task 05, Task 06

## Goal

Rebuild the BFF as a thin GraphQL proxy that forwards requests to User Service and Treasury Service, manages JWT sessions, and contains zero business logic.

## Scope

### Project Setup
- Initialize Bun project at `services/bff/`
- GraphQL server (e.g., `graphql-yoga` or `mercurius`)
- JWT session management

### GraphQL Schema
- `Query`:
  - `me` — Returns current user profile (proxies to User Service)
  - `balance` — Returns user's SyncUSD balance (proxies to Treasury Service)
  - `poolDepths` — Returns pool depths per chain (proxies to Treasury Service)
  - `recentTransfers` — Returns recent transfer history (proxies to Treasury Service)
- `Mutation`:
  - `registerUser(address, credentialId, publicKey, displayName?)` — Proxies to User Service
  - `addCredential(credentialId, publicKey)` — Proxies to User Service

### JWT Session Management
- Issue JWT on successful passkey authentication (frontend verifies passkey, sends proof to BFF)
- JWT contains: user address, credential ID, expiry
- Middleware validates JWT on protected queries/mutations
- JWT is for UI session state only — on-chain transactions are signed by passkeys directly

### Proxy Logic
- All resolvers forward to backend services via appropriate protocols
- BFF calls User Service via gRPC using `@grpc/grpc-js`. Proto loaded from `packages/proto/user/v1/user_service.proto`
- Treasury Service calls use internal HTTP APIs
- Transform responses into GraphQL-friendly shapes
- No database access, no business logic, no on-chain interaction

## Acceptance Criteria
- GraphQL playground accessible at `/graphql`
- `me` query returns user data (proxied from User Service)
- JWT issuance and validation works
- No direct database or blockchain calls in the BFF codebase
