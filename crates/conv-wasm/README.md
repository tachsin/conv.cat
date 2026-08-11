# conv-wasm

wasm-bindgen bindings over `conv-core`, compiled to WASM for the browser engine. License: **MIT**.

This is the only crate in the workspace allowed to depend on `wasm-bindgen`/`js-sys` or reference
browser/JS-shaped types (`JsValue`, `web-sys`, ...). It stays a thin translation layer over
`crates/conv-core`'s plain Rust API — no conversion logic of its own. The build output here is
consumed by `packages/engine`, which is what `apps/web`/`apps/desktop` actually import; app code
should never reach into this crate directly.

## What it exposes

- `convert(input, from, to, options?)` — dispatches to `conv_core::convert`. `from`/`to` are
  format ids (`Format::id()`, e.g. `"plain_text"`), not enum values — see `src/format.rs` for the
  id ↔ `Format` mapping, hand-maintained the same way `conv-core::registry`'s own match arms are.
  Every format-by-format ticket that adds a `Format` variant to `conv-core` must add it here too.
- `supportedFormats()` — the format catalog as plain JS objects (`{ id, category, mime,
  extensions, canRead, canWrite }`), so `packages/engine` never hardcodes a format id.
- `ConvertOptions` — a JS-constructible builder (`setMaxInputBytes`, `setOnProgress`,
  `setCancelToken`) rather than a decoded object bag, to avoid a serialization dependency
  (`serde-wasm-bindgen` et al.) just to parse an options argument.
- `CancelToken` — a JS-visible handle wrapping an `Arc<AtomicBool>`. See `src/progress.rs`'s
  module docs for exactly when cancelling one takes effect — it's same-thread cooperative
  (matching `conv-core`'s own contract for `ProgressSink::is_cancelled`), not preemptive.
- `memoryCeilingBytes()` — this build's hard input-size ceiling, in bytes. See below.
- Every error (`convert()` rejecting, an unrecognized format id) is a typed JS object thrown as
  the exception, shaped `{ kind, message, ...details }` — never a bare string. `kind` mirrors
  `conv_core::ConvertError`'s variants plus a couple of binding-level ones (`unknown_format`,
  `memory_limit_exceeded`). See `src/error.rs` and `packages/engine`'s `ConvertErrorKind`, which
  must stay in sync with this.

## Memory ceiling

wasm32-unknown-unknown has a 32-bit address space — 4 GiB is the hard architectural limit, not a
practical one, since a conversion needs room for the input bytes *and* the output *and* the
engine's own bookkeeping at once. `convert()` rejects input over `HARD_MEMORY_CEILING_BYTES`
(~3.26 GiB, see `src/lib.rs`) with a typed `memory_limit_exceeded` error before ever handing bytes
to `conv-core`, rather than letting a real conversion run into an allocation failure mid-flight.
`memoryCeilingBytes()` exposes the same constant so `packages/engine` can reject an oversized
`File` before even reading it into memory.

## Progress and cancellation

`ConvertOptions.setOnProgress`/`setCancelToken` wire straight into `conv_core::ProgressSink`.
Read `src/progress.rs`'s module docs before assuming more than this crate actually guarantees —
in short: a JS progress callback fires synchronously from inside the (synchronous)
`Converter::convert` call, and cancellation is only observed at whatever checkpoint the
converter itself polls, same as every other host `conv-core` supports. Nothing here is
WASM-specific; today's only registered converter (the plain-text passthrough) has no internal
checkpoints to poll, so cancelling a call already in flight isn't meaningfully testable yet —
that changes once a real, chunked converter lands.

## Bundle size and lazy loading

CI enforces a size budget on the release `.wasm` artifact (`.wasm-size-budget` at the repo root,
`.github/scripts/check-wasm-size.sh`). Today's build is a single artifact for the whole crate —
there's nothing to split yet, since only one converter (the identity passthrough) exists. The plan
for when that stops being true (tracked against the first codec-heavy category, image conversion):
gate `conv-core` categories behind Cargo features on this crate, build one `wasm-pack` artifact per
feature set, and have `packages/engine`'s category → loader map pick the right one — so a CSV-only
session never downloads image codecs. Not built now because there is nothing real to split; see
`packages/engine/README.md` "Bundle size and lazy loading" for what *is* lazy today (the whole
module is loaded on first use inside a Worker, not at page load).

## Building it

```bash
wasm-pack build crates/conv-wasm --target web --out-dir pkg --release
```

`--target web` emits a plain ES module — no bundler-specific loader required. See
`docs/BUILD.md` for the full pipeline, and note the build order: `pnpm install` at the repo root
now depends on this having already run once, since `packages/engine` has a real `file:`
dependency on `pkg/` (see its `package.json`). `scripts/build-all.sh` and CI both already build in
that order; only matters if you're running commands by hand.

## Testing

`cargo test -p conv-wasm` runs, but is deliberately narrow: any test that actually constructs a
`JsValue` at runtime (an error object, a `js_sys::Object`) aborts the whole test binary on a plain
native `cargo test` — wasm-bindgen's JS-host externs aren't implemented outside a real JS engine,
and the failure mode is a hard process abort (`SIGABRT`), not a catchable panic. See `src/lib.rs`'s
`tests` module comment for what was actually observed hitting this and why the affected test was
removed rather than worked around. A `wasm-bindgen-test`-based suite (`wasm-pack test --node` or
`--headless`) would cover the rest, but needs a browser or Node WASM runtime in CI that doesn't
exist yet — a deliberate follow-up, not an oversight. In the meantime,
`packages/engine/demo/README.md` documents a manual, real-browser verification path, and that path
was actually run (Chrome, in-session) to confirm `convert()`'s happy path, its typed-error path,
and pre-start cancellation all work through the real artifact — not just compiled, but executed.

## Status

Real bindings — no longer a scaffold. Grows one `Format` id at a time alongside `conv-core`'s own
format tickets.
