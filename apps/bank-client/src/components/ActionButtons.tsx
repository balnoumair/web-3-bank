import { Component } from 'solid-js';

interface ActionButtonsProps {
    onSend?: () => void;
    onRequest?: () => void;
}

const ActionButtons: Component<ActionButtonsProps> = (props) => {
    return (
        <div class="flex flex-col gap-3">
            {/* Send Funds */}
            <button
                onClick={props.onSend}
                class="card p-6 hover:border-[var(--color-accent-primary)] transition-all group cursor-pointer"
                style="background: linear-gradient(135deg, #2d5f4d 0%, #1f4939 100%)"
            >
                <div class="flex items-center gap-3">
                    <div class="w-10 h-10 rounded-lg bg-white/10 flex items-center justify-center group-hover:bg-white/20 transition-all">
                        <svg class="w-5 h-5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14 5l7 7m0 0l-7 7m7-7H3" />
                        </svg>
                    </div>
                    <div class="text-left">
                        <div class="text-white font-semibold">Send Funds</div>
                        <div class="text-sm text-white/70">Transfer USDC to any wallet</div>
                    </div>
                </div>
            </button>

            {/* Request Payment */}
            <button
                onClick={props.onRequest}
                class="card p-6 hover:border-[var(--color-border-hover)] transition-all group cursor-pointer"
            >
                <div class="flex items-center gap-3">
                    <div class="w-10 h-10 rounded-lg bg-[var(--color-bg-elevated)] flex items-center justify-center group-hover:bg-[#2a2a2a] transition-all">
                        <svg class="w-5 h-5 text-[var(--color-text-secondary)]" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                        </svg>
                    </div>
                    <div class="text-left">
                        <div class="text-white font-semibold">Request Payment</div>
                        <div class="text-sm text-[var(--color-text-muted)]">Create a payment link or QR</div>
                    </div>
                </div>
            </button>
        </div>
    );
};

export default ActionButtons;
