//! Raster image conversions — the first file-shaped category ported into `conv-core`.
//!
//! Scope today: [`Format::Bmp`](crate::Format::Bmp), [`Format::Qoi`](crate::Format::Qoi), and
//! [`Format::Png`](crate::Format::Png), every direction between them. This crate's own module
//! docs say "zero dependencies today; any dependency added here needs to earn its place", and
//! the release WASM artifact has a hard, CI-enforced size budget (`.wasm-size-budget`) that a
//! decoder-heavy dependency like the general-purpose `image` crate would blow through — so every
//! format here is hand-rolled rather than pulled in. BMP (no compression) and QOI (a small,
//! well-defined algorithm — see [`qoi`]) were cheap first steps; PNG (see [`png`]) is the harder
//! case this scope-out originally flagged, needing a real DEFLATE codec (see [`zlib`]) rather
//! than just chunk framing. JPEG/WebP remain out of scope — see [`png`]'s module docs for why
//! JPEG specifically is a bigger step than PNG was, not just "more of the same."
//!
//! [`raster`] is the shared decode target every format here converts through:
//! `bytes -> RawImage -> bytes`, so a new raster format only has to implement one decode and one
//! encode function, not a conversion function per pair — see [`converter::RasterConverter`],
//! which is `crate::Converter::convert`'s own rustdoc's example of this pattern.

mod bmp;
mod converter;
mod png;
mod qoi;
mod raster;
mod zlib;

pub use converter::RasterConverter;

use crate::Format;

/// Every raster format [`RasterConverter`] can decode and encode today.
///
/// `default_registry` in `crates/conv-core/src/lib.rs` registers [`RasterConverter`] for every
/// ordered pair drawn from this list rather than one hand-written `registry.register(...)` call
/// per direction — so landing a new raster format (JPEG, WebP, ...) is: add the `Format` variant,
/// teach [`converter::RasterConverter`]'s `decode`/`convert` match arms about it, add it here.
/// Every existing raster format becomes convertible to and from it automatically, no registration
/// call to remember.
pub const FORMATS: &[Format] = &[Format::Bmp, Format::Qoi, Format::Png];
