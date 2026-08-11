'use client';

import { useSyncExternalStore } from 'react';

import { applyTheme, DEFAULT_THEME, persistTheme, readStoredTheme } from '@/lib/theme';
import type { ThemeId } from '@/lib/theme';

// `useSyncExternalStore`, not `useState` + `useEffect`: the theme genuinely lives in an external
// store (`localStorage`, mirrored onto `<html data-theme>` by the blocking inline script in the
// root layout — see `lib/theme.ts#THEME_INIT_SCRIPT`), not in this component. Reading it via an
// effect-then-setState would report a different value on the client's first render pass than
// `getServerSnapshot` reports for SSR, which is exactly the class of hydration mismatch
// `useSyncExternalStore` exists to handle correctly (`getServerSnapshot` matches the server
// render; React reconciles to the real client value right after hydration, no manual effect
// needed).
const listeners = new Set<() => void>();

function subscribe(onStoreChange: () => void): () => void {
  listeners.add(onStoreChange);
  return () => listeners.delete(onStoreChange);
}

function getSnapshot(): ThemeId {
  return readStoredTheme() ?? DEFAULT_THEME;
}

function getServerSnapshot(): ThemeId {
  return DEFAULT_THEME;
}

function setTheme(next: ThemeId): void {
  applyTheme(next);
  persistTheme(next);
  for (const listener of listeners) listener();
}

/** Two themes only (`convcat-dark`/`convcat`), matching the brand this app is rebuilding — see
 * `src/lib/theme.ts`. A single toggle button, not a menu, since there are only two states. */
export function ThemeToggle() {
  const theme = useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
  const isDark = theme === 'convcat-dark';

  return (
    <button
      type="button"
      className="theme-picker-btn theme-picker-btn--icon"
      onClick={() => setTheme(isDark ? 'convcat' : 'convcat-dark')}
    >
      <span aria-hidden="true">{isDark ? '🌙' : '☀️'}</span>
      <span className="sr-only">Switch to {isDark ? 'light' : 'dark'} theme</span>
    </button>
  );
}
