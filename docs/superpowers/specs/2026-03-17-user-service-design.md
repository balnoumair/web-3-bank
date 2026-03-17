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

Shared `.proto` definitions in `packages/proto/` serve as the single source of truth for the API contract. The Rust service uses `tonic-build` in `build.rs` to generate server-side code at compile time. The BFF (Bun) uses `@grpc/grpc-js` + `@grpc/proto-loader` to call the service at runtime.

### Repository Layout

```
web3Bank/
├── packages/
│   └── proto/                                  # NEW — shared protobuf definitions
│       ├── user/v1/user_service.proto
│       └── treasury/v1/treasury_service.proto  # stub for future Treasury gRPC
└── services/                                   # NEW top-level directory
    └── user-service/
        ├── Cargo.toml
        ├── build.rs                            # tonic-build codegen
        ├── proto/                              # local copy of packages/proto/user/
        ├── src/
        │   ├── main.rs
        │   ├── config.rs
        │   ├── grpc/
        │   │   ├── mod.rs
        │   │   └── user_service.rs
        │   └── db/
        │       ├── mod.rs
        │       ├── users.rs
        │       ├── credentials.rs
        │       └── migrations/
        └── docker-compose.yml
```

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
}

message CreateUserRequest {
  string display_name = 1;
  string credential_id = 2;  // base64-encoded WebAuthn credential ID
  bytes  public_key    = 3;
  string tempo_address = 4;
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
  string user_id = 1;
}

message ListCredentialsResponse {
  repeated Credential credentials = 1;
}

message Credential {
  string id            = 1;
  string credential_id = 2;
  string tempo_address = 3;
  string created_at    = 4;
}

message AddCredentialRequest {
  string user_id       = 1;
  string credential_id = 2;
  bytes  public_key    = 3;
  string tempo_address = 4;
}

message AddCredentialResponse {
  string credential_id = 1;
}

message UpdateUserRequest {
  string user_id      = 1;
  string display_name = 2;  // optional — empty means no change
}

message UpdateUserResponse {
  string user_id      = 1;
  string display_name = 2;
  string updated_at   = 3;
}
```

---

## Data Model

### PostgreSQL Schema (`users` schema)

```sql
CREATE TABLE users.users (
    id           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    display_name TEXT        NOT NULL,
    status       TEXT        NOT NULL DEFAULT 'active',  -- 'active' | 'suspended'
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE users.credentials (
    id            UUID  PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID  NOT NULL REFERENCES users.users(id),
    credential_id BYTEA NOT NULL UNIQUE,   -- WebAuthn credential ID
    public_key    BYTEA NOT NULL,
    tempo_address TEXT  NOT NULL UNIQUE,   -- enforces one address per credential
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

One user may have multiple credentials (multi-device support). The `UNIQUE` constraint on `credentials.tempo_address` prevents duplicate address registration at the database level.

---

## Rust Implementation

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

### Module Responsibilities

- **`main.rs`** — tokio runtime, tonic server bind on `0.0.0.0:50051`, graceful shutdown on SIGTERM
- **`config.rs`** — reads `DATABASE_URL`, `PORT`, `LOG_LEVEL` from environment
- **`grpc/user_service.rs`** — implements the tonic-generated `UserServiceServer` trait; delegates to `db::*` for data access
- **`db/users.rs`** — CRUD for `users.users`
- **`db/credentials.rs`** — CRUD for `users.credentials`
- **`db/migrations/`** — sqlx migration files

### Validation & Error Mapping

| Rule | Enforcement | gRPC Status |
|---|---|---|
| `tempo_address` format | Regex check in Rust before insert | `INVALID_ARGUMENT` |
| `credential_id` uniqueness | `UNIQUE` DB constraint | `ALREADY_EXISTS` |
| Duplicate address registration | `UNIQUE` on `tempo_address` | `ALREADY_EXISTS` |
| User existence before adding credential | Explicit lookup before insert | `NOT_FOUND` |

gRPC status codes map cleanly to GraphQL errors in the BFF.

### Health Check

`tonic-health` exposes the standard gRPC health protocol on the same port (`50051`). Docker Compose and the BFF use `grpc_health_probe` for readiness checks.

---

## BFF Integration

The BFF switches its User Service client from `fetch()` (HTTP) to `@grpc/grpc-js`:

```ts
// services/bff/src/clients/user.ts
import * as grpc from '@grpc/grpc-js'
import * as protoLoader from '@grpc/proto-loader'

const packageDef = protoLoader.loadSync('../../packages/proto/user/v1/user_service.proto')
const { user: { v1: { UserService } } } = grpc.loadPackageDefinition(packageDef) as any

export const userClient = new UserService(
  process.env.USER_SERVICE_ADDR ?? 'user-service:50051',
  grpc.credentials.createInsecure()
)
```

The GraphQL schema is unchanged — only resolver internals update to call `userClient` instead of `fetch`.

---

## Architecture Document Updates

- `architecture/services.md` — BFF → User Service protocol updated from "Internal HTTP API" to "gRPC (port 50051)"
- `architecture/services.md` — User Service runtime updated from "TBD (Rust or Bun)" to "Rust"
- `tasks/07-user-service.md` — updated to reflect Rust + gRPC, `services/user-service/` path, and BFF client change
- `README.md` — User Service row updated

---

## Acceptance Criteria

- `cargo build` compiles without errors
- `cargo test` passes (unit tests for DB queries and validation logic)
- All 5 RPC methods work end-to-end against a real Postgres instance
- Duplicate address registration returns gRPC `ALREADY_EXISTS`
- `grpc_health_probe` responds healthy after startup
- BFF resolvers call User Service via gRPC (HTTP client code removed)
- Architecture and task documents updated as listed above
