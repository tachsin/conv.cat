'use client';

import { Moon, Sun } from 'lucide-react';

import { useTheme } from '@/lib/use-theme';

/** Two themes only (`convcat-dark`/`convcat`), matching the brand this app is rebuilding — see
 * `src/lib/theme.ts`. A single toggle button, not a menu, since there are only two states. */
export function ThemeToggle() {
  const [theme, setTheme] = useTheme();
  const isDark = theme === 'convcat-dark';

  return (
    <button
      type="button"
      className="theme-picker-btn theme-picker-btn--icon"
      onClick={() => setTheme(isDark ? 'convcat' : 'convcat-dark')}
    >
      {isDark ? <Moon aria-hidden="true" className="h-4 w-4" /> : <Sun aria-hidden="true" className="h-4 w-4" />}
      <span className="sr-only">Switch to {isDark ? 'light' : 'dark'} theme</span>
    </button>
  );
}
