import { createSignal, createResource, Show, type Component } from 'solid-js';
import { parseAmount, formatUsd } from '~/lib/format';
import { showToast } from './Toast';
import { gql } from '~/lib/graphql';
import { env } from '~/config/env';
import { sumUnavailableBalanceWei } from '~/lib/withdrawal-routing';
import {
  RESOLVE_USERNAME_QUERY,
  RESOLVE_RECIPIENT_ROUTING_QUERY,
  WITHDRAWAL_ROUTING_QUERY,
  type ResolveUsernameResponse,
  type ResolveRecipientRoutingResponse,
  type WithdrawalRoutingResponse,
} from '~/queries/auth';

export type TransactionType = 'deposit' | 'withdraw' | 'send';

interface TransactionModalProps {
  type: TransactionType;
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (params: {
    amount: bigint;
    to?: `0x${string}`;
    destChainId: string;
  }) => Promise<`0x${string}`>;
}

const labels: Record<TransactionType, { title: string; cta: string; desc: string }> = {
  deposit: {
    title: 'Deposit',
    cta: 'Confirm Deposit',
    desc: 'Deposit USDC into your Web3Bank account',
  },
  withdraw: {
    title: 'Withdraw',
    cta: 'Confirm Withdrawal',
    desc: 'Withdraw USDC from your Web3Bank account',
  },
  send: {
    title: 'Send',
    cta: 'Confirm Transfer',
    desc: 'Send funds to another user',
  },
};

