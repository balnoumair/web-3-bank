# Skills Map

This document defines:
- which skills are available globally (installed externally)
- which skills are recommended per role
- which repo-specific skills must be used as architectural gates

Global skills are referenced by **name** and are assumed to be installed
via the environment (e.g. `npx skills add ...`).

Repo skills are referenced by **relative path** and are committed to this repository.

---

## Global skills (inventory)

### Rust
- memory-safety-patterns
- rust-async-patterns

### Frontend
- astro
- solidjs-patterns
- tailwind-patterns

### Backend
- backend-development

### Smart contracts
- solidity-security
- web3-testing

### Tooling / monorepo
- turborepo

### Architecture & process
- architecture-patterns

### Agent workflow
- skill-creator

---

## Role-based skill recommendations

These sections indicate **which skills an agent should actively use**
for a given role.  
They do **not** imply exclusive loading; they define focus and priority.

---

### Frontend Agent (Astro + SolidJS)

**Global skills**
- astro
- solidjs-patterns
- tailwind-patterns
- testing-best-practices

**Repo skills**
- skills/passkeys-webauthn-aa-integration/SKILL.md
- skills/invariants-and-trust-boundaries/SKILL.md

---

### BFF Agent (Bun + GraphQL)

**Global skills**
- architecture-patterns
- backend-development

**Repo skills**
- skills/ddd-boundaries-and-fp-shape/SKILL.md
- skills/passkeys-webauthn-aa-integration/SKILL.md
- skills/invariants-and-trust-boundaries/SKILL.md

---

### Backend Agent (Rust + gRPC)

**Global skills**
- memory-safety-patterns
- rust-async-patterns
- architecture-patterns
- backend-development

**Repo skills**
- skills/event-indexing-and-projections/SKILL.md
- skills/domain-fintech/SKILL.md
- skills/ddd-boundaries-and-fp-shape/SKILL.md
- skills/passkeys-webauthn-aa-integration/SKILL.md
- skills/invariants-and-trust-boundaries/SKILL.md

---

### Smart Contract Agent (Solidity)

**Global skills**
- solidity-security
- web3-testing

**Repo skills**
- skills/vault-policy-model-and-api/SKILL.md
- skills/invariants-and-trust-boundaries/SKILL.md

---

## Usage notes

- Repo skills act as **architectural gates** and must be consulted
  before implementing or approving changes in their scope.
- Global skills provide guidance and patterns but must not override
  repo-specific invariants or architecture.
- If a conflict exists:
  **repo skills > architecture docs > global skills**.

---

## Evolution

- Global skills may be added or removed without changing repo semantics.
- Repo skills should be extended (not rewritten) as the system evolves.
- Version-specific extensions (v2+) should be documented inside the
  relevant repo skill files.
