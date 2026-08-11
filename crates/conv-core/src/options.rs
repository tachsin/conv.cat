//! [`ConvertOptions`]: the per-call parameters every [`crate::Converter`] receives alongside the
//! input bytes and the `from`/`to` [`crate::Format`] pair.

use std::fmt;
use std::sync::Arc;

use crate::progress::ProgressSink;

/// Per-call knobs for a conversion.
///
/// `ConvertOptions::default()` is always a safe, permissive choice (no size limit, no progress
/// sink) — callers that don't care about progress or limits can pass `&ConvertOptions::default()`
/// everywhere, as shown throughout `docs/adding-a-format.md`.
///
/// This struct is deliberately small today. Format-specific knobs (JPEG quality, CSV delimiter,
/// …) are expected to land here as fields *when a real converter needs them* (see
/// `docs/adding-a-format.md`), not added speculatively now for formats that don't exist yet.
#[derive(Clone, Default)]
pub struct ConvertOptions {
    /// If set, [`crate::convert`]/[`crate::convert_with`] reject input larger than this many
    /// bytes with [`crate::ConvertError::SizeLimitExceeded`] before a converter ever sees it.
    /// `None` means no limit is enforced by the dispatcher — a converter may still enforce its
    /// own, tighter limit (e.g. a decoded-dimensions cap) and return the same error variant.
    pub max_input_bytes: Option<u64>,

    /// Where to report progress and check for cancellation, if the caller wants either. `None`
    /// is equivalent to a sink that never cancels and discards every update — see
    /// [`ProgressSink`].
    pub progress: Option<Arc<dyn ProgressSink>>,
}

impl ConvertOptions {
    /// Reports `fraction` to the configured sink, if any. Converters should call this instead of
    /// matching on `self.progress` directly.
    pub fn report_progress(&self, fraction: f32) {
        if let Some(sink) = &self.progress {
            sink.on_progress(fraction);
        }
    }

    /// Returns `true` if a configured sink has asked the conversion to stop. Converters should
    /// poll this at safe checkpoints and return [`crate::ConvertError::Cancelled`] once it does.
    pub fn is_cancelled(&self) -> bool {
        self.progress
            .as_ref()
            .is_some_and(|sink| sink.is_cancelled())
    }
}

impl fmt::Debug for ConvertOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConvertOptions")
            .field("max_input_bytes", &self.max_input_bytes)
            .field("progress", &self.progress.is_some())
            .finish()
    }
}
