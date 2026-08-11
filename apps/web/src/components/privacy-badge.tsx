// Static markup, no client state — the differentiator this ticket calls out explicitly: "an
// explicit 'nothing uploaded' indicator, and a link to the source line that proves it. This is
// our differentiator — show it, do not just claim it." See README.md "Your files never leave
// your device — here's the source" for the same claim made at the repo level.

const WORKER_SOURCE_URL =
  'https://github.com/tachsin/conv.cat/blob/main/packages/engine/src/wasm/client.ts#L19-L22';

export function PrivacyBadge() {
  return (
    <div className="glass-card converter-highlight flex flex-col gap-2 rounded-xl p-4 text-sm sm:flex-row sm:items-start sm:gap-3">
      <span className="text-lg" aria-hidden="true">
        🔒
      </span>
      <div>
        <p className="font-semibold text-base-content">Nothing you convert is ever uploaded.</p>
        <p className="mt-1 text-base-content/65">
          Every conversion runs on this device — a Rust engine compiled to WebAssembly, in a Web
          Worker, right here in this tab. There is no upload endpoint for this app to call. Don't
          take our word for it:{' '}
          <a
            className="font-medium text-primary underline decoration-primary/40 underline-offset-2 hover:decoration-primary"
            href={WORKER_SOURCE_URL}
            target="_blank"
            rel="noreferrer noopener"
          >
            read the source line that starts the Worker
          </a>{' '}
          — or open your browser&apos;s network inspector while you convert a file and watch
          nothing go out.
        </p>
      </div>
    </div>
  );
}
