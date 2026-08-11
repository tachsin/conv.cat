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

This is currently a scaffold: no converters exist yet. See the backlog for the format-by-format
porting tickets (units, images, text/data, CAD, ...).
