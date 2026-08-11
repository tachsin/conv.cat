//! Wires `conv-core`'s [`ProgressSink`] hook to a JS progress callback and a JS-cancellable flag.
//!
//! ## Cancellation is same-thread cooperative, not preemptive — read this before relying on it
//!
//! [`Converter::convert`](conv_core::Converter::convert) is one synchronous call; there is no
//! point during it where JS code runs unless the converter itself calls back into JS (which
//! [`WasmProgressSink::on_progress`] does). A [`CancelToken::cancel`] call therefore only takes
//! effect at the *next* checkpoint a converter polls [`ProgressSink::is_cancelled`] — exactly the
//! same cooperative contract `conv-core` documents for every host, not a WASM-specific weakening
//! of it. Two consequences worth being explicit about:
//!
//! - **A converter with no internal checkpoints (like [`conv_core::formats::identity`]) can't be
//!   cancelled mid-call** — there's nothing to check between. `packages/engine`'s worker still
//!   gets real, useful cancellation out of this: it can refuse to *start* an already-cancelled
//!   job (checked once, before the wasm call), and it can cancel a queued job in a batch before
//!   its turn comes up. No format registered today needs more than that.
//! - **Cross-thread cancellation** (a `cancel()` call made from the main thread reaching a
//!   conversion running in a Web Worker) needs the flag to live in memory both threads can see.
//!   [`CancelToken`] here is a plain `Arc<AtomicBool>` inside *this* WASM instance's own linear
//!   memory, which a different thread (the main thread's JS, or another Worker) cannot reach at
//!   all — Worker isolates don't share linear memory. `packages/engine`'s worker client handles
//!   this today by keeping the `CancelToken` on the worker side and cancelling via a same-thread
//!   `postMessage` the worker processes between jobs, which is sufficient until a converter
//!   actually has internal checkpoints to poll. Genuine mid-call, cross-thread cancellation needs
//!   a `SharedArrayBuffer`-backed flag read with `Atomics.load` (real shared memory, not a
//!   message), which in turn needs the COOP/COEP cross-origin-isolation headers called out in the
//!   ticket this crate shipped under — deliberately not built now because there is no converter
//!   with an internal loop to observe it working. Build it when the first one lands (tracked
//!   alongside image conversion, the first category with real per-row/per-frame checkpoints).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use conv_core::ProgressSink;
use js_sys::Function;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

/// A cancel flag JS can flip from outside the running conversion. See the module docs for exactly
/// when flipping it takes effect.
#[wasm_bindgen]
#[derive(Clone, Default)]
pub struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

#[wasm_bindgen]
impl CancelToken {
    /// Creates a token in the not-cancelled state.
    #[wasm_bindgen(constructor)]
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    /// Requests cancellation. Idempotent — calling it more than once, or after the conversion has
    /// already finished, is a no-op.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Whether [`CancelToken::cancel`] has been called. Exposed mainly for tests and diagnostics;
    /// `packages/engine` doesn't need to poll this itself.
    #[wasm_bindgen(js_name = isCancelled)]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

impl CancelToken {
    /// Clones the underlying flag out for a [`WasmProgressSink`] to share — deliberately not
    /// `pub` at the wasm-bindgen boundary; JS interacts with the flag only through
    /// [`CancelToken::cancel`]/[`CancelToken::is_cancelled`].
    pub(crate) fn flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }
}

/// The [`ProgressSink`] implementation `convert()` builds for a single call: forwards
/// `on_progress` to a JS callback (if one was supplied) and reads cancellation from a shared
/// flag (if a [`CancelToken`] was supplied).
pub(crate) struct WasmProgressSink {
    pub(crate) on_progress: Option<Function>,
    pub(crate) cancelled: Arc<AtomicBool>,
}

// SAFETY: `crates/conv-wasm` compiles to nothing but a WASM module loaded into one JS
// engine/Worker at a time — its cdylib output is never linked into a native, genuinely
// multi-threaded process. `js_sys::Function` isn't `Send`/`Sync` in general because a real OS
// thread boundary can't safely share a JS engine's function object; that boundary does not exist
// here. (This crate *is* still typechecked with a plain `cargo build --workspace` on the host
// target, per its README — that's a compile-time sanity check, not a claim that the resulting
// host binary is ever run.) `AtomicBool` is genuinely `Send + Sync` on its own; this impl only
// needs to cover the `Function` field.
unsafe impl Send for WasmProgressSink {}
unsafe impl Sync for WasmProgressSink {}

impl ProgressSink for WasmProgressSink {
    fn on_progress(&self, fraction: f32) {
        let Some(callback) = &self.on_progress else {
            return;
        };
        // A progress callback throwing is the JS caller's bug, not this conversion's failure —
        // swallow it rather than aborting an otherwise-successful convert over a broken progress
        // bar. `call1`'s `Result` is `Err` only when the JS call itself throws.
        let _ = callback.call1(&JsValue::NULL, &JsValue::from_f64(fraction as f64));
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}
