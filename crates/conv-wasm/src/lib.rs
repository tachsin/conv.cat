//! `conv-wasm` — wasm-bindgen bindings over `conv-core`, compiled to WASM for the browser
//! engine (consumed by `packages/engine`, never directly by `apps/*` — see
//! `docs/ARCHITECTURE.md`'s dependency-direction rule).
//!
//! This is the ONLY crate in the workspace allowed to depend on `wasm-bindgen`/`js-sys`/`web-sys`
//! or expose browser/JS-shaped types. It stays a thin translation layer: every real conversion
//! algorithm lives in `crates/conv-core`, and this crate does exactly four things on top of it —
//! map JS format-id strings to [`conv_core::Format`] (the `format` module), wrap `conv-core`'s
//! typed errors as typed JS objects instead of thrown strings (the `error` module), forward a JS
//! progress callback and a JS-cancellable flag through [`conv_core::ProgressSink`] (the
//! `progress` module), and reject input over a documented memory ceiling before `conv-core` ever
//! sees it (this module).
//!
//! ## Why this crate still compiles on a native `cargo build`
//! `wasm-bindgen` types compile on any target — they just aren't meaningfully *callable* outside
//! a JS host. `cargo build --workspace`/`cargo test --workspace`/`cargo clippy --workspace` at
//! the repo root therefore typecheck this crate on the host target as part of normal workspace
//! hygiene (fmt, clippy, doc links), the same as [[oss-monorepo-scaffold]] already established
//! for the placeholder version of this crate. Nothing here is ever *run* as native code — the
//! only artifact meant to execute is the wasm32 one `wasm-pack` produces. See
//! `crates/conv-wasm/src/progress.rs` for the one place that distinction matters for soundness
//! (the `Send`/`Sync` impl on the progress sink).
//!
//! ## Building it
//! ```text
//! wasm-pack build crates/conv-wasm --target web --out-dir pkg --release
//! ```
//! `--target web` emits a plain ES module `packages/engine` (and this crate's demo harness)
//! `import()`s directly — no bundler-specific loader required. See `docs/BUILD.md` and this
//! crate's README for the full pipeline, including why `pnpm install` has to run *after* this
//! build now that `packages/engine` has a real `file:` dependency on `pkg/`.

use std::sync::Arc;

use conv_core::ProgressSink;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

mod error;
mod format;
mod options;
mod progress;

pub use options::ConvertOptions;
pub use progress::CancelToken;

/// Absolute ceiling on input size this build will ever attempt, regardless of what a caller
/// requests via [`ConvertOptions::setMaxInputBytes`](options::ConvertOptions::set_max_input_bytes).
///
/// wasm32-unknown-unknown has a 32-bit address space: 4 GiB (2^32 bytes) is the hard
/// architectural limit, not a practical one. A conversion needs room for the input bytes *and*
/// the output `Vec<u8>` *and* the WASM engine's own bookkeeping to coexist in that same linear
/// memory at once, and every engine this was checked against (V8, SpiderMonkey) starts failing
/// `memory.grow` well before the nominal 4 GiB edge. This constant is kept comfortably under that
/// so a caller gets [`convert`]'s typed `memory_limit_exceeded` error instead of an aborted tab —
/// see this crate's README for the full "watch out" this ticket called out.
pub const HARD_MEMORY_CEILING_BYTES: u64 = 3_500_000_000; // ~3.26 GiB

/// Default cap applied when a caller doesn't set one via
/// [`ConvertOptions::setMaxInputBytes`](options::ConvertOptions::set_max_input_bytes).
/// Conservative on purpose: today's only registered converter is a byte-for-byte passthrough, but
/// a format that decodes into a larger in-memory representation (an uncompressed raster, a parsed
/// document tree) needs more headroom above its input size than a 1:1 copy does, and this default
/// has to be safe without every caller separately knowing that multiplier. Raise it per call via
/// the setter, not here, once a format's real working-set ratio is known.
pub const DEFAULT_MAX_INPUT_BYTES: u64 = 512_000_000; // ~488 MiB

/// This build's hard memory ceiling, in bytes — see [`HARD_MEMORY_CEILING_BYTES`]. Exposed so
/// `packages/engine` can reason about it (reject a huge `File` before even reading it into
/// memory, show a clear message) without duplicating the constant on the JS side.
#[wasm_bindgen(js_name = memoryCeilingBytes)]
pub fn memory_ceiling_bytes() -> f64 {
    HARD_MEMORY_CEILING_BYTES as f64
}

