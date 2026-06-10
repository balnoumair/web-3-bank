# Banking Ledger — Delta

## MODIFIED Requirements

### Requirement: Withdraw burns SyncUSD and returns underlying

When a user withdraws, the Bank Contract SHALL burn the user's `SyncUSD` and release an equal amount of the underlying stablecoin from its reserve to the user's wallet. Withdrawal SHALL execute on the chain where the user's `SyncUSD` is held; it SHALL NOT be fulfilled custodially from another chain's pool or reserve. When the user holds `SyncUSD` on multiple chains, withdrawal MAY be executed on any active chain where the user holds balance, up to that chain's balance and reserve depth.

#### Scenario: Bob withdraws $2,000

- **WHEN** Bob calls `withdraw(USDC, 2000)` on a Bank Contract
- **THEN** 2,000 `SyncUSD` SHALL be burned from Bob
- **AND** 2,000 USDC SHALL be released from the reserve to Bob's wallet
- **AND** a `Withdrawn` event SHALL be emitted

#### Scenario: Balance on an inactive chain is reported unavailable, not moved

- **WHEN** Bob's only `SyncUSD` balance is on a chain that is inactive in RouteReceiver
- **THEN** no service SHALL move or release Bob's funds from another chain's pool or reserve
- **AND** the system SHALL report that amount as temporarily unavailable for withdrawal, with the reason
- **AND** the funds SHALL become withdrawable when the chain recovers or after a decommission drain relocates them
