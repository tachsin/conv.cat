//! Raster image conversions — the first file-shaped category ported into `conv-core`.
//!
//! Scope today: [`Format::Bmp`](crate::Format::Bmp), [`Format::Qoi`](crate::Format::Qoi),
//! [`Format::Png`](crate::Format::Png), [`Format::Ico`](crate::Format::Ico),
//! [`Format::Webp`](crate::Format::Webp) (lossless only), [`Format::Gif`](crate::Format::Gif),
//! and [`Format::Jpeg`](crate::Format::Jpeg) (baseline only), every direction between them. This
//! crate's own module docs say "zero dependencies today; any dependency added here needs to earn
//! its place", and the release WASM artifact has a hard, CI-enforced size budget
//! (`.wasm-size-budget`) that a decoder-heavy dependency like the general-purpose `image` crate
//! would blow through — so every format here is hand-rolled rather than pulled in. BMP (no
//! compression) and QOI (a small, well-defined algorithm — see [`qoi`]) were cheap first steps;
//! PNG (see [`png`]) is the harder case this scope-out originally flagged, needing a real DEFLATE
//! codec (see [`zlib`]) rather than just chunk framing. ICO (see [`ico`]) is a different kind of
//! easy: not a codec at all, just a directory of entries that are themselves PNG (or, for
//! decoding legacy files, raw bitmap) data — it costs almost nothing once PNG already exists.
//! WebP (see [`webp`] and [`vp8l`]) is the biggest step yet: its own bespoke LZ77-and-Huffman
//! scheme (not DEFLATE), four pixel transforms, a color cache, and a "meta prefix code" mechanism
//! letting different image regions use different Huffman tables — bigger than PNG's DEFLATE, not
//! smaller, contrary to how it might look from the outside as "just another PNG-shaped format".
//! Only the lossless (VP8L) half is in scope; lossy WebP (`VP8 `) is a real intra-frame video
//! codec — DCT/WHT transform, boolean arithmetic coding, block prediction — and stays out of
//! scope for the same reason AVIF's (AV1) and HEIC's (HEVC) video-codec cores do: evaluated and
//! explicitly declined, not silently skipped — see `docs/ROADMAP.md`'s "Out of scope" section for
//! the full reasoning. GIF (see [`gif`]) is back to a cheap step: its compression is LZW, a
//! dictionary-of-byte-sequences scheme with no Huffman coding at all, unchanged and
//! well-documented since 1989 — the only wrinkle worth validating empirically (not just trusting
//! memory for) was the exact bit where the LZW code width grows, a well-known historical pitfall
//! for this family of formats. JPEG (see [`jpeg`]) is a genuine step back up in size after GIF's
//! detour: a real, separable forward/inverse DCT, its own canonical-Huffman entropy coding
//! (unrelated to DEFLATE's or VP8L's), and — unlike every other format in this hub — genuinely
//! **lossy** by design, so there's no byte-exact golden file for a JPEG encode (see
//! `tests/golden.rs`'s JPEG section). Baseline sequential DCT only, using the standard Annex K
//! quantization/Huffman tables rather than a rate-distortion-optimizing encoder or an adaptive
//! Huffman-table builder — real interoperability without either, the same "standard tables, not
//! optimal ones" trade real encoders make when they skip a second pass. Only the encoder's own
//! 4:4:4 (no chroma subsampling) scope choice is unique to this crate's encoder; the *decoder*
//! handles whatever subsampling a real-world file actually uses, since 4:2:0 is what nearly every
//! photo on the web is encoded with.
//!
//! [`raster`] is the shared decode target every format here converts through:
//! `bytes -> RawImage -> bytes`, so a new raster format only has to implement one decode and one
//! encode function, not a conversion function per pair — see [`converter::RasterConverter`],
//! which is `crate::Converter::convert`'s own rustdoc's example of this pattern.

mod bmp;
mod converter;
mod gif;
mod ico;
mod jpeg;
mod png;
mod qoi;
mod raster;
mod vp8l;
mod webp;
mod zlib;

pub use converter::RasterConverter;

use crate::Format;

/// Every raster format [`RasterConverter`] can decode and encode today.
///
/// `default_registry` in `crates/conv-core/src/lib.rs` registers [`RasterConverter`] for every
/// ordered pair drawn from this list rather than one hand-written `registry.register(...)` call
/// per direction — so landing a new raster format is: add the `Format` variant, teach
/// [`converter::RasterConverter`]'s `decode`/`convert` match arms about it, add it here.
/// Every existing raster format becomes convertible to and from it automatically, no registration
/// call to remember.
pub const FORMATS: &[Format] = &[
    Format::Bmp,
    Format::Qoi,
    Format::Png,
    Format::Ico,
    Format::Webp,
    Format::Gif,
    Format::Jpeg,
];
