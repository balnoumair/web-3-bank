import { createSignal, Show, type Component } from 'solid-js';
import { parseAmount, formatUsd } from '~/lib/format';
import { showToast } from './Toast';

export type TransactionType = 'deposit' | 'withdraw' | 'send' | 'send-cross-chain';

interface TransactionModalProps {
  type: TransactionType;
  isOpen: boolean;
  onClose: () => void;
  onSubmit: (params: {
    amount: bigint;
    to?: `0x${string}`;
    destinationChainId?: bigint;
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
    desc: 'Send SyncUSD to another address',
  },
  'send-cross-chain': {
    title: 'Send (Cross-Chain)',
    cta: 'Confirm Transfer',
    desc: 'Send to a recipient on another chain',
  },
};

const TransactionModal: Component<TransactionModalProps> = (props) => {
  const [amount, setAmount] = createSignal('');
  const [recipient, setRecipient] = createSignal('');
  const [chainId, setChainId] = createSignal('');
  const [isPending, setIsPending] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const needsRecipient = () =>
    props.type === 'send' || props.type === 'send-cross-chain';
  const needsChainId = () => props.type === 'send-cross-chain';

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    setError(null);
    setIsPending(true);

    try {
      const parsedAmount = parseAmount(amount());
      const txHash = await props.onSubmit({
        amount: parsedAmount,
        to: needsRecipient() ? (recipient() as `0x${string}`) : undefined,
        destinationChainId: needsChainId() ? BigInt(chainId()) : undefined,
      });

      showToast({
        type: 'success',
        message: `${labels[props.type].title} confirmed`,
        txHash,
      });

      setAmount('');
      setRecipient('');
      setChainId('');
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
    if (needsChainId() && !chainId()) return false;
    return true;
  };

  return (
    <Show when={props.isOpen}>
      {/* Backdrop */}
      <div
        class="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm"
        onClick={props.onClose}
        style={{ animation: 'fadeIn 0.2s ease-out' }}
      />

      {/* Modal */}
      <div class="fixed inset-0 z-50 flex items-center justify-center p-4 pointer-events-none">
        <div
          class="pointer-events-auto bg-[#1a1a1a] border border-warm/10 rounded-2xl w-full max-w-md shadow-2xl"
          style={{ animation: 'fadeIn 0.3s ease-out' }}
        >
          {/* Header */}
          <div class="flex items-center justify-between p-6 pb-2">
            <div>
              <h2 class="text-xl font-bold text-white font-[Satoshi]">
                {labels[props.type].title}
              </h2>
              <p class="text-sm text-warm/60 mt-1">
                {labels[props.type].desc}
              </p>
            </div>
            <button
              onClick={props.onClose}
              class="text-warm/40 hover:text-warm transition-colors p-1"
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
              <label class="block text-sm font-medium text-warm/70 mb-2">
                Amount (USD)
              </label>
              <div class="relative">
                <span class="absolute left-4 top-1/2 -translate-y-1/2 text-warm/40 text-lg font-semibold">
                  $
                </span>
                <input
                  type="text"
                  inputMode="decimal"
                  placeholder="0.00"
                  value={amount()}
                  onInput={(e) => setAmount(e.currentTarget.value)}
                  disabled={isPending()}
                  class="w-full bg-brown border border-warm/10 rounded-xl pl-9 pr-4 py-3.5 text-white text-lg font-semibold placeholder-warm/20 focus:outline-none focus:border-hue/50 focus:ring-1 focus:ring-hue/20 transition-all disabled:opacity-50"
                />
              </div>
            </div>

            {/* Recipient */}
            <Show when={needsRecipient()}>
              <div>
                <label class="block text-sm font-medium text-warm/70 mb-2">
                  Recipient Address
                </label>
                <input
                  type="text"
                  placeholder="0x..."
                  value={recipient()}
                  onInput={(e) => setRecipient(e.currentTarget.value)}
                  disabled={isPending()}
                  class="w-full bg-brown border border-warm/10 rounded-xl px-4 py-3 text-white text-sm font-mono placeholder-warm/20 focus:outline-none focus:border-hue/50 focus:ring-1 focus:ring-hue/20 transition-all disabled:opacity-50"
                />
              </div>
            </Show>

            {/* Chain selector */}
            <Show when={needsChainId()}>
              <div>
                <label class="block text-sm font-medium text-warm/70 mb-2">
                  Destination Chain
                </label>
                <select
                  value={chainId()}
                  onChange={(e) => setChainId(e.currentTarget.value)}
                  disabled={isPending()}
                  class="w-full bg-brown border border-warm/10 rounded-xl px-4 py-3 text-white text-sm focus:outline-none focus:border-hue/50 transition-all disabled:opacity-50 appearance-none"
                >
                  <option value="">Select chain...</option>
                  <option value="84532">Base Sepolia</option>
                  <option value="421614">Arbitrum Sepolia</option>
                </select>
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
              class="w-full bg-hue hover:bg-hue/90 active:scale-[0.98] text-white font-semibold py-3.5 px-4 rounded-xl transition-all disabled:opacity-40 disabled:cursor-not-allowed flex items-center justify-center gap-2 mt-2"
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
