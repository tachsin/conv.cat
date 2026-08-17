//! PNG decode and encode — 8-bit truecolor (RGB) and truecolor-with-alpha (RGBA) only.
//!
//! Wire format: an 8-byte signature, then a sequence of length-prefixed, CRC32-checked chunks.
//! The three this module needs are `IHDR` (dimensions and pixel format), one or more `IDAT`
//! (the image data, concatenated then zlib-decompressed — see [`super::zlib`]), and `IEND`. Any
//! other chunk is skipped if its type's lowercase-first-letter "ancillary" bit says it's safe to
//! ignore (`tEXt`, `gAMA`, `pHYs`, ...), and rejected as an honest gap otherwise.
//!
//! Deliberately out of scope, same "honest gap" treatment [`super::bmp`] gives BMP's other DIB
//! header variants: palette (color type 3) and grayscale (0, 4) pixel formats, any bit depth
//! other than 8, and interlacing (Adam7). All are legal PNG, none are needed to prove this
//! crate's PNG support end to end, and each is a separable follow-up rather than a reason to hold
//! up the common case (an 8-bit RGB/RGBA screenshot or export, which is what the vast majority of
//! real-world PNGs are).
//!
//! ## Why PNG next, and why not JPEG the same way
//!
//! PNG's compressed stream is *lossless* — decode-then-encode of the same pixels is still a
//! byte-exact round trip through [`super::raster::RawImage`], so this format's golden fixtures
//! stay byte-exact just like BMP's and QOI's (see `docs/adding-a-format.md` Step 4). JPEG's DCT
//! quantization is lossy: a JPEG converter would need the golden-file suite's tolerance-band
//! comparison instead (`tests/support::assert_size_within_tolerance` /
//! `assert_starts_with_magic`), a real test-strategy decision on its own, on top of a DCT/Huffman
//! codec this crate has no code for yet. That combination made PNG the smaller, better-isolated
//! next step — see `docs/ROADMAP.md`'s image-format-catalog entry for the rest of the target
//! list.

use super::raster::{checked_rgba_len, RawImage};
use super::zlib;
use crate::{ConvertError, ConvertOptions, Format};

/// The 8-byte PNG magic. `pub(super)` so [`super::ico`] can recognize a PNG-compressed ICO entry
/// without duplicating this constant.
pub(super) const SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
const CHUNK_HEADER_LEN: usize = 8; // 4-byte length + 4-byte type
const CHUNK_CRC_LEN: usize = 4;

const COLOR_TYPE_RGB: u8 = 2;
const COLOR_TYPE_RGBA: u8 = 6;

