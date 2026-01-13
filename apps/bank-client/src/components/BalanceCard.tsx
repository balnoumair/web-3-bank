import { Component, createSignal, onMount } from 'solid-js';

interface BalanceCardProps {
    balance: number;
}

const BalanceCard: Component<BalanceCardProps> = (props) => {
    const [displayBalance, setDisplayBalance] = createSignal(0);

    // Animate balance on mount
    onMount(() => {
        let start = 0;
        const end = props.balance;
        const duration = 1000;
        const startTime = Date.now();

        const animate = () => {
            const currentTime = Date.now();
            const elapsed = currentTime - startTime;
            const progress = Math.min(elapsed / duration, 1);

            // Easing function for smooth animation
            const easeOutCubic = (t: number) => 1 - Math.pow(1 - t, 3);
            const currentValue = start + (end - start) * easeOutCubic(progress);

            setDisplayBalance(currentValue);

            if (progress < 1) {
                requestAnimationFrame(animate);
            }
        };

        animate();
    });

    const formatCurrency = (value: number) => {
        return new Intl.NumberFormat('en-US', {
            style: 'currency',
            currency: 'USD',
            minimumFractionDigits: 2,
            maximumFractionDigits: 2,
        }).format(value);
    };

    return (
        <div class="glass-card p-8 fade-in">
            <div class="flex flex-col items-center justify-center text-center">
                <div class="mb-4">
                    <div class="w-16 h-16 rounded-full bg-gradient-to-br from-indigo-500/20 to-purple-600/20 flex items-center justify-center mb-3">
                        <svg class="w-8 h-8 text-indigo-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h18M7 15h1m4 0h1m-7 4h12a3 3 0 003-3V8a3 3 0 00-3-3H6a3 3 0 00-3 3v8a3 3 0 003 3z" />
                        </svg>
                    </div>
                    <p class="text-sm font-medium text-[var(--color-text-secondary)] uppercase tracking-wider">Total Balance</p>
                </div>

                <div class="animate-count">
                    <h2 class="text-5xl md:text-6xl font-bold mb-2 bg-gradient-to-r from-white to-gray-300 bg-clip-text text-transparent">
                        {formatCurrency(displayBalance())}
                    </h2>
                    <p class="text-sm text-[var(--color-text-muted)]">Available in stablecoins</p>
                </div>

                {/* Balance Growth Indicator */}
                <div class="mt-6 flex items-center gap-2 px-4 py-2 rounded-full bg-green-500/10 border border-green-500/20">
                    <svg class="w-4 h-4 text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
                    </svg>
                    <span class="text-sm font-medium text-green-400">+2.4% this month</span>
                </div>
            </div>
        </div>
    );
};

export default BalanceCard;
