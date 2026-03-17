# User Service Design Spec

**Date:** 2026-03-17
**Task:** 07 — User Service
**Decision:** Rust + gRPC (replacing TBD runtime + internal HTTP from original task)

---

## Context

The User Service manages user profiles and passkey credential-to-address mappings. It is called by the BFF when the frontend needs user data and does not interact with the blockchain directly.

The original task left the runtime as "TBD (Rust or Bun)" and specified internal HTTP. This spec locks in **Rust** (consistent with Treasury Service) and **gRPC** as the internal protocol, with gRPC standardized across all backend service-to-service communication.

---

## Architecture

### Approach

Shared `.proto` definitions in `packages/proto/` serve as the single source of truth for the API contract. The Rust service uses `tonic-build` in `build.rs` to generate server-side code at compile time, pointing directly at `packages/proto/` — no local copy of the proto files is maintained in the service. The BFF (Bun) uses `@grpc/grpc-js` + `@grpc/proto-loader` to call the service at runtime.

### Repository Layout

```
web3Bank/
├── packages/
│   └── proto/                                  # NEW — shared protobuf definitions
│       ├── user/v1/user_service.proto
│       └── treasury/v1/treasury_service.proto  # stub for future Treasury gRPC
├── services/                                   # NEW top-level directory (Rust only; not a pnpm workspace member)
│   ├── Cargo.toml                              # Cargo workspace root (members: ["user-service"])
│   │                                           # Treasury added here when Task 04 is migrated
│   ├── Cargo.lock                              # MUST be committed
│   ├── docker-compose.yml                      # Shared Postgres + user-service (invoked via: docker compose -f services/docker-compose.yml up)
│   └── user-service/
│       ├── Cargo.toml
│       ├── build.rs          # tonic-build; reads ../../packages/proto/user/v1/
│       ├── rust-toolchain.toml
│       ├── src/
│       │   ├── main.rs
│       │   ├── config.rs
│       │   ├── grpc/
│       │   │   ├── mod.rs
│       │   │   └── user_service.rs
│       │   └── db/
│       │       ├── mod.rs
│       │       ├── users.rs
│       │       ├── credentials.rs
│       │       └── migrations/
│       └── Dockerfile
```

**`services/` and the pnpm workspace:** `services/` contains only Rust crates and is intentionally not listed in `pnpm-workspace.yaml`. Turbo will ignore it. Local development invokes compose directly: `docker compose -f services/docker-compose.yml up`.

**Cargo workspace:** `services/Cargo.toml` initially lists only `user-service` as a member. The workspace `Cargo.lock` must be committed. When Task 04 (Treasury) migrates its existing `Cargo.toml` to this workspace, it is added as a second member.

**Docker workspace isolation:** The `Dockerfile` does not need to copy the full workspace — it uses a minimal workspace `Cargo.toml` approach (see Docker section) that only includes `user-service`, avoiding the need to copy `treasury/` into the build context.

---

## Tempo Address Format

A valid Tempo address is a `0x`-prefixed, 40-character hex string derived from a P-256 public key (EVM-compatible 20-byte address encoding). Both lowercase and EIP-55 checksummed formats are accepted. The validation regex:

```
^0x[0-9a-fA-F]{40}$
```

This matches `architecture/authentication.md`'s registration flow, where the browser derives the Tempo address from the generated P-256 public key via `navigator.credentials.create()`.

---

## Proto Contract

**File:** `packages/proto/user/v1/user_service.proto`

