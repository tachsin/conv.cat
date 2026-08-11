import { ThemeToggle } from './theme-toggle';

export function SiteHeader() {
  return (
    <header className="nav-header">
      <div className="mx-auto flex max-w-3xl items-center justify-between gap-3 px-4 py-3 sm:px-6">
        <span className="nav-brand text-lg font-semibold tracking-tight text-base-content">
          conv<span className="text-primary">.cat</span> <span aria-hidden="true">🐾</span>
        </span>
        <ThemeToggle />
      </div>
    </header>
  );
}
