import { Component, For } from 'solid-js';

interface Transaction {
    id: string;
    name: string;
    type: string;
    date: string;
    amount: number;
    status: 'completed' | 'pending';
    icon: string;
}

const RecentActivity: Component = () => {
    const transactions: Transaction[] = [
        {
            id: '1',
            name: 'Apple Store, Regent St.',
            type: 'Electronics',
            date: '24 Oct, 18:40',
            amount: -1299.00,
            status: 'completed',
            icon: '🍎'
        },
        {
            id: '2',
            name: 'Circle Yield Deposit',
            type: 'Internal Transfer',
            date: '23 Oct, 09:15',
            amount: 5000.00,
            status: 'completed',
            icon: '🔵'
        },
        {
            id: '3',
            name: 'Artisan Coffee Co.',
            type: 'Food & Drink',
            date: '23 Oct, 08:02',
            amount: -4.50,
            status: 'pending',
            icon: '☕'
        },
        {
            id: '4',
            name: 'Transfer to Sarah M.',
            type: 'P2P Payment',
            date: '22 Oct, 18:55',
            amount: -450.00,
            status: 'completed',
            icon: '👤'
        },
    ];

    const formatCurrency = (value: number) => {
        const formatted = new Intl.NumberFormat('en-US', {
            style: 'currency',
            currency: 'USD',
            minimumFractionDigits: 2,
            maximumFractionDigits: 2,
        }).format(Math.abs(value));

        return value < 0 ? `-${formatted}` : `+${formatted}`;
    };

    const getStatusColor = (status: string) => {
        return status === 'completed' ? 'text-green-400' : 'text-orange-400';
    };

    const getAmountColor = (amount: number) => {
        if (amount > 0) return 'amount-positive';
        if (amount < 0) return 'amount-negative';
        return 'amount-neutral';
    };

    return (
        <div class="card p-6 mt-6">
            <div class="flex items-center justify-between mb-6">
                <h3 class="text-lg font-semibold text-white">Recent Activity</h3>
                <a href="#" class="text-sm text-green-400 hover:text-green-300 transition-colors">
                    View all history →
                </a>
            </div>

            <div class="space-y-4">
                <For each={transactions}>
                    {(tx) => (
                        <div class="flex items-center gap-4 p-3 rounded-lg hover:bg-[var(--color-bg-elevated)] transition-all">
                            {/* Icon */}
                            <div class="w-10 h-10 rounded-full bg-[var(--color-bg-elevated)] flex items-center justify-center text-lg flex-shrink-0">
                                {tx.icon}
                            </div>

                            {/* Transaction Info */}
                            <div class="flex-1 min-w-0">
                                <div class="font-medium text-white text-sm">{tx.name}</div>
                                <div class="text-xs text-[var(--color-text-muted)]">
                                    {tx.type} • {tx.date}
                                </div>
                            </div>

                            {/* Amount and Status */}
                            <div class="text-right flex-shrink-0">
                                <div class={`font-semibold text-sm ${getAmountColor(tx.amount)}`}>
                                    {formatCurrency(tx.amount)}
                                </div>
                                <div class={`text-xs uppercase tracking-wide ${getStatusColor(tx.status)}`}>
                                    • {tx.status}
                                </div>
                            </div>
                        </div>
                    )}
                </For>
            </div>
        </div>
    );
};

export default RecentActivity;
