//! Shared test-harness support for the golden-file conformance suite (`tests/golden.rs`,
//! `tests/golden_harness_selftest.rs`). See `tests/fixtures/README.md` for the fixture corpus
//! this drives, and `docs/ARCHITECTURE.md#the-conformance-suite` for why this suite exists.
//!
//! This module is compiled fresh into each integration-test binary that does `mod support;` —
//! that's how Cargo treats every file under `tests/` as its own crate. Not every binary uses
//! every helper here (`golden.rs` only needs the golden/hygiene helpers today; the malformed/hang
//! detection machinery is proven in `golden_harness_selftest.rs` instead), so `dead_code` is
//! allowed at the module level rather than per-binary.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use conv_core::{ConvertError, ConvertOptions, Format};

/// Default guard timeout for [`try_call_guarded`] / [`assert_malformed_produces_typed_error`].
///
/// Five seconds is generous for the tiny fixtures this corpus is meant to hold (see
/// `tests/fixtures/README.md`) — a well-behaved converter should return in milliseconds on input
/// this small. If a legitimate case genuinely needs longer, pass a bigger timeout explicitly
/// rather than raising this default for every case.
pub const DEFAULT_GUARD_TIMEOUT: Duration = Duration::from_secs(5);

/// Absolute path to `crates/conv-core/tests/fixtures/`.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Reads a fixture file, given a path relative to [`fixtures_dir`]. Panics with the resolved path
/// and the I/O error on failure, rather than letting a bare `unwrap` obscure which fixture is
/// missing.
pub fn read_fixture(rel: &str) -> Vec<u8> {
    let path = fixtures_dir().join(rel);
    fs::read(&path).unwrap_or_else(|err| panic!("failed to read fixture {path:?}: {err}"))
}

/// Runs one golden-file case: converts the bytes at `input_rel` from `from` to `to`, and asserts
/// the output is byte-for-byte identical to `golden_rel`. Both paths are relative to
/// [`fixtures_dir`].
///
/// Byte-exact comparison is only meaningful for deterministic encoders (PNG, BMP, QOI, lossless
/// text, and this crate's own `IdentityConverter`). For a lossy encoder (JPEG, WebP), assert
/// structural properties instead — see [`assert_size_within_tolerance`] and
/// [`assert_starts_with_magic`], and `docs/ARCHITECTURE.md#the-conformance-suite`.
///
/// ## Regenerating
///
/// Set `UPDATE_GOLDENS=1` to overwrite `golden_rel` with the converter's current output instead
/// of asserting against it:
///
/// ```text
/// UPDATE_GOLDENS=1 cargo test -p conv-core --test golden
/// ```
///
/// This is **not** a routine fix. A passing `UPDATE_GOLDENS` run is not a green light to commit —
/// `git diff` the regenerated file, understand why it changed, and justify the diff in the PR
/// description. See `CONTRIBUTING.md`.
pub fn run_golden_case(input_rel: &str, golden_rel: &str, from: Format, to: Format) {
    let input = read_fixture(input_rel);
    let golden_path = fixtures_dir().join(golden_rel);

    let output =
        conv_core::convert(&input, from, to, &ConvertOptions::default()).unwrap_or_else(|err| {
            panic!(
                "expected fixture {input_rel} to convert {from:?} -> {to:?} cleanly, got {err:?}"
            )
        });

    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        fs::write(&golden_path, &output)
            .unwrap_or_else(|err| panic!("failed to write golden {golden_path:?}: {err}"));
        eprintln!(
            "[UPDATE_GOLDENS] wrote {} bytes to {golden_path:?} — review the diff before committing",
            output.len()
        );
        return;
    }

    let expected = fs::read(&golden_path).unwrap_or_else(|err| {
        panic!(
            "missing golden file {golden_path:?}: {err}\n\
             run `UPDATE_GOLDENS=1 cargo test -p conv-core --test golden` to create it, then \
             review the file before committing"
        )
    });

    assert_eq!(
        output, expected,
        "golden mismatch: {input_rel} -> {golden_rel}. If this output change is intentional, \
         regenerate with `UPDATE_GOLDENS=1 cargo test -p conv-core --test golden` and explain the \
         diff in your PR description (see CONTRIBUTING.md)."
    );
}

/// Runs one golden-file case for a **lossy** encoder (JPEG, WebP — see
/// [`run_golden_case`]'s own doc comment): converts the bytes at `input_rel` from `from` to `to`
/// and asserts only that the output starts with `magic` (via [`assert_starts_with_magic`]), not
/// that it's byte-identical to anything stored on disk. `input_rel` is relative to
/// [`fixtures_dir`].
pub fn assert_convert_starts_with_magic(
    input_rel: &str,
    from: Format,
    to: Format,
    magic: &[u8],
    format_label: &str,
) {
    let input = read_fixture(input_rel);
    let output =
        conv_core::convert(&input, from, to, &ConvertOptions::default()).unwrap_or_else(|err| {
            panic!(
                "expected fixture {input_rel} to convert {from:?} -> {to:?} cleanly, got {err:?}"
            )
        });
    assert_starts_with_magic(&output, magic, format_label);
}

