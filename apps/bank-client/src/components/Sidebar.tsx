import { Component } from 'solid-js';
import { useLocation } from '@solidjs/router';

const Sidebar: Component = () => {
  const location = useLocation();
  const isActive = (path: string) => location.pathname === path;

  return (
    <div class="fixed left-0 top-0 h-screen w-52 bg-surface border-r border-edge/50 flex flex-col">
      {/* Logo */}
      <div class="p-5 border-b border-edge/50">
        <div class="flex items-center gap-3">
          <div class="w-8 h-8 bg-accent/15 rounded-lg flex items-center justify-center text-accent font-bold text-sm ring-1 ring-accent/20">
            W3
          </div>
          <span class="text-text font-bold text-lg font-[Satoshi] tracking-tight">
            Web3Bank
          </span>
        </div>
      </div>

      {/* Navigation */}
      <nav class="flex-1 p-3 flex flex-col gap-1">
        <a
          href="/"
          class={`flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all ${
            isActive('/')
              ? 'bg-accent/10 text-accent'
              : 'text-muted hover:text-text hover:bg-raised'
          }`}
        >
          <svg class="w-[18px] h-[18px]" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.8">
            <path stroke-linecap="round" stroke-linejoin="round" d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
          </svg>
          Dashboard
        </a>
        <a
          href="/settings/devices"
          class={`flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all ${
            isActive('/settings/devices')
              ? 'bg-accent/10 text-accent'
              : 'text-muted hover:text-text hover:bg-raised'
          }`}
        >
          <svg class="w-[18px] h-[18px]" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.8">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 11c1.657 0 3-1.343 3-3S13.657 5 12 5 9 6.343 9 8s1.343 3 3 3zm0 0c-3.866 0-7 1.79-7 4v2h14v-2c0-2.21-3.134-4-7-4z" />
          </svg>
          Passkeys
        </a>
      </nav>
    </div>
  );
};

export default Sidebar;
