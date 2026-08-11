//! Progress reporting and cooperative cancellation for long-running conversions.
//!
//! Conversions in this crate can run on multi-hundred-megabyte inputs, so a design that only
//! reports success or failure at the end is not good enough — a caller (a web worker driving a
//! progress bar, a desktop app with a cancel button) needs to hear from a conversion while it is
//! still running. [`ProgressSink`] is that hook. It is deliberately tiny — two methods, both
//! cheap to call often — so a converter can check in from inside a hot decode loop without
//! worrying about overhead.

/// Receives progress updates from a running conversion and can ask it to stop early.
///
/// Implement this on the caller side (a JS callback wrapped by `conv-wasm`, a Tauri event
/// channel, a CLI progress bar) and pass it in via [`crate::ConvertOptions::progress`].
/// Converters call [`ProgressSink::on_progress`] as they make headway and poll
/// [`ProgressSink::is_cancelled`] at points where stopping early is safe (e.g. between rows of a
/// raster image, between records of a CSV) — never mid-write of a single atomic unit of output.
///
/// Both methods take `&self`, not `&mut self`: implementations are expected to use interior
/// mutability (an `AtomicBool` for cancellation, an atomic counter or channel for progress) so
/// the same sink can be shared across threads via `Arc` without a lock on the hot path.
pub trait ProgressSink: Send + Sync {
    /// Called with a fraction in `0.0..=1.0` as the conversion makes headway.
    ///
    /// Converters that cannot cheaply estimate progress (most can, at least by bytes consumed or
    /// rows processed) may call this rarely, or only at `0.0`/`1.0` — callers must not assume
    /// evenly-spaced or frequent updates.
    fn on_progress(&self, fraction: f32);

    /// Polled by a converter at safe checkpoints; returning `true` asks it to stop and return
    /// [`crate::ConvertError::Cancelled`] instead of a result.
    ///
    /// Defaults to `false` so implementing only [`ProgressSink::on_progress`] is enough for a
    /// sink that only wants a progress bar, not a cancel button.
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// A [`ProgressSink`] that discards every update and never asks for cancellation.
///
/// Useful when a caller's own API always wants a concrete sink rather than threading an `Option`
/// through — for example `crates/conv-wasm` wrapping a JS progress callback that might not have
/// been supplied — plug this in instead of writing a one-off no-op type.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopProgress;

impl ProgressSink for NoopProgress {
    fn on_progress(&self, _fraction: f32) {}
}