/// What happened when a conversion was run under [`try_call_guarded`].
#[derive(Debug)]
pub enum GuardedOutcome {
    /// The conversion returned a typed [`ConvertError`] — the expected outcome for malformed
    /// input.
    TypedError(ConvertError),
    /// The conversion succeeded when it was expected to fail.
    UnexpectedSuccess(Vec<u8>),
    /// The conversion panicked instead of returning a typed error. Treated as a security bug
    /// (see `docs/SECURITY.md`) — converters must never panic on untrusted input.
    Panicked(String),
    /// The conversion did not return within the configured timeout — a possible hang/DoS on
    /// untrusted input (see `docs/SECURITY.md`).
    TimedOut,
}

/// Runs `f` on a dedicated thread with a hard timeout, catching a panic instead of letting it
/// escape — the building block behind [`assert_malformed_produces_typed_error`].
///
/// A misbehaving converter (one that panics, or loops forever) must never be allowed to bring
/// down the whole `cargo test` run or hang CI indefinitely; this turns either failure mode into
/// an ordinary, diagnosable [`GuardedOutcome`] instead.
///
/// If `f` times out, its thread is deliberately not joined — there is no safe way to cancel a std
/// thread. It is left running until the test process exits, which is harmless for a test binary
/// and is exactly the DoS-shaped failure this helper exists to catch before it ever reaches
/// production.
pub fn try_call_guarded<F>(timeout: Duration, f: F) -> GuardedOutcome
where
    F: FnOnce() -> Result<Vec<u8>, ConvertError> + Send + 'static,
{
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let result = panic::catch_unwind(AssertUnwindSafe(f));
        // The receiver may already be gone if the caller timed out and moved on — that's fine.
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(Ok(bytes))) => GuardedOutcome::UnexpectedSuccess(bytes),
        Ok(Ok(Err(err))) => GuardedOutcome::TypedError(err),
        Ok(Err(payload)) => GuardedOutcome::Panicked(panic_message(&payload)),
        Err(mpsc::RecvTimeoutError::Timeout) => GuardedOutcome::TimedOut,
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            GuardedOutcome::Panicked("worker thread disconnected without a result".to_string())
        }
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Convenience wrapper over [`try_call_guarded`] for the common case: read a fixture, convert it,
/// and assert it fails with a typed [`ConvertError`] — never a success, a panic, or a hang.
///
/// This is the malformed-input half of the conformance suite: every `.bad`-suffixed fixture (see
/// `tests/fixtures/README.md`) should have a corresponding call to this function in
/// `tests/golden.rs`. Always dispatches through `conv_core::convert` (the crate's default
/// registry), matching how a real caller would hit the converter.
pub fn assert_malformed_produces_typed_error(input_rel: &str, from: Format, to: Format) {
    let input = read_fixture(input_rel);
    let owned_rel = input_rel.to_string();

    let outcome = try_call_guarded(DEFAULT_GUARD_TIMEOUT, move || {
        conv_core::convert(&input, from, to, &ConvertOptions::default())
    });

    match outcome {
        GuardedOutcome::TypedError(_) => {}
        GuardedOutcome::UnexpectedSuccess(bytes) => panic!(
            "expected malformed fixture {owned_rel} to fail, but {from:?} -> {to:?} succeeded \
             with {} bytes — either the fixture isn't actually malformed, or the converter is too \
             lenient",
            bytes.len()
        ),
        GuardedOutcome::Panicked(msg) => panic!(
            "converter PANICKED on malformed fixture {owned_rel} instead of returning a typed \
             ConvertError — treated as a security bug, see docs/SECURITY.md. panic: {msg}"
        ),
        GuardedOutcome::TimedOut => panic!(
            "converter did not return within {DEFAULT_GUARD_TIMEOUT:?} on malformed fixture \
             {owned_rel} — possible hang/DoS on untrusted input, see docs/SECURITY.md"
        ),
    }
}

