import { Component } from 'solid-js';
import { useLocation } from '@solidjs/router';

const Sidebar: Component = () => {
  const location = useLocation();
  const isActive = (path: string) => location.pathname === path;

  return (
    <div class="fixed left-0 top-0 h-screen w-52 bg-[#161616] border-r border-warm/5 flex flex-col">
      {/* Logo */}
      <div class="p-5 border-b border-warm/5">
        <div class="flex items-center gap-3">
          <div class="w-8 h-8 bg-hue rounded-lg flex items-center justify-center text-white font-bold text-sm">
            W3
          </div>
          <span class="text-white font-bold text-lg font-[Satoshi] tracking-tight">
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
              ? 'bg-lichen/20 text-lush'
              : 'text-warm/60 hover:text-warm hover:bg-warm/5'
          }`}
        >
          <svg class="w-[18px] h-[18px]" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.8">
            <path stroke-linecap="round" stroke-linejoin="round" d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zM14 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zM14 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z" />
          </svg>
          Dashboard
        </a>

        <a
          href="#"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium text-warm/60 hover:text-warm hover:bg-warm/5 transition-all"
        >
          <svg class="w-[18px] h-[18px]" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.8">
            <path stroke-linecap="round" stroke-linejoin="round" d="M3 10h18M7 15h1m4 0h1m-7 4h12a3 3 0 003-3V8a3 3 0 00-3-3H6a3 3 0 00-3 3v8a3 3 0 003 3z" />
          </svg>
          Wallet
        </a>

        <a
          href="#"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium text-warm/60 hover:text-warm hover:bg-warm/5 transition-all"
        >
          <svg class="w-[18px] h-[18px]" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.8">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          Activity
        </a>

        <a
          href="#"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium text-warm/60 hover:text-warm hover:bg-warm/5 transition-all"
        >
          <svg class="w-[18px] h-[18px]" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.8">
            <path stroke-linecap="round" stroke-linejoin="round" d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
          </svg>
          Yield
        </a>

        <div class="my-3 border-t border-warm/5" />

        <a
          href="#"
          class="flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium text-warm/60 hover:text-warm hover:bg-warm/5 transition-all"
        >
          <svg class="w-[18px] h-[18px]" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.8">
            <path stroke-linecap="round" stroke-linejoin="round" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
            <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          </svg>
          Settings
        </a>
      </nav>

      {/* Bottom */}
      <div class="p-4 border-t border-warm/5">
        <div class="mb-3">
          <div class="text-[10px] font-medium text-warm/40 uppercase tracking-wider mb-1">
            Account Tier
          </div>
          <div class="flex items-center gap-2">
            <div class="w-1.5 h-1.5 rounded-full bg-success" />
            <span class="text-sm text-white font-medium">Premium</span>
          </div>
        </div>
      </div>
    </div>
  );
};

export default Sidebar;
