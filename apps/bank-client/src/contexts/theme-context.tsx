import {
  createContext,
  createSignal,
  onCleanup,
  onMount,
  useContext,
  type Component,
  type JSX,
} from 'solid-js';
import { isServer } from 'solid-js/web';

type ThemeMode = 'light' | 'dark' | 'system';

interface ThemeContextValue {
  mode: () => ThemeMode;
  setMode: (mode: ThemeMode) => void;
  resolved: () => 'light' | 'dark';
}

const ThemeContext = createContext<ThemeContextValue>();

export function useTheme() {
  const ctx = useContext(ThemeContext);
  if (!ctx) throw new Error('useTheme must be used within ThemeProvider');
  return ctx;
}

const STORAGE_KEY = 'w3b-theme';

export const ThemeProvider: Component<{ children: JSX.Element }> = (props) => {
  const [mode, setModeSignal] = createSignal<ThemeMode>('system');
  const [resolved, setResolved] = createSignal<'light' | 'dark'>('dark');

  const getSystemPref = (): 'light' | 'dark' => {
    if (isServer) return 'dark';
    return window.matchMedia('(prefers-color-scheme: light)').matches
      ? 'light'
      : 'dark';
  };

  const apply = (m: ThemeMode) => {
    if (isServer) return;
    const r = m === 'system' ? getSystemPref() : m;
    setResolved(r);
    document.documentElement.classList.toggle('light', r === 'light');
  };

  const setMode = (m: ThemeMode) => {
    setModeSignal(m);
    if (!isServer) localStorage.setItem(STORAGE_KEY, m);
    apply(m);
  };

  onMount(() => {
    const stored = localStorage.getItem(STORAGE_KEY) as ThemeMode | null;
    if (stored && ['light', 'dark', 'system'].includes(stored)) {
      setModeSignal(stored);
    }
    apply(mode());

    // React to OS preference changes when in "system" mode
    const mql = window.matchMedia('(prefers-color-scheme: light)');
    const handler = () => {
      if (mode() === 'system') apply('system');
    };
    mql.addEventListener('change', handler);
    onCleanup(() => mql.removeEventListener('change', handler));
  });

  return (
    <ThemeContext.Provider value={{ mode, setMode, resolved }}>
      {props.children}
    </ThemeContext.Provider>
  );
};