/// Decodes an 8-bit RGB or RGBA, non-interlaced PNG into a [`RawImage`].
///
/// Polls `options` for cancellation and reports progress (the `0.0..=0.5` half of a full
/// decode-then-encode conversion — see [`super::converter::RasterConverter`]) once per scanline,
/// after the single up-front zlib-decompression pass (chunk parsing and decompression aren't
/// naturally interruptible mid-step the way a per-row loop is, so cancellation there is checked
/// once before the expensive step starts rather than during it).
pub fn decode(input: &[u8], options: &ConvertOptions) -> Result<RawImage, ConvertError> {
    let malformed = || ConvertError::MalformedInput {
        format: Format::Png,
    };
    let unsupported = |feature| ConvertError::UnsupportedFeature {
        format: Format::Png,
        feature,
    };

    if options.is_cancelled() {
        return Err(ConvertError::Cancelled);
    }

    if input.len() < SIGNATURE.len() || input[..SIGNATURE.len()] != SIGNATURE {
        return Err(malformed());
    }

    let mut pos = SIGNATURE.len();
    let mut header: Option<(u32, u32, u8)> = None; // (width, height, color_type)
    let mut idat: Vec<u8> = Vec::new();
    let mut seen_iend = false;

    while pos < input.len() {
        let header_bytes = input
            .get(pos..pos + CHUNK_HEADER_LEN)
            .ok_or_else(malformed)?;
        let length = u32::from_be_bytes(header_bytes[0..4].try_into().unwrap()) as usize;
        let chunk_type: [u8; 4] = header_bytes[4..8].try_into().unwrap();

        let data_start = pos + CHUNK_HEADER_LEN;
        let data_end = data_start.checked_add(length).ok_or_else(malformed)?;
        let crc_end = data_end.checked_add(CHUNK_CRC_LEN).ok_or_else(malformed)?;
        if crc_end > input.len() {
            return Err(malformed());
        }
        let chunk_data = &input[data_start..data_end];
        let stored_crc = u32::from_be_bytes(input[data_end..crc_end].try_into().unwrap());
        if crc32_of_chunk(&chunk_type, chunk_data) != stored_crc {
            return Err(malformed());
        }

        match &chunk_type {
            b"IHDR" => {
                if header.is_some() || length != 13 {
                    return Err(malformed());
                }
                let width = u32::from_be_bytes(chunk_data[0..4].try_into().unwrap());
                let height = u32::from_be_bytes(chunk_data[4..8].try_into().unwrap());
                let bit_depth = chunk_data[8];
                let color_type = chunk_data[9];
                let compression_method = chunk_data[10];
                let filter_method = chunk_data[11];
                let interlace_method = chunk_data[12];

                if compression_method != 0 || filter_method != 0 {
                    return Err(malformed());
                }
                if interlace_method != 0 {
                    return Err(unsupported("png-interlaced"));
                }
                if bit_depth != 8 {
                    return Err(unsupported("png-bit-depth"));
                }
                if color_type != COLOR_TYPE_RGB && color_type != COLOR_TYPE_RGBA {
                    return Err(unsupported("png-color-type"));
                }
                header = Some((width, height, color_type));
            }
            b"IDAT" => {
                if header.is_none() {
                    return Err(malformed());
                }
                idat.extend_from_slice(chunk_data);
            }
            b"IEND" => {
                seen_iend = true;
                break;
            }
            other => {
                // Ancillary chunks (lowercase first letter, PNG §5.4) are safe to skip; an
                // unrecognized *critical* chunk is an honest gap, not silently ignorable.
                if other[0] & 0x20 == 0 {
                    return Err(unsupported("png-unknown-critical-chunk"));
                }
            }
        }

        pos = crc_end;
    }

    if !seen_iend {
        return Err(malformed());
    }
    let (width, height, color_type) = header.ok_or_else(malformed)?;
    let rgba_len = checked_rgba_len(width, height, Format::Png)?;

    let bpp: usize = if color_type == COLOR_TYPE_RGBA { 4 } else { 3 };
    let row_bytes = (width as u64)
        .checked_mul(bpp as u64)
        .ok_or_else(malformed)?;
    let expected_decompressed = row_bytes
        .checked_add(1) // filter-type byte
        .and_then(|n| n.checked_mul(u64::from(height)))
        .ok_or_else(malformed)?;
    let expected_decompressed = usize::try_from(expected_decompressed).map_err(|_| malformed())?;

    let raw = zlib::zlib_decompress(&idat, expected_decompressed).ok_or_else(malformed)?;

    let row_bytes = row_bytes as usize;
    let mut pixels = vec![0u8; rgba_len as usize];
    let mut prev_row = vec![0u8; row_bytes];
    let mut cur_row = vec![0u8; row_bytes];

    for y in 0..height as usize {
        if options.is_cancelled() {
            return Err(ConvertError::Cancelled);
        }
        options.report_progress(0.5 * (y as f32 / height as f32));

        let row_start = y * (row_bytes + 1);
        let filter_type = raw[row_start];
        let filtered = &raw[row_start + 1..row_start + 1 + row_bytes];
        unfilter_row(filter_type, filtered, &prev_row, &mut cur_row, bpp).ok_or_else(malformed)?;

        for x in 0..width as usize {
            let src = x * bpp;
            let dst = (y * width as usize + x) * 4;
            pixels[dst] = cur_row[src];
            pixels[dst + 1] = cur_row[src + 1];
            pixels[dst + 2] = cur_row[src + 2];
            pixels[dst + 3] = if bpp == 4 { cur_row[src + 3] } else { 255 };
        }

        std::mem::swap(&mut prev_row, &mut cur_row);
    }

    options.report_progress(0.5);
    RawImage::new(width, height, pixels, Format::Png)
}

