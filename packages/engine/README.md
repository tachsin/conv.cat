# @conv.cat/engine

The TypeScript wrapper around the conversion engine. License: **MIT**.

It exposes one interface to callers and picks the implementation at runtime: `conv-wasm`
(compiled from `crates/conv-core` via wasm-bindgen) in the browser, native `conv-core`
bindings inside the Tauri desktop app. Consumers (`apps/web`, `apps/desktop`) import this
package instead of reaching into `crates/conv-wasm` or a native addon directly — that keeps
the two apps on one API and lets the engine swap backends without call-site changes.

This package must never contain conversion logic itself (no format parsing/encoding, no
algorithms) — that belongs in `crates/conv-core`. It is glue, not an engine.

This is currently a scaffold: no runtime selection logic exists yet.
