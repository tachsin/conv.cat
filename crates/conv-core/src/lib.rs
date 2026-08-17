//! `conv-core` — the framework-free, browser-free conversion engine for conv.cat.
//!
//! ## Rules for this crate
//! - No `wasm-bindgen` types or attributes. No assumptions about a JS host, a DOM, or a browser
//!   event loop. It must compile and test as a plain native Rust crate, full stop.
//! - No web-framework or app-framework dependencies. This crate knows about formats and
//!   conversions, nothing else — zero dependencies today; any dependency added here needs to
//!   earn its place given that constraint.
//! - The only crate in the workspace allowed to bind this to JS/WASM is `crates/conv-wasm`.
//! - **Panics on untrusted input are a security bug, not a crash to shrug off.** Every converter
//!   parses bytes nobody has vetted. Return a typed [`ConvertError`] instead of `unwrap`,
//!   `expect`, `panic!`, unchecked indexing, or unchecked arithmetic on anything derived from
//!   `input`. See `docs/SECURITY.md`.
//!
//! See `docs/ARCHITECTURE.md` for how this crate fits into the rest of conv.cat, and
//! `docs/adding-a-format.md` for a full worked example of implementing [`Converter`] end to end.
//!
//! ## Quick tour
//! - [`Format`] / [`Category`] — the format catalog: id, category, MIME type, extensions,
//!   read/write capability.
//! - [`Converter`] — the trait every conversion algorithm implements. **Start here** if you're
//!   adding a format; its rustdoc is the authoritative implementation guide.
//! - [`Registry`] — the `(from, to) -> Converter` dispatch table.
//! - [`convert`] — the entry point most callers use: resolves the default registry and
//!   dispatches. [`convert_with`] does the same against a caller-supplied [`Registry`].
//! - [`ConvertError`] — the typed error enum; see its own docs for the "typed identifiers, not
//!   strings" rule.
//! - [`ConvertOptions`] / [`ProgressSink`] — per-call knobs, including the progress/cancellation
//!   hook every long-running converter is expected to use.
//!
//! Only a single placeholder format/converter is registered today ([`formats::identity`]) — real
//! formats land one at a time via the format-by-format backlog tickets.

#![warn(missing_docs)]

mod converter;
mod error;
mod options;
mod progress;
mod registry;

pub mod formats;

pub use converter::Converter;
pub use error::ConvertError;
pub use options::ConvertOptions;
pub use progress::{NoopProgress, ProgressSink};
pub use registry::{Category, Format, Registry};

use std::sync::OnceLock;

/// Converts `input` from `from` to `to`, dispatching through the process-wide default
/// [`Registry`].
///
/// This is the one entry point most callers need: the desktop app's native bindings and
/// `crates/conv-wasm`'s wasm-bindgen glue both call this (or [`convert_with`]) rather than
/// resolving converters themselves.
///
/// Before dispatching, this function:
/// 1. Rejects input larger than [`ConvertOptions::max_input_bytes`], if set, with
///    [`ConvertError::SizeLimitExceeded`] — without ever handing the bytes to a converter.
/// 2. Checks [`ConvertOptions::is_cancelled`] once up front, so a caller that constructs an
///    already-cancelled sink and calls this doesn't pay for a registry lookup.
/// 3. Resolves `from -> to` in the registry, returning [`ConvertError::UnsupportedPair`] if
///    nothing is registered for it.
///
/// The registered [`Converter`] is then responsible for its own finer-grained cancellation and
/// progress reporting during the actual conversion work.
///
/// ## Why `&[u8]` in, `Vec<u8>` out, and not a stream
///
/// This crate's converters run on multi-hundred-megabyte files, and loading a multi-gigabyte one
/// fully into memory (doubly so inside a WASM linear memory) is a known failure mode — so
/// "streaming eventually" is a real goal, not a nice-to-have. This trait shape is nonetheless a
/// deliberate MVP boundary rather than an oversight: a genuinely streaming `Read`/`Write`-based
/// `Converter` signature is a much bigger design (it has to work across the wasm-bindgen
/// boundary too, not just natively) and doesn't have a concrete first consumer yet — video/audio,
/// the category most likely to hit multi-gigabyte inputs, is explicitly routed through
/// `packages/media`/ffmpeg-wasm instead of this crate (see `docs/ARCHITECTURE.md`'s media
/// boundary section). What ships today to make large inputs safe in the meantime:
/// [`ConvertOptions::max_input_bytes`] caps how much a caller will even attempt, and
/// [`ConvertOptions::progress`]/cancellation lets a caller abort a slow in-memory conversion
/// early instead of only finding out at the end. If a format category genuinely needs streaming
/// before a redesign happens, that's a signal to revisit this signature, not to work around it.
pub fn convert(
    input: &[u8],
    from: Format,
    to: Format,
    options: &ConvertOptions,
) -> Result<Vec<u8>, ConvertError> {
    convert_with(default_registry(), input, from, to, options)
}

/// Same as [`convert`], but dispatches through a caller-supplied `registry` instead of the
/// process-wide default one.
///
/// Public so an embedder that wants a smaller or custom format set (a test harness, a future CLI
/// built with a reduced registry) gets the same size-limit and cancellation handling [`convert`]
/// does, instead of having to reimplement it.
pub fn convert_with(
    registry: &Registry,
    input: &[u8],
    from: Format,
    to: Format,
    options: &ConvertOptions,
) -> Result<Vec<u8>, ConvertError> {
    if let Some(limit) = options.max_input_bytes {
        let actual = input.len() as u64;
        if actual > limit {
            return Err(ConvertError::SizeLimitExceeded { limit, actual });
        }
    }

    if options.is_cancelled() {
        return Err(ConvertError::Cancelled);
    }

    let converter = registry
        .resolve(from, to)
        .ok_or(ConvertError::UnsupportedPair { from, to })?;

    converter.convert(input, from, to, options)
}

/// The process-wide default registry [`convert`] dispatches through, built once on first use.
fn default_registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut registry = Registry::new();
        registry.register(
            Format::PlainText,
            Format::PlainText,
            Box::new(formats::identity::IdentityConverter),
        );

        // Image: one converter, registered for every ordered pair drawn from the raster formats
        // it supports — see `formats::image::FORMATS` for why a loop instead of one
        // `registry.register` call per direction, and its module docs for why BMP/QOI and not
        // more (yet).
        for &from in formats::image::FORMATS {
            for &to in formats::image::FORMATS {
                if from != to {
                    registry.register(from, to, Box::new(formats::image::RasterConverter));
                }
            }
        }

        // Units: one converter, registered for every in-scope category's self-pair — see
        // `formats::units` module docs for why units are modeled as self-pairs rather than
        // distinct from/to `Format`s.
        for format in [
            Format::UnitsLength,
            Format::UnitsMass,
            Format::UnitsVolume,
            Format::UnitsCooking,
            Format::UnitsTemperature,
            Format::UnitsFuelConsumption,
            Format::UnitsLifeAge,
            Format::UnitsClothingSize,
        ] {
            registry.register(format, format, Box::new(formats::units::UnitsConverter));
        }

        registry
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_has_the_placeholder_pair_registered() {
        let registry = default_registry();
        assert!(registry
            .resolve(Format::PlainText, Format::PlainText)
            .is_some());
    }

    #[test]
    fn convert_with_reports_unsupported_pair_for_an_empty_registry() {
        let empty = Registry::new();
        let result = convert_with(
            &empty,
            b"x",
            Format::PlainText,
            Format::PlainText,
            &ConvertOptions::default(),
        );
        assert!(matches!(result, Err(ConvertError::UnsupportedPair { .. })));
    }
}