/// Encodes a [`RawImage`] as PNG: always 8-bit RGBA (color type 6), no filtering (filter type
/// `None` on every scanline), no interlacing — the simplest legal PNG, same spirit as
/// [`super::bmp::encode`]'s "byte-exact and deterministic" BMP output. Always RGBA (never drops
/// down to RGB even for a fully-opaque image) for the same reason [`super::qoi::encode`] always
/// declares 4 channels: [`RawImage`] carries alpha unconditionally, so there's no information to
/// lose by keeping it.
///
/// Polls `options` for cancellation and reports progress (the `0.5..=1.0` half — see
/// [`super::converter::RasterConverter`]) once per scanline while assembling the raw (pre-zlib)
/// scanline buffer; compression and chunk framing are a single pass over that buffer afterward.
pub fn encode(image: &RawImage, options: &ConvertOptions) -> Result<Vec<u8>, ConvertError> {
    let width = image.width;
    let height = image.height;
    let row_bytes = width as usize * 4;

    let mut raw = Vec::with_capacity((row_bytes + 1) * height as usize);
    for y in 0..height as usize {
        if options.is_cancelled() {
            return Err(ConvertError::Cancelled);
        }
        options.report_progress(0.5 + 0.5 * (y as f32 / height as f32));

        raw.push(0); // filter type: None
        let row_start = y * row_bytes;
        raw.extend_from_slice(&image.pixels[row_start..row_start + row_bytes]);
    }

    let compressed = zlib::zlib_compress(&raw);

    let mut out = Vec::with_capacity(SIGNATURE.len() + 64 + compressed.len());
    out.extend_from_slice(&SIGNATURE);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(COLOR_TYPE_RGBA);
    ihdr.push(0); // compression method
    ihdr.push(0); // filter method
    ihdr.push(0); // interlace method
    write_chunk(&mut out, b"IHDR", &ihdr);
    write_chunk(&mut out, b"IDAT", &compressed);
    write_chunk(&mut out, b"IEND", &[]);

    options.report_progress(1.0);
    Ok(out)
}

/// Reverses one of PNG's five per-scanline filters (§9 of the spec) in place into `cur_row`,
/// given `filtered` (the raw bytes this scanline decompressed to, filter-type byte already
/// stripped) and `prev_row` (the already-reconstructed previous scanline, all zero for row 0 —
/// the spec's own convention for "no previous row"). `bpp` is the pixel stride used for the
/// "left" reference (`Sub`/`Average`/`Paeth` look `bpp` bytes back within the *same* row, not one
/// pixel back — those coincide only when there's no sub-pixel byte structure to skip over).
fn unfilter_row(
    filter_type: u8,
    filtered: &[u8],
    prev_row: &[u8],
    cur_row: &mut [u8],
    bpp: usize,
) -> Option<()> {
    match filter_type {
        0 => cur_row.copy_from_slice(filtered),
        1 => {
            for i in 0..filtered.len() {
                let left = if i >= bpp { cur_row[i - bpp] } else { 0 };
                cur_row[i] = filtered[i].wrapping_add(left);
            }
        }
        2 => {
            for i in 0..filtered.len() {
                cur_row[i] = filtered[i].wrapping_add(prev_row[i]);
            }
        }
        3 => {
            for i in 0..filtered.len() {
                let left = if i >= bpp {
                    u16::from(cur_row[i - bpp])
                } else {
                    0
                };
                let up = u16::from(prev_row[i]);
                let avg = ((left + up) / 2) as u8;
                cur_row[i] = filtered[i].wrapping_add(avg);
            }
        }
        4 => {
            for i in 0..filtered.len() {
                let left = if i >= bpp { cur_row[i - bpp] } else { 0 };
                let up = prev_row[i];
                let up_left = if i >= bpp { prev_row[i - bpp] } else { 0 };
                cur_row[i] = filtered[i].wrapping_add(paeth_predictor(left, up, up_left));
            }
        }
        _ => return None,
    }
    Some(())
}

/// The Paeth predictor, verbatim from PNG spec §9.2 (Annex/pseudocode) — picks whichever of
/// left/above/upper-left is numerically closest to `left + above - upper_left`, ties broken in
/// left, then above, then upper-left order.
fn paeth_predictor(left: u8, above: u8, upper_left: u8) -> u8 {
    let a = i32::from(left);
    let b = i32::from(above);
    let c = i32::from(upper_left);
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    if pa <= pb && pa <= pc {
        left
    } else if pb <= pc {
        above
    } else {
        upper_left
    }
}

