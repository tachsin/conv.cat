'use client';

// Shared by every component that needs to read or flip the theme (ThemeToggle, CommandPalette) —
// see theme.ts for why this is `useSyncExternalStore` over the external `localStorage`/`data-theme`
// state rather than `useState`.

import { useSyncExternalStore } from 'react';

import { applyTheme, DEFAULT_THEME, persistTheme, readStoredTheme } from './theme';
import type { ThemeId } from './theme';

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

export function useTheme(): [ThemeId, (next: ThemeId) => void] {
  const theme = useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
  function setTheme(next: ThemeId): void {
    applyTheme(next);
    persistTheme(next);
    for (const listener of listeners) listener();
  }
  return [theme, setTheme];
}
