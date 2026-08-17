//! ICO decode and encode — a thin container format, not a codec of its own.
//!
//! An ICO file is an `ICONDIR` header, a directory of `ICONDIRENTRY` records (one per embedded
//! image size), and then the embedded image data itself, in one of two shapes per entry:
//! - **PNG-compressed** — the entry's bytes are a complete, standalone PNG file. Legal for any
//!   size since Windows Vista, and what every modern encoder (including [`encode`], below) emits.
//! - **Raw DIB** — a `BITMAPINFOHEADER` (no `BITMAPFILEHEADER`, unlike a standalone `.bmp`)
//!   followed by bottom-up pixel data and a legacy 1-bit-per-pixel "AND mask". This crate only
//!   decodes the 32-bit-per-pixel (BGRA, real alpha) variant — the AND mask is redundant once
//!   alpha is present, so [`decode`] never needs to read it. Other bit depths (1/4/8/24, which
//!   need a palette or lack alpha entirely) are an honest gap, same treatment
//!   [`super::bmp`] gives BMP's own out-of-scope variants.
//!
//! [`encode`] always wraps a single PNG entry — reusing [`super::png::encode`] entirely rather
//! than re-implementing the legacy DIB+AND-mask packing, which buys nothing an already-more-common
//! PNG entry doesn't (every icon size, Vista and later).
//!
//! An ICO can hold multiple sizes of the same image; this crate's `Format`/`RawImage` model is
//! one image in, one image out, so [`decode`] picks the single largest entry by declared
//! width × height (ties keep the first one encountered) and decodes only that one — an ICO
//! containing an image this crate can decode at some *other* size doesn't get a second attempt at
//! a smaller entry, matching the "honest gap, don't guess" spirit of the rest of this crate rather
//! than adding cross-entry fallback logic for a rare case.

use super::png;
use super::raster::{checked_rgba_len, RawImage};
use crate::{ConvertError, ConvertOptions, Format};

const ICONDIR_LEN: usize = 6;
const ICONDIRENTRY_LEN: usize = 16;
const RESOURCE_TYPE_ICON: u16 = 1;
const RESOURCE_TYPE_CURSOR: u16 = 2;

const DIB_HEADER_LEN: usize = 40; // BITMAPINFOHEADER, no BITMAPFILEHEADER prefix in an ICO entry
const DIB_BI_RGB: u32 = 0;
const SUPPORTED_DIB_BIT_COUNT: u16 = 32;

/// `ICONDIRENTRY`'s `bWidth`/`bHeight` are one byte each, `0` meaning 256 — so a single-frame ICO
/// this crate encodes can never exceed 256×256. Rejected as an honest gap ([`encode`]) rather
/// than silently producing a directory entry that misdeclares the real image size.
const MAX_ICO_DIMENSION: u32 = 256;

/// Decodes the largest embedded image in an ICO file into a [`RawImage`] — see this module's
/// docs for entry selection and the PNG-vs-raw-DIB dispatch.
pub fn decode(input: &[u8], options: &ConvertOptions) -> Result<RawImage, ConvertError> {
    let malformed = || ConvertError::MalformedInput {
        format: Format::Ico,
    };
    let unsupported = |feature| ConvertError::UnsupportedFeature {
        format: Format::Ico,
        feature,
    };

    if options.is_cancelled() {
        return Err(ConvertError::Cancelled);
    }

    if input.len() < ICONDIR_LEN {
        return Err(malformed());
    }
    let resource_type = read_u16_le(input, 2);
    let count = read_u16_le(input, 4) as usize;

    if resource_type == RESOURCE_TYPE_CURSOR {
        return Err(unsupported("ico-cursor-format"));
    }
    if resource_type != RESOURCE_TYPE_ICON {
        return Err(malformed());
    }
    if count == 0 {
        return Err(malformed());
    }

    let entries_len = count.checked_mul(ICONDIRENTRY_LEN).ok_or_else(malformed)?;
    let entries_end = ICONDIR_LEN.checked_add(entries_len).ok_or_else(malformed)?;
    if entries_end > input.len() {
        return Err(malformed());
    }

    let mut best_area = 0u32;
    let mut best_offset = 0usize;
    let mut best_size = 0usize;
    for i in 0..count {
        let entry_off = ICONDIR_LEN + i * ICONDIRENTRY_LEN;
        let width = declared_dimension(input[entry_off]);
        let height = declared_dimension(input[entry_off + 1]);
        let bytes_in_res = read_u32_le(input, entry_off + 8) as usize;
        let image_offset = read_u32_le(input, entry_off + 12) as usize;

        let area = width * height;
        if area > best_area {
            best_area = area;
            best_offset = image_offset;
            best_size = bytes_in_res;
        }
    }

    let data_end = best_offset.checked_add(best_size).ok_or_else(malformed)?;
    let entry_data = input.get(best_offset..data_end).ok_or_else(malformed)?;

    if entry_data.len() >= png::SIGNATURE.len()
        && entry_data[..png::SIGNATURE.len()] == png::SIGNATURE
    {
        return png::decode(entry_data, options).map_err(tag_as_ico);
    }

    decode_dib_entry(entry_data, options)
}

