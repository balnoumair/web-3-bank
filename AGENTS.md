# AGENTS.md

## Purpose
This file is the entry point for all agents working on this repository.
Agents must read this file first and treat referenced documents as canonical.

---

## Required reading (always)
Before performing any task, you MUST read:
- docs/00_OVERVIEW.md
- docs/01_ARCHITECTURE_V1.md

---

## Skills
For recommended skills (global + repo-specific) and which roles should use them, read:
- docs/07_SKILLS_MAP.md

---

## Role-based Context Routing

### Frontend Agent
Read:
- docs/02_FRONTEND_CONTEXT.md
- docs/06_EVENTS_AND_LEDGER.md

### BFF Agent
Read:
- docs/03_BFF_CONTEXT.md
- docs/06_EVENTS_AND_LEDGER.md

### Backend Agent
Read:
- docs/04_BACKEND_CONTEXT.md
- docs/06_EVENTS_AND_LEDGER.md

### Smart Contract Agent
Read:
- docs/05_BLOCKCHAIN_CONTEXT.md
- docs/06_EVENTS_AND_LEDGER.md

---

## Architecture Principles
- DDD-first: invariants live in the backend domain
- FP-leaning: pure core, effects at the edges
- Backend orchestrates, never owns funds
- On-chain state is authoritative

---

## Invariants (must never be broken)
- Backend alone cannot move funds
- Users can always exit without backend (withdraw/revoke)
- Delegation is explicit, bounded, and revocable
- Never trust submission; only trust on-chain observation

---

## Versioning
v1 is defined by docs/01_ARCHITECTURE_V1.md  
Future versions go in docs/99_FUTURE_V2.md