```proto
syntax = "proto3";
package user.v1;

service UserService {
  rpc CreateUser(CreateUserRequest) returns (CreateUserResponse);
  rpc GetUserByAddress(GetUserByAddressRequest) returns (GetUserByAddressResponse);
  rpc ListCredentials(ListCredentialsRequest) returns (ListCredentialsResponse);
  rpc AddCredential(AddCredentialRequest) returns (AddCredentialResponse);
  rpc UpdateUser(UpdateUserRequest) returns (UpdateUserResponse);
  rpc RevokeCredential(RevokeCredentialRequest) returns (RevokeCredentialResponse);
}

// credential_id is raw bytes throughout — the BFF must base64url-decode
// the value received from the browser before sending it to this service.
message CreateUserRequest {
  optional string display_name  = 1;  // defaults to "" on server if absent
  bytes           credential_id = 2;  // raw WebAuthn credential ID bytes
  bytes           public_key    = 3;  // raw P-256 public key bytes
  string          tempo_address = 4;  // 0x-prefixed 40-char hex
}

message CreateUserResponse {
  string user_id = 1;
}

message GetUserByAddressRequest {
  string tempo_address = 1;
}

message GetUserByAddressResponse {
  string user_id      = 1;
  string display_name = 2;
  string status       = 3;
  string created_at   = 4;
}

message ListCredentialsRequest {
  string user_id     = 1;
  bool   active_only = 2;  // if true, exclude revoked credentials
}

message ListCredentialsResponse {
  repeated Credential credentials = 1;
}

// public_key is intentionally omitted from this message. Tempo verifies
// WebAuthn signatures on-chain at the protocol level — the BFF never
// performs server-side assertion verification. If that changes, add
// bytes public_key = 6 here.
message Credential {
  string          id            = 1;
  bytes           credential_id = 2;
  string          tempo_address = 3;
  string          created_at    = 4;
  optional string revoked_at    = 5;  // absent = active, present = revoked
}

message AddCredentialRequest {
  string user_id       = 1;  // injected by BFF from JWT session; not supplied by client
  bytes  credential_id = 2;
  bytes  public_key    = 3;
  string tempo_address = 4;
}

// credential_id returned as string (base64url) so the BFF can forward it
// to the GraphQL client without a Buffer-to-string conversion step.
message AddCredentialResponse {
  string credential_id = 1;  // base64url-encoded credential ID
}

// optional keyword (proto3 syntax) lets the service distinguish
// "not provided" from "explicitly set to empty string"
message UpdateUserRequest {
  string          user_id      = 1;
  optional string display_name = 2;
}

message UpdateUserResponse {
  string user_id      = 1;
  string display_name = 2;
  string updated_at   = 3;
}

message RevokeCredentialRequest {
  string user_id       = 1;
  bytes  credential_id = 2;
}

message RevokeCredentialResponse {}
```

---

## Data Model

### PostgreSQL Schema (`users` schema)

