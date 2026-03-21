# Architecture TODO — Treasury & User-Service

Remaining work from the Rust Best Practices + Clean DDD/Hexagonal assessment.
The foundation (repository ports/adapters, status enums, eth module split, Clippy config) is complete.

---

## 1. Enrich Domain Models (Both Services)

**Problem:** Entities are anemic data structs — all business logic lives in application modules.

### Treasury

Move these rules **out of** `hot_path.rs` / `cold_path.rs` / `watcher.rs` and **into** domain types or domain services:

- [ ] **Relay eligibility** (`hot_path.rs:258-321`): "is destination chain active?" + "does pool have sufficient depth?" → `domain/relay.rs` with a `RelayDecision` enum (`Approved`, `RejectedInactiveChain`, `RejectedInsufficientDepth`)
- [ ] **Verification matching** (`watcher.rs:281-294`): "does released event match initiated event?" → method on `ReleasedEvent` or a `domain/verification.rs` service returning `AlertType`
- [ ] **Rebalance orchestration** (`cold_path.rs:350-420`): the "should we rebalance?" decision logic → keep `compute_rebalance_ops()` (already good), but extract the per-op submission decision into domain

### User-Service

- [ ] **Invariant enforcement on construction**: `User::new()` and `Credential::new()` constructors that validate inputs, rather than constructing bare structs
- [ ] **Aggregate root enforcement**: credential operations should go through `User` methods (e.g., `user.add_credential(...)`, `user.deactivate_credential(...)`) instead of calling `CredentialRepository` directly

---

## 2. Newtype Wrappers (Both Services)

**Problem:** Raw primitives (`u64`, `String`, `Uuid`) used for domain concepts — easy to mix up parameters.

### Treasury

- [ ] `ChainId(u64)` — used in config, repository traits, event structs, RPC calls
- [ ] `OperationId(String)` — rebalance op ID
- [ ] `TxHash(String)` — source/dest transaction hashes
- [ ] `EventHash(String)` — source event hash (relay idempotency key)

### User-Service

- [ ] `TempoAddress(String)` — with validation baked into `TryFrom<String>` (regex already exists in `domain/validation.rs`)

---

## 3. Integration Tests (Treasury)

**Problem:** Only `domain/rebalance.rs` and `eth/encoding.rs` have unit tests. No coverage for the main modules.

- [ ] **Mock RPC endpoints** with `wiremock` for `hot_path`, `cold_path`, `watcher`, `pool_manager`
- [ ] **DB integration tests** with `#[sqlx::test]` for all 4 repository adapters (`PgRelayRepository`, `PgRebalanceRepository`, `PgWatcherRepository`, `PgPoolSnapshotRepository`)
- [ ] **gRPC handler tests** — user-service already has these as a reference pattern

### Suggested test file structure:
```
services/treasury/src/db/relay_repo.rs        → add #[cfg(test)] mod tests
services/treasury/src/db/rebalance_repo.rs    → add #[cfg(test)] mod tests
services/treasury/src/db/watcher_repo.rs      → add #[cfg(test)] mod tests
services/treasury/src/db/pool_snapshot_repo.rs → add #[cfg(test)] mod tests
```

---

## 4. Documentation (Both Services)

**Problem:** Minimal doc comments on public APIs, most modules lack `//!` headers.

- [ ] Add `//!` module-level docs to: `hot_path`, `cold_path`, `watcher`, `pool_manager`, `server`, `db/*` (treasury); `db/*`, `grpc/*` (user-service)
- [ ] Add `///` doc comments to all public types and methods in `domain/` for both services
- [ ] Add `///` doc comments to repository trait methods explaining business invariants, not just "inserts a row"

---

## 5. Error Hierarchy Unification (User-Service)

**Problem:** `DomainError` and `CredentialError` are separate hierarchies with no `From` conversion.

- [ ] Either add `From<CredentialError> for DomainError` or merge into a single error enum
- [ ] Audit that all error paths surface meaningful context (not just `Infrastructure(e.to_string())`)

---

## Priority Order

| Priority | Item | Impact | Effort |
|----------|------|--------|--------|
| 1 | Enrich domain models | High — moves business rules to testable, infra-free code | Medium |
| 2 | Integration tests (treasury) | High — currently zero coverage on main modules | Medium |
| 3 | Newtype wrappers | Medium — prevents parameter mix-ups at compile time | Low-Medium |
| 4 | Error hierarchy unification | Low-Medium — cleaner error handling | Low |
| 5 | Documentation | Low — improves onboarding but code is fairly readable | Low |
