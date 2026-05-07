import { Title } from "@solidjs/meta";
import { createSignal, For, Show } from "solid-js";
import { useAuth } from "~/contexts/auth-context";
import { useBalance } from "~/hooks/use-balance";
import { useTransfers } from "~/hooks/use-transfers";
import { useDeposit } from "~/hooks/use-deposit";
import { useWithdraw } from "~/hooks/use-withdraw";
import { useTransfer } from "~/hooks/use-transfer";
import { useSetUsername } from "~/hooks/use-username";
import { formatUsd, formatDate, truncateAddress } from "~/lib/format";
import AnimatedNumber from "~/components/AnimatedNumber";
import Skeleton from "~/components/Skeleton";
import TransactionModal, { type TransactionType } from "~/components/TransactionModal";

export default function Home() {
  const auth = useAuth();
  const balance = useBalance();
  const transfers = useTransfers(8);

  const userAddress = () =>
    auth.user()?.tempoAddress as `0x${string}` | undefined;

  const deposit = useDeposit(userAddress);
  const withdraw = useWithdraw(userAddress);
  const transfer = useTransfer(userAddress);

  const [modalType, setModalType] = createSignal<TransactionType | null>(null);

  const setUsername = useSetUsername();
  const [usernameInput, setUsernameInput] = createSignal('');
  const [usernameError, setUsernameError] = createSignal<string | null>(null);

  const handleSetUsername = async (e: Event) => {
    e.preventDefault();
    setUsernameError(null);
    try {
      await setUsername.mutateAsync(usernameInput().trim());
      setUsernameInput('');
    } catch (err) {
      setUsernameError(err instanceof Error ? err.message : 'Failed to set username');
    }
  };

  const balanceNumber = () => {
    const raw = balance.data;
    if (!raw) return 0;
    return Number(raw) / 1e6; // SyncUSD has 6 decimals
  };

  const handleModalSubmit = async (params: {
    amount: bigint;
    to?: `0x${string}`;
  }) => {
    const type = modalType();
    if (!type) throw new Error("No modal type");

    switch (type) {
      case "deposit":
        return deposit.mutateAsync({ amount: params.amount });
      case "withdraw":
        return withdraw.mutateAsync({ amount: params.amount });
      case "send":
        if (!params.to) throw new Error("Recipient required");
        return transfer.mutateAsync({
          to: params.to,
          amount: params.amount,
          destChainId: params.destChainId,
        });
    }
  };

  return (
    <div class="animate-in">
      <Title>Dashboard - Web3Bank</Title>

      {/* Greeting */}
      <div class="mb-8">
        <h1 class="text-2xl font-bold text-text font-[Satoshi] tracking-tight">
          <Show when={auth.user()} fallback="Dashboard">
            {(user) => <>Hello, {user().displayName}</>}
          </Show>
        </h1>
        <p class="text-muted text-sm mt-1">
          Your banking overview.
        </p>
      </div>

      {/* Username setup prompt */}
      <Show when={auth.user() && !auth.user()?.username}>
        <div class="bg-accent/[0.06] border border-accent/20 rounded-2xl p-4 mb-6">
          <p class="text-sm font-medium text-text mb-3">
            Set a username so others can send you funds
          </p>
          <form onSubmit={handleSetUsername} class="flex gap-2">
            <input
              type="text"
              placeholder="e.g. alice123"
              value={usernameInput()}
              onInput={(e) => setUsernameInput(e.currentTarget.value)}
              disabled={setUsername.isPending}
              class="flex-1 bg-raised border border-edge rounded-xl px-3 py-2 text-sm text-text placeholder-faint focus:outline-none focus:border-accent/50 focus:ring-1 focus:ring-accent/20 disabled:opacity-50"
            />
            <button
              type="submit"
              disabled={!usernameInput().trim() || setUsername.isPending}
              class="bg-accent hover:bg-accent-hover text-accent-fg text-sm font-semibold px-4 py-2 rounded-xl transition-all disabled:opacity-40 disabled:cursor-not-allowed"
            >
              {setUsername.isPending ? 'Saving...' : 'Save'}
            </button>
          </form>
          <Show when={usernameError()}>
            <p class="text-error text-xs mt-2">{usernameError()}</p>
          </Show>
        </div>
      </Show>

      {/* Balance card */}
      <div class="bg-surface border border-edge rounded-2xl p-6 mb-6 relative overflow-hidden">
        <div class="absolute top-0 right-0 w-48 h-48 bg-accent/[0.04] rounded-full blur-[60px] pointer-events-none -translate-y-1/2 translate-x-1/4" />
        <div class="relative">
          <div class="text-sm text-muted mb-1">Total Balance</div>
          <Show
            when={!balance.isLoading}
            fallback={<Skeleton height="3rem" width="200px" />}
          >
            <div class="flex items-baseline gap-3">
              <AnimatedNumber
                value={balanceNumber()}
                prefix="$"
                decimals={2}
                class="text-4xl font-bold text-text font-[Satoshi] tracking-tight"
              />
              <span class="text-sm text-subtle">SyncUSD</span>
            </div>
          </Show>
        </div>
      </div>

      {/* Quick actions */}
      <div class="grid grid-cols-3 gap-3 mb-6">
        <button
          onClick={() => setModalType("deposit")}
          class="flex items-center gap-3 bg-accent/[0.06] hover:bg-accent/[0.12] border border-accent/15 rounded-xl p-4 text-left transition-all active:scale-[0.98] group"
        >
          <div class="w-10 h-10 rounded-lg bg-accent/15 flex items-center justify-center group-hover:bg-accent/25 transition-colors">
            <svg class="w-5 h-5 text-accent" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m0 0l-4-4m4 4l4-4" />
            </svg>
          </div>
          <div>
            <div class="text-sm font-semibold text-text">Deposit</div>
            <div class="text-xs text-muted">Add funds to your account</div>
          </div>
        </button>

        <button
          onClick={() => setModalType("withdraw")}
          class="flex items-center gap-3 bg-raised border border-edge hover:border-faint rounded-xl p-4 text-left transition-all active:scale-[0.98] group"
        >
          <div class="w-10 h-10 rounded-lg bg-overlay/50 flex items-center justify-center group-hover:bg-overlay transition-colors">
            <svg class="w-5 h-5 text-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M12 20V4m0 0l-4 4m4-4l4 4" />
            </svg>
          </div>
          <div>
            <div class="text-sm font-semibold text-text">Withdraw</div>
            <div class="text-xs text-muted">Withdraw funds</div>
          </div>
        </button>

        <button
          onClick={() => setModalType("send")}
          class="flex items-center gap-3 bg-raised border border-edge hover:border-faint rounded-xl p-4 text-left transition-all active:scale-[0.98] group"
        >
          <div class="w-10 h-10 rounded-lg bg-overlay/50 flex items-center justify-center group-hover:bg-overlay transition-colors">
            <svg class="w-5 h-5 text-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
              <path stroke-linecap="round" stroke-linejoin="round" d="M14 5l7 7m0 0l-7 7m7-7H3" />
            </svg>
          </div>
          <div>
            <div class="text-sm font-semibold text-text">Send</div>
            <div class="text-xs text-muted">Send to a user</div>
          </div>
        </button>
      </div>

      {/* Recent activity */}
      <div class="bg-surface border border-edge rounded-2xl p-6">
        <h3 class="text-base font-bold text-text font-[Satoshi] mb-5">
          Recent Activity
        </h3>

        <Show
          when={!transfers.isLoading}
          fallback={
            <div class="flex flex-col gap-3">
              <Skeleton height="3rem" />
              <Skeleton height="3rem" />
              <Skeleton height="3rem" />
              <Skeleton height="3rem" />
            </div>
          }
        >
          <Show
            when={transfers.data && transfers.data.length > 0}
            fallback={
              <div class="py-12 text-center">
                <p class="text-subtle text-sm">No transactions yet.</p>
                <p class="text-faint text-xs mt-1">
                  Deposit funds to get started.
                </p>
              </div>
            }
          >
            <div class="flex flex-col gap-1 stagger">
              <For each={transfers.data}>
                {(tx) => {
                  const isIncoming = () =>
                    tx.to.toLowerCase() ===
                    auth.user()?.tempoAddress?.toLowerCase();

                  return (
                    <div class="flex items-center gap-3 p-3 rounded-xl hover:bg-raised transition-colors">
                      {/* Icon */}
                      <div
                        class={`w-9 h-9 rounded-full flex items-center justify-center flex-shrink-0 ${
                          isIncoming()
                            ? 'bg-success/10 text-success'
                            : 'bg-muted/10 text-muted'
                        }`}
                      >
                        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            d={
                              isIncoming()
                                ? "M12 4v16m0 0l-4-4m4 4l4-4"
                                : "M14 5l7 7m0 0l-7 7m7-7H3"
                            }
                          />
                        </svg>
                      </div>

                      {/* Details */}
                      <div class="flex-1 min-w-0">
                        <div class="text-sm font-medium text-text">
                          {isIncoming() ? "Received" : "Sent"}
                        </div>
                        <div class="text-xs text-subtle truncate">
                          {isIncoming()
                            ? `From ${truncateAddress(tx.from)}`
                            : `To ${truncateAddress(tx.to)}`}{" "}
                          &middot; {formatDate(tx.timestamp)}
                        </div>
                      </div>

                      {/* Amount */}
                      <div class="text-right flex-shrink-0">
                        <div
                          class={`text-sm font-semibold ${
                            isIncoming() ? 'text-success' : 'text-text'
                          }`}
                        >
                          {isIncoming() ? "+" : "-"}
                          {formatUsd(tx.amount)}
                        </div>
                      </div>
                    </div>
                  );
                }}
              </For>
            </div>
          </Show>
        </Show>
      </div>

      {/* Transaction Modal */}
      <Show when={modalType()}>
        {(type) => (
          <TransactionModal
            type={type()}
            isOpen={true}
            onClose={() => setModalType(null)}
            onSubmit={handleModalSubmit}
          />
        )}
      </Show>
    </div>
  );
}
