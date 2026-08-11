# conv-wasm demo harness

A manual QA page proving the WASM path end to end: a real file, converted through Rust
(`conv-core`) → WASM (`conv-wasm`) → this package's public `@conv.cat/engine` interface, running
in a Web Worker so the UI thread never blocks. **This is not the product UI** — the real
single-page converter shell is a separate, not-yet-built backlog ticket ("apps/web: the
single-page converter shell"). This exists so the architecture this ticket shipped
(`crates/conv-wasm` + `packages/engine`) has a runnable proof in the repo, not just tests.

Deliberately crude (no design, no framework, inline styles) so nobody mistakes it for the real
thing.

## Running it

From the repo root:

```bash
./scripts/build-all.sh          # or, minimally: build conv-wasm's pkg/, then `pnpm build`
pnpm --filter @conv.cat/engine demo
```

Then open the printed URL (`http://localhost:8787/` by default). Pick a file, leave both format
selects on `plain_text` (the only pair registered today — see `crates/conv-core`'s README for
why), hit Convert, and download the result. It should be byte-identical to the input: the only
converter registered anywhere right now is the identity passthrough used to exercise the
pipeline before any real format lands (see `crates/conv-core/src/formats/identity.rs`).

## Why a hand-rolled static server

The browser's ES module loader only resolves bare specifiers (`import ... from "conv-wasm"`) via
an [import map](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/script/type/importmap)
— there's no bundler here to do it the way `apps/web`'s real build eventually will. `index.html`
declares one mapping `"conv-wasm"` to the file pnpm actually linked
(`../node_modules/conv-wasm/conv_wasm.js`, from this package's own `file:` dependency — see
`package.json`). `serve.mjs` is a zero-dependency `node:http` server whose only real job is
getting the `.wasm` file's `Content-Type` right (`application/wasm` — `WebAssembly.instantiateStreaming`
checks it; getting this wrong doesn't break the demo, but would quietly mask exactly the kind of
production server misconfiguration this exists to catch early).

**Import maps and module Workers**: verified empirically (Chrome 151, in-session) that the
document's import map does **not** apply inside the module Worker `wasm/client.ts` spawns — an
import map is scoped to the `Window` that declares it, and that scope doesn't extend to a Worker's
own module graph. `wasm/worker.ts`'s `import('conv-wasm')` (correct for a real bundler-based
consumer, which resolves a Worker's bare specifiers itself) therefore fails here with "Failed to
resolve module specifier" if served unmodified. `serve.mjs` works around this the same way it
handles the `.wasm` MIME type: it patches *only the HTTP response* for
`dist/wasm/worker.js`, rewriting `import('conv-wasm')` to the resolved relative path — `dist/` on
disk is untouched, and the rewrite is isolated to this dev-only server, not `packages/engine`'s
real source. Not a concern for the real `apps/web` build — a bundler resolves the Worker's bare
specifiers as part of building it, the same way it resolves every other import.

## What this deliberately doesn't cover

- The native/Tauri path — see `packages/engine/README.md` "The Tauri contract" for what exists
  today (a real `TauriBackend` written and typechecked against a documented command contract) and
  what doesn't (an actual `apps/desktop` Tauri app to run it in).
- Any real format conversion — there isn't one yet. This harness will get more interesting to use
  as `crates/conv-core` grows real converters; it doesn't need to change to support them, since it
  already drives everything through `listFormats()` rather than hardcoding `plain_text`.
