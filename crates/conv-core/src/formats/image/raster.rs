//! [`RawImage`]: the common decode target every raster format in this module converts through.

use crate::{ConvertError, Format};

/// A maximum on `width * height` a decoder here will accept, independent of how small the
/// *encoded* bytes are.
///
/// A malicious or corrupt file can declare huge dimensions while being tiny on disk (QOI, being a
/// compressed format, is especially prone to this — a handful of `QOI_OP_RUN` chunks can claim an
/// enormous image). Decoding that unconditionally would let a few bytes of input trigger a
/// multi-gigabyte allocation, a denial-of-service on untrusted input — exactly what
/// `crate::Converter`'s rules exist to prevent. 4096×4096 (67,108,864 RGBA8 bytes, ~64 MiB) is
/// comfortably above any fixture this crate ships and above what a browser tab can usually afford
/// to hold multiple copies of anyway.
const MAX_PIXELS: u64 = 4096 * 4096;

/// A decoded raster image: RGBA8, row-major, top-to-bottom (row 0 is the top row), no padding
/// between rows. Every format's `decode` normalizes into this shape; every format's `encode`
/// starts from it.
#[derive(Debug)]
pub struct RawImage {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, RGBA8, row-major, top-to-bottom.
    pub pixels: Vec<u8>,
}

impl RawImage {
    /// Builds a [`RawImage`] from an already-decoded pixel buffer, checking `pixels` is exactly
    /// the length [`checked_rgba_len`] computes for `width`/`height`. Callers must call
    /// [`checked_rgba_len`] (or [`checked_pixel_count`]) themselves *before* allocating `pixels`
    /// — this constructor only re-validates the result, it doesn't protect against an oversized
    /// allocation that already happened.
    pub fn new(
        width: u32,
        height: u32,
        pixels: Vec<u8>,
        format: Format,
    ) -> Result<Self, ConvertError> {
        let expected = checked_rgba_len(width, height, format)?;
        if pixels.len() as u64 != expected {
            return Err(ConvertError::MalformedInput { format });
        }
        Ok(RawImage {
            width,
            height,
            pixels,
        })
    }
}

/// Validates `width`/`height` are non-zero and that a `width * height` pixel count (and the RGBA8
/// byte count derived from it) fits without overflow and within [`MAX_PIXELS`] — **call this
/// before allocating a pixel buffer**, not after, so a file that merely *declares* huge dimensions
/// can't force a large allocation before it's rejected. Returns the pixel count on success.
pub fn checked_pixel_count(width: u32, height: u32, format: Format) -> Result<u64, ConvertError> {
    let malformed = || ConvertError::MalformedInput { format };
    if width == 0 || height == 0 {
        return Err(malformed());
    }
    let pixel_count = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(malformed)?;
    if pixel_count > MAX_PIXELS {
        return Err(malformed());
    }
    Ok(pixel_count)
}

/// Same as [`checked_pixel_count`], but returns the RGBA8 byte count (`pixel_count * 4`).
pub fn checked_rgba_len(width: u32, height: u32, format: Format) -> Result<u64, ConvertError> {
    let pixel_count = checked_pixel_count(width, height, format)?;
    pixel_count
        .checked_mul(4)
        .ok_or(ConvertError::MalformedInput { format })
}
