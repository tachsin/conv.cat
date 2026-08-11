# @conv.cat/engine

The TypeScript wrapper around the conversion engine. License: **MIT**.

It exposes one interface (`ConvEngine`, `src/types.ts`) to callers and picks the implementation
at runtime: `conv-wasm` (compiled from `crates/conv-core` via wasm-bindgen), running in a Web
Worker, in the browser; native Tauri commands inside the desktop app. Consumers (`apps/web`,
`apps/desktop`) import only from `src/index.ts` (`getEngine()`/`convert()`) — never reach into
`src/wasm/` or `src/tauri/` directly. That's what lets the two apps share one API and lets this
package swap backends without call-site changes; see `docs/ARCHITECTURE.md` "One engine, two
runtimes" for the full picture.

This package must never contain conversion logic itself (no format parsing/encoding, no
algorithms) — that belongs in `crates/conv-core`. It is glue, not an engine.

## Layout

```
src/
├─ index.ts       # getEngine()/convert() — the only exports app code should import
├─ types.ts       # ConvEngine, ConvertOptions, FormatInfo, ConvertErrorKind — the contract
├─ errors.ts      # ConvertError + fromBackendError() (rebuilds it from either backend's throw)
├─ backend.ts      # detectBackend(): "wasm" | "tauri"
├─ wasm/
│  ├─ client.ts    # main-thread facade: owns the Worker, posts jobs, returns Promises
│  ├─ worker.ts    # runs INSIDE the Worker — the only place conv-wasm is actually called
│  └─ protocol.ts   # the postMessage message shapes between the two
└─ tauri/
   └─ client.ts    # calls @tauri-apps/api — see "The Tauri contract" below
```

## Why `getEngine()` is async

`getEngine(): Promise<ConvEngine>`, not a plain synchronous getter. The backend it doesn't pick
still shouldn't ship: importing `./tauri/client.js` pulls in `@tauri-apps/api`, importing
`./wasm/client.js` pulls in the Worker/WASM plumbing, and neither should be in a bundle that will
never use it. `index.ts` therefore `import()`s the chosen backend dynamically instead of
statically importing both at the top of the file — a static import gives a bundler no signal to
split the unused one into its own chunk. Confirmed this mattered in practice, not just in theory:
this package's demo harness (bundler-free, see `demo/README.md`) failed outright on
`@tauri-apps/api`'s bare specifiers until `index.ts` was switched to dynamic imports.

## The WASM backend: a Web Worker, always

`WasmBackend.convert()` always runs the conversion in a dedicated Worker (`src/wasm/worker.ts`) —
non-negotiable per the ticket this shipped under, since a multi-hundred-MB conversion must never
block the UI thread. Some things worth knowing before touching this code:

- **`input` is transferred, not cloned**, when the underlying buffer qualifies (a plain
  `ArrayBuffer`, not a view into a larger one, not shared) — see `protocol.ts#transferListFor`.
  Zero-copy handoff to the Worker for the common case (bytes freshly read from a `File`), instead
  of doubling memory to copy a huge file just to hand it over. `ConvEngine.convert`'s doc comment
  states this as part of the *public contract*, not an implementation detail of this backend
  alone — a caller must not read `input` again after calling `convert()`, on either backend.
- **The `conv-wasm` module loads lazily**, on the first `convert`/`listFormats` call the Worker
  actually receives, via a dynamic `import('conv-wasm')` — not at Worker startup. A session that
  never converts anything never downloads the `.wasm` file. See "Bundle size and lazy loading"
  below for what's still ahead of this.
- **Cancellation is `AbortSignal`-based** on this package's public interface, translated
  internally to a same-thread `postMessage('cancel')` that reaches a `conv-wasm` `CancelToken` in
  the Worker. Read `crates/conv-wasm/src/progress.rs`'s module docs for what this backend can and
  can't actually interrupt today — it's a `conv-core`-wide cooperative-cancellation contract, not
  a WASM-specific limitation.
- **`getMemoryCeilingBytes()` never loads a second copy of the WASM module on the main thread** —
  it's piggybacked on the `list-formats` response (see `protocol.ts`), which already forces the
  Worker to load the module. A naive `import('conv-wasm')` on the main thread just to read one
  constant would defeat the entire point of loading it only inside the Worker.

## The Tauri contract

`TauriBackend` (`src/tauri/client.ts`) is real, typechecked TypeScript calling the stable
`@tauri-apps/api` surface — but `apps/desktop` doesn't implement the Rust side yet (it's still a
scaffold; see its README and the backlog ticket for the real cross-platform Tauri app). This is
the contract that ticket needs to implement, kept here so both sides have one place to check for
drift:

| Tauri command | Args | Returns / behavior |
| --- | --- | --- |
| `convert` | `{ jobId: number, input: Uint8Array, from: string, to: string, maxInputBytes?: number }` | Resolves with the converted bytes (`Uint8Array` or a plain number array — either is accepted). Rejects with `{ kind, message, ...details }`, the same shape `crates/conv-wasm/src/error.rs` produces — see `packages/engine/src/errors.ts#fromBackendError`, which handles either backend identically. |
| `cancel_conversion` | `{ jobId: number }` | Best-effort; `TauriBackend` doesn't await or retry it (fire-and-forget, matching this package's cooperative-cancellation contract elsewhere — see the WASM backend notes above). |
| `list_formats` | *(none)* | Resolves with `FormatInfo[]` — the native equivalent of `conv-wasm`'s `supportedFormats()`, same shape (`{ id, category, mime, extensions, canRead, canWrite }`). |

| Tauri event | Payload | When |
| --- | --- | --- |
| `conv-progress:{jobId}` | `number` (a `0..=1` fraction) | Emitted as the native conversion makes headway — only listened for when the caller passed `onProgress`. |

`TauriBackend.getMemoryCeilingBytes()` always resolves `null` — the native path is an ordinary OS
process, not a 32-bit WASM linear memory, so there's no fixed ceiling analogous to
`conv-wasm`'s to report.

## Bundle size and lazy loading

The `.wasm` artifact itself is budget-enforced in CI (`.wasm-size-budget`,
`.github/scripts/check-wasm-size.sh`) — see `crates/conv-wasm/README.md` for the current number
and the plan for splitting it per category once there's something real to split (image codecs
being the trigger). What this package controls today: the module is never in the main JS bundle
at all (it's `import()`ed inside the Worker, on first use), so a session that only ever converts,
say, a CSV pays zero WASM cost until it actually needs `conv-wasm` — which, until a text/CSV
converter exists in `conv-core`, is never.

## Cross-origin isolation (COEP/COOP)

Not required by anything this package ships today: `conv-wasm`'s WASM build is single-threaded,
and cancellation is same-thread cooperative (see above), so nothing here needs `SharedArrayBuffer`
or the cross-origin-isolation headers that gate it. It **will** become required if either (a) a
converter needs real WASM threads (`wasm-bindgen-rayon` or similar), or (b) genuine cross-thread,
mid-call cancellation is built (a `SharedArrayBuffer`-backed flag read via `Atomics.load`, instead
of today's `postMessage`-based one — see `crates/conv-wasm/src/progress.rs`'s module docs for why
that's a real, deliberate gap and not an oversight). `packages/media`'s ffmpeg-wasm path has the same
requirement for its own threading, and the legacy (pre-OSS) site had a real conflict between COEP
and third-party ad embeds — worth checking before either lands, and worth solving once, for both
packages, rather than twice.

## Demo

`demo/` is a manual QA harness proving the WASM path end to end against a real browser — not the
product UI. See `demo/README.md`.