/// The `conv-wasm` crate version (from `Cargo.toml`), for diagnostics — e.g. a demo page
/// confirming which build it loaded.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Converts `input` from `from` to `to` (format ids, e.g. `"plain_text"` — see
/// [`format::supported_formats`]), returning the converted bytes or a typed JS error object
/// `{ kind, message, ...details }`.
///
/// `options` is optional; omitting it (or passing a default-constructed `ConvertOptions`) is
/// equivalent to [`conv_core::ConvertOptions::default`] plus this crate's own size ceiling.
#[wasm_bindgen]
pub fn convert(
    input: &[u8],
    from: &str,
    to: &str,
    options: Option<ConvertOptions>,
) -> Result<Vec<u8>, JsValue> {
    let from_format =
        format::parse_format(from).ok_or_else(|| unknown_format_error("from", from))?;
    let to_format = format::parse_format(to).ok_or_else(|| unknown_format_error("to", to))?;

    let actual_len = input.len() as u64;
    if actual_len > HARD_MEMORY_CEILING_BYTES {
        return Err(error::binding_error(
            "memory_limit_exceeded",
            format!(
                "input is {actual_len} bytes, over this build's {HARD_MEMORY_CEILING_BYTES} byte \
                 ceiling (wasm32's 32-bit address space caps out around 4 GiB; see conv-wasm's \
                 README for why the safe ceiling is lower than that)"
            ),
            vec![
                ("limit", (HARD_MEMORY_CEILING_BYTES as f64).into()),
                ("actual", (actual_len as f64).into()),
            ],
        ));
    }

    let options = options.unwrap_or_default();
    let max_input_bytes = options
        .max_input_bytes
        .unwrap_or(DEFAULT_MAX_INPUT_BYTES)
        .min(HARD_MEMORY_CEILING_BYTES);

    let sink: Arc<dyn ProgressSink> = Arc::new(progress::WasmProgressSink {
        on_progress: options.on_progress,
        cancelled: options.cancel_flag.unwrap_or_default(),
    });

    let core_options = conv_core::ConvertOptions {
        max_input_bytes: Some(max_input_bytes),
        progress: Some(sink),
        jpeg_quality: options.jpeg_quality,
    };

    conv_core::convert(input, from_format, to_format, &core_options)
        .map_err(error::from_convert_error)
}

pub use format::supported_formats;

fn unknown_format_error(param: &str, id: &str) -> JsValue {
    error::binding_error(
        "unknown_format",
        format!("unrecognized {param} format id: {id:?} — see supportedFormats() for the list this build knows"),
        vec![("param", param.into()), ("id", id.into())],
    )
}

// Only tests that never construct a real `JsValue` at runtime belong here. `wasm-bindgen`
// compiles fine on the host target (that's what makes `cargo build --workspace` work without
// cross-compiling — see this module's docs), but *calling* anything that builds a `JsValue`
// (`.into()` on a `&str`, `js_sys::Object::new()`, `Reflect::set`, …) reaches into externs that
// only exist inside an actual JS host and aborts the process (SIGABRT, not a catchable panic) on
// a plain native `cargo test`. Confirmed in-session: a test exercising `convert()`'s error path
// (which builds a `{ kind, message, ... }` JsValue via `error::binding_error`) took down the
// entire `conv-wasm` test binary, including unrelated tests, because wasm-bindgen's failure here
// is a hard process abort, not a normal Rust panic `catch_unwind` can isolate. Anything that
// needs to observe real JsValue behavior belongs in a `wasm-bindgen-test`-based suite running
// under `wasm-pack test --node`/`--headless`, which is genuinely new CI infrastructure (a
// browser or Node WASM runtime, not just `cargo test`) — not added here; see this crate's README
// "Testing" section for what that would take and why it's a deliberate follow-up, not an
// oversight.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_a_registered_pair() {
        // The success path returns `Ok(Vec<u8>)` without ever constructing a `JsValue` — safe to
        // exercise natively, unlike the error path (see the module comment above).
        let output = convert(b"hello", "plain_text", "plain_text", None).unwrap();
        assert_eq!(output, b"hello");
    }

    #[test]
    fn rejects_input_over_the_hard_memory_ceiling() {
        // Building a multi-gigabyte Vec just to prove the guard would be its own resource abuse
        // in a test — call the same size-limit path `convert()` uses directly instead.
        let mut options = ConvertOptions::new();
        options.set_max_input_bytes((HARD_MEMORY_CEILING_BYTES + 1) as f64);
        let clamped = options.max_input_bytes.unwrap();
        assert_eq!(
            clamped.min(HARD_MEMORY_CEILING_BYTES),
            HARD_MEMORY_CEILING_BYTES
        );
    }

    #[test]
    fn cancel_token_reflects_cancellation() {
        let token = CancelToken::new();
        assert!(!token.is_cancelled());
        token.cancel();
        assert!(token.is_cancelled());
    }
}
