import { type Component, type JSX, Show, createEffect } from 'solid-js';
import { useLocation, useNavigate } from '@solidjs/router';
import { useAuth } from '~/contexts/auth-context';
import { truncateAddress } from '~/lib/format';
import Sidebar from './Sidebar';
import ToastContainer from './Toast';

interface BankLayoutProps {
  children: JSX.Element;
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

  const handleLogout = () => {
    auth.logout();
    navigate('/login');
  };

  return (
    <>
      <ToastContainer />

      <Show when={!isPublicRoute()} fallback={<div>{props.children}</div>}>
        <Show when={!auth.isLoading() && auth.isAuthenticated()} fallback={<div class="min-h-screen bg-[#141414]" />}>
        <div class="min-h-screen bg-[#141414]">
          <Sidebar />

          {/* Main content */}
          <div class="ml-52">
            {/* Top bar */}
            <header class="h-16 border-b border-warm/5 bg-[#141414] flex items-center justify-end px-8 sticky top-0 z-30">
              {/* Right side */}
              <div class="flex items-center gap-3">
                {/* Profile */}
                <Show when={auth.user()}>
                  {(user) => (
                    <div class="flex items-center gap-3 pl-3 border-l border-warm/8 group relative">
                      <div class="text-right">
                        <div class="text-sm font-medium text-white">
                          {user().displayName}
                        </div>
                        <div class="text-xs text-warm/40 font-mono">
                          {truncateAddress(user().tempoAddress)}
                        </div>
                      </div>
                      <div class="w-9 h-9 rounded-full bg-gradient-to-br from-hue to-hue/60 flex items-center justify-center text-white text-xs font-bold cursor-pointer">
                        {getInitials(user().displayName)}
                      </div>

                      {/* Dropdown */}
                      <div class="hidden group-hover:block absolute right-0 top-full mt-1 w-44 bg-[#1a1a1a] border border-warm/10 rounded-lg shadow-xl z-50 overflow-hidden">
                        <button
                          onClick={handleLogout}
                          class="w-full text-left px-4 py-2.5 text-sm text-warm/70 hover:text-white hover:bg-warm/5 transition-colors flex items-center gap-2"
                        >
                          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.8">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
                          </svg>
                          Sign out
                        </button>
                      </div>
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
