# Adding a format

This is the single doc you need to ship a new conversion. Read this end to end before opening a
PR that adds a format, and you shouldn't need to go spelunking through the rest of the codebase.

**Read this first if you don't write Rust: format work means writing Rust.** Every conversion
algorithm lives in `crates/conv-core`, a plain Rust crate — see
[`docs/ARCHITECTURE.md`](ARCHITECTURE.md). There is no "add a converter in TypeScript" path;
that boundary is deliberate (it's what lets the same conversion run natively on desktop, not just
in the browser). If Rust isn't your thing, that's genuinely fine — translation
(`packages/data/src/i18n`), format catalog metadata, docs, and testing are all real contributions
that don't touch `crates/conv-core`. This doc still shows you where those pieces live, at the end.

Video and audio are the one exception: they go through ffmpeg-wasm in `packages/media`, not
`crates/conv-core`. If your format is a video or audio container/codec, stop reading this and see
[`docs/ARCHITECTURE.md` § The media boundary](ARCHITECTURE.md#the-media-boundary-video-and-audio-stay-on-ffmpeg-wasm)
instead — wrapping ffmpeg in Rust is explicitly out of scope and that PR will be closed.

> **A note on timing.** The `Converter` trait and format registry described below have landed
> (backlog ticket "conv-core foundation") — `crates/conv-core/src/converter.rs`'s rustdoc on
> `Converter` is the authoritative version if anything here has drifted; please send a docs PR to
> fix the drift rather than working around it silently. Units is the first real category to land
> (backlog ticket "First vertical slice: port units to conv-core" — a representative 8-category
> subset) — see the callout below for why its walkthrough isn't this one. No image or text/data
> converter exists yet, so this doc's QOI walkthrough is still the best template for those.

## Worked example: adding QOI encoding to the image category

We'll walk through adding [QOI](https://qoiformat.org/) (Quite OK Image) as an output format for
the image category — a real, currently-missing format, small enough to show end to end. The same
six steps apply to any file-shaped format in any category (images, text/data).

> **Units — and any other non-file-shaped category — don't follow this walkthrough as-is.** The
> `Converter` trait is `&[u8] -> Vec<u8>`, built around file bytes in, file bytes out. A unit
> conversion is `(value, from_unit, to_unit) -> value` — there's no file to decode. Rather than
> bending this walkthrough's steps to fit, units established its own convention: one `Format`
> variant per *category* (not per unit — the legacy catalog's 977 units would mean 977 hand-written
> variants), registered as a self-pair, with the actual from-unit/to-unit identity carried in a
> small hand-parsed UTF-8 text payload instead of `Format` itself. Full writeup, including why, is
> in `crates/conv-core/src/formats/units/mod.rs`'s module docs — read that first if your category
> is numeric/structured data rather than a file format. **Timezones** (Phase 3) is the next
> category expected to need this pattern rather than the QOI one.

### Step 1 — Decide the format id and where the code lives

Conversion code is organized by category under `crates/conv-core/src/formats/`:

```
crates/conv-core/src/
├─ lib.rs                # crate docs, the `convert`/`convert_with` entry points, default registry
├─ converter.rs          # the `Converter` trait — start here
├─ registry.rs           # the `Format`/`Category` enums + the `Registry` dispatch table
├─ options.rs            # `ConvertOptions`
├─ progress.rs           # `ProgressSink`, the progress/cancellation hook
├─ error.rs              # `ConvertError`
└─ formats/
   ├─ mod.rs
   ├─ identity.rs         # placeholder passthrough converter — not a real format, ignore it
   ├─ units/              # ← doesn't exist yet; you add it for your category
   ├─ text/                # ← doesn't exist yet; you add it for your category
   └─ image/               # ← doesn't exist yet; you add it for your category
      ├─ mod.rs
      ├─ png.rs
      ├─ bmp.rs
      └─ qoi.rs           # ← new file
```

Pick a short, lowercase, stable id for the format — `qoi` — you'll reuse it as the `Format` enum
variant name, the catalog entry id, and the i18n key segment. Changing it later is a breaking
change across three files, so get it right up front (match the format's own name/extension where
one obviously exists).

### Step 2 — Implement the `Converter` trait

Every converter implements one trait, defined in `crates/conv-core/src/converter.rs` (its rustdoc
is the authoritative version — this is a summary):

```rust
// crates/conv-core/src/converter.rs
pub trait Converter: Send + Sync {
    fn convert(
        &self,
        input: &[u8],
        from: Format,
        to: Format,
        options: &ConvertOptions,
    ) -> Result<Vec<u8>, ConvertError>;
}
```

Your implementation:

```rust
// crates/conv-core/src/formats/image/qoi.rs

use crate::{ConvertError, ConvertOptions, Converter, Format};
use crate::formats::image::raster::{decode_raster, RawImage};

pub struct QoiEncoder;

impl Converter for QoiEncoder {
    fn convert(
        &self,
        input: &[u8],
        from: Format,
        _to: Format,
        _options: &ConvertOptions,
    ) -> Result<Vec<u8>, ConvertError> {
        let image: RawImage = decode_raster(input, from)
            .map_err(|_| ConvertError::MalformedInput { format: from })?;

        Ok(encode_qoi(&image))
    }
}

fn encode_qoi(image: &RawImage) -> Vec<u8> {
    // ... the actual QOI encoding algorithm goes here ...
    todo!()
}
```

Rules the `Converter` rustdoc calls out explicitly, because they're easy to get wrong on the
first pass:

- **No `unwrap()`/`expect()`/`panic!()` reachable from `input`.** These converters run on
  untrusted bytes; a panic here is a denial-of-service, not a crash you can shrug off. Return
  `ConvertError::MalformedInput` (or a more specific variant) instead. This is treated as a
  security bug — see [SECURITY.md](SECURITY.md).
- **No `wasm-bindgen`, `web-sys`, or `js-sys` in `crates/conv-core`.** If you find yourself
  reaching for a browser API, the code belongs in `crates/conv-wasm` instead, translating between
  JS-friendly shapes and this crate's plain Rust API — see that crate's README.
- **Use the progress/cancellation hook if the work is long-running.** Poll
  `options.is_cancelled()` between iterations (rows, frames, records) and return
  `ConvertError::Cancelled` promptly once it flips; call `options.report_progress(fraction)` when
  you have a natural unit to measure by. A fast, single-pass converter can get away with checking
  once — see `IdentityConverter` in `crates/conv-core/src/formats/identity.rs` for the minimal
  version of this.

### Step 3 — Register it in the format registry

Add the enum variant and its metadata in `crates/conv-core/src/registry.rs`. `Format` is
`#[non_exhaustive]`, precisely so adding a variant here isn't a breaking change for
`crates/conv-wasm` or any other downstream consumer — you don't need to do anything extra for
that, just be aware existing `match`es elsewhere already have a wildcard arm:

```rust
pub enum Format {
    PlainText, // the foundation's placeholder — leave it alone
    Png,
    Bmp,
    Qoi, // ← new
    // ...
}

impl Format {
    pub fn id(&self) -> &'static str {
        match self {
            Format::Qoi => "qoi",
            // ...
        }
    }

    pub fn category(&self) -> Category {
        match self {
            Format::Qoi => Category::Image,
            // ...
        }
    }

    pub fn mime(&self) -> &'static str {
        match self {
            Format::Qoi => "image/qoi",
            // ...
        }
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Format::Qoi => &["qoi"],
            // ...
        }
    }

    pub fn can_read(&self) -> bool {
        match self {
            Format::Qoi => true,
            // ...
        }
    }

    pub fn can_write(&self) -> bool {
        match self {
            Format::Qoi => true,
            // ...
        }
    }
}
```

Then register the conversion pair(s) your converter handles, in the function that builds the
default registry (`default_registry` in `crates/conv-core/src/lib.rs`):

```rust
registry.register(Format::Png, Format::Qoi, Box::new(formats::image::qoi::QoiEncoder));
registry.register(Format::Bmp, Format::Qoi, Box::new(formats::image::qoi::QoiEncoder));
```

You do not need to touch `crates/conv-wasm` or `packages/engine` for a format that uses existing
option types — dispatch is table-driven, so a new registry entry is visible through the WASM and
native bindings automatically. You only touch those layers if your format needs a genuinely new
`ConvertOptions` field that doesn't exist yet.

### Step 4 — Golden-file tests

This is not optional, and it's what makes a stranger's converter PR reviewable at all — see
[`docs/ARCHITECTURE.md` § The conformance suite](ARCHITECTURE.md#the-conformance-suite). The
harness (fixture layout, golden-file helpers, malformed-input guard) already exists —
`crates/conv-core/tests/support/mod.rs` — so a new format's job is to add fixtures and a handful
of `#[test]` functions, not to build any of this from scratch. See
`crates/conv-core/tests/fixtures/README.md` for the full corpus rules.

Fixtures live under `crates/conv-core/tests/fixtures/<category>/<format>/`, `<format>` being the
*target* format's id:

```
crates/conv-core/tests/
├─ golden.rs                # one #[test] per fixture case, `mod support;`
├─ support/mod.rs            # the shared harness — golden compare, malformed-input guard, etc.
└─ fixtures/
   ├─ README.md
   └─ image/
      └─ qoi/
         ├─ checker_4x4.png        # small, self-generated, licence-clean input
         ├─ checker_4x4.qoi        # expected output — byte-exact, QOI is deterministic
         └─ truncated.png.bad      # deliberately corrupt input
```

Keep fixtures tiny (a handful of pixels is enough to exercise the encoding) and either
self-generated or CC0 — never commit an image pulled off the web into this repo. `.bad` is the
convention for a deliberately-corrupt fixture, so it can't be mistaken for a real asset.

```rust
// crates/conv-core/tests/golden.rs

mod support;

use conv_core::Format;

#[test]
fn png_to_qoi_matches_golden() {
    support::run_golden_case(
        "image/qoi/checker_4x4.png",
        "image/qoi/checker_4x4.qoi",
        Format::Png,
        Format::Qoi,
    );
}

#[test]
fn truncated_png_returns_typed_error_not_panic() {
    support::assert_malformed_produces_typed_error(
        "image/qoi/truncated.png.bad",
        Format::Png,
        Format::Qoi,
    );
}
```

`support::run_golden_case` reads both files off disk, converts through the real public
`conv_core::convert` API, and asserts a byte-exact match — QOI is a deterministic, lossless
encoder, so that's the right assertion. `support::assert_malformed_produces_typed_error` runs the
conversion on a watchdog thread with a timeout and catches a panic instead of letting it escape, so
a converter that panics or hangs on hostile input fails as a normal, readable test failure instead
of crashing the test binary or hanging CI — that failure mode is proven independently (without
needing a real parser) in `crates/conv-core/tests/golden_harness_selftest.rs`.

If your format is a lossy encoder (JPEG-style), don't add a byte-exact golden at all — assert
structural properties instead: `support::assert_size_within_tolerance` (dimensions/size within a
tolerance band) and `support::assert_starts_with_magic` (a valid header), rather than a byte-exact
diff that breaks on every upstream codec version bump. See [CONTRIBUTING.md](CONTRIBUTING.md) for
the rule on regenerating goldens when a diff is legitimate.

Finally, add every new fixture file to the `assert_no_stray_files` manifest at the bottom of
`golden.rs` — that test fails the build if a fixture exists that no test actually exercises (a
typo'd filename, forgotten `.bad` case, or accidental OS cruft).

Run it with:

```bash
cargo test -p conv-core
```

If a change legitimately alters a converter's output, regenerate the affected goldens with:

```bash
UPDATE_GOLDENS=1 cargo test -p conv-core --test golden
```

then review the diff and explain it in your PR description — see
[CONTRIBUTING.md](CONTRIBUTING.md#running-tests).

### Step 5 — Add the catalog entry

Format metadata that the UI needs (display grouping, icons, ordering) lives in
`packages/data`, separate from the Rust registry so `apps/web` and `apps/desktop` can read it
without linking Rust:

```json
// packages/data/src/catalogs/image.json
{
  "id": "qoi",
  "category": "image",
  "labelKey": "formats.image.qoi.name",
  "mime": "image/qoi",
  "extensions": ["qoi"],
  "canRead": true,
  "canWrite": true
}
```

Keep the `id` identical to the Rust `Format::id()` string from Step 3 — that's the join key
between the catalog and the engine, and nothing currently enforces they match, so a typo here
silently breaks that format in the UI without any error. Double-check it by hand until a lint
exists for it.

### Step 6 — Add the i18n keys

Display strings are translation keys, never hardcoded English, and never returned as strings from
`conv-core` (see [`docs/ARCHITECTURE.md` § Typed errors and identifiers](ARCHITECTURE.md#typed-errors-and-identifiers-not-human-strings)).
Add the key to every locale file under `packages/data/src/i18n/translations/`:

```json
// packages/data/src/i18n/translations/en.json
{
  "formats": {
    "image": {
      "qoi": {
        "name": "QOI (Quite OK Image)",
        "description": "A simple, fast, lossless image format."
      }
    }
  }
}
```

conv.cat targets six locales (`en`, `de`, `el`, `es`, `fr`, `tr`). Add the same key path to all
six files. If you don't speak all of them, put the English string in every locale file rather
than skipping the key — a missing key falls back to raw English mid-sentence at runtime, which
is worse than an untranslated-but-present string, and it's exactly the drift problem the i18n
re-architecture effort exists to prevent (see [ROADMAP.md](ROADMAP.md)). Say in your PR
description which locales you left in English so a translator can follow up. There is no CI
key-drift check yet for the new repo, so a maintainer checks this by hand — see
[docs/ai-contributions.md](ai-contributions.md) if this is an AI-assisted contribution, since a
plausible-looking but wrong translation is exactly the kind of thing that needs a human check.

### Step 7 — Open the PR

Use the checklist in the PR template. For a format contribution specifically, a reviewer expects:

- [ ] The converter implements `Converter` and lives under `crates/conv-core/src/formats/<category>/`.
- [ ] It is registered in `crates/conv-core/src/registry.rs`.
- [ ] Golden fixtures exist for at least one valid conversion and one malformed-input case.
- [ ] `cargo test -p conv-core` and `cargo clippy -- -D warnings` are clean.
- [ ] The catalog entry exists in `packages/data` and its `id` matches the Rust `Format` id.
- [ ] The i18n key exists in `en.json` at minimum, with a note on which other locales are covered.
- [ ] `./.github/scripts/check-licence-boundary.sh` passes (it will, unless you imported from `apps/*`).

## If you don't write Rust

Real contributions that never touch `crates/conv-core`:

- **Translation** — fill in a missing i18n key or a whole locale in `packages/data/src/i18n`.
- **Catalog data** — unit definitions, timezone data, format metadata in `packages/data`.
- **Docs** — this file included; if a step above turned out to be wrong once the real trait
  landed, that's a docs PR.
- **Testing** — expanding the golden-fixture corpus for an existing converter, including more
  malformed-input cases, doesn't require writing the converter itself.

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and PR conventions that apply to all of the above.
