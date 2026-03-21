import { Title } from "@solidjs/meta";
import { createSignal, For, Show } from "solid-js";
import { useAuth } from "~/contexts/auth-context";
import { useBalance } from "~/hooks/use-balance";
import { useTransfers } from "~/hooks/use-transfers";
import { useDeposit } from "~/hooks/use-deposit";
import { useWithdraw } from "~/hooks/use-withdraw";
import { useTransfer } from "~/hooks/use-transfer";
import { useTransferCrossChain } from "~/hooks/use-transfer-cross-chain";
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
  const crossChainTransfer = useTransferCrossChain(userAddress);

  const [modalType, setModalType] = createSignal<TransactionType | null>(null);

  const balanceNumber = () => {
    const raw = balance.data;
    if (!raw) return 0;
    return Number(raw) / 1e6; // SyncUSD has 6 decimals
  };

  const handleModalSubmit = async (params: {
    amount: bigint;
    to?: `0x${string}`;
    destinationChainId?: bigint;
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
        return transfer.mutateAsync({ to: params.to, amount: params.amount });
      case "send-cross-chain":
        if (!params.to || !params.destinationChainId)
          throw new Error("Recipient and chain required");
        return crossChainTransfer.mutateAsync({
          to: params.to,
          amount: params.amount,
          destinationChainId: params.destinationChainId,
        });
    }
  };

  return (
    <div class="animate-in">
      <Title>Dashboard - Web3Bank</Title>

      {/* Greeting */}
      <div class="mb-8">
        <h1 class="text-2xl font-bold text-white font-[Satoshi] tracking-tight">
          <Show when={auth.user()} fallback="Dashboard">
            {(user) => <>Hello, {user().displayName}</>}
          </Show>
        </h1>
        <p class="text-warm/50 text-sm mt-1">
          Your stablecoin banking overview.
        </p>
      </div>

      {/* Top row: Balance + Actions */}
      <div class="grid grid-cols-[1fr_300px] gap-6 mb-6">
        {/* Balance card */}
        <div class="bg-[#1a1a1a] border border-warm/8 rounded-2xl p-6">
          <div class="text-sm text-warm/50 mb-1">Total Balance</div>
          <Show
            when={!balance.isLoading}
            fallback={<Skeleton height="3rem" width="200px" />}
          >
            <div class="flex items-baseline gap-3">
              <AnimatedNumber
                value={balanceNumber()}
                prefix="$"
                decimals={2}
                class="text-4xl font-bold text-white font-[Satoshi] tracking-tight"
              />
              <span class="text-sm text-warm/40">SyncUSD</span>
            </div>
          </Show>

          {/* Mini chart placeholder */}
          <div class="mt-6 flex items-end gap-1.5 h-20">
            {[40, 30, 55, 25, 50, 35, 52, 28, 48, 38, 58, 32].map(
              (h) => (
                <div
                  class="flex-1 rounded-sm bg-gradient-to-t from-lichen/30 to-lichen/10 transition-all hover:from-lichen/50 hover:to-lichen/20"
                  style={{ height: `${h}%` }}
                />
              ),
            )}
          </div>
          <div class="flex justify-between mt-2 text-[10px] text-warm/25 px-0.5">
            <span>Mon</span>
            <span>Tue</span>
            <span>Wed</span>
            <span>Thu</span>
            <span>Fri</span>
            <span>Sat</span>
            <span>Sun</span>
          </div>
        </div>

        {/* Quick actions */}
        <div class="flex flex-col gap-3">
          <button
            onClick={() => setModalType("deposit")}
            class="flex items-center gap-3 bg-lichen/15 hover:bg-lichen/25 border border-lichen/20 rounded-xl p-4 text-left transition-all active:scale-[0.98] group"
          >
            <div class="w-10 h-10 rounded-lg bg-lichen/20 flex items-center justify-center group-hover:bg-lichen/30 transition-colors">
              <svg class="w-5 h-5 text-lush" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 4v16m0 0l-4-4m4 4l4-4" />
              </svg>
            </div>
            <div>
              <div class="text-sm font-semibold text-white">Deposit</div>
              <div class="text-xs text-warm/40">Add USDC to your account</div>
            </div>
          </button>

          <button
            onClick={() => setModalType("withdraw")}
            class="flex items-center gap-3 bg-brown border border-warm/8 hover:border-warm/15 rounded-xl p-4 text-left transition-all active:scale-[0.98] group"
          >
            <div class="w-10 h-10 rounded-lg bg-warm/5 flex items-center justify-center group-hover:bg-warm/10 transition-colors">
              <svg class="w-5 h-5 text-warm/60" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M12 20V4m0 0l-4 4m4-4l4 4" />
              </svg>
            </div>
            <div>
              <div class="text-sm font-semibold text-white">Withdraw</div>
              <div class="text-xs text-warm/40">Withdraw USDC</div>
            </div>
          </button>

          <button
            onClick={() => setModalType("send")}
            class="flex items-center gap-3 bg-brown border border-warm/8 hover:border-warm/15 rounded-xl p-4 text-left transition-all active:scale-[0.98] group"
          >
            <div class="w-10 h-10 rounded-lg bg-warm/5 flex items-center justify-center group-hover:bg-warm/10 transition-colors">
              <svg class="w-5 h-5 text-warm/60" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M14 5l7 7m0 0l-7 7m7-7H3" />
              </svg>
            </div>
            <div>
              <div class="text-sm font-semibold text-white">Send</div>
              <div class="text-xs text-warm/40">Transfer to any address</div>
            </div>
          </button>

          <button
            onClick={() => setModalType("send-cross-chain")}
            class="flex items-center gap-3 bg-brown border border-warm/8 hover:border-warm/15 rounded-xl p-4 text-left transition-all active:scale-[0.98] group"
          >
            <div class="w-10 h-10 rounded-lg bg-warm/5 flex items-center justify-center group-hover:bg-warm/10 transition-colors">
              <svg class="w-5 h-5 text-warm/60" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M8 7h12m0 0l-4-4m4 4l-4 4m0 6H4m0 0l4 4m-4-4l4-4" />
              </svg>
            </div>
            <div>
              <div class="text-sm font-semibold text-white">Cross-Chain</div>
              <div class="text-xs text-warm/40">Send to another chain</div>
            </div>
          </button>
        </div>
      </div>

      {/* Bottom row: Activity + Savings */}
      <div class="grid grid-cols-[1fr_300px] gap-6">
        {/* Recent activity */}
        <div class="bg-[#1a1a1a] border border-warm/8 rounded-2xl p-6">
          <div class="flex items-center justify-between mb-5">
            <h3 class="text-base font-bold text-white font-[Satoshi]">
              Recent Activity
            </h3>
            <a href="#" class="text-xs text-hue hover:text-hue/80 transition-colors font-medium">
              View all
            </a>
          </div>

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
                  <p class="text-warm/30 text-sm">No transactions yet.</p>
                  <p class="text-warm/20 text-xs mt-1">
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
                      <div class="flex items-center gap-3 p-3 rounded-xl hover:bg-warm/3 transition-colors">
                        {/* Icon */}
                        <div
                          class={`w-9 h-9 rounded-full flex items-center justify-center flex-shrink-0 ${
                            isIncoming()
                              ? 'bg-success/10 text-success'
                              : 'bg-hue/10 text-hue'
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
                          <div class="text-sm font-medium text-white">
                            {isIncoming() ? "Received" : "Sent"}
                          </div>
                          <div class="text-xs text-warm/40 truncate">
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
                              isIncoming() ? 'text-success' : 'text-white'
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

        {/* Right column: Savings card */}
        <div class="flex flex-col gap-6">
          <div class="bg-gradient-to-br from-tropic to-tropic/60 border border-lichen/20 rounded-2xl p-6 relative overflow-hidden">
            <div class="absolute -right-8 -bottom-8 w-32 h-32 rounded-full bg-lichen/10" />
            <div class="absolute -right-4 -bottom-4 w-20 h-20 rounded-full bg-lichen/10" />

            <h3 class="text-base font-bold text-lush font-[Satoshi] mb-2">
              Stable Savings
            </h3>
            <p class="text-sm text-warm/60 mb-4 relative z-10">
              Earn up to{" "}
              <span class="font-bold text-white">4.5% APY</span> on your
              SyncUSD holdings.
            </p>
            <button class="relative z-10 px-4 py-2 bg-white/10 hover:bg-white/15 border border-white/10 rounded-lg text-sm font-medium text-white transition-all active:scale-[0.98]">
              Learn More
            </button>
          </div>

          {/* Network status */}
          <div class="bg-[#1a1a1a] border border-warm/8 rounded-2xl p-6">
            <h3 class="text-base font-bold text-white font-[Satoshi] mb-4">
              Network
            </h3>
            <div class="flex flex-col gap-3">
              <div class="flex items-center justify-between">
                <span class="text-sm text-warm/60">Tempo</span>
                <div class="flex items-center gap-1.5">
                  <div class="w-1.5 h-1.5 rounded-full bg-success" />
                  <span class="text-xs text-success">Active</span>
                </div>
              </div>
              <div class="flex items-center justify-between">
                <span class="text-sm text-warm/60">Base</span>
                <div class="flex items-center gap-1.5">
                  <div class="w-1.5 h-1.5 rounded-full bg-success" />
                  <span class="text-xs text-success">Active</span>
                </div>
              </div>
              <div class="flex items-center justify-between">
                <span class="text-sm text-warm/60">Arbitrum</span>
                <div class="flex items-center gap-1.5">
                  <div class="w-1.5 h-1.5 rounded-full bg-success" />
                  <span class="text-xs text-success">Active</span>
                </div>
              </div>
            </div>
          </div>
        </div>
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
