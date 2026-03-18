# User Service Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust gRPC User Service that manages user profiles and WebAuthn passkey credential mappings, backed by PostgreSQL.

**Architecture:** Shared proto definitions in `packages/proto/` are the API contract; `tonic-build` generates Rust server code from them at compile time. The service exposes 6 RPCs via `tonic`, persists data with `sqlx` (compile-time verified queries using offline mode), and shares a Postgres instance with future services via `services/docker-compose.yml`.

**Tech Stack:** Rust 1.77, tonic 0.11, prost 0.12, sqlx 0.7 (postgres + chrono + uuid + macros), tokio (full), tonic-health, once_cell, base64 0.22, thiserror, Docker Compose

---

## File Map

### New Files

| File | Responsibility |
|---|---|
| `packages/proto/user/v1/user_service.proto` | gRPC API contract — single source of truth |
| `packages/proto/treasury/v1/treasury_service.proto` | Stub for future Treasury gRPC |
| `services/Cargo.toml` | Cargo workspace root (members: user-service) |
| `services/docker-compose.yml` | Shared Postgres + user-service |
| `services/user-service/Cargo.toml` | Crate manifest and dependencies |
| `services/user-service/build.rs` | tonic-build proto codegen |
| `services/user-service/rust-toolchain.toml` | Pin toolchain to 1.77.0 |
| `services/user-service/Dockerfile` | Multi-stage build (context = repo root) |
| `services/user-service/src/main.rs` | Tokio entrypoint, tonic server, health, migrations |
| `services/user-service/src/config.rs` | Config struct from env vars |
| `services/user-service/src/grpc/mod.rs` | include_proto! + re-exports |
| `services/user-service/src/grpc/user_service.rs` | All 6 RPC implementations |
| `services/user-service/src/db/mod.rs` | PgPool factory, module re-exports |
| `services/user-service/src/db/users.rs` | CRUD for `users.users` |
| `services/user-service/src/db/credentials.rs` | CRUD + revocation for `users.credentials` |
| `services/user-service/src/db/migrations/20260317000001_create_users_schema.sql` | Schema + tables |

### Modified Files

| File | Change |
|---|---|
| `architecture/services.md` | User Service runtime → Rust; protocol → gRPC |
| `architecture/authentication.md` | Registration: browser sends publicKey |
| `tasks/07-user-service.md` | Update to reflect Rust + gRPC decisions |
| `tasks/08-bff-graphql-proxy.md` | Update registerUser mutation + proxy to gRPC |
| `README.md` | Update User Service row |

---

## Chunk 1: Project Scaffold

### Task 1: Proto Definitions

**Files:**
- Create: `packages/proto/user/v1/user_service.proto`
- Create: `packages/proto/treasury/v1/treasury_service.proto`

- [ ] **Step 1.1: Create proto directories**

```bash
mkdir -p packages/proto/user/v1
mkdir -p packages/proto/treasury/v1
```

- [ ] **Step 1.2: Write user_service.proto**

Create `packages/proto/user/v1/user_service.proto`:

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

// credential_id is raw bytes throughout. The BFF must base64url-decode
// the value received from the browser before sending it to this service.
message CreateUserRequest {
  optional string display_name  = 1;  // defaults to "" on server if absent
  bytes           credential_id = 2;  // raw WebAuthn credential ID bytes
  bytes           public_key    = 3;  // raw P-256 public key bytes
  string          tempo_address = 4;  // 0x-prefixed 40-char hex
}
message CreateUserResponse { string user_id = 1; }

message GetUserByAddressRequest { string tempo_address = 1; }
message GetUserByAddressResponse {
  string user_id      = 1;
  string display_name = 2;
  string status       = 3;
  string created_at   = 4;
}

message ListCredentialsRequest {
  string user_id     = 1;
  bool   active_only = 2;
}
message ListCredentialsResponse { repeated Credential credentials = 1; }

// public_key intentionally omitted: Tempo verifies WebAuthn on-chain.
message Credential {
  string          id            = 1;
  bytes           credential_id = 2;
  string          tempo_address = 3;
  string          created_at    = 4;
  optional string revoked_at    = 5;
}

// user_id injected by BFF from JWT session — never from the client.
message AddCredentialRequest {
  string user_id       = 1;
  bytes  credential_id = 2;
  bytes  public_key    = 3;
  string tempo_address = 4;
}
// credential_id returned as base64url string for direct GraphQL forwarding.
message AddCredentialResponse { string credential_id = 1; }

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

- [ ] **Step 1.3: Write treasury stub proto**

Create `packages/proto/treasury/v1/treasury_service.proto`:

```proto
syntax = "proto3";
package treasury.v1;

// Stub — Treasury gRPC contract will be defined in Task 04/05.
service TreasuryService {}
```

- [ ] **Step 1.4: Commit proto definitions**

```bash
git add packages/proto/
git commit -m "feat: add proto definitions for user service and treasury stub"
```

---

### Task 2: Cargo Workspace + Service Scaffold

**Files:**
- Create: `services/Cargo.toml`
- Create: `services/user-service/Cargo.toml`
- Create: `services/user-service/build.rs`
- Create: `services/user-service/rust-toolchain.toml`
- Create: `services/user-service/src/main.rs` (stub)

- [ ] **Step 2.1: Create workspace Cargo.toml**

Create `services/Cargo.toml`:

```toml
[workspace]
members = ["user-service"]
resolver = "2"
```

- [ ] **Step 2.2: Create rust-toolchain.toml**

Create `services/user-service/rust-toolchain.toml`:

```toml
[toolchain]
channel = "1.77.0"
```

- [ ] **Step 2.3: Create service Cargo.toml**

Create `services/user-service/Cargo.toml`:

```toml
[package]
name = "user-service"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio       = { version = "1", features = ["full"] }
tonic       = "0.11"
tonic-health = "0.11"
prost       = "0.12"
sqlx        = { version = "0.7", features = [
    "runtime-tokio-native-tls",
    "postgres", "uuid", "chrono", "macros",
] }
uuid        = { version = "1", features = ["v4"] }
chrono      = "0.4"
tracing     = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
once_cell   = "1"
regex       = "1"
base64      = "0.22"
thiserror   = "1"

[build-dependencies]
tonic-build = "0.11"

[dev-dependencies]
sqlx        = { version = "0.7", features = [
    "runtime-tokio-native-tls",
    "postgres", "uuid", "chrono", "macros", "test-utils",
] }
tokio        = { version = "1", features = ["full"] }
tokio-stream = "0.1"
```