fn decode_dib_entry(data: &[u8], options: &ConvertOptions) -> Result<RawImage, ConvertError> {
    let malformed = || ConvertError::MalformedInput {
        format: Format::Ico,
    };
    let unsupported = |feature| ConvertError::UnsupportedFeature {
        format: Format::Ico,
        feature,
    };

    if data.len() < DIB_HEADER_LEN {
        return Err(malformed());
    }
    let bi_size = read_u32_le(data, 0);
    if bi_size != DIB_HEADER_LEN as u32 {
        return Err(unsupported("ico-dib-header-variant"));
    }
    let bi_width = read_i32_le(data, 4);
    let bi_height = read_i32_le(data, 8); // XOR colour data + AND mask, combined
    let bi_planes = read_u16_le(data, 12);
    let bi_bit_count = read_u16_le(data, 14);
    let bi_compression = read_u32_le(data, 16);

    if bi_planes != 1 {
        return Err(malformed());
    }
    if bi_compression != DIB_BI_RGB {
        return Err(unsupported("ico-dib-compression"));
    }
    if bi_bit_count != SUPPORTED_DIB_BIT_COUNT {
        return Err(unsupported("ico-dib-bit-depth"));
    }
    if bi_width <= 0 || bi_height <= 0 || bi_height % 2 != 0 {
        return Err(malformed());
    }

    let width = bi_width as u32;
    let height = (bi_height as u32) / 2;
    let rgba_len = checked_rgba_len(width, height, Format::Ico)?;

    let row_bytes = (u64::from(width)).checked_mul(4).ok_or_else(malformed)?; // 32bpp: already 4-byte aligned
    let pixel_data_len = row_bytes
        .checked_mul(u64::from(height))
        .ok_or_else(malformed)?;
    let pixel_start = DIB_HEADER_LEN as u64;
    let pixel_end = pixel_start
        .checked_add(pixel_data_len)
        .ok_or_else(malformed)?;
    if pixel_end > data.len() as u64 {
        return Err(malformed());
    }

    let row_bytes = row_bytes as usize;
    let width = width as usize;
    let height = height as usize;
    let mut pixels = vec![0u8; rgba_len as usize];

    for file_row in 0..height {
        if options.is_cancelled() {
            return Err(ConvertError::Cancelled);
        }
        options.report_progress(0.5 * (file_row as f32 / height as f32));

        // Bottom-up storage, same convention as a standalone BMP's pixel array.
        let image_row = height - 1 - file_row;
        let row_start = DIB_HEADER_LEN + file_row * row_bytes;
        let row = &data[row_start..row_start + row_bytes];

        for x in 0..width {
            let src = x * 4;
            let (b, g, r, a) = (row[src], row[src + 1], row[src + 2], row[src + 3]);
            let dst = (image_row * width + x) * 4;
            pixels[dst] = r;
            pixels[dst + 1] = g;
            pixels[dst + 2] = b;
            pixels[dst + 3] = a;
        }
    }

    options.report_progress(0.5);
    RawImage::new(width as u32, height as u32, pixels, Format::Ico)
}

