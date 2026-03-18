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
