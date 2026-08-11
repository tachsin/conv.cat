//! Integration tests against `conv-core`'s public API only (no `mod` access to crate internals)
//! — this is what a downstream consumer (`conv-wasm`, a future CLI, a third-party embedder)
//! actually sees. Real golden-fixture tests (`tests/fixtures/<category>/<format>/...`) land per
//! format, as each one ships — see `docs/adding-a-format.md`. This file only proves the
//! foundation itself (dispatch, errors, the progress/cancellation hook) works end to end.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use conv_core::{
    convert, convert_with, ConvertError, ConvertOptions, Format, ProgressSink, Registry,
};

#[test]
fn identity_placeholder_converts_end_to_end() {
    let input = b"round trip me";
    let output = convert(
        input,
        Format::PlainText,
        Format::PlainText,
        &ConvertOptions::default(),
    )
    .expect("PlainText -> PlainText is registered by default");

    assert_eq!(output, input);
}

#[test]
fn unsupported_pair_is_a_typed_error_not_a_panic() {
    // Nothing is registered on a fresh `Registry`, so dispatching through it via `convert_with`
    // exercises the exact "nobody implemented this pair" path `convert` would take once a second
    // `Format` variant exists to ask for a genuinely different pair.
    let empty = Registry::new();
    let result = convert_with(
        &empty,
        b"anything",
        Format::PlainText,
        Format::PlainText,
        &ConvertOptions::default(),
    );

    assert!(matches!(result, Err(ConvertError::UnsupportedPair { .. })));
}

#[test]
fn size_limit_is_enforced_before_a_converter_ever_sees_the_bytes() {
    let options = ConvertOptions {
        max_input_bytes: Some(1),
        ..ConvertOptions::default()
    };

    let result = convert(b"ab", Format::PlainText, Format::PlainText, &options);

    assert!(matches!(
        result,
        Err(ConvertError::SizeLimitExceeded {
            limit: 1,
            actual: 2
        })
    ));
}

/// A [`ProgressSink`] that reports itself cancelled from the first check, and records whether it
/// was ever asked.
struct AlwaysCancelled {
    was_polled: AtomicBool,
}

impl ProgressSink for AlwaysCancelled {
    fn on_progress(&self, _fraction: f32) {}

    fn is_cancelled(&self) -> bool {
        self.was_polled.store(true, Ordering::SeqCst);
        true
    }
}

#[test]
fn cancellation_hook_short_circuits_the_conversion() {
    let sink = Arc::new(AlwaysCancelled {
        was_polled: AtomicBool::new(false),
    });
    let options = ConvertOptions {
        progress: Some(sink.clone() as Arc<dyn ProgressSink>),
        ..ConvertOptions::default()
    };

    let result = convert(b"data", Format::PlainText, Format::PlainText, &options);

    assert!(matches!(result, Err(ConvertError::Cancelled)));
    assert!(sink.was_polled.load(Ordering::SeqCst));
}
