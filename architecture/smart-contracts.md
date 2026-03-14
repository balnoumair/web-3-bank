# Smart Contracts Architecture

> **Status:** 🚧 WIP — Core architecture defined for implementation.

## 1. SyncUSD (The Stablecoin Token)

`SyncUSD` is a multi-chain, 1:1 backed stablecoin designed for native cross-chain settlement via Chainlink CCIP's Burn-and-Mint mechanism.

### Implementation per Chain
- **Tempo Blockchain (Primary):** Deployed as a **TIP-20** token. 
  - *Benefits:* Integrates natively with Tempo's payment infrastructure, meaning we get guaranteed blockspace (payment lanes), ability to pay gas in stablecoins (fee sponsorship via EIP-2718), native reconciliation memos, and TIP-403 compliance policy features out of the box.
- **Other EVM Chains (Base, Arbitrum):** Deployed as a standard **ERC-20** token with Chainlink CCIP `BurnMintERC20` extensions.

### Core Token Functions
- `mint(address to, uint256 amount)`: Restricted. Only the Bank Contract and the CCIP Token Pool can call this.
- `burn(uint256 amount)`: Restricted. Only the Bank Contract and the CCIP Token Pool can call this.
- `transfer`, `transferFrom`: Standard ERC-20 / TIP-20 behavior.

### CCIP Integration
The token uses Chainlink CCIP's **Burn-and-Mint** pattern for cross-chain routing. When the Treasury Service executes a rebalance:
1. CCIP burns `SyncUSD` on the source chain (e.g., Tempo).
2. The CCIP network transmits the proof of burn to the destination chain.
3. CCIP mints the equivalent amount of `SyncUSD` on the destination chain (e.g., Base).
This guarantees the total circulating supply across all chains remains constant and perfectly reflects the underlying USDC reserves locked in the Bank Contracts.

## 2. The Bank Contract (Liquidity Pool)

The Bank Contract acts as a non-custodial gateway and liquidity provider for `SyncUSD`. There is one Bank Contract deployed on every active chain.

### Core Responsibilities
1. **On-Ramp/Off-Ramp:** Holds the external stablecoin reserves (e.g., USDC, USDT) that back `SyncUSD`.
2. **Liquidity Pool:** Manages the local supply of `SyncUSD` to facilitate instantaneous cross-chain user transfers (the "Hot Path").

### Key Methods

#### `deposit(address underlyingToken, uint256 amount)`
- **Action:** User sends `USDC` to the Bank Contract.
- **Execution:** The contract escrows the `USDC` in its reserve. It dynamically mints the exact `amount` of `SyncUSD` to the user's address on that local chain.

#### `withdraw(address underlyingToken, uint256 amount)`
- **Action:** User redeems `SyncUSD` for the underlying asset.
- **Execution:** The user burns their `SyncUSD`. The contract releases the equivalent `amount` of `USDC` from its reserve pool back to the user's wallet. This guarantees the non-custodial nature of the bank.

#### `transferHotPath(address to, uint256 amount, uint256 destinationChainId)`
- **Action:** Used strictly for instantaneous cross-chain transfers between users.
- **Execution:** 
  1. The Bank Contract pulls `amount` of `SyncUSD` from the sender's wallet in the source chain and locks it in its local liquidity pool.
  2. The BFF instantly detects this event in real-time.
  3. The BFF communicates to the destination chain's Bank Contract.
  4. The destination Bank Contract releases `amount` of `SyncUSD` from its own liquidity pool to the receiver's wallet.

*(Note: If Both users are on the same chain, they just execute a standard `transfer()` of the `SyncUSD` token. No Bank Contract interaction is required).*

---
*Last updated: March 2026*
