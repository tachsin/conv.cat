export function SiteFooter() {
  return (
    <footer className="converter-footer mx-auto max-w-3xl px-4 sm:px-6">
      <p>
        conv.cat is open source (
        <a className="underline hover:text-base-content/70" href="https://github.com/tachsin/conv.cat">
          MIT + AGPL-3.0-only
        </a>
        ). &quot;conv.cat&quot; and the mascot are not covered by either licence — see the{' '}
        <a
          className="underline hover:text-base-content/70"
          href="https://github.com/tachsin/conv.cat#trademark"
        >
          trademark note
        </a>
        .
      </p>
    </footer>
  );
}
