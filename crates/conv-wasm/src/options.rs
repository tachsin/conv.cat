//! [`ConvertOptions`]: the JS-constructible builder for [`crate::convert`]'s per-call knobs.
//!
//! A plain wasm-bindgen struct with setters, not a JS object decoded field-by-field — keeps this
//! crate free of a serialization dependency (`serde-wasm-bindgen` et al.) purely to parse an
//! options bag, which would cost more of the size budget than the feature is worth. `js_sys` is
//! already a dependency for the callback/error plumbing, so this reuses it rather than adding to
//! it.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use js_sys::Function;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::progress::CancelToken;

/// Builder for [`crate::convert`]'s optional third argument. Construct with `new
/// ConvertOptions()`, call the setters that apply, pass it in — every setter is optional, and
/// `convert()` treats an entirely-default `ConvertOptions` the same as passing none at all.
#[wasm_bindgen]
#[derive(Default)]
pub struct ConvertOptions {
    pub(crate) max_input_bytes: Option<u64>,
    pub(crate) on_progress: Option<Function>,
    pub(crate) cancel_flag: Option<Arc<AtomicBool>>,
    pub(crate) jpeg_quality: Option<u8>,
}

#[wasm_bindgen]
impl ConvertOptions {
    /// Creates an options builder with no limit, no progress callback, and no cancel token —
    /// equivalent to `conv_core::ConvertOptions::default()` plus this crate's own default size
    /// ceiling (see [`crate::DEFAULT_MAX_INPUT_BYTES`]).
    #[wasm_bindgen(constructor)]
    pub fn new() -> ConvertOptions {
        ConvertOptions::default()
    }

    /// Caps input size for this call. Values above this build's hard ceiling
    /// ([`crate::HARD_MEMORY_CEILING_BYTES`], see [`crate::memory_ceiling_bytes`]) are silently
    /// clamped down to it rather than honored — this setter can only make the effective limit
    /// tighter, never looser than what a 32-bit WASM build can safely attempt.
    #[wasm_bindgen(js_name = setMaxInputBytes)]
    pub fn set_max_input_bytes(&mut self, bytes: f64) {
        // `bytes` arrives as an f64 (the only numeric type wasm-bindgen hands JS `number`s as);
        // clamp to non-negative before the lossy-but-safe cast; JS `Number.MAX_SAFE_INTEGER` is
        // already far above `HARD_MEMORY_CEILING_BYTES`, so precision loss above that never
        // matters — the value gets clamped down long before it would.
        self.max_input_bytes = Some(bytes.max(0.0) as u64);
    }

    /// Registers a callback invoked with a `0.0..=1.0` fraction as the conversion makes headway.
    /// See [`conv_core::ProgressSink::on_progress`]'s docs for what "headway" means — not every
    /// converter reports it evenly, or more than twice.
    #[wasm_bindgen(js_name = setOnProgress)]
    pub fn set_on_progress(&mut self, callback: Function) {
        self.on_progress = Some(callback);
    }

    /// Attaches a [`CancelToken`] whose `cancel()` this call will honor at its next checkpoint.
    /// See `crates/conv-wasm/src/progress.rs`'s module docs for exactly what "next checkpoint"
    /// means today. Takes the token by reference — the caller keeps its own handle and can keep
    /// calling `cancel()`/`isCancelled()` on it after this call returns.
    #[wasm_bindgen(js_name = setCancelToken)]
    pub fn set_cancel_token(&mut self, token: &CancelToken) {
        self.cancel_flag = Some(token.flag());
    }

    /// JPEG encode quality, `1..=100` — see
    /// [`conv_core::ConvertOptions::jpeg_quality`](conv_core::ConvertOptions). Ignored by every
    /// other format's encoder; out-of-range values are clamped, not rejected.
    #[wasm_bindgen(js_name = setJpegQuality)]
    pub fn set_jpeg_quality(&mut self, quality: u8) {
        self.jpeg_quality = Some(quality);
    }
}
