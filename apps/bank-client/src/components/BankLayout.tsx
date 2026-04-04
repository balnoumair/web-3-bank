import { type Component, type JSX, Show, createEffect, createSignal } from 'solid-js';
import { useLocation, useNavigate } from '@solidjs/router';
import { useAuth } from '~/contexts/auth-context';
import { useTheme } from '~/contexts/theme-context';
import { truncateAddress } from '~/lib/format';
import Sidebar from './Sidebar';
import ToastContainer from './Toast';

interface BankLayoutProps {
  children: JSX.Element;
}

/* ── Theme toggle (3-state: system / light / dark) ── */
function ThemeToggle(props: { class?: string }) {
  const { mode, setMode } = useTheme();

  const options = [
    { value: 'system' as const, label: 'System', icon: () => (
      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z" />
      </svg>
    )},
    { value: 'light' as const, label: 'Light', icon: () => (
      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
      </svg>
    )},
    { value: 'dark' as const, label: 'Dark', icon: () => (
      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
      </svg>
    )},
  ];

  return (
    <div class={`flex items-center bg-raised rounded-lg p-0.5 border border-edge ${props.class ?? ''}`}>
      {options.map((opt) => (
        <button
          onClick={() => setMode(opt.value)}
          class={`p-1.5 rounded-md transition-colors ${
            mode() === opt.value
              ? 'bg-accent/15 text-accent'
              : 'text-subtle hover:text-muted'
          }`}
          title={opt.label}
        >
          {opt.icon()}
        </button>
      ))}
    </div>
  );
}

const BankLayout: Component<BankLayoutProps> = (props) => {
  const auth = useAuth();
  const location = useLocation();
  const navigate = useNavigate();

  const publicRoutes = ['/login', '/register'];
  const isPublicRoute = () => publicRoutes.includes(location.pathname);

  createEffect(() => {
    if (!auth.isLoading()) {
      if (!auth.isAuthenticated() && !isPublicRoute()) {
        navigate('/login', { replace: true });
      } else if (auth.isAuthenticated() && isPublicRoute()) {
        navigate('/', { replace: true });
      }
    }
  });

  const getInitials = (name: string) =>
    name
      .split(' ')
      .map((w) => w[0])
      .join('')
      .toUpperCase()
      .slice(0, 2);

  const [showDropdown, setShowDropdown] = createSignal(false);

  const handleLogout = () => {
    setShowDropdown(false);
    auth.logout();
    navigate('/login');
  };

  return (
    <>
      <ToastContainer />

      <Show
        when={!isPublicRoute()}
        fallback={
          <div class="relative">
            {/* Floating theme toggle on public pages */}
            <div class="fixed top-4 right-4 z-50">
              <ThemeToggle />
            </div>
            {props.children}
          </div>
        }
      >
        <Show when={!auth.isLoading() && auth.isAuthenticated()} fallback={<div class="min-h-screen bg-bg" />}>
        <div class="min-h-screen bg-bg">
          <Sidebar />

          {/* Main content */}
          <div class="ml-52">
            {/* Top bar */}
            <header class="h-16 border-b border-edge/50 bg-bg/80 backdrop-blur-xl flex items-center justify-end px-8 sticky top-0 z-30">
              <div class="flex items-center gap-4">
                {/* Theme toggle */}
                <ThemeToggle />

                {/* Profile */}
                <Show when={auth.user()}>
                  {(user) => (
                    <div class="flex items-center gap-3 pl-4 border-l border-edge relative">
                      <button
                        onClick={() => setShowDropdown((prev) => !prev)}
                        class="flex items-center gap-3 cursor-pointer"
                      >
                        <div class="text-right">
                          <div class="text-sm font-medium text-text">
                            {user().displayName}
                          </div>
                          <div class="text-xs text-subtle font-mono">
                            {truncateAddress(user().tempoAddress)}
                          </div>
                        </div>
                        <div class="w-9 h-9 rounded-full bg-accent/15 flex items-center justify-center text-accent text-xs font-bold ring-1 ring-accent/20">
                          {getInitials(user().displayName)}
                        </div>
                      </button>

                      {/* Dropdown */}
                      <Show when={showDropdown()}>
                        <div class="fixed inset-0 z-40" onClick={() => setShowDropdown(false)} />
                        <div class="absolute right-0 top-full mt-1 w-44 bg-surface border border-edge rounded-lg z-50 overflow-hidden"
                          style={{ "box-shadow": "var(--shadow-float)" }}>
                          <button
                            onClick={handleLogout}
                            class="w-full text-left px-4 py-2.5 text-sm text-muted hover:text-text hover:bg-raised transition-colors flex items-center gap-2"
                          >
                            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.8">
                              <path stroke-linecap="round" stroke-linejoin="round" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
                            </svg>
                            Sign out
                          </button>
                        </div>
                      </Show>
                    </div>
                  )}
                </Show>
              </div>
            </header>

            {/* Page content */}
            <main class="p-8">{props.children}</main>
          </div>
        </div>
        </Show>
      </Show>
    </>
  );
};

export default BankLayout;
