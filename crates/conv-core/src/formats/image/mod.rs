//! Raster image conversions — the first file-shaped category ported into `conv-core`.
//!
//! Scope today: [`Format::Bmp`](crate::Format::Bmp) ⇄ [`Format::Qoi`](crate::Format::Qoi), both
//! directions. Chosen deliberately over the obvious alternative (pulling in the general-purpose
//! `image` crate to cover PNG/JPEG/WebP in one shot): this crate's own module docs say "zero
//! dependencies today; any dependency added here needs to earn its place", and the release WASM
//! artifact has a hard, CI-enforced size budget (`.wasm-size-budget`) that a decoder-heavy
//! dependency would blow through immediately. BMP (no compression) and QOI (a small, well-defined
//! algorithm — see [`qoi`]) are both cheap to hand-roll and both deterministic/lossless, which
//! also keeps their golden-file tests byte-exact (see `docs/ARCHITECTURE.md`'s conformance-suite
//! section). PNG/JPEG/WebP need either a real DEFLATE implementation or a deliberate,
//! budget-reviewed dependency decision — a separate, larger follow-up.
//!
//! [`raster`] is the shared decode target every format here converts through:
//! `bytes -> RawImage -> bytes`, so a new raster format only has to implement one decode and one
//! encode function, not a conversion function per pair — see [`converter::RasterConverter`],
//! which is `crate::Converter::convert`'s own rustdoc's example of this pattern.

mod bmp;
mod converter;
mod qoi;
mod raster;

pub use converter::RasterConverter;
