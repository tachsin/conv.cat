//! `conv-core` — the framework-free, browser-free conversion engine for conv.cat.
//!
//! Rules for this crate:
//! - No `wasm-bindgen` types or attributes. No assumptions about a JS host, a DOM, or a
//!   browser event loop. It must compile and test as a plain native Rust crate, full stop.
//! - No web-framework or app-framework dependencies. This crate knows about formats and
//!   conversions, nothing else.
//! - The only crate in the workspace allowed to bind this to JS/WASM is `crates/conv-wasm`.
//!
//! Scaffold only: no converters live here yet. See the backlog for the format-by-format
//! porting tickets (units, images, text/data, CAD, ...).

#[cfg(test)]
mod tests {
    #[test]
    fn scaffold_compiles_and_tests_natively() {
        assert_eq!(2 + 2, 4);
    }
}
