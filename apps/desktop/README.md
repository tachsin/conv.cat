# @conv.cat/desktop

The Tauri desktop app — cross-platform conv.cat on the native `conv-core` engine, no WASM
round-trip. License: **AGPL-3.0-only**.

Like `apps/web`, this package is UI/shell only. No conversion logic, no format parsing, no
catalog data. It talks to the conversion engine through `@conv.cat/engine`, which resolves to
the native `conv-core` bindings here instead of the WASM build used in the browser. If it needs
a converter that doesn't exist yet, that converter is added to `crates/conv-core`, not here.

This is currently a scaffold: no Tauri project has been initialized yet. See the backlog for
the ticket that wires up the real cross-platform app.
