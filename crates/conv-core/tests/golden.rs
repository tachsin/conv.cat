//! The golden-file conformance suite: the executable spec every `conv-core` converter must
//! satisfy. See `tests/fixtures/README.md` for the corpus layout and
//! `docs/ARCHITECTURE.md#the-conformance-suite` for why it exists.
//!
//! One `#[test]` per fixture case, grouped by category/format — this mirrors the fixture tree on
//! disk and matches the pattern documented in
//! `docs/adding-a-format.md#step-4--golden-file-tests`. When you add a fixture, add its files to
//! the `assert_no_stray_files` manifest at the bottom of this file too: that test fails the build
//! if a fixture exists that no test actually exercises.

mod support;

use conv_core::Format;

// ─── text / plain_text ──────────────────────────────────────────────────────
//
// `IdentityConverter` (PlainText -> PlainText) is a passthrough, so every golden file here is
// byte-identical to its input by construction. That's deliberate: these cases exist to prove the
// fixture-driven harness itself — reading real files off disk, dispatching through the public
// `conv_core::convert` API, comparing byte-for-byte, end to end — so the next contributor adding
// a real format (image, units, text/CAD — see the backlog) has a proven pattern to copy, not just
// documentation to trust.

#[test]
fn plain_text_identity_hello() {
    support::run_golden_case(
        "text/plain_text/hello.input.txt",
        "text/plain_text/hello.golden.txt",
        Format::PlainText,
        Format::PlainText,
    );
}

#[test]
fn plain_text_identity_empty_input() {
    support::run_golden_case(
        "text/plain_text/empty.input.txt",
        "text/plain_text/empty.golden.txt",
        Format::PlainText,
        Format::PlainText,
    );
}

#[test]
fn plain_text_identity_non_utf8_bytes_pass_through_unchanged() {
    // Guards against a future edit accidentally adding UTF-8 validation to `IdentityConverter` —
    // conv-core converters operate on `&[u8]`, not `&str`, and this format's contract is
    // byte-transparent, not text-transparent. This fixture intentionally contains invalid UTF-8.
    support::run_golden_case(
        "text/plain_text/non_utf8_bytes.input.txt",
        "text/plain_text/non_utf8_bytes.golden.txt",
        Format::PlainText,
        Format::PlainText,
    );
}

// ─── malformed-input corpus ─────────────────────────────────────────────────
//
// No `.bad` fixtures exist yet, and that's expected rather than a gap: `IdentityConverter`
// performs no parsing at all, so it has no malformed-input failure mode to exercise — every byte
// sequence is "valid" to it by definition. The first converter that actually parses its input
// (image conversion is next, backlog ticket f737b331) must add at least one `.bad` fixture here
// and a call to `support::assert_malformed_produces_typed_error`, per
// `docs/adding-a-format.md#step-4--golden-file-tests`. `assert_no_stray_files` below fails the
// build if a `.bad` fixture ever gets committed without a matching test, so this corpus can't
// silently rot once real fixtures land.
//
// The malformed-input harness itself (panic containment + hang timeout) is proven independently
// of any real parser in `tests/golden_harness_selftest.rs`, using test-double converters.

// ─── corpus hygiene ──────────────────────────────────────────────────────────

#[test]
fn fixtures_tree_has_no_stray_or_orphaned_files() {
    support::assert_no_stray_files(&[
        "README.md",
        "text/plain_text/hello.input.txt",
        "text/plain_text/hello.golden.txt",
        "text/plain_text/empty.input.txt",
        "text/plain_text/empty.golden.txt",
        "text/plain_text/non_utf8_bytes.input.txt",
        "text/plain_text/non_utf8_bytes.golden.txt",
    ]);
}
