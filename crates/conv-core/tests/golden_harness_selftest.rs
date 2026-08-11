//! Proves the golden-file harness's own failure-detection machinery works, independent of any
//! real converter existing yet: a converter that panics on malformed input must be caught and
//! reported as a clean test failure (not crash the test binary), and a converter that hangs must
//! be caught within a timeout (not hang CI forever). See `tests/support/mod.rs` for the
//! implementation and `docs/ARCHITECTURE.md#the-conformance-suite` for why this matters — this
//! harness doubles as the security regression suite for contributed converters, so it has to be
//! trustworthy before the next contributor's converter leans on it.
//!
//! Uses local test-double `Converter`s registered into a throwaway `Registry`, dispatched via
//! `conv_core::convert_with` — never the crate's default registry — so this file exercises only
//! the harness, not any real conversion behaviour.

mod support;

use std::time::Duration;

use conv_core::{convert_with, ConvertError, ConvertOptions, Converter, Format, Registry};

use support::GuardedOutcome;

struct PanicsOnConvert;

impl Converter for PanicsOnConvert {
    fn convert(
        &self,
        _input: &[u8],
        _from: Format,
        _to: Format,
        _options: &ConvertOptions,
    ) -> Result<Vec<u8>, ConvertError> {
        panic!("intentional panic — harness self-test double, not a real converter");
    }
}

struct HangsOnConvert;

impl Converter for HangsOnConvert {
    fn convert(
        &self,
        _input: &[u8],
        _from: Format,
        _to: Format,
        _options: &ConvertOptions,
    ) -> Result<Vec<u8>, ConvertError> {
        // Far longer than any timeout used below — the point is proving the guard reports
        // `TimedOut` well before this would ever complete, not actually waiting it out.
        std::thread::sleep(Duration::from_secs(3600));
        Ok(Vec::new())
    }
}

struct FailsCleanly;

impl Converter for FailsCleanly {
    fn convert(
        &self,
        _input: &[u8],
        from: Format,
        _to: Format,
        _options: &ConvertOptions,
    ) -> Result<Vec<u8>, ConvertError> {
        Err(ConvertError::MalformedInput { format: from })
    }
}

struct SucceedsUnconditionally;

impl Converter for SucceedsUnconditionally {
    fn convert(
        &self,
        input: &[u8],
        _from: Format,
        _to: Format,
        _options: &ConvertOptions,
    ) -> Result<Vec<u8>, ConvertError> {
        Ok(input.to_vec())
    }
}

fn registry_with(converter: Box<dyn Converter>) -> Registry {
    let mut registry = Registry::new();
    registry.register(Format::PlainText, Format::PlainText, converter);
    registry
}

#[test]
fn guard_catches_a_panicking_converter_without_crashing_the_test_binary() {
    let registry = registry_with(Box::new(PanicsOnConvert));
    let outcome = support::try_call_guarded(Duration::from_secs(2), move || {
        convert_with(
            &registry,
            b"x",
            Format::PlainText,
            Format::PlainText,
            &ConvertOptions::default(),
        )
    });

    assert!(
        matches!(outcome, GuardedOutcome::Panicked(_)),
        "expected the guard to catch the panic and report `Panicked`, got {outcome:?} — if this \
         regresses, a genuinely panicking converter could take down the whole test run instead of \
         failing one test"
    );
}

#[test]
fn guard_detects_a_hanging_converter_within_the_timeout() {
    let registry = registry_with(Box::new(HangsOnConvert));
    let outcome = support::try_call_guarded(Duration::from_millis(200), move || {
        convert_with(
            &registry,
            b"x",
            Format::PlainText,
            Format::PlainText,
            &ConvertOptions::default(),
        )
    });

    assert!(
        matches!(outcome, GuardedOutcome::TimedOut),
        "expected the guard to time out on a hanging converter and report `TimedOut`, got \
         {outcome:?} — if this regresses, a converter that hangs on hostile input could hang CI \
         instead of failing it"
    );
}

#[test]
fn guard_reports_a_clean_typed_error_as_typed_error() {
    let registry = registry_with(Box::new(FailsCleanly));
    let outcome = support::try_call_guarded(Duration::from_secs(2), move || {
        convert_with(
            &registry,
            b"x",
            Format::PlainText,
            Format::PlainText,
            &ConvertOptions::default(),
        )
    });

    assert!(
        matches!(
            outcome,
            GuardedOutcome::TypedError(ConvertError::MalformedInput { .. })
        ),
        "expected `TypedError(MalformedInput)`, got {outcome:?}"
    );
}

#[test]
fn guard_reports_an_unexpected_success_as_unexpected_success() {
    let registry = registry_with(Box::new(SucceedsUnconditionally));
    let outcome = support::try_call_guarded(Duration::from_secs(2), move || {
        convert_with(
            &registry,
            b"x",
            Format::PlainText,
            Format::PlainText,
            &ConvertOptions::default(),
        )
    });

    assert!(
        matches!(outcome, GuardedOutcome::UnexpectedSuccess(_)),
        "expected `UnexpectedSuccess`, got {outcome:?} — a fixture that's supposed to be \
         malformed but converts cleanly should be caught the same way a real malformed-input \
         regression would be"
    );
}
