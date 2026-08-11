//! `conv-wasm` — wasm-bindgen bindings over `conv-core`, compiled to WASM for the browser
//! engine (consumed by `packages/engine`).
//!
//! This is the ONLY crate in the workspace allowed to depend on `wasm-bindgen` or expose
//! browser/JS-shaped types. Keep it thin: it should translate between JS-friendly shapes and
//! `conv-core`'s plain Rust API, not implement any conversion logic itself.
//!
//! Scaffold only: no real bindings yet, just a placeholder that proves the wasm-bindgen
//! toolchain wires up end to end.

use wasm_bindgen::prelude::*;

/// Scaffold placeholder. Replace with real bindings as `conv-core` grows converters.
#[wasm_bindgen]
pub fn scaffold_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