- [ ] **Step 2.4: Create build.rs**

Create `services/user-service/build.rs`:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("../../packages/proto/user/v1/user_service.proto")?;
    Ok(())
}
```

- [ ] **Step 2.5: Create stub main.rs**

Create `services/user-service/src/main.rs`:

```rust
fn main() {}
```

- [ ] **Step 2.6: Verify the workspace compiles**

`protoc` must be installed. If not:
```bash
brew install protobuf          # macOS
# or: apt-get install -y protobuf-compiler
```

```bash
cd services
cargo build
```

Expected: compiles with no errors. `OUT_DIR` will contain generated proto code.

- [ ] **Step 2.7: Commit scaffold**

```bash
git add services/
git commit -m "feat: add cargo workspace scaffold and tonic-build setup"
```

---

## Chunk 2: DB Layer

### Task 3: Migration

**Files:**
- Create: `services/user-service/src/db/migrations/20260317000001_create_users_schema.sql`

- [ ] **Step 3.1: Create migrations directory**

```bash
mkdir -p services/user-service/src/db/migrations
```

- [ ] **Step 3.2: Write schema migration**

Create `services/user-service/src/db/migrations/20260317000001_create_users_schema.sql`:

```sql
CREATE SCHEMA IF NOT EXISTS users;

CREATE TABLE users.users (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    display_name TEXT        NOT NULL DEFAULT '',
    status       TEXT        NOT NULL DEFAULT 'active',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE users.credentials (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID        NOT NULL REFERENCES users.users(id),
    credential_id BYTEA       NOT NULL UNIQUE,
    public_key    BYTEA       NOT NULL,
    tempo_address TEXT        NOT NULL UNIQUE,
    revoked_at    TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

- [ ] **Step 3.3: Commit migration**

```bash
git add services/user-service/src/db/migrations/
git commit -m "feat: add users schema migration"
```

---

### Task 4: Config + Module Stubs

**Files:**
- Create: `services/user-service/src/config.rs`
- Create: `services/user-service/src/db/mod.rs`
- Create: `services/user-service/src/db/users.rs` (stub)
- Create: `services/user-service/src/db/credentials.rs` (stub)
- Create: `services/user-service/src/grpc/mod.rs`
- Create: `services/user-service/src/grpc/user_service.rs` (stub)

- [ ] **Step 4.1: Write config.rs**

Create `services/user-service/src/config.rs`:

```rust
use std::net::SocketAddr;

#[derive(Debug)]
pub struct Config {
    pub database_url: String,
    pub grpc_addr: SocketAddr,
    pub log_level: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL is required".to_string())?;

        let grpc_addr = std::env::var("GRPC_ADDR")
            .unwrap_or_else(|_| "0.0.0.0:50051".to_string())
            .parse::<SocketAddr>()
            .map_err(|e| format!("Invalid GRPC_ADDR: {e}"))?;

        let log_level = std::env::var("LOG_LEVEL")
            .unwrap_or_else(|_| "info".to_string());

        Ok(Config { database_url, grpc_addr, log_level })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_missing_database_url() {
        std::env::remove_var("DATABASE_URL");
        let result = Config::from_env();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("DATABASE_URL"));
    }

    #[test]
    fn test_config_defaults() {
        std::env::set_var("DATABASE_URL", "postgres://localhost/test");
        std::env::remove_var("GRPC_ADDR");
        std::env::remove_var("LOG_LEVEL");
        let config = Config::from_env().unwrap();
        assert_eq!(config.grpc_addr.to_string(), "0.0.0.0:50051");
        assert_eq!(config.log_level, "info");
        std::env::remove_var("DATABASE_URL");
    }
}
```

- [ ] **Step 4.2: Create db/mod.rs**

Create `services/user-service/src/db/mod.rs`:

```rust
pub mod credentials;
pub mod users;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
}
```

- [ ] **Step 4.3: Create stub db/users.rs**

Create `services/user-service/src/db/users.rs`:

```rust
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub display_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

- [ ] **Step 4.4: Create stub db/credentials.rs**

Create `services/user-service/src/db/credentials.rs`:

```rust
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct CredentialRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub credential_id: Vec<u8>,
    pub public_key: Vec<u8>,
    pub tempo_address: String,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
```

- [ ] **Step 4.5: Create grpc/mod.rs**

Create `services/user-service/src/grpc/mod.rs`:

```rust
pub mod user_service;

tonic::include_proto!("user.v1");

// Re-export server types for use in main.rs
pub use user_service_server::UserServiceServer;
```

- [ ] **Step 4.6: Create stub grpc/user_service.rs**

Create `services/user-service/src/grpc/user_service.rs`:

```rust
// gRPC service implementation — filled in Task 7
```

- [ ] **Step 4.7: Update main.rs to declare modules**

Replace `services/user-service/src/main.rs`:

```rust
mod config;
mod db;
mod grpc;

fn main() {}
```

- [ ] **Step 4.8: Verify compilation**

```bash
cd services
cargo build
```

Expected: compiles. Fix any missing imports.

- [ ] **Step 4.9: Run config tests**

```bash
cd services
cargo test config
```

Expected: 2 tests pass.

- [ ] **Step 4.10: Commit**

```bash
git add services/user-service/src/
git commit -m "feat: add config, db module stubs, and grpc module wiring"
```

---

### Task 5: DB — users.rs

**Files:**
- Modify: `services/user-service/src/db/users.rs`

Tests use `#[sqlx::test]` which creates a fresh isolated Postgres database per test and runs migrations automatically. Requires a running Postgres:

```bash
# Start Postgres before running DB tests
docker compose -f services/docker-compose.yml up -d postgres
export DATABASE_URL="postgres://postgres:postgres@localhost:5432/web3bank"
```

- [ ] **Step 5.1: Write failing tests**

Replace `services/user-service/src/db/users.rs` with struct + tests only (no functions):

```rust
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub display_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_insert_user_default_display_name(pool: PgPool) {
        let id = insert_user(&pool, None).await.unwrap();
        let row = get_user_by_id(&pool, id).await.unwrap().expect("user should exist");
        assert_eq!(row.display_name, "");
        assert_eq!(row.status, "active");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_insert_user_with_display_name(pool: PgPool) {
        let id = insert_user(&pool, Some("Alice")).await.unwrap();
        let row = get_user_by_id(&pool, id).await.unwrap().expect("user should exist");
        assert_eq!(row.display_name, "Alice");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_get_user_by_id_not_found(pool: PgPool) {
        let result = get_user_by_id(&pool, Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_update_display_name(pool: PgPool) {
        let id = insert_user(&pool, Some("Old Name")).await.unwrap();
        update_display_name(&pool, id, "New Name").await.unwrap();
        let row = get_user_by_id(&pool, id).await.unwrap().expect("user should exist");
        assert_eq!(row.display_name, "New Name");
    }
}
```

- [ ] **Step 5.2: Run tests to verify they fail**

```bash
cd services
DATABASE_URL="postgres://postgres:postgres@localhost:5432/web3bank" \
  cargo test db::users::tests -- --nocapture 2>&1 | head -20
```

Expected: compilation error — `insert_user`, `get_user_by_id`, `update_display_name` not defined.

- [ ] **Step 5.3: Implement users.rs**

Replace file with the full implementation (struct + functions + tests):

```rust
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub display_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn insert_user(pool: &PgPool, display_name: Option<&str>) -> Result<Uuid, sqlx::Error> {
    let name = display_name.unwrap_or("");
    let row = sqlx::query!(
        "INSERT INTO users.users (display_name) VALUES ($1) RETURNING id",
        name
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub async fn get_user_by_id(pool: &PgPool, id: Uuid) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as!(
        UserRow,
        "SELECT id, display_name, status, created_at, updated_at
         FROM users.users WHERE id = $1",
        id
    )
    .fetch_optional(pool)
    .await
}

pub async fn update_display_name(
    pool: &PgPool,
    id: Uuid,
    display_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE users.users SET display_name = $1, updated_at = now() WHERE id = $2",
        display_name,
        id
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_insert_user_default_display_name(pool: PgPool) {
        let id = insert_user(&pool, None).await.unwrap();
        let row = get_user_by_id(&pool, id).await.unwrap().expect("user should exist");
        assert_eq!(row.display_name, "");
        assert_eq!(row.status, "active");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_insert_user_with_display_name(pool: PgPool) {
        let id = insert_user(&pool, Some("Alice")).await.unwrap();
        let row = get_user_by_id(&pool, id).await.unwrap().expect("user should exist");
        assert_eq!(row.display_name, "Alice");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_get_user_by_id_not_found(pool: PgPool) {
        let result = get_user_by_id(&pool, Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_update_display_name(pool: PgPool) {
        let id = insert_user(&pool, Some("Old Name")).await.unwrap();
        update_display_name(&pool, id, "New Name").await.unwrap();
        let row = get_user_by_id(&pool, id).await.unwrap().expect("user should exist");
        assert_eq!(row.display_name, "New Name");
    }
}
```

- [ ] **Step 5.4: Run tests (expect pass)**

```bash
cd services
DATABASE_URL="postgres://postgres:postgres@localhost:5432/web3bank" \
  cargo test db::users::tests -- --nocapture
```

Expected: 4 tests pass.

- [ ] **Step 5.5: Commit**

```bash
git add services/user-service/src/db/users.rs
git commit -m "feat: implement users DB layer with tests"
```

---

### Task 6: DB — credentials.rs

**Files:**
- Modify: `services/user-service/src/db/credentials.rs`

- [ ] **Step 6.1: Write failing tests**

Replace `services/user-service/src/db/credentials.rs` with struct + error types + tests (no functions):

```rust
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct CredentialRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub credential_id: Vec<u8>,
    pub public_key: Vec<u8>,
    pub tempo_address: String,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct UserWithCredential {
    pub user_id: Uuid,
    pub display_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("last active credential — cannot revoke")]
    LastActiveCredential,
    #[error("credential not found")]
    NotFound,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::users::insert_user;

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_insert_and_get_by_address(pool: PgPool) {
        let user_id = insert_user(&pool, Some("Alice")).await.unwrap();
        insert_credential(&pool, user_id, b"cred-bytes", b"pk-bytes",
            "0xabcdef1234567890abcdef1234567890abcdef12").await.unwrap();

        let row = get_user_by_address(&pool, "0xabcdef1234567890abcdef1234567890abcdef12")
            .await.unwrap().expect("should find user");
        assert_eq!(row.user_id, user_id);
        assert_eq!(row.display_name, "Alice");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_get_user_by_address_not_found(pool: PgPool) {
        let result = get_user_by_address(&pool, "0x0000000000000000000000000000000000000000")
            .await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_duplicate_address_rejected(pool: PgPool) {
        let user_id = insert_user(&pool, None).await.unwrap();
        let addr = "0xabcdef1234567890abcdef1234567890abcdef12";
        insert_credential(&pool, user_id, b"cred-1", b"pk", addr).await.unwrap();
        let result = insert_credential(&pool, user_id, b"cred-2", b"pk", addr).await;
        assert!(result.is_err());
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_list_credentials_active_only(pool: PgPool) {
        let user_id = insert_user(&pool, None).await.unwrap();
        let addr1 = "0xaaaa111111111111111111111111111111111111";
        let addr2 = "0xbbbb222222222222222222222222222222222222";
        insert_credential(&pool, user_id, b"cred1", b"pk1", addr1).await.unwrap();
        insert_credential(&pool, user_id, b"cred2", b"pk2", addr2).await.unwrap();

        // cred2 stays active; revoke cred1 (2 active → allowed)
        revoke_credential(&pool, user_id, b"cred1").await.unwrap();

        let all = list_credentials(&pool, user_id, false).await.unwrap();
        assert_eq!(all.len(), 2);

        let active = list_credentials(&pool, user_id, true).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].credential_id, b"cred2");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_revoke_last_credential_fails(pool: PgPool) {
        let user_id = insert_user(&pool, None).await.unwrap();
        insert_credential(&pool, user_id, b"only-cred", b"pk",
            "0xcccc333333333333333333333333333333333333").await.unwrap();

        let err = revoke_credential(&pool, user_id, b"only-cred").await.unwrap_err();
        assert!(matches!(err, CredentialError::LastActiveCredential));
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_revoke_nonexistent_credential_fails(pool: PgPool) {
        let user_id = insert_user(&pool, None).await.unwrap();
        // Need 2 credentials so the last-active check passes
        insert_credential(&pool, user_id, b"cred1", b"pk1",
            "0xaaaa111111111111111111111111111111111111").await.unwrap();
        insert_credential(&pool, user_id, b"cred2", b"pk2",
            "0xbbbb222222222222222222222222222222222222").await.unwrap();

        let result = revoke_credential(&pool, user_id, b"nonexistent").await;
        assert!(matches!(result.unwrap_err(), CredentialError::NotFound));
    }
}
```

- [ ] **Step 6.2: Run tests to verify they fail**

```bash
cd services
DATABASE_URL="postgres://postgres:postgres@localhost:5432/web3bank" \
  cargo test db::credentials::tests -- --nocapture 2>&1 | head -20
```

Expected: compilation error — functions not defined.

- [ ] **Step 6.3: Implement credentials.rs**

Replace file with the full implementation:

```rust
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct CredentialRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub credential_id: Vec<u8>,
    pub public_key: Vec<u8>,
    pub tempo_address: String,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct UserWithCredential {
    pub user_id: Uuid,
    pub display_name: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("last active credential — cannot revoke")]
    LastActiveCredential,
    #[error("credential not found")]
    NotFound,
}

/// Insert a credential. Caller must verify user existence before calling.
pub async fn insert_credential(
    pool: &PgPool,
    user_id: Uuid,
    credential_id: &[u8],
    public_key: &[u8],
    tempo_address: &str,
) -> Result<Uuid, CredentialError> {
    let row = sqlx::query!(
        "INSERT INTO users.credentials (user_id, credential_id, public_key, tempo_address)
         VALUES ($1, $2, $3, $4) RETURNING id",
        user_id,
        credential_id,
        public_key,
        tempo_address,
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

/// Resolve a Tempo address to user info via credentials → users JOIN.
pub async fn get_user_by_address(
    pool: &PgPool,
    tempo_address: &str,
) -> Result<Option<UserWithCredential>, sqlx::Error> {
    sqlx::query_as!(
        UserWithCredential,
        "SELECT u.id AS user_id, u.display_name, u.status, u.created_at
         FROM users.credentials c
         JOIN users.users u ON u.id = c.user_id
         WHERE c.tempo_address = $1",
        tempo_address,
    )
    .fetch_optional(pool)
    .await
}

/// List credentials for a user. Pass `active_only = true` to exclude revoked ones.
pub async fn list_credentials(
    pool: &PgPool,
    user_id: Uuid,
    active_only: bool,
) -> Result<Vec<CredentialRow>, sqlx::Error> {
    if active_only {
        sqlx::query_as!(
            CredentialRow,
            "SELECT id, user_id, credential_id, public_key, tempo_address, revoked_at, created_at
             FROM users.credentials
             WHERE user_id = $1 AND revoked_at IS NULL
             ORDER BY created_at ASC",
            user_id,
        )
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as!(
            CredentialRow,
            "SELECT id, user_id, credential_id, public_key, tempo_address, revoked_at, created_at
             FROM users.credentials
             WHERE user_id = $1
             ORDER BY created_at ASC",
            user_id,
        )
        .fetch_all(pool)
        .await
    }
}

/// Revoke a credential.
/// - `LastActiveCredential` if this would leave the user with zero active credentials.
/// - `NotFound` if the credential doesn't exist / already revoked for this user.
/// Uses SELECT ... FOR UPDATE inside a transaction to prevent concurrent races.
pub async fn revoke_credential(
    pool: &PgPool,
    user_id: Uuid,
    credential_id: &[u8],
) -> Result<(), CredentialError> {
    let mut tx = pool.begin().await?;

    // Lock all active credentials for this user to prevent concurrent revocations.
    let active = sqlx::query!(
        "SELECT id FROM users.credentials
         WHERE user_id = $1 AND revoked_at IS NULL
         FOR UPDATE",
        user_id,
    )
    .fetch_all(&mut *tx)
    .await?;

    if active.len() <= 1 {
        tx.rollback().await?;
        return Err(CredentialError::LastActiveCredential);
    }

    let updated = sqlx::query!(
        "UPDATE users.credentials
         SET revoked_at = now()
         WHERE user_id = $1 AND credential_id = $2 AND revoked_at IS NULL",
        user_id,
        credential_id,
    )
    .execute(&mut *tx)
    .await?;

    if updated.rows_affected() == 0 {
        tx.rollback().await?;
        return Err(CredentialError::NotFound);
    }

    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::users::insert_user;

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_insert_and_get_by_address(pool: PgPool) {
        let user_id = insert_user(&pool, Some("Alice")).await.unwrap();
        insert_credential(&pool, user_id, b"cred-bytes", b"pk-bytes",
            "0xabcdef1234567890abcdef1234567890abcdef12").await.unwrap();

        let row = get_user_by_address(&pool, "0xabcdef1234567890abcdef1234567890abcdef12")
            .await.unwrap().expect("should find user");
        assert_eq!(row.user_id, user_id);
        assert_eq!(row.display_name, "Alice");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_get_user_by_address_not_found(pool: PgPool) {
        let result = get_user_by_address(&pool, "0x0000000000000000000000000000000000000000")
            .await.unwrap();
        assert!(result.is_none());
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_duplicate_address_rejected(pool: PgPool) {
        let user_id = insert_user(&pool, None).await.unwrap();
        let addr = "0xabcdef1234567890abcdef1234567890abcdef12";
        insert_credential(&pool, user_id, b"cred-1", b"pk", addr).await.unwrap();
        let result = insert_credential(&pool, user_id, b"cred-2", b"pk", addr).await;
        assert!(result.is_err());
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_list_credentials_active_only(pool: PgPool) {
        let user_id = insert_user(&pool, None).await.unwrap();
        let addr1 = "0xaaaa111111111111111111111111111111111111";
        let addr2 = "0xbbbb222222222222222222222222222222222222";
        insert_credential(&pool, user_id, b"cred1", b"pk1", addr1).await.unwrap();
        insert_credential(&pool, user_id, b"cred2", b"pk2", addr2).await.unwrap();

        revoke_credential(&pool, user_id, b"cred1").await.unwrap();

        let all = list_credentials(&pool, user_id, false).await.unwrap();
        assert_eq!(all.len(), 2);

        let active = list_credentials(&pool, user_id, true).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].credential_id, b"cred2");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_revoke_last_credential_fails(pool: PgPool) {
        let user_id = insert_user(&pool, None).await.unwrap();
        insert_credential(&pool, user_id, b"only-cred", b"pk",
            "0xcccc333333333333333333333333333333333333").await.unwrap();

        let err = revoke_credential(&pool, user_id, b"only-cred").await.unwrap_err();
        assert!(matches!(err, CredentialError::LastActiveCredential));
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_revoke_nonexistent_credential_fails(pool: PgPool) {
        let user_id = insert_user(&pool, None).await.unwrap();
        insert_credential(&pool, user_id, b"cred1", b"pk1",
            "0xaaaa111111111111111111111111111111111111").await.unwrap();
        insert_credential(&pool, user_id, b"cred2", b"pk2",
            "0xbbbb222222222222222222222222222222222222").await.unwrap();

        let result = revoke_credential(&pool, user_id, b"nonexistent").await;
        assert!(matches!(result.unwrap_err(), CredentialError::NotFound));
    }
}
```

- [ ] **Step 6.4: Run tests (expect pass)**

```bash
cd services
DATABASE_URL="postgres://postgres:postgres@localhost:5432/web3bank" \
  cargo test db::credentials::tests -- --nocapture
```

Expected: 6 tests pass.

- [ ] **Step 6.5: Commit**

```bash
git add services/user-service/src/db/credentials.rs
git commit -m "feat: implement credentials DB layer with revocation and tests"
```

---

## Chunk 3: gRPC Service + Server

### Task 7: gRPC Service Implementation

**Files:**
- Modify: `services/user-service/src/grpc/mod.rs`
- Replace: `services/user-service/src/grpc/user_service.rs`

Integration tests spin up a real tonic server on a random port and connect a real client. This requires a running Postgres (same `DATABASE_URL` as DB tests).

- [ ] **Step 7.1: Update grpc/mod.rs**

Replace `services/user-service/src/grpc/mod.rs`:

```rust
pub mod user_service;

tonic::include_proto!("user.v1");

pub use user_service_server::UserServiceServer;
```

- [ ] **Step 7.2: Write failing gRPC integration tests**

Create `services/user-service/src/grpc/user_service.rs` with `todo!()` stubs + tests:

```rust
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use once_cell::sync::Lazy;
use regex::Regex;
use sqlx::PgPool;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::db::{credentials, users};
use crate::grpc::{
    user_service_server::UserService,
    AddCredentialRequest, AddCredentialResponse,
    CreateUserRequest, CreateUserResponse,
    Credential,
    GetUserByAddressRequest, GetUserByAddressResponse,
    ListCredentialsRequest, ListCredentialsResponse,
    RevokeCredentialRequest, RevokeCredentialResponse,
    UpdateUserRequest, UpdateUserResponse,
};

static TEMPO_ADDR_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^0x[0-9a-fA-F]{40}$").expect("valid regex"));

pub struct UserServiceImpl {
    pub pool: PgPool,
}

#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn create_user(
        &self, request: Request<CreateUserRequest>,
    ) -> Result<Response<CreateUserResponse>, Status> { todo!() }

    async fn get_user_by_address(
        &self, request: Request<GetUserByAddressRequest>,
    ) -> Result<Response<GetUserByAddressResponse>, Status> { todo!() }

    async fn list_credentials(
        &self, request: Request<ListCredentialsRequest>,
    ) -> Result<Response<ListCredentialsResponse>, Status> { todo!() }

    async fn add_credential(
        &self, request: Request<AddCredentialRequest>,
    ) -> Result<Response<AddCredentialResponse>, Status> { todo!() }

    async fn update_user(
        &self, request: Request<UpdateUserRequest>,
    ) -> Result<Response<UpdateUserResponse>, Status> { todo!() }

    async fn revoke_credential(
        &self, request: Request<RevokeCredentialRequest>,
    ) -> Result<Response<RevokeCredentialResponse>, Status> { todo!() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grpc::{user_service_client::UserServiceClient, UserServiceServer};
    use std::net::TcpListener;
    use tonic::transport::Server;
    use tokio_stream::wrappers::TcpListenerStream;

    async fn start_test_server(pool: PgPool) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let svc = UserServiceServer::new(UserServiceImpl { pool });
        tokio::spawn(async move {
            Server::builder()
                .add_service(svc)
                .serve_with_incoming(TcpListenerStream::new(
                    tokio::net::TcpListener::from_std(listener).unwrap(),
                ))
                .await
                .unwrap();
        });
        format!("http://{addr}")
    }

    async fn client(addr: &str) -> UserServiceClient<tonic::transport::Channel> {
        UserServiceClient::connect(addr.to_string()).await.unwrap()
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_create_user_success(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let resp = client(&addr).await
            .create_user(CreateUserRequest {
                display_name: Some("Bob".to_string()),
                credential_id: b"cred-bytes".to_vec(),
                public_key: b"pk-bytes".to_vec(),
                tempo_address: "0xaaaa111111111111111111111111111111111111".to_string(),
            }).await.unwrap().into_inner();
        assert!(!resp.user_id.is_empty());
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_create_user_invalid_address(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let err = client(&addr).await
            .create_user(CreateUserRequest {
                display_name: None,
                credential_id: b"cred".to_vec(),
                public_key: b"pk".to_vec(),
                tempo_address: "not-an-address".to_string(),
            }).await.unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_create_user_duplicate_address(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let tempo_addr = "0xaaaa111111111111111111111111111111111111";

        client(&addr).await.create_user(CreateUserRequest {
            display_name: None, credential_id: b"cred1".to_vec(),
            public_key: b"pk1".to_vec(), tempo_address: tempo_addr.to_string(),
        }).await.unwrap();

        let err = client(&addr).await.create_user(CreateUserRequest {
            display_name: None, credential_id: b"cred2".to_vec(),
            public_key: b"pk2".to_vec(), tempo_address: tempo_addr.to_string(),
        }).await.unwrap_err();
        assert_eq!(err.code(), tonic::Code::AlreadyExists);
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_get_user_by_address_success(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let tempo_addr = "0xbbbb222222222222222222222222222222222222";

        client(&addr).await.create_user(CreateUserRequest {
            display_name: Some("Charlie".to_string()),
            credential_id: b"cred".to_vec(), public_key: b"pk".to_vec(),
            tempo_address: tempo_addr.to_string(),
        }).await.unwrap();

        let resp = client(&addr).await
            .get_user_by_address(GetUserByAddressRequest {
                tempo_address: tempo_addr.to_string(),
            }).await.unwrap().into_inner();
        assert_eq!(resp.display_name, "Charlie");
        assert_eq!(resp.status, "active");
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_get_user_by_address_not_found(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let err = client(&addr).await
            .get_user_by_address(GetUserByAddressRequest {
                tempo_address: "0xdddd444444444444444444444444444444444444".to_string(),
            }).await.unwrap_err();
        assert_eq!(err.code(), tonic::Code::NotFound);
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_revoke_last_credential_fails(pool: PgPool) {
        let addr = start_test_server(pool).await;
        let tempo_addr = "0xeeee555555555555555555555555555555555555";

        let create_resp = client(&addr).await.create_user(CreateUserRequest {
            display_name: None, credential_id: b"only-cred".to_vec(),
            public_key: b"pk".to_vec(), tempo_address: tempo_addr.to_string(),
        }).await.unwrap().into_inner();

        let err = client(&addr).await
            .revoke_credential(RevokeCredentialRequest {
                user_id: create_resp.user_id,
                credential_id: b"only-cred".to_vec(),
            }).await.unwrap_err();
        assert_eq!(err.code(), tonic::Code::FailedPrecondition);
    }

    #[sqlx::test(migrations = "src/db/migrations")]
    async fn test_update_user_display_name(pool: PgPool) {
        let addr = start_test_server(pool).await;

        let create_resp = client(&addr).await.create_user(CreateUserRequest {
            display_name: Some("Initial".to_string()),
            credential_id: b"cred".to_vec(), public_key: b"pk".to_vec(),
            tempo_address: "0xffff666666666666666666666666666666666666".to_string(),
        }).await.unwrap().into_inner();

        let update_resp = client(&addr).await.update_user(UpdateUserRequest {
            user_id: create_resp.user_id,
            display_name: Some("Updated".to_string()),
        }).await.unwrap().into_inner();
        assert_eq!(update_resp.display_name, "Updated");
    }
}
```

- [ ] **Step 7.3: Run tests to confirm they reach the handler (todo! panics)**

```bash
cd services
DATABASE_URL="postgres://postgres:postgres@localhost:5432/web3bank" \
  cargo test grpc::user_service::tests -- --nocapture 2>&1 | head -30
```

Expected: compiles successfully. Tests panic with "not yet implemented". This confirms the server spins up and routing works.

- [ ] **Step 7.4: Implement all 6 RPCs**

Replace the `#[tonic::async_trait] impl UserService for UserServiceImpl` block with:

```rust
#[tonic::async_trait]
impl UserService for UserServiceImpl {
    async fn create_user(
        &self, request: Request<CreateUserRequest>,
    ) -> Result<Response<CreateUserResponse>, Status> {
        let req = request.into_inner();
        if !TEMPO_ADDR_RE.is_match(&req.tempo_address) {
            return Err(Status::invalid_argument("invalid tempo_address format"));
        }

        let user_id = users::insert_user(&self.pool, req.display_name.as_deref())
            .await.map_err(|e| Status::internal(e.to_string()))?;

        credentials::insert_credential(
            &self.pool, user_id, &req.credential_id, &req.public_key, &req.tempo_address,
        )
        .await
        .map_err(|e| match &e {
            credentials::CredentialError::Db(db_err) => {
                if db_err.as_database_error()
                    .and_then(|d| d.code())
                    .map_or(false, |c| c == "23505")
                {
                    Status::already_exists("address or credential already registered")
                } else {
                    Status::internal(db_err.to_string())
                }
            }
            _ => Status::internal(e.to_string()),
        })?;

        Ok(Response::new(CreateUserResponse { user_id: user_id.to_string() }))
    }

    async fn get_user_by_address(
        &self, request: Request<GetUserByAddressRequest>,
    ) -> Result<Response<GetUserByAddressResponse>, Status> {
        let req = request.into_inner();
        let row = credentials::get_user_by_address(&self.pool, &req.tempo_address)
            .await.map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("user not found for address"))?;

        Ok(Response::new(GetUserByAddressResponse {
            user_id: row.user_id.to_string(),
            display_name: row.display_name,
            status: row.status,
            created_at: row.created_at.to_rfc3339(),
        }))
    }

    async fn list_credentials(
        &self, request: Request<ListCredentialsRequest>,
    ) -> Result<Response<ListCredentialsResponse>, Status> {
        let req = request.into_inner();
        let user_id: Uuid = req.user_id.parse()
            .map_err(|_| Status::invalid_argument("invalid user_id"))?;

        let rows = credentials::list_credentials(&self.pool, user_id, req.active_only)
            .await.map_err(|e| Status::internal(e.to_string()))?;

        let creds = rows.into_iter().map(|r| Credential {
            id: r.id.to_string(),
            credential_id: r.credential_id,
            tempo_address: r.tempo_address,
            created_at: r.created_at.to_rfc3339(),
            revoked_at: r.revoked_at.map(|t| t.to_rfc3339()),
        }).collect();

        Ok(Response::new(ListCredentialsResponse { credentials: creds }))
    }

    async fn add_credential(
        &self, request: Request<AddCredentialRequest>,
    ) -> Result<Response<AddCredentialResponse>, Status> {
        let req = request.into_inner();
        if !TEMPO_ADDR_RE.is_match(&req.tempo_address) {
            return Err(Status::invalid_argument("invalid tempo_address format"));
        }
        let user_id: Uuid = req.user_id.parse()
            .map_err(|_| Status::invalid_argument("invalid user_id"))?;

        users::get_user_by_id(&self.pool, user_id)
            .await.map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("user not found"))?;

        credentials::insert_credential(
            &self.pool, user_id, &req.credential_id, &req.public_key, &req.tempo_address,
        )
        .await
        .map_err(|e| match &e {
            credentials::CredentialError::Db(db_err) => {
                if db_err.as_database_error()
                    .and_then(|d| d.code())
                    .map_or(false, |c| c == "23505")
                {
                    Status::already_exists("address or credential already registered")
                } else {
                    Status::internal(db_err.to_string())
                }
            }
            _ => Status::internal(e.to_string()),
        })?;

        Ok(Response::new(AddCredentialResponse {
            credential_id: URL_SAFE_NO_PAD.encode(&req.credential_id),
        }))
    }

    async fn update_user(
        &self, request: Request<UpdateUserRequest>,
    ) -> Result<Response<UpdateUserResponse>, Status> {
        let req = request.into_inner();
        let user_id: Uuid = req.user_id.parse()
            .map_err(|_| Status::invalid_argument("invalid user_id"))?;

        if let Some(name) = &req.display_name {
            users::update_display_name(&self.pool, user_id, name)
                .await.map_err(|e| Status::internal(e.to_string()))?;
        }

        let row = users::get_user_by_id(&self.pool, user_id)
            .await.map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("user not found"))?;

        Ok(Response::new(UpdateUserResponse {
            user_id: row.id.to_string(),
            display_name: row.display_name,
            updated_at: row.updated_at.to_rfc3339(),
        }))
    }

    async fn revoke_credential(
        &self, request: Request<RevokeCredentialRequest>,
    ) -> Result<Response<RevokeCredentialResponse>, Status> {
        let req = request.into_inner();
        let user_id: Uuid = req.user_id.parse()
            .map_err(|_| Status::invalid_argument("invalid user_id"))?;

        users::get_user_by_id(&self.pool, user_id)
            .await.map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("user not found"))?;

        credentials::revoke_credential(&self.pool, user_id, &req.credential_id)
            .await
            .map_err(|e| match e {
                credentials::CredentialError::LastActiveCredential =>
                    Status::failed_precondition("cannot revoke last active credential"),
                credentials::CredentialError::NotFound =>
                    Status::not_found("credential not found"),
                credentials::CredentialError::Db(db_err) =>
                    Status::internal(db_err.to_string()),
            })?;

        Ok(Response::new(RevokeCredentialResponse {}))
    }
}
```

- [ ] **Step 7.5: Run all gRPC integration tests (expect pass)**

```bash
cd services
DATABASE_URL="postgres://postgres:postgres@localhost:5432/web3bank" \
  cargo test grpc::user_service::tests -- --nocapture
```

Expected: 7 tests pass.

- [ ] **Step 7.6: Commit**

```bash
git add services/user-service/src/grpc/
git commit -m "feat: implement all 6 gRPC RPCs with integration tests"
```

---

### Task 8: main.rs + sqlx Offline Mode

**Files:**
- Modify: `services/user-service/src/main.rs`
- Create: `services/user-service/.sqlx/` (generated by cargo sqlx prepare)

- [ ] **Step 8.1: Write main.rs**

Replace `services/user-service/src/main.rs`:

```rust
mod config;
mod db;
mod grpc;

use grpc::user_service::UserServiceImpl;
use grpc::UserServiceServer;
use tonic::transport::Server;
use tonic_health::ServingStatus;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::from_env().map_err(|e| {
        eprintln!("Configuration error: {e}");
        e
    })?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&config.log_level))
        .init();

    tracing::info!("Connecting to database...");
    let pool = db::create_pool(&config.database_url).await?;

    tracing::info!("Running migrations...");
    sqlx::migrate!("src/db/migrations").run(&pool).await?;

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    health_reporter
        .set_service_status("user.v1.UserService", ServingStatus::Serving)
        .await;

    tracing::info!(addr = %config.grpc_addr, "Starting gRPC server");
    Server::builder()
        .add_service(health_service)
        .add_service(UserServiceServer::new(UserServiceImpl { pool }))
        .serve(config.grpc_addr)
        .await?;

    Ok(())
}
```

- [ ] **Step 8.2: Verify compilation**

```bash
cd services
cargo build
```

Expected: compiles. Fix any import errors.

- [ ] **Step 8.3: Run all tests**

```bash
cd services
DATABASE_URL="postgres://postgres:postgres@localhost:5432/web3bank" cargo test
```

Expected: all tests pass (config: 2, db::users: 4, db::credentials: 6, grpc: 7 = 19 total).

- [ ] **Step 8.4: Generate sqlx offline query cache**

With Postgres running and accessible:

```bash
cd services/user-service
DATABASE_URL="postgres://postgres:postgres@localhost:5432/web3bank" \
  cargo sqlx prepare
```

This creates `services/user-service/.sqlx/` with cached query metadata. Without this, `SQLX_OFFLINE=true` builds will fail.

- [ ] **Step 8.5: Verify offline build**

```bash
cd services
SQLX_OFFLINE=true cargo build
```

Expected: compiles without a database connection.

- [ ] **Step 8.6: Commit**

```bash
git add services/user-service/src/main.rs services/user-service/.sqlx/
git commit -m "feat: add tokio main, tonic server, health check, sqlx offline cache"
```

---

## Chunk 4: Docker + Document Updates

### Task 9: Dockerfile

**Files:**
- Create: `services/user-service/Dockerfile`

- [ ] **Step 9.1: Write Dockerfile**

Create `services/user-service/Dockerfile`:

```dockerfile
# Build context must be repo root (needs packages/proto/ for build.rs)
ARG RUST_VERSION=1.77.0
FROM rust:${RUST_VERSION} AS builder
WORKDIR /app

# Install protoc (required by tonic-build)
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

# Proto files needed by build.rs
COPY packages/proto/ packages/proto/

# Copy lock file for reproducible builds, then service source
COPY services/Cargo.lock services/Cargo.lock
COPY services/user-service/ services/user-service/

# Override to a single-member workspace — avoids copying treasury/ into context
RUN printf '[workspace]\nmembers = ["user-service"]\nresolver = "2"\n' > services/Cargo.toml

# Build from workspace root; binary lands at services/target/release/user-service
WORKDIR /app/services
RUN SQLX_OFFLINE=true cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/services/target/release/user-service /usr/local/bin/
CMD ["user-service"]
```

- [ ] **Step 9.2: Build image from repo root**

```bash
# Run from repo root
docker build -f services/user-service/Dockerfile -t user-service:local .
```

Expected: image builds successfully. If it fails, check:
- `protoc` installed in builder layer (the `apt-get install protobuf-compiler` line)
- `.sqlx/` directory is present and committed
- `services/Cargo.lock` exists

- [ ] **Step 9.3: Commit**

```bash
git add services/user-service/Dockerfile
git commit -m "feat: add user-service Dockerfile"
```

---

### Task 10: Docker Compose

**Files:**
- Create: `services/docker-compose.yml`

- [ ] **Step 10.1: Write docker-compose.yml**

Create `services/docker-compose.yml`:

```yaml
# Run from repo root: docker compose -f services/docker-compose.yml up
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
      context: ..
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

- [ ] **Step 10.2: Start stack and verify health**

```bash
# From repo root
docker compose -f services/docker-compose.yml up -d
sleep 10
docker compose -f services/docker-compose.yml ps
```

Expected: `postgres` is `healthy`, `user-service` is `running`.

- [ ] **Step 10.3: Probe gRPC health**

Install `grpc_health_probe` if needed:
```bash
brew install grpc-health-probe   # macOS via Homebrew
# or: https://github.com/grpc-ecosystem/grpc-health-probe/releases
```

```bash
grpc_health_probe -addr=localhost:50051 -service=user.v1.UserService
```

Expected output: `status: SERVING`

- [ ] **Step 10.4: Tear down**

```bash
docker compose -f services/docker-compose.yml down
```

- [ ] **Step 10.5: Commit**

```bash
git add services/docker-compose.yml
git commit -m "feat: add shared docker-compose with postgres and user-service"
```

---

### Task 11: Update Architecture + Task Documents

**Files:**
- Modify: `architecture/services.md`
- Modify: `architecture/authentication.md`
- Modify: `tasks/07-user-service.md`
- Modify: `tasks/08-bff-graphql-proxy.md`
- Modify: `README.md`

- [ ] **Step 11.1: Update architecture/services.md**

In the **Inter-Service Communication** table, change:

```
| BFF → User Service | Internal HTTP API |
```
to:
```
| BFF → User Service | gRPC (port 50051) |
```

In the **User Service** section header table, change:

```
| **Runtime** | TBD (Rust or Bun) |
```
to:
```
| **Runtime** | Rust |
| **Protocol** | gRPC (tonic, port 50051) |
```

- [ ] **Step 11.2: Update architecture/authentication.md**

In the Registration sequence diagram, update the browser-to-BFF call:

```
Browser->>BFF: POST /api/users (Address, CredentialID)
```
to:
```
Browser->>BFF: POST /api/users (Address, CredentialID, PublicKey)
```

- [ ] **Step 11.3: Update tasks/07-user-service.md**

Replace with:

```markdown
# Task 07: User Service

**Service:** `user-service`
**Runtime:** Rust (tonic gRPC)
**Location:** `services/user-service/`
**Depends on:** None
**Can parallelize with:** Task 01, Task 02, Task 04, Task 05, Task 06, Task 08

## Goal

Implement the User Service that manages user profiles and passkey credential mappings.

## Spec

`docs/superpowers/specs/2026-03-17-user-service-design.md`

## Implementation Plan

`docs/superpowers/plans/2026-03-17-user-service.md`

## Acceptance Criteria

- `cargo build --workspace` from `services/` compiles without errors
- `services/Cargo.lock` committed
- All 6 gRPC RPCs work end-to-end against a real Postgres instance
- Duplicate address registration returns `ALREADY_EXISTS`
- Invalid address format returns `INVALID_ARGUMENT`
- `GetUserByAddress` for unknown address returns `NOT_FOUND`
- Revoking the last active credential returns `FAILED_PRECONDITION`
- `grpc_health_probe -addr=localhost:50051 -service=user.v1.UserService` returns `SERVING`
- BFF container reaches User Service on port 50051 via `services/docker-compose.yml`
```

- [ ] **Step 11.4: Update tasks/08-bff-graphql-proxy.md**

Change the `registerUser` mutation line from:
```
- `registerUser(address, credentialId)` — Proxies to User Service `POST /users`
```
to:
```
- `registerUser(address, credentialId, publicKey, displayName?)` — Proxies to User Service `CreateUser` gRPC
```

Change the BFF → User Service proxy description from HTTP to:
```
BFF calls User Service via gRPC using `@grpc/grpc-js`. Proto loaded from `packages/proto/user/v1/user_service.proto`.
```

- [ ] **Step 11.5: Update README.md**

In the Core Components table, change the User Service row from:
```
| **User Service** | User profiles, passkey credential-to-address mapping, account state. | TBD |
```
to:
```
| **User Service** (`services/user-service`) | User profiles, passkey credential-to-address mapping, account state. | Rust (tonic gRPC) |
```

- [ ] **Step 11.6: Commit all doc updates**

```bash
git add architecture/ tasks/ README.md
git commit -m "docs: update services, auth flow, and task docs for Rust gRPC user service"
```

---

## Final Verification

- [ ] **Run full test suite from scratch**

```bash
docker compose -f services/docker-compose.yml up -d postgres
sleep 5
cd services
DATABASE_URL="postgres://postgres:postgres@localhost:5432/web3bank" cargo test
```

Expected: all 18 tests pass.

- [ ] **Verify offline release build**

```bash
cd services
SQLX_OFFLINE=true cargo build --release
```

Expected: compiles without a database.

- [ ] **End-to-end compose health check**

```bash
# From repo root
docker compose -f services/docker-compose.yml up -d
sleep 10
grpc_health_probe -addr=localhost:50051 -service=user.v1.UserService
docker compose -f services/docker-compose.yml down
```

Expected: `status: SERVING`