```sql
CREATE SCHEMA IF NOT EXISTS users;

CREATE TABLE users.users (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    display_name TEXT        NOT NULL DEFAULT '',
    status       TEXT        NOT NULL DEFAULT 'active',  -- 'active' | 'suspended'
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE users.credentials (
    id            UUID  PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID  NOT NULL REFERENCES users.users(id),
    credential_id BYTEA NOT NULL UNIQUE,
    public_key    BYTEA NOT NULL,
    tempo_address TEXT  NOT NULL UNIQUE,
    revoked_at    TIMESTAMPTZ,        -- NULL = active, non-NULL = revoked
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

- `credential_id` is stored as raw bytes (`BYTEA`), matching the proto `bytes` field throughout. **`Credential.id` (UUID string) is never used as a revocation key** — `RevokeCredentialRequest.credential_id` uses the raw WebAuthn `credential_id` bytes, matching `Credential.credential_id` from `ListCredentials`.
- `GetUserByAddress` resolves a Tempo address to a user by querying `users.credentials` first (`WHERE tempo_address = $1`), then fetching the corresponding `users.users` row via `user_id`. There is no `tempo_address` column on `users.users`.
- Soft-delete revocation (`revoked_at`) preserves audit history.
- `display_name` defaults to `''` when not provided at registration.

---

## Rust Implementation

### Toolchain

`services/user-service/rust-toolchain.toml` pins the toolchain:

```toml
[toolchain]
channel = "1.77.0"
```

The `Dockerfile` `RUST_VERSION` build arg must match this value. Both are updated together.

### Dependencies (`Cargo.toml`)

| Crate | Purpose |
|---|---|
| `tokio` | Async runtime |
| `tonic` | gRPC server + codegen integration |
| `tonic-health` | Standard gRPC health protocol |
| `prost` | Protobuf serialization (generated via tonic-build) |
| `sqlx` | Compile-time verified async Postgres queries |
| `uuid` | UUID generation |
| `tracing` + `tracing-subscriber` | Structured logging |

### Configuration (`config.rs`)

| Variable | Default | Description |
|---|---|---|
| `DATABASE_URL` | — (required) | PostgreSQL connection string |
| `GRPC_ADDR` | `0.0.0.0:50051` | Host and port to bind the gRPC server |
| `LOG_LEVEL` | `info` | Tracing log level |

`GRPC_ADDR` defaults to `0.0.0.0:50051` — binding to `127.0.0.1` would make the service unreachable from other Docker containers.

### Module Responsibilities

- **`main.rs`** — tokio runtime, binds tonic server on `GRPC_ADDR`, graceful shutdown on SIGTERM
- **`config.rs`** — parses env vars into a `Config` struct
- **`grpc/user_service.rs`** — implements the tonic-generated `UserServiceServer` trait; delegates to `db::*`
- **`db/users.rs`** — CRUD for `users.users`
- **`db/credentials.rs`** — CRUD for `users.credentials` (including revocation)
- **`db/migrations/`** — sqlx migration files

### Validation & Error Mapping

| Rule | Enforcement | gRPC Status |
|---|---|---|
| `tempo_address` format (`^0x[0-9a-fA-F]{40}$`) | Regex check before insert | `INVALID_ARGUMENT` |
| `credential_id` uniqueness | `UNIQUE` DB constraint | `ALREADY_EXISTS` |
| Duplicate address registration | `UNIQUE` on `tempo_address` | `ALREADY_EXISTS` |
| `tempo_address` not found in `GetUserByAddress` | Explicit check; never return empty | `NOT_FOUND` |
| User not found before `AddCredential` / `RevokeCredential` | Lookup before mutation | `NOT_FOUND` |
| Credential not found before `RevokeCredential` | Lookup before update | `NOT_FOUND` |
| Revoking the user's last active credential | Inside a transaction: `SELECT id FROM credentials WHERE user_id = $1 AND revoked_at IS NULL FOR UPDATE` — lock rows, count results in application code; reject if count would reach zero | `FAILED_PRECONDITION` |

**Authorization:** The User Service trusts all callers (internal network only, no public exposure). Authorization — ensuring a JWT session user can only modify their own data — is enforced exclusively at the BFF layer before the gRPC call is made.

### Health Check

`tonic-health` exposes the standard gRPC health protocol on the same port. `grpc_health_probe` is used for readiness checks in Docker Compose and any future orchestration.

### Docker

The `Dockerfile` uses a **single-service workspace** approach to avoid needing `treasury/` in the build context. The build runs from the workspace root (`/app/services/`) so Cargo places the binary at `services/target/release/user-service`:

```dockerfile
# services/user-service/Dockerfile
# Build context: repo root
ARG RUST_VERSION=1.77.0
FROM rust:${RUST_VERSION} AS builder
WORKDIR /app

# Proto files (needed by build.rs at compile time)
COPY packages/proto/ packages/proto/

# Copy lock file first (reproducible builds), then service source
COPY services/Cargo.lock services/Cargo.lock
COPY services/user-service/ services/user-service/

# Override Cargo.toml to a single-member workspace so we don't need treasury/
RUN printf '[workspace]\nmembers = ["user-service"]\n' > services/Cargo.toml

# Build from workspace root — binary lands at services/target/release/
WORKDIR /app/services
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/services/target/release/user-service /usr/local/bin/
CMD ["user-service"]
```

The real `services/Cargo.toml` (listing all workspace members) is used for local development; the Dockerfile overrides it at build time to a single-member workspace, avoiding the need to copy unrelated crates.

---

## Docker Compose (Shared Postgres)

Run locally with: `docker compose -f services/docker-compose.yml up`

```yaml
# services/docker-compose.yml
services:
  postgres:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: postgres
      POSTGRES_DB: web3bank
    ports:
      - "5432:5432"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U postgres"]
      interval: 5s
      timeout: 5s
      retries: 5
    networks:
      - internal

  user-service:
    build:
      context: ..          # repo root — needed for packages/proto/
      dockerfile: services/user-service/Dockerfile
    environment:
      DATABASE_URL: postgres://postgres:postgres@postgres:5432/web3bank
      GRPC_ADDR: "0.0.0.0:50051"
    ports:
      - "50051:50051"
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - internal

