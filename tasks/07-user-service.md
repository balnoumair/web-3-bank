# Task 07: User Service

**Service:** `user-service`
**Depends on:** None
**Can parallelize with:** Task 01, Task 02, Task 04, Task 05, Task 06, Task 08

## Goal

Implement the User Service that manages user profiles and passkey credential mappings.

## Scope

### Project Setup
- Initialize service project (runtime TBD — Rust or Bun)
- PostgreSQL schema (`users`)
- Docker Compose integration (shared Postgres instance with Treasury)

### Data Model
- `users` table: id, display_name, created_at, updated_at, status (active/suspended)
- `credentials` table: id, user_id, credential_id (WebAuthn), public_key, tempo_address, created_at
- One user can have multiple credentials (multi-device support)

### API Endpoints (Internal HTTP)
- `POST /users` — Create user with initial credential
- `GET /users/:address` — Fetch user by Tempo address
- `GET /users/:id/credentials` — List credentials for a user
- `POST /users/:id/credentials` — Add a new credential (new device)
- `PATCH /users/:id` — Update profile (display name, preferences)

### Validation
- Tempo address format validation
- Credential ID uniqueness
- Prevent duplicate address registration

## Acceptance Criteria
- Service starts and connects to PostgreSQL
- All CRUD endpoints work correctly
- Duplicate address registration is rejected
- Health check endpoint responds
