# Vault + Policy Model and Contract API

## When to use
Use this skill when defining or changing:
- Vault contract functions/events
- Policy schema and enforcement logic
- Internal vs external transfers
- Withdrawal and revocation flows

## Goal
Keep the vault minimal, auditable, and policy-driven while preserving exit guarantees.

## Inputs
- Token(s): v1 = USDC
- Policy parameters: maxPerTx, maxPerWindow, windowSeconds, expiry, allowlists
- Transfer types: internal (vault ledger) vs external (ERC-20 transfer)

## Procedure
1) Define minimal state
- Per-user delegated balance
- Per-policy limits and window counters (spent, lastReset)
- Optional allowlist pointers (if used)

2) Define minimal API surface
- deposit(user, token, amount)
- createPolicy(params) → policyId
- revokePolicy(policyId)
- execute(policyId, to, amount, kind=Internal|External)
- withdraw(amount) / withdrawTo(to, amount)
- withdrawAll() (emergency exit)

3) Enforce policy on-chain
- Token match
- Not expired
- Amount <= maxPerTx
- spentInWindow + amount <= maxPerWindow
- Sliding-window reset logic (based on lastReset + windowSeconds)
- Recipient checks (if configured)

4) Emit events for indexing
- Deposit, Withdrawal
- PolicyCreated, PolicyRevoked
- InternalTransfer, ExternalTransfer

## Outputs
- Contract ABI sketch (functions + events)
- Event list used by backend indexer

## Constraints
- Emergency exit must not be blockable by backend.
- Avoid upgradeability in v1 unless you add a timelock + user-escape strategy.
- Avoid storing “history”; events are the history.

