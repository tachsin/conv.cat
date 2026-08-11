# conv-wasm

wasm-bindgen bindings over `conv-core`, compiled to WASM for the browser engine. License: **MIT**.

This is the only crate in the workspace allowed to depend on `wasm-bindgen` or reference
browser/JS-shaped types (`JsValue`, `web-sys`, `js-sys`, ...). Keep it a thin translation
layer over `crates/conv-core`'s plain Rust API — no conversion logic of its own. The build
output here is consumed by `packages/engine`, which is what `apps/web` actually imports; app
code should never reach into this crate directly.

This is currently a scaffold: only a placeholder binding exists, to prove the wasm-bindgen
toolchain wires up end to end. Real bindings land as `conv-core` grows converters.
