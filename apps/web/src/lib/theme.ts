// Theme persistence — two DaisyUI themes only (`convcat-dark`, `convcat`), matching the brand
// this app is rebuilding (see `.agentsroom` memory `conv-next-architecture` "Theming / brand").
// No zustand/cookie machinery like the legacy app: this is a single client-only page with no
// server-rendered theme-dependent markup, so `localStorage` + a `data-theme` attribute on
// `<html>` is the whole mechanism.

export type ThemeId = 'convcat-dark' | 'convcat';

export const THEMES: readonly ThemeId[] = ['convcat-dark', 'convcat'];
export const DEFAULT_THEME: ThemeId = 'convcat-dark';
const STORAGE_KEY = 'convcat-theme';

export function isThemeId(value: unknown): value is ThemeId {
  return value === 'convcat-dark' || value === 'convcat';
}

export function readStoredTheme(): ThemeId | null {
  if (typeof window === 'undefined') return null;
  const stored = window.localStorage.getItem(STORAGE_KEY);
  return isThemeId(stored) ? stored : null;
}

export function persistTheme(theme: ThemeId): void {
  window.localStorage.setItem(STORAGE_KEY, theme);
}

export function applyTheme(theme: ThemeId): void {
  document.documentElement.setAttribute('data-theme', theme);
}

/**
 * Source for the blocking inline `<script>` the root layout renders in `<head>`, executed before
 * first paint so the page never flashes the default theme and then swaps — the same technique
 * `next-themes` and the legacy app's own theme store both rely on. Kept as a plain string (not a
 * function `.toString()`'d) so minification/bundling of the surrounding module can never change
 * what actually ships inline.
 */
export const THEME_INIT_SCRIPT = `(function(){try{var t=localStorage.getItem('${STORAGE_KEY}');if(t!=='convcat-dark'&&t!=='convcat'){t='${DEFAULT_THEME}';}document.documentElement.setAttribute('data-theme',t);}catch(e){}})();`;
