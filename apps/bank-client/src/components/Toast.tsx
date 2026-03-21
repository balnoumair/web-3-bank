import { createSignal, For, onCleanup, type Component } from 'solid-js';

export interface ToastItem {
  id: string;
  type: 'success' | 'error' | 'info';
  message: string;
  txHash?: string;
}

const [toasts, setToasts] = createSignal<ToastItem[]>([]);

export function showToast(toast: Omit<ToastItem, 'id'>) {
  const id = crypto.randomUUID();
  setToasts((prev) => [...prev, { ...toast, id }]);

  setTimeout(() => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, 5000);
}

function dismissToast(id: string) {
  setToasts((prev) => prev.filter((t) => t.id !== id));
}

const borderColor = {
  success: 'border-l-success',
  error: 'border-l-error',
  info: 'border-l-info',
};

const iconMap = {
  success: (
    <svg class="w-5 h-5 text-success" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
      <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
    </svg>
  ),
  error: (
    <svg class="w-5 h-5 text-error" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
      <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
    </svg>
  ),
  info: (
    <svg class="w-5 h-5 text-info" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
      <path stroke-linecap="round" stroke-linejoin="round" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
    </svg>
  ),
};

const ToastContainer: Component = () => {
  return (
    <div class="fixed bottom-6 right-6 z-50 flex flex-col gap-3 pointer-events-none">
      <For each={toasts()}>
        {(toast) => (
          <div
            class={`pointer-events-auto bg-brown border border-warm/10 ${borderColor[toast.type]} border-l-4 rounded-lg px-4 py-3 shadow-2xl min-w-[320px] max-w-[420px] flex items-start gap-3`}
            style={{ animation: 'slideIn 0.3s ease-out' }}
          >
            <div class="flex-shrink-0 mt-0.5">{iconMap[toast.type]}</div>
            <div class="flex-1 min-w-0">
              <p class="text-sm text-white font-medium">{toast.message}</p>
              {toast.txHash && (
                <p class="text-xs text-warm/60 mt-1 truncate">
                  Tx: {toast.txHash}
                </p>
              )}
            </div>
            <button
              onClick={() => dismissToast(toast.id)}
              class="flex-shrink-0 text-warm/40 hover:text-warm transition-colors"
            >
              <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </div>
        )}
      </For>
    </div>
  );
};

export default ToastContainer;