networks:
  internal:
```

Treasury service will be added to this file when Task 04 is complete.

---

## BFF Integration

The BFF switches its User Service client from `fetch()` (HTTP) to `@grpc/grpc-js`.

**credential_id encoding:** The browser's WebAuthn API returns `credential_id` as a base64url string. The BFF must base64url-decode it to raw bytes before populating any `bytes credential_id` gRPC field. `AddCredentialResponse.credential_id` is returned as a base64url `string` (not bytes) so the BFF can forward it to the GraphQL client directly without re-encoding a `Buffer`.

**user_id injection:** The BFF must never accept `user_id` from the GraphQL client. For `addCredential` and `revokeCredential`, the BFF extracts `user_id` from the validated JWT session and injects it into the gRPC request. This is where authorization is enforced.

The GraphQL `registerUser` mutation is updated to include `publicKey`. `displayName` is optional (maps to the `optional string display_name` proto field; if absent the server defaults to `""`):

```graphql
# Updated mutation signatures in BFF schema (tasks/08-bff-graphql-proxy.md)
type Mutation {
  registerUser(address: String!, credentialId: String!, publicKey: String!, displayName: String): RegisterUserResult!
  addCredential(credentialId: String!, publicKey: String!, tempoAddress: String!): AddCredentialResult!
}
```

```ts
// services/bff/src/clients/user.ts
import * as grpc from '@grpc/grpc-js'
import * as protoLoader from '@grpc/proto-loader'

// import.meta.dir is Bun's equivalent of __dirname — resolves relative to
// this source file, not the process working directory.
const PROTO_PATH = import.meta.dir + '/../../../packages/proto/user/v1/user_service.proto'

const packageDef = protoLoader.loadSync(PROTO_PATH, { keepCase: true })
const { user: { v1: { UserService } } } = grpc.loadPackageDefinition(packageDef) as any

export const userClient = new UserService(
  process.env.USER_SERVICE_ADDR ?? 'user-service:50051',
  grpc.credentials.createInsecure()
)
```

**`GetUserByAddress` on login:** A `NOT_FOUND` response means the address is not registered. The BFF should return an unauthenticated / registration-required signal to the frontend, not an internal server error.

---

## Architecture Document Updates

- `architecture/services.md` — BFF → User Service protocol updated from "Internal HTTP API" to "gRPC (port 50051)"; User Service runtime updated from "TBD (Rust or Bun)" to "Rust"
- `architecture/authentication.md` — Registration sequence updated: browser sends `(address, credentialId, publicKey)` to BFF (was `(address, credentialId)`)
- `tasks/07-user-service.md` — updated to reflect Rust + gRPC, `services/user-service/` path, BFF client change
- `tasks/08-bff-graphql-proxy.md` — `registerUser` mutation updated to include `publicKey` and optional `displayName`; BFF → User Service proxy changed from HTTP to gRPC
- `README.md` — User Service row updated

---

## Acceptance Criteria

- `cargo build --workspace` from `services/` compiles without errors
- `cargo test` passes (unit tests for DB queries and validation logic)
- `services/Cargo.lock` is committed
- All 6 RPC methods work end-to-end against a real Postgres instance
- Duplicate address registration returns `ALREADY_EXISTS`
- Invalid address format returns `INVALID_ARGUMENT`
- `GetUserByAddress` for unknown address returns `NOT_FOUND`
- Revoking the last active credential returns `FAILED_PRECONDITION`
- Credential revocation sets `revoked_at`; revoked credentials excluded when `active_only = true`
- `grpc_health_probe` responds healthy after startup
- BFF container reaches User Service on port 50051 via `services/docker-compose.yml`
- BFF `addCredential` resolver injects `user_id` from JWT (not from client input)
- BFF resolvers call User Service via gRPC; HTTP client code removed
- Architecture and task documents updated as listed above