const TransactionModal: Component<TransactionModalProps> = (props) => {
  const [amount, setAmount] = createSignal('');
  const [recipient, setRecipient] = createSignal('');
  const [isPending, setIsPending] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const needsRecipient = () => props.type === 'send';
  const isWithdraw = () => props.type === 'withdraw';

  const [withdrawalRouting] = createResource(
    () => (props.isOpen && isWithdraw() ? true : null),
    async () => {
      const data = await gql<WithdrawalRoutingResponse>(WITHDRAWAL_ROUTING_QUERY);
      return data.withdrawalRouting;
    },
  );

  const unavailableBalanceWei = () => {
    const entries = withdrawalRouting();
    if (!entries) return 0n;
    return sumUnavailableBalanceWei(entries);
  };

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    setError(null);
    setIsPending(true);

    try {
      const parsedAmount = parseAmount(amount());

      let to: `0x${string}` | undefined;
      let destChainId = String(env.tempoChainId);
      if (needsRecipient()) {
        const raw = recipient().trim();
        if (raw.startsWith('0x')) {
          const addr = raw as `0x${string}`;
          const routing = await gql<ResolveRecipientRoutingResponse>(
            RESOLVE_RECIPIENT_ROUTING_QUERY,
            { tempoAddress: addr }
          );
          to = routing.resolveRecipientRouting.tempoAddress as `0x${string}`;
          destChainId = routing.resolveRecipientRouting.destChainId;
        } else {
          // Strip leading @ if present, then resolve username → address.
          const uname = raw.startsWith('@') ? raw.slice(1) : raw;
          const data = await gql<ResolveUsernameResponse>(RESOLVE_USERNAME_QUERY, { username: uname });
          to = data.resolveUsername.tempoAddress as `0x${string}`;
          destChainId =
            data.resolveUsername.destChainId != null && data.resolveUsername.destChainId !== ''
              ? String(data.resolveUsername.destChainId)
              : String(env.tempoChainId);
        }
      }

      const txHash = await props.onSubmit({
        amount: parsedAmount,
        to,
        destChainId: needsRecipient() ? destChainId : String(env.tempoChainId),
      });

      showToast({
        type: 'success',
        message: `${labels[props.type].title} confirmed`,
        txHash,
      });

      setAmount('');
      setRecipient('');
      props.onClose();
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Transaction failed';
      setError(msg);
      showToast({ type: 'error', message: msg });
    } finally {
      setIsPending(false);
    }
  };

  const canSubmit = () => {
    if (!amount() || isPending()) return false;
    if (needsRecipient() && !recipient()) return false;
    return true;
  };

  return (
    <Show when={props.isOpen}>
      {/* Backdrop */}
      <div
        class="fixed inset-0 z-40 backdrop-blur-sm"
        onClick={props.onClose}
        style={{ animation: 'fadeIn 0.2s ease-out', "background-color": "var(--backdrop-overlay)" }}
      />

      {/* Modal */}
      <div class="fixed inset-0 z-50 flex items-center justify-center p-4 pointer-events-none">
        <div
          class="pointer-events-auto bg-surface border border-edge rounded-2xl w-full max-w-md"
          style={{ animation: 'fadeIn 0.3s ease-out', "box-shadow": "var(--shadow-float)" }}
        >
          {/* Header */}
          <div class="flex items-center justify-between p-6 pb-2">
            <div>
              <h2 class="text-xl font-bold text-text font-[Satoshi]">
                {labels[props.type].title}
              </h2>
              <p class="text-sm text-muted mt-1">
                {labels[props.type].desc}
              </p>
            </div>
            <button
              onClick={props.onClose}
              class="text-subtle hover:text-muted transition-colors p-1"
            >
              <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>

          {/* Form */}
          <form onSubmit={handleSubmit} class="p-6 pt-4 flex flex-col gap-4">
            {/* Amount */}
            <div>
              <label class="block text-sm font-medium text-muted mb-2">
                Amount (USD)
              </label>
              <div class="relative">
                <span class="absolute left-4 top-1/2 -translate-y-1/2 text-subtle text-lg font-semibold">
                  $
                </span>
                <input
                  type="text"
                  inputMode="decimal"
                  placeholder="0.00"
                  value={amount()}
                  onInput={(e) => setAmount(e.currentTarget.value)}
                  disabled={isPending()}
                  class="w-full bg-raised border border-edge rounded-xl pl-9 pr-4 py-3.5 text-text text-lg font-semibold placeholder-faint focus:outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/20 transition-all disabled:opacity-50"
                />
              </div>
            </div>

            {/* Temporarily unavailable balance (withdraw only) */}
            <Show when={isWithdraw() && unavailableBalanceWei() > 0n}>
              <div class="bg-info/10 border border-info/20 rounded-lg p-3">
                <p class="text-info text-sm">
                  {formatUsd(unavailableBalanceWei())} temporarily unavailable — held on a chain
                  that cannot process withdrawals right now. Funds are safe and will become
                  withdrawable when the chain recovers or is decommissioned.
                </p>
              </div>
            </Show>

            {/* Recipient */}
            <Show when={needsRecipient()}>
              <div>
                <label class="block text-sm font-medium text-muted mb-2">
                  Username
                </label>
                <input
                  type="text"
                  placeholder="@username"
                  value={recipient()}
                  onInput={(e) => setRecipient(e.currentTarget.value)}
                  disabled={isPending()}
                  class="w-full bg-raised border border-edge rounded-xl px-4 py-3 text-text text-sm placeholder-faint focus:outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/20 transition-all disabled:opacity-50"
                />
              </div>
            </Show>

            {/* Error */}
            <Show when={error()}>
              <div class="bg-error/10 border border-error/20 rounded-lg p-3">
                <p class="text-error text-sm">{error()}</p>
              </div>
            </Show>

            {/* Submit */}
            <button
              type="submit"
              disabled={!canSubmit()}
              class="w-full bg-accent hover:bg-accent-hover active:scale-[0.98] text-accent-fg font-semibold py-3.5 px-4 rounded-xl transition-all disabled:opacity-40 disabled:cursor-not-allowed flex items-center justify-center gap-2 mt-2"
            >
              <Show
                when={!isPending()}
                fallback={
                  <span class="flex items-center gap-2">
                    <svg class="w-4 h-4 animate-spin" viewBox="0 0 24 24" fill="none">
                      <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" stroke-dasharray="60" stroke-linecap="round" />
                    </svg>
                    Waiting for passkey...
                  </span>
                }
              >
                <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
                </svg>
                {labels[props.type].cta}
              </Show>
            </button>
          </form>
        </div>
      </div>
    </Show>
  );
};

export default TransactionModal;
