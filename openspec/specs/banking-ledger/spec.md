# Banking Ledger Specification

## Purpose

Represent user balances as on-chain `SyncUSD` tokens, 1:1 backed by externally-held stablecoins (USDC) escrowed in Bank Contracts. The user's balance **is** their token balance — there is no off-chain ledger.

## Requirements

### Requirement: SyncUSD as the unit of account

User balances SHALL be denominated in `SyncUSD`, a stablecoin issued by the bank and backed 1:1 by USDC held in reserve by the Bank Contract on each active chain. The total `SyncUSD` supply across all chains SHALL equal the total underlying USDC reserves.

### Requirement: One Bank Contract per active chain

Each active chain SHALL host exactly one Bank Contract. The Bank Contract SHALL hold the chain's underlying stablecoin reserve and SHALL manage the chain's local `SyncUSD` liquidity pool.

### Requirement: Deposit mints SyncUSD against escrowed USDC

When a user deposits an underlying stablecoin into a Bank Contract, the contract SHALL escrow the underlying token in its reserve and SHALL mint an equal amount of `SyncUSD` to the user's address on the same chain.

#### Scenario: Bob deposits $5,000 USDC

- **WHEN** Bob calls `deposit(USDC, 5000)` on the Tempo Bank Contract
- **THEN** 5,000 USDC SHALL be transferred from Bob to the contract's reserve
- **AND** 5,000 `SyncUSD` SHALL be minted to Bob's Tempo address
- **AND** a `Deposited` event SHALL be emitted

### Requirement: Withdraw burns SyncUSD and returns underlying

When a user withdraws, the Bank Contract SHALL burn the user's `SyncUSD` and release an equal amount of the underlying stablecoin from its reserve to the user's wallet.

#### Scenario: Bob withdraws $2,000

- **WHEN** Bob calls `withdraw(USDC, 2000)` on a Bank Contract
- **THEN** 2,000 `SyncUSD` SHALL be burned from Bob
- **AND** 2,000 USDC SHALL be released from the reserve to Bob's wallet
- **AND** a `Withdrawn` event SHALL be emitted

### Requirement: Same-chain transfer is a plain token transfer

A transfer between two users on the same chain SHALL execute as a standard ERC-20 `transfer()` of `SyncUSD` between their addresses. The Bank Contract SHALL NOT be involved.

### Requirement: Mint and burn are restricted

`SyncUSD.mint` and `SyncUSD.burn` SHALL be callable only by the Bank Contract on that chain and the CCIP Token Pool. All other callers SHALL be rejected.
