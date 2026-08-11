# conv-core

The conversion engine, as a plain native Rust crate. License: **MIT**.

This crate is framework-free and browser-free by rule, not by accident:

- No `wasm-bindgen` types or attributes anywhere in here. No assumptions about a JS host,
  a DOM, or a browser event loop.
- It must compile and test with `cargo build` / `cargo test`, from this directory or from
  the workspace root, with zero WASM tooling involved. If a change here requires `wasm-pack`
  or a JS runtime to build or test, it does not belong in this crate.
- The only crate allowed to expose this to JS/WASM is `crates/conv-wasm`, via wasm-bindgen
  bindings that wrap this crate's plain Rust API.

The foundation is in place: the `Converter` trait, the `Format`/`Category` registry, the typed
`ConvertError` enum, the `convert`/`convert_with` dispatch entry points, and a progress/
cancellation hook (`ConvertOptions`/`ProgressSink`) — see `src/lib.rs`'s rustdoc for the tour, and
[`docs/adding-a-format.md`](../../docs/adding-a-format.md) for a full worked example. No real
converters exist yet beyond a placeholder identity conversion used to exercise the pipeline in
tests — see the backlog for the format-by-format porting tickets (units, images, text/data,
...) that add real ones.