/// Asserts `actual` is within `tolerance` (a fraction, e.g. `0.10` for ±10%) of `expected`.
///
/// For lossy encoders (JPEG, WebP) where a byte-exact golden would break on every upstream codec
/// version bump — see `docs/ARCHITECTURE.md#the-conformance-suite`.
pub fn assert_size_within_tolerance(actual: usize, expected: usize, tolerance: f64) {
    assert!(
        (0.0..=1.0).contains(&tolerance),
        "tolerance must be a fraction in 0.0..=1.0, got {tolerance}"
    );
    let expected_f = expected as f64;
    let actual_f = actual as f64;
    let allowed = expected_f * tolerance;
    let diff = (actual_f - expected_f).abs();
    assert!(
        diff <= allowed,
        "output size {actual} bytes is outside the ±{:.0}% tolerance band around {expected} bytes \
         (diff {diff}, allowed {allowed})",
        tolerance * 100.0
    );
}

/// Asserts `bytes` starts with `magic` — a minimal "valid header" structural check for lossy or
/// otherwise non-byte-exact formats.
pub fn assert_starts_with_magic(bytes: &[u8], magic: &[u8], format_label: &str) {
    let head = &bytes[..bytes.len().min(magic.len())];
    assert!(
        head == magic,
        "output does not start with the expected {format_label} magic bytes {magic:02x?} — got \
         {head:02x?}"
    );
}

/// Recursively lists every regular file under `dir`, as paths relative to `dir`, sorted for a
/// deterministic diff.
fn walk_relative(dir: &Path) -> Vec<PathBuf> {
    fn walk(base: &Path, current: &Path, out: &mut Vec<PathBuf>) {
        let entries = fs::read_dir(current)
            .unwrap_or_else(|err| panic!("failed to read directory {current:?}: {err}"));
        for entry in entries {
            let entry = entry
                .unwrap_or_else(|err| panic!("failed to read an entry under {current:?}: {err}"));
            let path = entry.path();
            let file_type = entry
                .file_type()
                .unwrap_or_else(|err| panic!("failed to stat {path:?}: {err}"));
            if file_type.is_dir() {
                walk(base, &path, out);
            } else if file_type.is_file() {
                let rel = path
                    .strip_prefix(base)
                    .expect("walked path is always under base")
                    .to_path_buf();
                out.push(rel);
            }
        }
    }

    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// Fails the test if `tests/fixtures/` contains a file not listed in `known` (paths relative to
/// [`fixtures_dir`], `/`-separated regardless of host OS), or if `known` lists a path that
/// doesn't exist on disk.
///
/// This is the corpus's own regression test: a stray file (OS cruft like `.DS_Store`, a typo'd
/// filename, a fixture nobody wired a test to) is exactly the kind of thing that erodes trust in
/// "the corpus is the spec" over time, so it fails loudly instead of silently accumulating.
pub fn assert_no_stray_files(known: &[&str]) {
    let known_set: BTreeSet<String> = known.iter().map(|s| s.to_string()).collect();

    let actual: Vec<String> = walk_relative(&fixtures_dir())
        .into_iter()
        .map(|p| p.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/"))
        .collect();
    let actual_set: BTreeSet<String> = actual.iter().cloned().collect();

    let unexpected: Vec<&String> = actual.iter().filter(|p| !known_set.contains(*p)).collect();
    assert!(
        unexpected.is_empty(),
        "tests/fixtures/ contains file(s) not declared as known fixtures: {unexpected:?}\n\
         Either add a covering test and list them in the `assert_no_stray_files` call, or delete \
         them if they're leftover cruft."
    );

    let missing: Vec<&String> = known_set
        .iter()
        .filter(|p| !actual_set.contains(*p))
        .collect();
    assert!(
        missing.is_empty(),
        "the fixture manifest lists file(s) that don't exist on disk: {missing:?}"
    );
}

#[cfg(test)]
mod support_self_tests {
    use super::*;

    #[test]
    fn size_within_tolerance_accepts_values_inside_the_band() {
        assert_size_within_tolerance(105, 100, 0.10);
        assert_size_within_tolerance(95, 100, 0.10);
        assert_size_within_tolerance(100, 100, 0.0);
    }

    #[test]
    #[should_panic(expected = "outside the")]
    fn size_within_tolerance_rejects_a_value_outside_the_band() {
        assert_size_within_tolerance(120, 100, 0.10);
    }

    #[test]
    fn starts_with_magic_accepts_a_matching_header() {
        assert_starts_with_magic(
            b"\x89PNG\r\n\x1a\nrest-of-file",
            b"\x89PNG\r\n\x1a\n",
            "PNG",
        );
    }

    #[test]
    #[should_panic(expected = "does not start with")]
    fn starts_with_magic_rejects_a_wrong_header() {
        assert_starts_with_magic(b"not a png", b"\x89PNG\r\n\x1a\n", "PNG");
    }

    #[test]
    #[should_panic(expected = "does not start with")]
    fn starts_with_magic_rejects_output_shorter_than_the_magic() {
        assert_starts_with_magic(b"\x89PN", b"\x89PNG\r\n\x1a\n", "PNG");
    }
}
