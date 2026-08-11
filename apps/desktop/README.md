# @conv.cat/desktop

The Tauri desktop app — cross-platform conv.cat on the native `conv-core` engine, no WASM
round-trip. License: **AGPL-3.0-only**.

Like `apps/web`, this package is UI/shell only. No conversion logic, no format parsing, no
catalog data. It talks to the conversion engine through `@conv.cat/engine`, which resolves to
the native `conv-core` bindings here instead of the WASM build used in the browser. If it needs
a converter that doesn't exist yet, that converter is added to `crates/conv-core`, not here.

This is currently a scaffold: no Tauri project has been initialized yet (no `src-tauri/`, no
`tauri.conf.json`, nothing added to the Cargo workspace). See the backlog for the ticket that
wires up the real cross-platform app.

## The contract this app owes `packages/engine`

`packages/engine`'s `TauriBackend` (`packages/engine/src/tauri/client.ts`) is already written and
typechecked against a specific set of Tauri commands/events — it just has nothing to call yet.
The full, authoritative contract (exact args/return shapes) lives in
`packages/engine/README.md` "The Tauri contract"; the short version is three commands
(`convert`, `cancel_conversion`, `list_formats`) and one event pattern
(`conv-progress:{jobId}`), all backed directly by `crates/conv-core` — no WASM round-trip, no
`crates/conv-wasm` involvement (that crate is browser-only). Implementing that contract, not
reinventing a different one, is what makes `apps/web` and this app interchangeable behind
`@conv.cat/engine`.
