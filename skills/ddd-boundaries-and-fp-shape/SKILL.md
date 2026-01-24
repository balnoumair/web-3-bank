# DDD Boundaries and FP Shape (BFF + Backend)

## When to use
Use this skill when designing modules, APIs, or refactors across:
- BFF GraphQL layer
- Rust backend gRPC + domain
- Shared models (careful!)

## Goal
Keep a clear DDD architecture with a functional-leaning shape:
- pure domain core
- explicit effect boundaries
- no duplicated domain model in the BFF

## Inputs
- Feature/change description
- Proposed schemas: GraphQL types, gRPC protos
- Modules/crates touched

## Rules
1) Backend owns invariants
- Limits, policy evaluation, payment lifecycle rules belong to backend domain and/or on-chain.

2) BFF is orchestration
- BFF can compose calls and shape responses.
- BFF must not re-implement policy logic or ledger invariants.

3) FP-leaning layering
- Domain: pure functions and types (no RPC/DB/chain clients)
- Application: orchestrates domain + effects
- Adapters: gRPC/DB/chain implementations

4) Avoid type leakage
- Do not expose DB structs or RPC DTOs as domain types.
- Define domain types and map at boundaries.

## Outputs
- A boundary decision: which layer owns each rule
- A module layout suggestion (crates/modules)
- A mapping plan (GraphQL ↔ commands/queries ↔ gRPC ↔ domain)

## Constraints
- If a rule changes funds movement, it must be enforced on-chain and/or backend domain.
- Do not let “convenience” push invariants into GraphQL resolvers.

