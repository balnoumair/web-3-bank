# Invariants and Trust Boundaries

## When to use
Use this skill whenever you propose, review, or implement:
- any fund-moving flow (deposit, pay, withdraw)
- any change to vault/policy behavior
- any new backend capability touching blockchain or balances
- any BFF feature that might drift into “domain logic”

## Goal
Ensure all designs and changes preserve the project’s non-negotiable invariants and trust boundaries.

## Inputs
- Proposed feature/change description (or PR diff summary)
- The relevant flow(s): login, enable policy, internal payment, external payment, withdrawal
- Components involved: frontend, BFF, backend, contracts

## Checklist (must pass)

### Authority & custody
- Backend alone cannot move funds.
- Any funds movement requires:
  - an on-chain policy that permits it, and/or
  - a user-signed on-chain authorization.
- BFF never becomes an authority layer.

### Exit guarantees
- Users can always:
  - revoke policies, and
  - withdraw delegated funds,
  without backend availability.

### Truth & consistency
- On-chain events are authoritative.
- Never mark actions “final” based on submission; only on observation.
- Projections are rebuildable from chain events.

### Delegation
- Delegation is explicit, bounded, and revocable.
- Limits are enforced on-chain (not only in backend).

## Outputs
- “Pass” with reasons, or “Fail” with the invariant(s) violated
- Concrete mitigation suggestions if failing

## Constraints
- Do not approve any design that introduces backend-only signing for fund movement.
- Do not add “admin bypass” paths that can block user exits.