fn write_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32_of_chunk(chunk_type, data).to_be_bytes());
}

fn crc32_of_chunk(chunk_type: &[u8], data: &[u8]) -> u32 {
    let crc = crc32_update(0xFFFF_FFFF, chunk_type);
    let crc = crc32_update(crc, data);
    crc ^ 0xFFFF_FFFF
}

/// CRC-32 (ISO-HDLC / gzip / PNG's own — polynomial `0xEDB88320`, reflected). Bit-by-bit rather
/// than table-driven: PNG chunks here are at most a handful of MiB, and a 256-entry lookup table
/// is bytes this crate's WASM size budget doesn't need to spend on a rarely-hot path.
fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProgressSink;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A real PNG file, built independently in Python (zlib.compress + hand-written PNG chunk
    /// framing — not this module's own encoder), that deliberately uses all five scanline filter
    /// types (`None`, `Sub`, `Up`, `Average`, `Paeth`, one per row) against a 4x5 RGBA image.
    /// This module's own `encode` only ever emits filter `None`, so nothing in the round-trip
    /// tests above would catch a bug in `unfilter_row`'s other four branches — this fixture is
    /// the independent check that does. See `docs/` — no, this is deliberately *not* a
    /// `tests/fixtures/` golden (this is validating the decoder itself, not exercising the public
    /// `conv_core::convert` API end to end; the real BMP/QOI-target goldens for PNG live under
    /// `tests/fixtures/image/{bmp,qoi,png}/`).
    const PNG_ALL_FILTERS: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x05, 0x08, 0x06, 0x00, 0x00, 0x00, 0x62,
        0xad, 0x4d, 0xdb, 0x00, 0x00, 0x00, 0x43, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x60,
        0x60, 0x60, 0x38, 0xa1, 0xca, 0xce, 0x78, 0xda, 0x8b, 0x8f, 0xe5, 0x5c, 0xbe, 0x28, 0xe7,
        0x45, 0x46, 0x6e, 0x53, 0xc6, 0xb3, 0x40, 0x01, 0x66, 0x55, 0x76, 0x66, 0x20, 0x66, 0x65,
        0x66, 0x02, 0x0a, 0xb0, 0x22, 0x63, 0x66, 0xb1, 0x2c, 0xa6, 0x3c, 0x09, 0x39, 0x46, 0x16,
        0x09, 0x39, 0x26, 0x20, 0x66, 0x66, 0x61, 0x01, 0xcb, 0x00, 0xb5, 0xc0, 0x30, 0x00, 0x0b,
        0x75, 0x09, 0x3b, 0xd2, 0xb3, 0x3b, 0x55, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44,
        0xae, 0x42, 0x60, 0x82,
    ];

    /// The RGBA pixels `PNG_ALL_FILTERS` decodes to (row-major, top-to-bottom, matching
    /// `RawImage`'s own layout) — read directly from Pillow's `Image.tobytes()` on the same
    /// pixel data before it went through any filtering/compression, i.e. ground truth from a
    /// second, independent codec, not derived from this module in any way.
    const PNG_ALL_FILTERS_RGBA: &[u8] = &[
        0x00, 0x00, 0x00, 0xc8, 0x25, 0x07, 0x01, 0xcb, 0x4a, 0x0e, 0x04, 0xce, 0x6f, 0x15, 0x09,
        0xd1, 0x0b, 0x35, 0x01, 0xcd, 0x30, 0x3c, 0x02, 0xd0, 0x55, 0x43, 0x05, 0xd3, 0x7a, 0x4a,
        0x0a, 0xd6, 0x16, 0x6a, 0x02, 0xd2, 0x3b, 0x71, 0x03, 0xd5, 0x60, 0x78, 0x06, 0xd8, 0x85,
        0x7f, 0x0b, 0xdb, 0x21, 0x9f, 0x03, 0xd7, 0x46, 0xa6, 0x04, 0xda, 0x6b, 0xad, 0x07, 0xdd,
        0x90, 0xb4, 0x0c, 0xe0, 0x2c, 0xd4, 0x04, 0xdc, 0x51, 0xdb, 0x05, 0xdf, 0x76, 0xe2, 0x08,
        0xe2, 0x9b, 0xe9, 0x0d, 0xe5,
    ];

    #[test]
    fn decode_matches_an_independently_built_png_exercising_every_filter_type() {
        let decoded = decode(PNG_ALL_FILTERS, &ConvertOptions::default()).unwrap();
        assert_eq!(decoded.width, 4);
        assert_eq!(decoded.height, 5);
        assert_eq!(decoded.pixels, PNG_ALL_FILTERS_RGBA);
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

    fn checkerboard(width: u32, height: u32) -> RawImage {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                let white = (x + y).is_multiple_of(2);
                let v = if white { 255 } else { 0 };
                pixels[idx] = v;
                pixels[idx + 1] = v;
                pixels[idx + 2] = v;
                pixels[idx + 3] = 255;
            }
        }
        RawImage::new(width, height, pixels, Format::Png).unwrap()
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
    fn round_trip_preserves_alpha() {
        let mut image = checkerboard(2, 2);
        image.pixels[3] = 128; // first pixel's alpha
        let bytes = encode(&image, &ConvertOptions::default()).unwrap();
        let decoded = decode(&bytes, &ConvertOptions::default()).unwrap();
        assert_eq!(decoded.pixels, image.pixels);
    }

    #[test]
    fn decode_rejects_bad_signature() {
        let mut bytes = encode(&checkerboard(2, 2), &ConvertOptions::default()).unwrap();
        bytes[1] ^= 0xFF;
        assert!(matches!(
            decode(&bytes, &ConvertOptions::default()),
            Err(ConvertError::MalformedInput {
                format: Format::Png
            })
        ));
    }

    #[test]
    fn decode_rejects_a_corrupted_chunk_crc() {
        let mut bytes = encode(&checkerboard(3, 3), &ConvertOptions::default()).unwrap();
        let last = bytes.len() - 1; // inside IEND's CRC
        bytes[last] ^= 0xFF;
        assert!(matches!(
            decode(&bytes, &ConvertOptions::default()),
            Err(ConvertError::MalformedInput {
                format: Format::Png
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
                format: Format::Png
            })
        ));
    }

    #[test]
    fn decode_reports_palette_color_type_as_unsupported_not_malformed() {
        let mut bytes = encode(&checkerboard(2, 2), &ConvertOptions::default()).unwrap();
        // IHDR color type byte: signature(8) + length(4) + "IHDR"(4) + width(4) + height(4) +
        // bit_depth(1) = offset 25.
        let color_type_offset = 8 + 4 + 4 + 4 + 4 + 1;
        bytes[color_type_offset] = 3; // palette
                                      // IHDR data is 13 bytes starting at offset 16 (colour_type, compression_method,
                                      // filter_method, interlace_method are the last 4 of those 13).
        let ihdr_crc_start = color_type_offset + 1 + 1 + 1 + 1;
        let crc = crc32_of_chunk(b"IHDR", &bytes[8 + 8..ihdr_crc_start]);
        bytes[ihdr_crc_start..ihdr_crc_start + 4].copy_from_slice(&crc.to_be_bytes());
        assert!(matches!(
            decode(&bytes, &ConvertOptions::default()),
            Err(ConvertError::UnsupportedFeature {
                format: Format::Png,
                feature: "png-color-type"
            })
        ));
    }

    #[test]
    fn decode_checks_cancellation_on_more_than_just_the_first_row() {
        let bytes = encode(&checkerboard(1, 3), &ConvertOptions::default()).unwrap();
        let (options, sink) = options_cancelling_after(1);
        let result = decode(&bytes, &options);
        assert!(matches!(result, Err(ConvertError::Cancelled)));
        assert!(sink.calls.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn encode_checks_cancellation_on_more_than_just_the_first_row() {
        let image = checkerboard(1, 3);
        let (options, sink) = options_cancelling_after(1);
        let result = encode(&image, &options);
        assert!(matches!(result, Err(ConvertError::Cancelled)));
        assert!(sink.calls.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn paeth_predictor_matches_spec_examples() {
        // left dominates when it's the closest.
        assert_eq!(paeth_predictor(10, 20, 30), 10);
        // exact tie between left and above resolves to left per the spec's tie-break order.
        assert_eq!(paeth_predictor(5, 5, 5), 5);
    }
}
