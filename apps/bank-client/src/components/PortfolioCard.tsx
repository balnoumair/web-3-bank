import { Component, createSignal, onMount } from 'solid-js';

interface PortfolioCardProps {
    value: number;
    change: number;
}

const PortfolioCard: Component<PortfolioCardProps> = (props) => {
    const [displayValue, setDisplayValue] = createSignal(0);

    // Animate value on mount
    onMount(() => {
        let start = 0;
        const end = props.value;
        const duration = 1000;
        const startTime = Date.now();

        const animate = () => {
            const currentTime = Date.now();
            const elapsed = currentTime - startTime;
            const progress = Math.min(elapsed / duration, 1);
            const easeOutCubic = (t: number) => 1 - Math.pow(1 - t, 3);
            const currentValue = start + (end - start) * easeOutCubic(progress);

            setDisplayValue(currentValue);

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
        <div class="card p-6">
            <div class="mb-4">
                <div class="text-sm text-[var(--color-text-muted)] mb-1">Total Portfolio Value</div>
                <div class="flex items-baseline gap-3">
                    <h2 class="text-4xl font-bold text-white">
                        {formatCurrency(displayValue())}
                    </h2>
                    <span class="text-sm text-[var(--color-text-secondary)]">USDC</span>
                    <span class="text-sm font-medium text-green-400 ml-auto">
                        +{props.change}%
                    </span>
                </div>
            </div>

            {/* Simplified Chart Placeholder */}
            <div class="mt-6 h-32 bg-[var(--color-bg-elevated)] rounded-lg p-4 flex items-end justify-between gap-2">
                {/* Simple bar chart representation */}
                <div class="flex-1 h-16 bg-gradient-to-t from-green-900/50 to-green-700/30 rounded-t"></div>
                <div class="flex-1 h-12 bg-gradient-to-t from-green-900/50 to-green-700/30 rounded-t"></div>
                <div class="flex-1 h-20 bg-gradient-to-t from-green-900/50 to-green-700/30 rounded-t"></div>
                <div class="flex-1 h-10 bg-gradient-to-t from-green-900/50 to-green-700/30 rounded-t"></div>
                <div class="flex-1 h-18 bg-gradient-to-t from-green-900/50 to-green-700/30 rounded-t"></div>
                <div class="flex-1 h-14 bg-gradient-to-t from-green-900/50 to-green-700/30 rounded-t"></div>
                <div class="flex-1 h-19 bg-gradient-to-t from-green-900/50 to-green-700/30 rounded-t"></div>
            </div>

            {/* Day labels */}
            <div class="flex justify-between mt-2 px-4 text-xs text-[var(--color-text-muted)]">
                <span>MON</span>
                <span>TUE</span>
                <span>WED</span>
                <span>THU</span>
                <span>FRI</span>
                <span>SAT</span>
                <span>SUN</span>
            </div>
        </div>
    );
};

export default PortfolioCard;
