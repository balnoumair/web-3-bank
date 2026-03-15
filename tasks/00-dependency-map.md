# Task Dependency Map

## Parallelization Guide

Each task is tagged with the service it touches. Tasks on different services can always run in parallel. Tasks on the same service can only run in parallel if they don't depend on each other.

## Phases

```
Phase 1 (fully parallel — no dependencies):
├── Task 01: SyncUSD Token Contract          [contracts]
├── Task 04: Treasury Service Scaffold       [treasury-service]
└── Task 07: User Service                    [user-service]

Phase 2 (after their deps complete):
├── Task 02: Bank Contract                   [contracts]        ← needs 01
├── Task 05: Treasury Hot Path Relay         [treasury-service] ← needs 04, 02
├── Task 06: Treasury Watcher                [treasury-service] ← needs 04, 02
└── Task 08: BFF GraphQL Proxy              [bff]              ← needs 07

Phase 3 (after phase 2):
├── Task 03: Contract Deployment Scripts     [contracts]        ← needs 01, 02
├── Task 09: Frontend Rebuild wagmi/viem     [frontend]         ← needs 08, 01
└── Task 10: Treasury Cold Path Rebalancing  [treasury-service] ← needs 04, 03

Phase 4 (everything wired together):
└── Task 11: E2E Testnet Integration         [all services]     ← needs everything
```

## Visual Dependency Graph

```
          01 SyncUSD ──────────┬──────────────────────────┐
          [contracts]          │                          │
               │               │                          │
               ▼               │                          │
          02 Bank Contract     │                          │
          [contracts]          │                          │
               │               │                          │
          ┌────┼───────────────┤                          │
          │    │               │                          │
          ▼    ▼               ▼                          ▼
    05 Hot Path  06 Watcher   03 Deploy Scripts    09 Frontend
    [treasury]   [treasury]   [contracts]          [frontend]
          │         │              │                    │
          │         │              ▼                    │
          │         │         10 Cold Path              │
          │         │         [treasury]                │
          │         │              │                    │
          └─────────┴──────────────┴────────────────────┘
                                  │
                                  ▼
                         11 E2E Integration
                           [all services]

   04 Treasury Scaffold ──► 05, 06, 10 (treasury tasks need scaffold first)
   07 User Service ────────► 08 BFF (BFF needs something to proxy to)
   08 BFF ─────────────────► 09 Frontend (frontend needs BFF API)
```

## Maximum Parallelism Per Phase

| Phase | Tasks Running | Services Involved |
|-------|--------------|-------------------|
| 1 | 3 | contracts, treasury-service, user-service |
| 2 | 4 | contracts, treasury-service (x2), bff |
| 3 | 3 | contracts, frontend, treasury-service |
| 4 | 1 | all |
