# conv-core golden-file fixtures

This directory is the executable spec every `conv-core` converter must satisfy — see
[`docs/ARCHITECTURE.md` § The conformance suite](../../../../docs/ARCHITECTURE.md#the-conformance-suite)
for why it exists and
[`docs/adding-a-format.md` Step 4](../../../../docs/adding-a-format.md#step-4--golden-file-tests)
for how a new format's fixtures fit in.

## Layout

```
tests/fixtures/<category>/<format>/
```

`<category>` matches `conv_core::Category` (`image`, `text`, `units`); `<format>` matches
the target format's `conv_core::Format::id()`, per `docs/adding-a-format.md`.

## Fixture kinds

- **Valid golden pair** — `<name>.<source-ext>` (input) + `<name>.<target-ext>` (expected output,
  byte-exact) for a deterministic encoder. Where source and target format are the same
  (identity-style conversions) and the extensions would collide, use `<name>.input.<ext>` /
  `<name>.golden.<ext>` instead — see `text/plain_text/` for an example.
- **Malformed input** — any file ending in `.bad`. Must make the converter return a typed
  `ConvertError`, never panic or hang. Test it with
  `tests/support::assert_malformed_produces_typed_error` (see `tests/golden.rs`).
- **Lossy output** — for a non-deterministic/lossy encoder (JPEG, WebP), don't add a byte-exact
  golden at all; assert structural properties instead (dimensions, valid header, size within a
  tolerance band) using the helpers in `tests/support/mod.rs`
  (`assert_size_within_tolerance`, `assert_starts_with_magic`).

## Rules

- Keep fixtures tiny — a handful of pixels/rows/bytes is enough to exercise an encoder.
- Self-generated or CC0 only. Never commit an image, document, or model pulled off the web into
  this repo, even a small one — licence provenance has to be unambiguous.
- Every file in this directory must be exercised by a test in `crates/conv-core/tests/*.rs`, and
  listed in the `assert_no_stray_files` manifest in that test file. This is enforced: an orphaned
  fixture (or accidental OS cruft like `.DS_Store`) fails `cargo test`.

## Regenerating a golden file

```bash
UPDATE_GOLDENS=1 cargo test -p conv-core --test golden
```

This overwrites every golden file exercised by `tests/golden.rs` with the corresponding
converter's *current* output.

**This is not a routine fix.** A passing `UPDATE_GOLDENS` run is not a green light to commit. If a
converter's output legitimately changed (a bug fix, an upstream codec update), regenerate,
`git diff` the result, and explain *what* changed and *why the new output is correct* in your PR
description — see [`CONTRIBUTING.md`](../../../../docs/CONTRIBUTING.md#running-tests). A PR that
silently regenerates goldens just to make a failing test pass is the one case that gets sent back
without review.

## What exists today

- `text/plain_text/` — golden fixtures for `IdentityConverter`, the placeholder
  `PlainText -> PlainText` passthrough that exercises the registry/dispatch pipeline (see
  `crates/conv-core/src/formats/identity.rs`). No `.bad` fixture here — `IdentityConverter`
  performs no parsing and has no malformed-input failure mode to exercise. The malformed-input
  harness itself (panic containment + hang timeout) is proven independently, without needing a
  real parser, in `crates/conv-core/tests/golden_harness_selftest.rs`.
- `units/<category>/` — one directory per in-scope units category (`length`, `temperature`,
  `fuel_consumption`, `life_age`, `clothing_size`, ...), see `crates/conv-core/src/formats/units/`.
- `image/{bmp,qoi,png}/` — the raster hub (see `crates/conv-core/src/formats/image/`): every
  ordered pair among BMP/QOI/PNG, plus malformed-input cases per format. PNG's harder decoder
  cases (real dynamic-Huffman-compressed input, every PNG scanline filter type) are covered as
  Rust unit tests in `png.rs` itself instead of fixtures here — see that module's doc comment for
  why: they check the codec against an independently-built PNG, not the public `conv_core::convert`
  API this suite is for.

Text/data conversion (CSV, JSON, HTML, Markdown) is still an open backlog ticket and has no
directory here yet. Adding one follows the same shape as the categories above — see
`docs/adding-a-format.md` Step 4.
