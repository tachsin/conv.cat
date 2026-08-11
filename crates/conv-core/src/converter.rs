//! The [`Converter`] trait: the one interface every conversion algorithm in this crate
//! implements.

use crate::{ConvertError, ConvertOptions, Format};

/// Converts bytes from one [`Format`] to another.
///
/// This is the only trait a format contribution needs to implement — see
/// `docs/adding-a-format.md` for a full worked example (adding QOI image encoding) and
/// `docs/ARCHITECTURE.md` for how it fits into the rest of conv.cat.
///
/// # Implementing a new converter
///
/// 1. Pick (or reuse) a `from` and `to` [`Format`]. A single type can implement `Converter` for
///    more than one pair if the logic is genuinely shared (e.g. one struct handling both
///    `Png -> Qoi` and `Bmp -> Qoi` by decoding to a common raster representation first) — the
///    trait doesn't know or care which pair it was registered for; `from`/`to` are passed in so a
///    shared implementation can still branch on them if it needs to.
/// 2. Implement [`Converter::convert`]: decode `input` as `from`, produce bytes shaped like `to`,
///    respecting whatever `options` sets.
/// 3. Register it for its pair(s) with [`crate::Registry::register`] — in the default registry
///    that backs [`crate::convert`] (see the function that builds it in `crates/conv-core/src/lib.rs`)
///    for a real, shipping format.
///
/// # Rules an implementation must follow
///
/// - **Never panic on `input`.** No `unwrap()`, `expect()`, `panic!()`, unchecked slice indexing,
///   or unchecked arithmetic on anything derived from the byte slice. These bytes come from a
///   file nobody has vetted; a panic reachable from user input is a denial-of-service, and is
///   treated as a security bug in this crate (`docs/SECURITY.md`). Return
///   [`ConvertError::MalformedInput`] (or a more specific variant) instead — every failure mode
///   this trait needs is represented in [`ConvertError`], so there should never be a reason to
///   reach for `unwrap`.
/// - **Respect cancellation at safe checkpoints.** If the conversion is long-running (loops over
///   rows, frames, records, …), poll [`ConvertOptions::is_cancelled`] between iterations and
///   return [`ConvertError::Cancelled`] promptly once it does — don't only check once at the
///   start. A trivial, fast converter (this crate's own
///   [`crate::formats::identity::IdentityConverter`] included) can get away with checking once,
///   since there is no meaningful in-progress state to interrupt.
/// - **Report progress when the work has a natural unit to measure.** Call
///   [`ConvertOptions::report_progress`] with a `0.0..=1.0` fraction as you go — by bytes
///   consumed, rows processed, frames encoded, whatever fits the format. Not every converter can
///   do this meaningfully (a single-pass in-memory transform might only ever report `0.0` then
///   `1.0`), and that's fine; callers must not assume even spacing or a minimum number of calls.
/// - **No `wasm-bindgen`/`web-sys`/`js-sys`.** Implementations are plain, native, framework-free
///   Rust — see the crate-level docs. If a converter needs a browser API, that glue belongs in
///   `crates/conv-wasm`, not here.
/// - **Stay in-memory for now.** `input`/the return value are plain byte slices/vectors, not
///   `Read`/`Write` streams. This is a deliberate MVP boundary, not an oversight — see the note
///   on [`crate::convert`] for the reasoning and what mitigates it today (the size-limit and
///   cancellation hooks on [`ConvertOptions`]).
///
/// Implementations must be [`Send`] + [`Sync`]: the registry stores them behind `Box<dyn
/// Converter>` in a process-wide static, and both a browser worker and a native desktop thread
/// pool need to be able to call through it.
pub trait Converter: Send + Sync {
    /// Performs the conversion.
    ///
    /// `from`/`to` are passed even though a given `Converter` is usually registered for exactly
    /// one pair, so an implementation shared across several pairs can still branch on them.
    ///
    /// Returns the converted bytes, or a typed [`ConvertError`] — must never panic, for any value
    /// of `input`, however malformed.
    fn convert(
        &self,
        input: &[u8],
        from: Format,
        to: Format,
        options: &ConvertOptions,
    ) -> Result<Vec<u8>, ConvertError>;
}