/// Encodes a [`RawImage`] as a single-entry, PNG-compressed ICO — see this module's docs for why
/// PNG rather than the legacy raw-DIB entry shape.
pub fn encode(image: &RawImage, options: &ConvertOptions) -> Result<Vec<u8>, ConvertError> {
    if options.is_cancelled() {
        return Err(ConvertError::Cancelled);
    }
    if image.width > MAX_ICO_DIMENSION || image.height > MAX_ICO_DIMENSION {
        return Err(ConvertError::UnsupportedFeature {
            format: Format::Ico,
            feature: "ico-size-too-large",
        });
    }

    // `png::encode` reports progress across the same 0.5..=1.0 half of a full conversion that
    // this function itself owns (see `super::converter::RasterConverter`) — wrapping it needs no
    // rescaling, since both functions are filling in exactly the same band.
    let png_bytes = png::encode(image, options)?;

    let mut out = Vec::with_capacity(ICONDIR_LEN + ICONDIRENTRY_LEN + png_bytes.len());
    out.extend_from_slice(&0u16.to_le_bytes()); // idReserved
    out.extend_from_slice(&RESOURCE_TYPE_ICON.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // idCount

    let image_offset = (ICONDIR_LEN + ICONDIRENTRY_LEN) as u32;
    out.push(encoded_dimension(image.width));
    out.push(encoded_dimension(image.height));
    out.push(0); // bColorCount — not a palette image
    out.push(0); // bReserved
    out.extend_from_slice(&1u16.to_le_bytes()); // wPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // wBitCount
    out.extend_from_slice(&(png_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&image_offset.to_le_bytes());

    out.extend_from_slice(&png_bytes);
    Ok(out)
}

fn declared_dimension(byte: u8) -> u32 {
    if byte == 0 {
        256
    } else {
        u32::from(byte)
    }
}

fn encoded_dimension(value: u32) -> u8 {
    if value >= 256 {
        0
    } else {
        value as u8
    }
}

/// Re-tags a [`ConvertError`] produced while decoding a PNG-compressed ICO entry so it reports
/// [`Format::Ico`], not [`Format::Png`] — the caller asked to convert an ICO; "malformed PNG
/// inside it" is, from that caller's perspective, a malformed ICO.
fn tag_as_ico(err: ConvertError) -> ConvertError {
    match err {
        ConvertError::MalformedInput { .. } => ConvertError::MalformedInput {
            format: Format::Ico,
        },
        ConvertError::UnsupportedFeature { feature, .. } => ConvertError::UnsupportedFeature {
            format: Format::Ico,
            feature,
        },
        other => other,
    }
}

fn read_u16_le(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}

fn read_u32_le(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}

fn read_i32_le(input: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProgressSink;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A real 32bpp raw-DIB ICO, built by Pillow (`Image.save(..., bitmap_format="bmp")`) — an
    /// independent encoder, not this module's own (which only ever emits PNG-compressed
    /// entries). Confirms `decode_dib_entry` against ground truth this module had no hand in
    /// producing, the same cross-validation strategy `png.rs`'s filter-coverage test uses.
    const ICO_DIB: &[u8] = &[
        0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x03, 0x03, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x4c,
        0x00, 0x00, 0x00, 0x16, 0x00, 0x00, 0x00, 0x28, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00,
        0x06, 0x00, 0x00, 0x00, 0x01, 0x00, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x24, 0x00, 0x00,
        0x00, 0xc4, 0x0e, 0x00, 0x00, 0xc4, 0x0e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x05, 0xa0, 0x0a, 0xff, 0x41, 0xa0, 0x46, 0xb4, 0x7d, 0xa0, 0x82, 0xff, 0x05,
        0x5a, 0x0a, 0xb4, 0x23, 0x5a, 0x46, 0xff, 0x41, 0x5a, 0x82, 0xb4, 0x05, 0x14, 0x0a, 0xff,
        0x05, 0x14, 0x46, 0xb4, 0x05, 0x14, 0x82, 0xff,
    ];

    /// `ICO_DIB`'s pixels, RGBA row-major top-to-bottom — read directly from the RGBA tuples fed
    /// into Pillow before encoding, not derived from this module in any way.
    const ICO_DIB_RGBA: &[u8] = &[
        0x0a, 0x14, 0x05, 0xff, 0x46, 0x14, 0x05, 0xb4, 0x82, 0x14, 0x05, 0xff, 0x0a, 0x5a, 0x05,
        0xb4, 0x46, 0x5a, 0x23, 0xff, 0x82, 0x5a, 0x41, 0xb4, 0x0a, 0xa0, 0x05, 0xff, 0x46, 0xa0,
        0x41, 0xb4, 0x82, 0xa0, 0x7d, 0xff,
    ];

    #[test]
    fn decode_matches_an_independently_built_raw_dib_ico() {
        let decoded = decode(ICO_DIB, &ConvertOptions::default()).unwrap();
        assert_eq!(decoded.width, 3);
        assert_eq!(decoded.height, 3);
        assert_eq!(decoded.pixels, ICO_DIB_RGBA);
    }

    fn checkerboard(width: u32, height: u32) -> RawImage {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                let v = if (x + y).is_multiple_of(2) { 255 } else { 0 };
                pixels[idx] = v;
                pixels[idx + 1] = v;
                pixels[idx + 2] = v;
                pixels[idx + 3] = 255;
            }
        }
        RawImage::new(width, height, pixels, Format::Ico).unwrap()
    }

    #[test]
    fn round_trip_preserves_pixels_exactly() {
        let image = checkerboard(5, 3);
        let bytes = encode(&image, &ConvertOptions::default()).unwrap();
        let decoded = decode(&bytes, &ConvertOptions::default()).unwrap();
        assert_eq!(decoded.width, image.width);
        assert_eq!(decoded.height, image.height);
        assert_eq!(decoded.pixels, image.pixels);
    }

    #[test]
    fn encode_rejects_images_larger_than_256() {
        let image = checkerboard(300, 10);
        let result = encode(&image, &ConvertOptions::default());
        assert!(matches!(
            result,
            Err(ConvertError::UnsupportedFeature {
                format: Format::Ico,
                feature: "ico-size-too-large"
            })
        ));
    }

    #[test]
    fn encode_dimension_256_maps_to_the_zero_sentinel() {
        let image = checkerboard(256, 1);
        let bytes = encode(&image, &ConvertOptions::default()).unwrap();
        assert_eq!(bytes[6], 0); // bWidth
                                 // Round trip still recovers the true size via the DIB/PNG header, not the byte field.
        let decoded = decode(&bytes, &ConvertOptions::default()).unwrap();
        assert_eq!(decoded.width, 256);
    }

    #[test]
    fn decode_rejects_a_cursor_resource_type_as_unsupported() {
        let mut bytes = encode(&checkerboard(2, 2), &ConvertOptions::default()).unwrap();
        bytes[2..4].copy_from_slice(&RESOURCE_TYPE_CURSOR.to_le_bytes());
        assert!(matches!(
            decode(&bytes, &ConvertOptions::default()),
            Err(ConvertError::UnsupportedFeature {
                format: Format::Ico,
                feature: "ico-cursor-format"
            })
        ));
    }

    #[test]
    fn decode_rejects_zero_entries() {
        let mut bytes = encode(&checkerboard(2, 2), &ConvertOptions::default()).unwrap();
        bytes[4..6].copy_from_slice(&0u16.to_le_bytes());
        assert!(matches!(
            decode(&bytes, &ConvertOptions::default()),
            Err(ConvertError::MalformedInput {
                format: Format::Ico
            })
        ));
    }

    #[test]
    fn decode_rejects_truncated_input() {
        let bytes = encode(&checkerboard(4, 4), &ConvertOptions::default()).unwrap();
        let truncated = &bytes[..bytes.len() / 2];
        assert!(matches!(
            decode(truncated, &ConvertOptions::default()),
            Err(ConvertError::MalformedInput {
                format: Format::Ico
            })
        ));
    }

    #[test]
    fn decode_picks_the_largest_entry_among_several() {
        // Two entries pointing at the same PNG bytes but declaring different sizes — decode
        // should pick the one this test can tell apart, i.e. the larger declared size.
        let small = checkerboard(2, 2);
        let large = checkerboard(6, 6);
        let small_png = png::encode(&small, &ConvertOptions::default()).unwrap();
        let large_png = png::encode(&large, &ConvertOptions::default()).unwrap();

        let mut out = Vec::new();
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&RESOURCE_TYPE_ICON.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes()); // two entries

        let first_entry_off = ICONDIR_LEN as u32;
        let second_entry_off = first_entry_off + ICONDIRENTRY_LEN as u32;
        let small_offset = second_entry_off + ICONDIRENTRY_LEN as u32;
        let large_offset = small_offset + small_png.len() as u32;

        // Entry 0: declares the *small* size, points at the small PNG.
        out.push(2);
        out.push(2);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(small_png.len() as u32).to_le_bytes());
        out.extend_from_slice(&small_offset.to_le_bytes());

        // Entry 1: declares the *large* size, points at the large PNG.
        out.push(6);
        out.push(6);
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(large_png.len() as u32).to_le_bytes());
        out.extend_from_slice(&large_offset.to_le_bytes());

        out.extend_from_slice(&small_png);
        out.extend_from_slice(&large_png);

        let decoded = decode(&out, &ConvertOptions::default()).unwrap();
        assert_eq!(decoded.width, 6);
        assert_eq!(decoded.height, 6);
    }

    struct CancelAfterNPolls {
        calls: AtomicUsize,
        cancel_after: usize,
    }

    impl ProgressSink for CancelAfterNPolls {
        fn on_progress(&self, _fraction: f32) {}
        fn is_cancelled(&self) -> bool {
            self.calls.fetch_add(1, Ordering::SeqCst) >= self.cancel_after
        }
    }

    fn options_cancelling_after(cancel_after: usize) -> (ConvertOptions, Arc<CancelAfterNPolls>) {
        let sink = Arc::new(CancelAfterNPolls {
            calls: AtomicUsize::new(0),
            cancel_after,
        });
        let options = ConvertOptions {
            progress: Some(sink.clone() as Arc<dyn ProgressSink>),
            ..ConvertOptions::default()
        };
        (options, sink)
    }

    #[test]
    fn decode_dib_entry_checks_cancellation_on_more_than_just_the_first_row() {
        let (options, sink) = options_cancelling_after(1);
        let result = decode(ICO_DIB, &options);
        assert!(matches!(result, Err(ConvertError::Cancelled)));
        assert!(sink.calls.load(Ordering::SeqCst) >= 2);
    }
}
