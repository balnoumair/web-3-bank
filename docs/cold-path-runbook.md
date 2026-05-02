# Cold Path Rebalance Runbook

## Pause cold path

Use the existing Bank `pause()` mechanism from the emergency pauser address on any chain that should stop accepting state-changing Bank operations. While paused, `rebalance()` and `ccipReceive()` both revert, alongside deposits, withdrawals, and hot-path transfer/release calls.

If Treasury should stop submitting new cold-path transactions without pausing user-facing Bank operations, disable the Treasury cold-path worker or remove the affected chain from the active set in `RouteReceiver.sol`.

## Adjust max rebalance amount

Governance updates the per-chain cap with:

```text
Bank.setMaxRebalanceAmount(amount)
```

The launch default is 5% of total SyncUSD supply per chain. Treasury reads `maxRebalanceAmount()` from the source Bank before every submission and splits larger planned rebalances into sequential cap-sized operations. A zero cap disables cold-path submissions from that source chain.

## Configure CCIP allowlists

Governance must configure each Bank pair before cold-path traffic can flow:

```text
Bank.setCcipRouter(ccipRouter)
Bank.setAllowlistedDestChain(destChainId, true)
Bank.setAllowlistedSourceContract(sourceChainId, sourceBank, true)
```

`rebalance()` rejects non-allowlisted destinations. `ccipReceive()` only accepts calls from the configured CCIP router, rejects non-allowlisted source chain and source Bank pairs, and rejects replayed `messageId` values.

## Investigate a stuck CCIP message

1. Find the `treasury.rebalance_ops` row by `ccip_message_id` or Treasury correlation id.
2. Confirm the source-chain transaction emitted `RebalanceInitiated(messageId, destChainId, amount)`.
3. Check Chainlink CCIP explorer/status for the same `messageId`.
4. On the destination Bank, verify whether `processedMessages(messageId)` is already true and whether `RebalanceCompleted(messageId, sourceChainId, amount)` was emitted.
5. If no destination completion appears after 30 minutes, mark the operation `failed`, alert operators, and re-plan a new rebalance only after confirming the original message cannot still mint.
