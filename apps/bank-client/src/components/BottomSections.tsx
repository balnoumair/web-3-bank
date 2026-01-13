import { Component } from 'solid-js';

const BottomSections: Component = () => {
    return (
        <div class="grid grid-cols-1 md:grid-cols-2 gap-6 mt-6">
            {/* Your Assets */}
            <div class="card p-6">
                <h3 class="text-lg font-semibold text-white mb-4">Your Assets</h3>
                <div class="space-y-3">
                    <div class="flex items-center justify-between p-3 rounded-lg hover:bg-[var(--color-bg-elevated)] transition-all">
                        <div class="flex items-center gap-3">
                            <div class="w-8 h-8 rounded-full bg-blue-500/20 flex items-center justify-center">
                                <span class="text-blue-400 text-sm font-bold">₮</span>
                            </div>
                            <div>
                                <div class="text-sm font-medium text-white">USDT</div>
                                <div class="text-xs text-[var(--color-text-muted)]">Tether USD</div>
                            </div>
                        </div>
                        <div class="text-right">
                            <div class="text-sm font-semibold text-white">$89,421.00</div>
                            <div class="text-xs text-green-400">+2.1%</div>
                        </div>
                    </div>

                    <div class="flex items-center justify-between p-3 rounded-lg hover:bg-[var(--color-bg-elevated)] transition-all">
                        <div class="flex items-center gap-3">
                            <div class="w-8 h-8 rounded-full bg-orange-500/20 flex items-center justify-center">
                                <span class="text-orange-400 text-sm font-bold">◈</span>
                            </div>
                            <div>
                                <div class="text-sm font-medium text-white">DAI</div>
                                <div class="text-xs text-[var(--color-text-muted)]">Dai Stablecoin</div>
                            </div>
                        </div>
                        <div class="text-right">
                            <div class="text-sm font-semibold text-white">$54,120.10</div>
                            <div class="text-xs text-green-400">+1.8%</div>
                        </div>
                    </div>

                    <div class="flex items-center justify-between p-3 rounded-lg hover:bg-[var(--color-bg-elevated)] transition-all">
                        <div class="flex items-center gap-3">
                            <div class="w-8 h-8 rounded-full bg-blue-600/20 flex items-center justify-center">
                                <span class="text-blue-500 text-sm font-bold">⬡</span>
                            </div>
                            <div>
                                <div class="text-sm font-medium text-white">USDC</div>
                                <div class="text-xs text-[var(--color-text-muted)]">USD Coin</div>
                            </div>
                        </div>
                        <div class="text-right">
                            <div class="text-sm font-semibold text-white">$105,051.00</div>
                            <div class="text-xs text-green-400">+3.2%</div>
                        </div>
                    </div>
                </div>
            </div>

            {/* Stable Savings */}
            <div class="card p-6 relative overflow-hidden" style="background: linear-gradient(135deg, #2d5f4d 0%, #1f4939 100%)">
                <div class="relative z-10">
                    <h3 class="text-lg font-semibold text-white mb-2">Stable Savings</h3>
                    <p class="text-sm text-white/80 mb-6">
                        Earn up to <span class="font-bold text-white">4.5% APY</span> on your USDC holdings with our managed treasury accounts. Institutional-grade security.
                    </p>
                    <button class="btn bg-white/20 hover:bg-white/30 text-white border-white/20 hover:border-white/30">
                        Learn More
                    </button>
                </div>

                {/* Decorative element */}
                <div class="absolute right-0 bottom-0 w-32 h-32 bg-white/5 rounded-full -mr-16 -mb-16"></div>
            </div>
        </div>
    );
};

export default BottomSections;
