//! WebP (lossless/VP8L only) decode and encode.
//!
//! A WebP file is a RIFF container (`"RIFF"` + size + `"WEBP"`) wrapping one chunk; this module
//! only reads/writes the lossless one (`"VP8L"`). Lossy WebP (`"VP8 "`, a real intra-frame video
//! codec — block prediction, DCT/WHT, boolean arithmetic coding) is a different, JPEG-scale
//! undertaking of its own and out of scope here — see [`super`]'s module docs.
//!
//! The VP8L bitstream itself (Huffman decode, backward references, the color cache, meta-prefix
//! groups) lives in [`super::vp8l`]; this module owns the RIFF framing, the four pixel transforms
//! (predictor, color, subtract-green, color-indexing) that wrap the raw pixel stream, and the
//! bridge to [`RawImage`].
//!
//! Transform correctness (particularly the predictor transform's 14 prediction modes and the
//! color-indexing transform's pixel-bundling scheme for small palettes) was validated the same
//! way [`super::vp8l`] was: a Python prototype cross-checked pixel-exact against Pillow (libwebp)
//! decoding real files. The one exception is the **color transform** — no image handed to
//! `libwebp`'s own encoder (via Pillow or `cwebp -m 6 -q 100`) across everything tried here ever
//! chose it, so it's implemented directly from the bitstream spec's explicit formulas (unlike the
//! Huffman bit-order question `vp8l.rs` had to resolve empirically, this transform's math is
//! fully and unambiguously specified) and checked with a hand-computed unit test instead.

use super::raster::{checked_rgba_len, RawImage};
use super::vp8l::{self, BitReader, BitWriter};
use crate::{ConvertError, ConvertOptions, Format};

const VP8L_SIGNATURE: u8 = 0x2f;
const MAX_DIMENSION: u32 = 16384; // VP8L's own 14-bit width/height field limit (value + 1)

fn malformed() -> ConvertError {
    ConvertError::MalformedInput {
        format: Format::Webp,
    }
}

fn unsupported(feature: &'static str) -> ConvertError {
    ConvertError::UnsupportedFeature {
        format: Format::Webp,
        feature,
    }
}

fn from_stop(stop: vp8l::Stop) -> ConvertError {
    match stop {
        vp8l::Stop::Malformed => malformed(),
        vp8l::Stop::Cancelled => ConvertError::Cancelled,
    }
}

// ─── RIFF container ──────────────────────────────────────────────────────────

/// Parses the RIFF/WEBP/VP8L wrapper and returns the VP8L payload (the bytes starting at the
/// `0x2f` signature).
fn parse_riff(input: &[u8]) -> Result<&[u8], ConvertError> {
    if input.len() < 20 {
        return Err(malformed());
    }
    if &input[0..4] != b"RIFF" || &input[8..12] != b"WEBP" || &input[12..16] != b"VP8L" {
        return Err(malformed());
    }
    let payload_len = u32::from_le_bytes(input[16..20].try_into().unwrap()) as usize;
    let payload = input.get(20..20 + payload_len).ok_or_else(malformed)?;
    Ok(payload)
}

fn write_riff(payload: &[u8]) -> Vec<u8> {
    let mut chunk = payload.to_vec();
    if chunk.len() % 2 == 1 {
        chunk.push(0); // RIFF chunks are padded to an even byte count
    }
    let riff_size = 4 + 8 + chunk.len(); // "WEBP" + ("VP8L" + u32 len) + payload

    let mut out = Vec::with_capacity(8 + riff_size);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(riff_size as u32).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(b"VP8L");
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&chunk);
    out
}

// ─── Pixel transforms (decode-side inverses only — encode never emits one) ──

enum Transform {
    Predictor {
        size_bits: u32,
        tw: usize,
        data: Vec<u32>,
    },
    Color {
        size_bits: u32,
        tw: usize,
        data: Vec<u32>,
    },
    SubtractGreen,
    ColorIndexing {
        table: Vec<u32>,
        width_bits: u32,
        orig_width: usize,
    },
}

fn average2(a: u8, b: u8) -> u8 {
    ((u16::from(a) + u16::from(b)) / 2) as u8
}

fn channels(argb: u32) -> [u8; 4] {
    [
        (argb >> 24) as u8,
        (argb >> 16) as u8,
        (argb >> 8) as u8,
        argb as u8,
    ]
}

fn combine(c: [u8; 4]) -> u32 {
    (u32::from(c[0]) << 24) | (u32::from(c[1]) << 16) | (u32::from(c[2]) << 8) | u32::from(c[3])
}

fn average2_argb(a: u32, b: u32) -> u32 {
    let (ca, cb) = (channels(a), channels(b));
    combine([
        average2(ca[0], cb[0]),
        average2(ca[1], cb[1]),
        average2(ca[2], cb[2]),
        average2(ca[3], cb[3]),
    ])
}

fn clamp(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

fn clamp_add_subtract_full(a: i32, b: i32, c: i32) -> u8 {
    clamp(a + b - c)
}

fn clamp_add_subtract_half(a: i32, b: i32) -> u8 {
    clamp(a + (a - b) / 2)
}

fn select(l: u32, t: u32, tl: u32) -> u32 {
    let (cl, ct, ctl) = (channels(l), channels(t), channels(tl));
    let p = [
        i32::from(cl[0]) + i32::from(ct[0]) - i32::from(ctl[0]),
        i32::from(cl[1]) + i32::from(ct[1]) - i32::from(ctl[1]),
        i32::from(cl[2]) + i32::from(ct[2]) - i32::from(ctl[2]),
        i32::from(cl[3]) + i32::from(ct[3]) - i32::from(ctl[3]),
    ];
    let dist = |c: [u8; 4]| -> i32 { (0..4).map(|i| (p[i] - i32::from(c[i])).abs()).sum::<i32>() };
    if dist(cl) < dist(ct) {
        l
    } else {
        t
    }
}

/// The 14 predictor modes (bitstream spec §4.1) — the green channel of the predictor transform's
/// own sub-image names which one applies to a given block. `None` for an out-of-range mode
/// (14/15 are not defined) — malformed input, not a case to guess at.
fn predict(mode: u8, l: u32, t: u32, tl: u32, tr: u32) -> Option<u32> {
    Some(match mode {
        0 => 0xff00_0000,
        1 => l,
        2 => t,
        3 => tr,
        4 => tl,
        5 => average2_argb(average2_argb(l, tr), t),
        6 => average2_argb(l, tl),
        7 => average2_argb(l, t),
        8 => average2_argb(tl, t),
        9 => average2_argb(t, tr),
        10 => average2_argb(average2_argb(l, tl), average2_argb(t, tr)),
        11 => select(l, t, tl),
        12 => {
            let (cl, ct, ctl) = (channels(l), channels(t), channels(tl));
            combine(std::array::from_fn(|i| {
                clamp_add_subtract_full(i32::from(cl[i]), i32::from(ct[i]), i32::from(ctl[i]))
            }))
        }
        13 => {
            let (c_avg, ctl) = (channels(average2_argb(l, t)), channels(tl));
            combine(std::array::from_fn(|i| {
                clamp_add_subtract_half(i32::from(c_avg[i]), i32::from(ctl[i]))
            }))
        }
        _ => return None,
    })
}

fn apply_predictor_inverse(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    size_bits: u32,
    tw: usize,
    tdata: &[u32],
) -> Result<(), ConvertError> {
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let pred = if x == 0 && y == 0 {
                0xff00_0000
            } else if y == 0 {
                pixels[idx - 1]
            } else if x == 0 {
                pixels[idx - width]
            } else {
                let l = pixels[idx - 1];
                let t = pixels[idx - width];
                let tl = pixels[idx - width - 1];
                // Rightmost column: the top row's own leftmost pixel stands in for TR (there is
                // no pixel to the upper-right of the last column) — bitstream spec §4.1.
                let tr = if x == width - 1 {
                    pixels[(y - 1) * width]
                } else {
                    pixels[idx - width + 1]
                };
                let mode = ((tdata[(y >> size_bits) * tw + (x >> size_bits)] >> 8) & 0xff) as u8;
                predict(mode, l, t, tl, tr).ok_or_else(malformed)?
            };
            let residual = channels(pixels[idx]);
            let predicted = channels(pred);
            pixels[idx] = combine(std::array::from_fn(|i| {
                residual[i].wrapping_add(predicted[i])
            }));
        }
    }
    Ok(())
}

fn color_transform_delta(t: u8, c: u8) -> i32 {
    // Reinterpret both operands as signed 8-bit (bitstream spec §4.2's uint8->int8 mapping:
    // [128..255] becomes [-128..-1]) before the multiply.
    (i32::from(t as i8) * i32::from(c as i8)) >> 5
}

fn apply_color_transform_inverse(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    size_bits: u32,
    tw: usize,
    tdata: &[u32],
) {
    for y in 0..height {
        for x in 0..width {
            let idx = y * width + x;
            let element = tdata[(y >> size_bits) * tw + (x >> size_bits)];
            // Stored as alpha=255, red=red_to_blue, green=green_to_blue, blue=green_to_red.
            let green_to_red = element as u8;
            let green_to_blue = (element >> 8) as u8;
            let red_to_blue = (element >> 16) as u8;

            let c = channels(pixels[idx]);
            let (alpha, mut red, green, mut blue) = (c[0], c[1], c[2], c[3]);
            red = red.wrapping_add(color_transform_delta(green_to_red, green) as u8);
            blue = blue.wrapping_add(color_transform_delta(green_to_blue, green) as u8);
            blue = blue.wrapping_add(color_transform_delta(red_to_blue, red) as u8);
            pixels[idx] = combine([alpha, red, green, blue]);
        }
    }
}

fn apply_subtract_green_inverse(pixels: &mut [u32]) {
    for argb in pixels.iter_mut() {
        let c = channels(*argb);
        let green = c[2];
        *argb = combine([
            c[0],
            c[1].wrapping_add(green),
            green,
            c[3].wrapping_add(green),
        ]);
    }
}

fn apply_color_indexing_inverse(
    pixels: Vec<u32>,
    height: usize,
    table: &[u32],
    width_bits: u32,
    orig_width: usize,
) -> Result<Vec<u32>, ConvertError> {
    if width_bits == 0 {
        return pixels
            .into_iter()
            .map(|px| {
                let idx = ((px >> 8) & 0xff) as usize;
                table.get(idx).copied().ok_or_else(malformed)
            })
            .collect();
    }

    let bundle = 1usize << width_bits;
    let bits_per = 8 / bundle;
    let mask = (1u32 << bits_per) - 1;
    let bundled_width = orig_width.div_ceil(bundle);
    let mut out = vec![0u32; orig_width * height];
    for y in 0..height {
        for x in 0..orig_width {
            let bundled_x = x / bundle;
            let sub = x % bundle;
            let bundled_px = pixels[y * bundled_width + bundled_x];
            let green = (bundled_px >> 8) & 0xff;
            let idx = ((green >> (sub * bits_per)) & mask) as usize;
            out[y * orig_width + x] = table.get(idx).copied().ok_or_else(malformed)?;
        }
    }
    Ok(out)
}

// ─── Top-level decode / encode ───────────────────────────────────────────────

/// Decodes a WebP lossless (VP8L) file into a [`RawImage`].
pub fn decode(input: &[u8], options: &ConvertOptions) -> Result<RawImage, ConvertError> {
    if options.is_cancelled() {
        return Err(ConvertError::Cancelled);
    }

    let payload = parse_riff(input)?;
    let mut reader = BitReader::new(payload);

    if reader.read_bits(8) != Some(u32::from(VP8L_SIGNATURE)) {
        return Err(malformed());
    }
    let mut width = reader.read_bits(14).ok_or_else(malformed)? as usize + 1;
    let height = reader.read_bits(14).ok_or_else(malformed)? as usize + 1;
    let _alpha_is_used = reader.read_bits(1).ok_or_else(malformed)?;
    let version = reader.read_bits(3).ok_or_else(malformed)?;
    if version != 0 {
        return Err(unsupported("webp-version"));
    }

    let mut transforms = Vec::new();
    loop {
        let has_transform = reader.read_bits(1).ok_or_else(malformed)?;
        if has_transform == 0 {
            break;
        }
        let transform_type = reader.read_bits(2).ok_or_else(malformed)?;
        match transform_type {
            0 | 1 => {
                let size_bits = 2 + reader.read_bits(3).ok_or_else(malformed)?;
                let tw = width.div_ceil(1usize << size_bits);
                let th = height.div_ceil(1usize << size_bits);
                let data = vp8l::decode_image_stream(&mut reader, tw, th, false, options, &|_| {})
                    .map_err(from_stop)?;
                transforms.push(if transform_type == 0 {
                    Transform::Predictor {
                        size_bits,
                        tw,
                        data,
                    }
                } else {
                    Transform::Color {
                        size_bits,
                        tw,
                        data,
                    }
                });
            }
            2 => transforms.push(Transform::SubtractGreen),
            3 => {
                let color_table_size = reader.read_bits(8).ok_or_else(malformed)? as usize + 1;
                let raw = vp8l::decode_image_stream(
                    &mut reader,
                    color_table_size,
                    1,
                    false,
                    options,
                    &|_| {},
                )
                .map_err(from_stop)?;
                // The color table is subtraction-coded: each entry is the running per-channel
                // sum (mod 256) of the decoded values up to and including it.
                let mut table = Vec::with_capacity(color_table_size);
                let mut acc = [0u8; 4];
                for &px in &raw {
                    let c = channels(px);
                    acc = std::array::from_fn(|i| acc[i].wrapping_add(c[i]));
                    table.push(combine(acc));
                }
                let width_bits = if color_table_size <= 2 {
                    3
                } else if color_table_size <= 4 {
                    2
                } else if color_table_size <= 16 {
                    1
                } else {
                    0
                };
                let orig_width = width;
                if width_bits > 0 {
                    width = width.div_ceil(1usize << width_bits);
                }
                transforms.push(Transform::ColorIndexing {
                    table,
                    width_bits,
                    orig_width,
                });
            }
            _ => unreachable!("read_bits(2) is always in 0..4"),
        }
    }

    let rgba_len = checked_rgba_len(width as u32, height as u32, Format::Webp)?;
    let _ = rgba_len; // validated for its side effect: rejects an oversized/zero image up front

    let mut pixels = vp8l::decode_image_stream(&mut reader, width, height, true, options, &|f| {
        options.report_progress(0.5 * f);
    })
    .map_err(from_stop)?;

    let mut current_width = width;
    for transform in transforms.iter().rev() {
        if options.is_cancelled() {
            return Err(ConvertError::Cancelled);
        }
        match transform {
            Transform::Predictor {
                size_bits,
                tw,
                data,
            } => {
                apply_predictor_inverse(&mut pixels, current_width, height, *size_bits, *tw, data)?;
            }
            Transform::Color {
                size_bits,
                tw,
                data,
            } => {
                apply_color_transform_inverse(
                    &mut pixels,
                    current_width,
                    height,
                    *size_bits,
                    *tw,
                    data,
                );
            }
            Transform::SubtractGreen => apply_subtract_green_inverse(&mut pixels),
            Transform::ColorIndexing {
                table,
                width_bits,
                orig_width,
            } => {
                pixels =
                    apply_color_indexing_inverse(pixels, height, table, *width_bits, *orig_width)?;
                current_width = *orig_width;
            }
        }
    }

    let final_width = current_width as u32;
    let final_height = height as u32;
    let rgba_len = checked_rgba_len(final_width, final_height, Format::Webp)?;
    let mut rgba = vec![0u8; rgba_len as usize];
    for (i, &argb) in pixels.iter().enumerate() {
        let c = channels(argb);
        rgba[i * 4] = c[1]; // red
        rgba[i * 4 + 1] = c[2]; // green
        rgba[i * 4 + 2] = c[3]; // blue
        rgba[i * 4 + 3] = c[0]; // alpha
    }

    options.report_progress(0.5);
    RawImage::new(final_width, final_height, rgba, Format::Webp)
}

/// Encodes a [`RawImage`] as a minimal, valid WebP lossless file — no transforms, no color
/// cache, no backward references, a single Huffman group per channel. See [`super::vp8l`]'s
/// "Encode" section docs for why VP8L can't take the same "stored block" shortcut PNG's
/// encoder does, and why that trade is still the right one here.
pub fn encode(image: &RawImage, options: &ConvertOptions) -> Result<Vec<u8>, ConvertError> {
    if options.is_cancelled() {
        return Err(ConvertError::Cancelled);
    }
    if image.width > MAX_DIMENSION || image.height > MAX_DIMENSION {
        return Err(unsupported("webp-size-too-large"));
    }

    let pixel_count = (image.width as usize) * (image.height as usize);
    let mut pixels = Vec::with_capacity(pixel_count);
    let mut alpha_is_used = false;
    for i in 0..pixel_count {
        let base = i * 4;
        let (r, g, b, a) = (
            image.pixels[base],
            image.pixels[base + 1],
            image.pixels[base + 2],
            image.pixels[base + 3],
        );
        if a != 255 {
            alpha_is_used = true;
        }
        pixels
            .push((u32::from(a) << 24) | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b));
    }

    // One continuous `BitWriter` for the header and the image-data section together — VP8L has
    // no byte-alignment point before the very end of the stream, so finishing (and thus
    // byte-padding) a writer between them would corrupt everything after.
    let mut writer = BitWriter::new();
    writer.write_bits(u32::from(VP8L_SIGNATURE), 8);
    writer.write_bits(image.width - 1, 14);
    writer.write_bits(image.height - 1, 14);
    writer.write_bits(u32::from(alpha_is_used), 1);
    writer.write_bits(0, 3); // version
    writer.write_bits(0, 1); // no transforms

    vp8l::encode_image_stream(&mut writer, &pixels, options, &|f| {
        options.report_progress(0.5 + 0.5 * f);
    })
    .map_err(from_stop)?;

    let payload = writer.finish();

    options.report_progress(1.0);
    Ok(write_riff(&payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real WebP lossless file built by Pillow (`Image.save(..., lossless=True)`, `libwebp`
    /// under the hood) — an independent encoder, not this module's own. 2x2, using the
    /// color-indexing transform with the `width_bits == 3` pixel-bundling case (a 2-color
    /// palette, 8 indices packed per byte). Ground truth is the exact RGBA tuples fed into
    /// Pillow before encoding, not anything derived from this module.
    const TINY_WEBP: &[u8] = &[
        0x52, 0x49, 0x46, 0x46, 0x22, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38,
        0x4c, 0x16, 0x00, 0x00, 0x00, 0x2f, 0x01, 0x40, 0x00, 0x00, 0x0f, 0x70, 0x0a, 0xa8, 0x2b,
        0xf8, 0x9e, 0xc2, 0x63, 0xfe, 0x83, 0x07, 0x23, 0x10, 0xd1, 0xff, 0x10,
    ];
    const TINY_WEBP_RGBA: &[u8] = &[
        10, 20, 30, 255, 10, 20, 30, 255, 10, 20, 30, 255, 200, 100, 50, 255,
    ];

    /// Same provenance as [`TINY_WEBP`], 5x5, using the `width_bits == 1` pixel-bundling case (a
    /// 16-color palette, 2 indices packed per byte) — the different bundling factor is the point.
    const PAL16_WEBP: &[u8] = &[
        0x52, 0x49, 0x46, 0x46, 0x46, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38,
        0x4c, 0x39, 0x00, 0x00, 0x00, 0x2f, 0x04, 0x00, 0x01, 0x00, 0x7f, 0x20, 0x16, 0x4c, 0x76,
        0xe7, 0xef, 0x9c, 0xc2, 0x3c, 0x28, 0x48, 0xdb, 0x80, 0x85, 0xed, 0xce, 0xfc, 0x27, 0xec,
        0x1d, 0x14, 0xc4, 0xb6, 0x0d, 0xfd, 0xdd, 0xbe, 0x54, 0x50, 0x41, 0x16, 0x15, 0x54, 0x50,
        0x41, 0x27, 0x59, 0x04, 0x8b, 0xe8, 0x7f, 0xf4, 0x29, 0xfb, 0x86, 0xf7, 0x6b, 0x1b, 0xbc,
        0x8e, 0x07, 0x00,
    ];

    /// Same provenance, 30x30, a checkerboard pattern chosen to force heavy LZ77 backward-
    /// reference and color-cache use (long runs of two repeating colors) rather than mostly
    /// literals — the case [`TINY_WEBP`]/[`PAL16_WEBP`] (both tiny, few distinct colors) don't
    /// exercise. Uses no color-indexing transform (only 2 colors, but libwebp chose plain
    /// literal and backward-reference coding here instead — real encoder behavior, not something
    /// this test dictated).
    const PATTERN30_WEBP: &[u8] = &[
        0x52, 0x49, 0x46, 0x46, 0x38, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38,
        0x4c, 0x2c, 0x00, 0x00, 0x00, 0x2f, 0x1d, 0x40, 0x07, 0x00, 0x0f, 0x70, 0x35, 0xe9, 0xab,
        0x21, 0x5f, 0x8d, 0x7b, 0xfe, 0xe3, 0x01, 0x86, 0x01, 0xa8, 0x79, 0x26, 0x11, 0xc8, 0x69,
        0x25, 0x32, 0xad, 0x81, 0x8c, 0xa5, 0xdf, 0x00, 0x22, 0xfa, 0x3f, 0x0b, 0x1b, 0xab, 0x31,
        0x47, 0x7b, 0xdf, 0x00,
    ];

    /// Same provenance, 20x20, chosen (via `random.seed(42)`) to land on the predictor +
    /// subtract-green transform combination with a real meta-prefix (entropy image, multiple
    /// Huffman groups) — the one case among these fixtures with enough entropy that `libwebp`
    /// didn't fall back to a single group. None of the other fixtures here exercise the predictor
    /// transform or meta-prefix groups at all.
    const RANDOM20_WEBP: &[u8] = &[
        0x52, 0x49, 0x46, 0x46, 0x04, 0x05, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38,
        0x4c, 0xf7, 0x04, 0x00, 0x00, 0x2f, 0x13, 0xc0, 0x04, 0x00, 0xcd, 0x74, 0x21, 0xa2, 0xff,
        0x01, 0x85, 0x00, 0x00, 0x10, 0xb6, 0x6d, 0xdb, 0xb6, 0x6d, 0xdb, 0xb6, 0x6d, 0xdb, 0xb6,
        0x6d, 0xdb, 0xb6, 0x6d, 0xdb, 0xb6, 0x36, 0x14, 0x00, 0x00, 0x40, 0xfc, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f, 0x10, 0x10, 0x00, 0x20, 0xd8, 0xd4, 0xb7, 0xdb,
        0xb6, 0x6d, 0xdb, 0xb6, 0x6d, 0xdb, 0xb6, 0x6d, 0xdb, 0xb6, 0x6d, 0xdb, 0xb6, 0x6d, 0x5b,
        0x11, 0x30, 0xb4, 0x01, 0x66, 0x7b, 0xaa, 0x5d, 0xbf, 0xeb, 0xc7, 0x9f, 0x2e, 0x16, 0xef,
        0x6e, 0xfa, 0xe9, 0xc0, 0x8e, 0x89, 0x6d, 0x29, 0x22, 0x43, 0xf7, 0x31, 0xc9, 0x53, 0xce,
        0x9d, 0x5f, 0xcc, 0x68, 0xb5, 0x0c, 0xc8, 0x8e, 0xe0, 0x67, 0xf5, 0xb3, 0xc2, 0xd8, 0xda,
        0x45, 0xc7, 0xc2, 0x82, 0x7c, 0x93, 0xef, 0x56, 0x23, 0xe1, 0xd5, 0x07, 0xb2, 0x69, 0xd9,
        0x12, 0xc7, 0x5a, 0x35, 0x51, 0x85, 0xa7, 0x93, 0x82, 0x3a, 0xab, 0x6c, 0x6c, 0x24, 0x19,
        0x90, 0x69, 0x1e, 0x46, 0x9e, 0xbd, 0xea, 0x7a, 0xc9, 0x2b, 0xf6, 0x4e, 0xb6, 0xd0, 0xec,
        0x1e, 0x61, 0x76, 0x6f, 0x9e, 0xb4, 0x93, 0x82, 0x4c, 0xa6, 0xeb, 0xc8, 0x93, 0x23, 0x1e,
        0x41, 0xac, 0xaf, 0xf7, 0x6a, 0x97, 0x63, 0x4c, 0xc1, 0x66, 0x56, 0xbe, 0x5d, 0x8c, 0x27,
        0xf8, 0x60, 0x8f, 0x79, 0xc8, 0xfc, 0xe8, 0x14, 0xea, 0xda, 0xbd, 0xb0, 0x05, 0xdf, 0x44,
        0x3f, 0xc9, 0x99, 0xf0, 0xde, 0x0c, 0xc6, 0x7d, 0x11, 0x4f, 0x4d, 0xf0, 0x8b, 0x90, 0xf4,
        0xa8, 0xe3, 0x4a, 0x19, 0xde, 0xd6, 0xd3, 0x7a, 0xa9, 0xff, 0x3f, 0x97, 0x17, 0x20, 0x26,
        0x73, 0xa9, 0x9e, 0xab, 0xaf, 0x6e, 0xd4, 0x79, 0x6e, 0x18, 0xc3, 0x5d, 0xac, 0xf2, 0x3d,
        0x79, 0xec, 0x20, 0xa7, 0x66, 0xdf, 0x89, 0xa3, 0x1c, 0x30, 0xab, 0xad, 0x56, 0x53, 0x09,
        0x30, 0x3e, 0xbb, 0xd7, 0x66, 0x61, 0x81, 0x5f, 0x0a, 0xe3, 0xc2, 0xda, 0x3d, 0x86, 0x7d,
        0x7c, 0x16, 0xf7, 0x43, 0xa5, 0xd2, 0xd1, 0x69, 0x9c, 0xb0, 0x5e, 0x95, 0x19, 0x02, 0xbf,
        0x9c, 0x8a, 0x4b, 0x08, 0x95, 0xb2, 0x7d, 0xd6, 0xf9, 0xad, 0x4f, 0x58, 0x02, 0xec, 0x84,
        0x6b, 0x38, 0xf4, 0xb4, 0xf6, 0x69, 0xe0, 0xb7, 0x1d, 0xd5, 0xa4, 0xa1, 0xa4, 0xc8, 0x9c,
        0xb2, 0xe4, 0xe2, 0x68, 0x70, 0xde, 0xa2, 0xe8, 0xa6, 0xfa, 0x2d, 0x77, 0x0d, 0x71, 0xc3,
        0x59, 0xe7, 0xf7, 0xd1, 0x40, 0xf0, 0xcb, 0x04, 0x55, 0x04, 0x0e, 0x1f, 0x04, 0x46, 0xe7,
        0x68, 0x72, 0x68, 0x91, 0xf3, 0xa9, 0x4b, 0xa0, 0xb1, 0x51, 0xc5, 0xee, 0xc7, 0xb8, 0xe8,
        0x0b, 0xf1, 0x3d, 0xe6, 0x98, 0x5a, 0x5d, 0x36, 0x98, 0xf7, 0xbf, 0x17, 0xf6, 0x5e, 0x31,
        0xa3, 0xe2, 0xe7, 0xd4, 0x99, 0xdd, 0xad, 0x16, 0xdb, 0xde, 0x18, 0xa2, 0x9e, 0xe3, 0x38,
        0xcd, 0x90, 0xe7, 0xbb, 0xb2, 0x76, 0x05, 0x1b, 0x8c, 0xb2, 0x83, 0xbc, 0x1f, 0xa1, 0xb0,
        0xf3, 0x99, 0x23, 0x3f, 0xce, 0xc6, 0xce, 0x4e, 0xb2, 0x63, 0x5c, 0x9f, 0x5c, 0x8d, 0x46,
        0xf7, 0xfd, 0xd4, 0x08, 0xbb, 0x2d, 0xc0, 0x0f, 0x60, 0x61, 0xe7, 0xff, 0x78, 0x84, 0x5a,
        0xe3, 0xef, 0x29, 0xc5, 0xbb, 0x79, 0x28, 0x05, 0xac, 0x26, 0x3c, 0x31, 0xa5, 0x3e, 0xa3,
        0xd5, 0x5a, 0x63, 0x2d, 0xa2, 0xda, 0xde, 0xbf, 0x3f, 0x11, 0x7f, 0x14, 0x21, 0x5c, 0xc3,
        0x58, 0xa4, 0xbe, 0x32, 0x13, 0x05, 0xd3, 0x6e, 0x45, 0x67, 0xb0, 0xd0, 0x5a, 0x36, 0x73,
        0x1f, 0x21, 0x96, 0x36, 0x8e, 0xe6, 0x00, 0x05, 0x21, 0x55, 0x71, 0xc5, 0xbf, 0x1e, 0x0d,
        0x8a, 0x07, 0x76, 0x1d, 0x53, 0xed, 0x32, 0xba, 0x02, 0xe7, 0xc7, 0x03, 0x40, 0x7e, 0xbd,
        0x40, 0x30, 0x56, 0xcf, 0x3d, 0x42, 0x29, 0xab, 0xcb, 0x28, 0xd4, 0x26, 0x6b, 0x4c, 0x4c,
        0x06, 0x82, 0xe6, 0x34, 0xad, 0x7c, 0x7a, 0xdd, 0x26, 0x74, 0xc0, 0xa5, 0xba, 0x49, 0x35,
        0x4d, 0xaa, 0x61, 0x12, 0xa1, 0xf7, 0xdd, 0xc2, 0x02, 0xdb, 0x48, 0x52, 0xbe, 0xb5, 0xff,
        0x04, 0x28, 0x6e, 0x60, 0xc5, 0x38, 0x29, 0xd2, 0xbb, 0x46, 0x36, 0xb8, 0xb5, 0x65, 0x06,
        0xdb, 0x6f, 0xb3, 0x13, 0x91, 0x68, 0xf9, 0x63, 0x65, 0x9c, 0x3b, 0xd2, 0xf7, 0xad, 0xbe,
        0xca, 0xb6, 0x67, 0x97, 0xc1, 0x19, 0xff, 0xe5, 0x6f, 0x9e, 0x4e, 0xd0, 0xdc, 0x25, 0x2c,
        0xa6, 0x6d, 0xe3, 0xdf, 0xd8, 0x8a, 0x37, 0xd8, 0x4c, 0x90, 0xe7, 0x09, 0xb5, 0x9c, 0xef,
        0x26, 0x63, 0x71, 0xa2, 0x93, 0xea, 0x46, 0x2a, 0x23, 0x85, 0x49, 0xec, 0x3d, 0x6e, 0xa3,
        0x50, 0xfc, 0x2d, 0xe6, 0x1c, 0x6c, 0xc0, 0xb5, 0xda, 0x98, 0xae, 0x24, 0x31, 0xc6, 0x96,
        0x64, 0x97, 0x43, 0xaf, 0xde, 0xe4, 0xed, 0x46, 0xf1, 0x49, 0x91, 0x41, 0xb3, 0x71, 0x93,
        0x38, 0x5d, 0x22, 0x78, 0xd6, 0x25, 0xea, 0xe6, 0x9e, 0xd7, 0x6e, 0x26, 0x7b, 0x48, 0x07,
        0xee, 0x43, 0x1b, 0x4d, 0xdd, 0xbe, 0x3a, 0xc8, 0xed, 0x40, 0x6d, 0xf7, 0xbe, 0x66, 0xc6,
        0xa8, 0x7a, 0x63, 0x2f, 0xbf, 0x9e, 0x86, 0x70, 0x34, 0x4d, 0x79, 0xd0, 0x9e, 0x55, 0x28,
        0x24, 0x4e, 0x0a, 0xad, 0x39, 0x61, 0x90, 0xc6, 0x1b, 0xb5, 0x39, 0x7e, 0xc1, 0xf6, 0x02,
        0x0f, 0x8a, 0xd1, 0x3a, 0xf0, 0xe0, 0xb9, 0x70, 0x6e, 0x76, 0x8f, 0xc5, 0x11, 0xc6, 0xb2,
        0xf8, 0xf6, 0xa2, 0x72, 0xb2, 0x0d, 0x3f, 0xc2, 0x86, 0x57, 0xfd, 0x77, 0xcd, 0x14, 0x2d,
        0xfe, 0x17, 0x11, 0x84, 0xcb, 0x88, 0x25, 0xa8, 0x47, 0x08, 0x57, 0x6a, 0xba, 0xea, 0x09,
        0xd0, 0x9f, 0xd7, 0x8d, 0xb9, 0x44, 0x2b, 0x1f, 0xbf, 0x35, 0xe4, 0x6e, 0xc4, 0x08, 0x3b,
        0x3d, 0x75, 0xf9, 0xe5, 0x74, 0xfb, 0x51, 0x1a, 0x21, 0x9c, 0xcb, 0x34, 0xff, 0xb5, 0x77,
        0x08, 0x11, 0x28, 0xef, 0xba, 0xe4, 0xa8, 0xe8, 0xd1, 0xa9, 0x7e, 0x12, 0x40, 0x3d, 0x13,
        0x88, 0x5f, 0x84, 0x42, 0x2f, 0x13, 0x08, 0x7f, 0x72, 0xbe, 0x7d, 0x09, 0xc8, 0xda, 0xd5,
        0x29, 0x1b, 0x0e, 0xd6, 0x58, 0xd6, 0x75, 0xde, 0xe1, 0x2c, 0x26, 0x0e, 0xc1, 0x8e, 0xf1,
        0xb6, 0xa1, 0x1c, 0x46, 0x06, 0x14, 0x1b, 0x10, 0xda, 0xcb, 0x32, 0x1f, 0x94, 0x40, 0x64,
        0x60, 0x3e, 0xa9, 0x88, 0x06, 0x71, 0xe0, 0x8b, 0x6b, 0xea, 0xe1, 0x4d, 0xc8, 0x8b, 0x21,
        0xe1, 0x4c, 0x1c, 0xe4, 0xf7, 0x69, 0x3f, 0x4b, 0xd4, 0x4d, 0x7a, 0xf5, 0xb0, 0xd6, 0xf6,
        0xe8, 0x78, 0xaa, 0x03, 0xcc, 0xe7, 0xfa, 0x6c, 0x5a, 0x6b, 0x58, 0xb0, 0xe7, 0x38, 0x97,
        0xe7, 0x55, 0xa1, 0xf7, 0xf4, 0xdb, 0x90, 0xe7, 0x60, 0x52, 0x64, 0x30, 0xed, 0x8b, 0x15,
        0x27, 0x3e, 0x12, 0xb6, 0x5c, 0x6c, 0xf5, 0x8f, 0x4f, 0x4a, 0x60, 0x5d, 0x23, 0x32, 0x49,
        0x70, 0x20, 0x43, 0x01, 0x34, 0xe2, 0xfc, 0x8e, 0x7e, 0x76, 0x1d, 0x21, 0x2c, 0xcc, 0x7b,
        0x65, 0x47, 0xc5, 0xe8, 0x18, 0xbd, 0x3b, 0x6a, 0xb5, 0xdf, 0x86, 0x0a, 0x6a, 0xc5, 0x41,
        0xb7, 0xf3, 0x0b, 0x64, 0x26, 0x1c, 0x7a, 0xfb, 0x49, 0x00, 0xfc, 0x0b, 0xa5, 0x37, 0xc6,
        0x5b, 0x00, 0x1b, 0x69, 0xa8, 0x90, 0x35, 0xea, 0xea, 0x36, 0xf1, 0x14, 0xea, 0x07, 0x6e,
        0x60, 0x62, 0xfa, 0xf2, 0x16, 0xc5, 0x13, 0x39, 0x43, 0x81, 0xb3, 0x76, 0x73, 0x66, 0x92,
        0x38, 0xca, 0x18, 0x96, 0x17, 0x48, 0xd9, 0xa2, 0x91, 0x6a, 0xc6, 0x17, 0x67, 0x28, 0x55,
        0x50, 0x43, 0xae, 0x75, 0x54, 0x95, 0xda, 0xb0, 0xf3, 0x19, 0x2b, 0x69, 0x87, 0x72, 0x8a,
        0x8e, 0x41, 0x63, 0x7b, 0xee, 0xa3, 0x8b, 0x6d, 0x5e, 0x41, 0x33, 0x0e, 0x25, 0xab, 0x3d,
        0x96, 0x0f, 0x7f, 0xa5, 0x84, 0x86, 0x16, 0x4a, 0x61, 0x3c, 0xdb, 0x41, 0xd6, 0x3b, 0xc4,
        0x23, 0x97, 0xce, 0x08, 0x32, 0x62, 0xe0, 0x43, 0xcb, 0x47, 0x6b, 0x11, 0xde, 0xd4, 0x85,
        0x27, 0xf8, 0x88, 0xf4, 0xe9, 0x6a, 0x53, 0x94, 0x8d, 0xdf, 0xaf, 0x61, 0x20, 0x60, 0x18,
        0xa0, 0x38, 0xee, 0x06, 0x12, 0x6a, 0x9b, 0x52, 0x4a, 0x0c, 0x75, 0x09, 0xeb, 0x06, 0x0e,
        0x6f, 0x4c, 0xec, 0x18, 0xb4, 0x35, 0xdd, 0x6f, 0xb5, 0x0f, 0x06, 0x0a, 0xc7, 0xa4, 0x0c,
        0x4e, 0x1a, 0xc0, 0x00, 0x2b, 0x7f, 0x14, 0x78, 0x13, 0x2c, 0x66, 0x68, 0x76, 0x48, 0x73,
        0x12, 0x14, 0xb3, 0x73, 0x2f, 0xe1, 0x5e, 0x6d, 0x05, 0x98, 0x52, 0x45, 0x80, 0xcc, 0xd6,
        0x3d, 0xcf, 0xdb, 0x8d, 0xa3, 0x2f, 0xcf, 0xae, 0xce, 0x29, 0xa7, 0xdc, 0x1f, 0x81, 0x10,
        0x26, 0x00, 0x93, 0x9d, 0x00, 0xec, 0x4b, 0x3f, 0x8c, 0x3f, 0xa0, 0xfb, 0xda, 0x82, 0xd3,
        0x6d, 0xc1, 0x6b, 0x92, 0x9d, 0x46, 0x03, 0x50, 0x1e, 0xc5, 0xde, 0x3b, 0x45, 0x0b, 0xc2,
        0x02, 0xc5, 0x91, 0xea, 0x56, 0x69, 0x61, 0xc7, 0xa7, 0x49, 0xce, 0x82, 0x93, 0x0a, 0x4a,
        0x76, 0x4e, 0x91, 0x85, 0xed, 0x3f, 0x82, 0x52, 0xc4, 0xf9, 0x11, 0x5c, 0x8f, 0xcd, 0xa6,
        0x4c, 0xc9, 0xeb, 0xe5, 0x77, 0xb4, 0xb3, 0xd7, 0x17, 0x18, 0x21, 0xbe, 0xa4, 0x11, 0x97,
        0x49, 0xc4, 0xc9, 0x1b, 0xca, 0xb4, 0x58, 0xcb, 0xc5, 0xdb, 0x40, 0xe1, 0xc4, 0xb5, 0x87,
        0x02, 0x00,
    ];

    const RANDOM20_WEBP_RGBA: &[u8] = &[
        57, 12, 140, 255, 125, 114, 71, 255, 52, 44, 216, 255, 16, 15, 47, 255, 111, 119, 13, 255,
        101, 214, 112, 255, 229, 142, 3, 255, 81, 216, 174, 255, 142, 79, 110, 255, 172, 52, 47,
        255, 194, 49, 183, 255, 176, 135, 22, 255, 235, 63, 193, 255, 40, 150, 185, 255, 98, 35,
        23, 255, 116, 148, 40, 255, 119, 51, 194, 255, 142, 232, 186, 255, 83, 189, 181, 255, 107,
        136, 36, 255, 87, 125, 83, 255, 236, 194, 138, 255, 112, 166, 28, 255, 117, 16, 161, 255,
        205, 137, 33, 255, 108, 161, 108, 255, 255, 202, 234, 255, 73, 135, 71, 255, 126, 134, 219,
        255, 204, 185, 112, 255, 70, 252, 46, 255, 24, 56, 78, 255, 81, 216, 32, 255, 197, 195,
        239, 255, 128, 5, 58, 255, 136, 174, 57, 255, 150, 222, 80, 255, 232, 1, 134, 255, 91, 54,
        152, 255, 101, 78, 191, 255, 82, 0, 165, 255, 250, 9, 57, 255, 185, 157, 122, 255, 29, 123,
        40, 255, 43, 248, 35, 255, 64, 65, 243, 255, 84, 135, 216, 255, 108, 102, 159, 255, 204,
        191, 224, 255, 231, 61, 126, 255, 115, 32, 173, 255, 10, 117, 112, 255, 3, 36, 30, 255,
        117, 34, 16, 255, 169, 36, 121, 255, 142, 248, 109, 255, 67, 242, 124, 255, 242, 208, 97,
        255, 48, 49, 220, 255, 181, 216, 210, 255, 239, 27, 50, 255, 31, 206, 173, 255, 55, 127,
        98, 255, 97, 229, 71, 255, 216, 93, 142, 255, 236, 127, 38, 255, 226, 50, 25, 255, 7, 47,
        121, 255, 85, 208, 248, 255, 246, 109, 205, 255, 30, 84, 194, 255, 1, 199, 135, 255, 232,
        146, 216, 255, 249, 79, 97, 255, 151, 111, 29, 255, 31, 160, 29, 255, 25, 244, 80, 255, 29,
        41, 95, 255, 35, 34, 120, 255, 206, 61, 126, 255, 20, 41, 214, 255, 161, 133, 104, 255,
        160, 122, 135, 255, 202, 67, 153, 255, 234, 161, 37, 255, 4, 234, 51, 255, 37, 109, 135,
        255, 67, 178, 35, 255, 125, 189, 145, 255, 80, 224, 154, 255, 4, 153, 53, 255, 68, 135, 59,
        255, 54, 79, 139, 255, 144, 107, 175, 255, 104, 135, 250, 255, 128, 26, 47, 255, 216, 141,
        22, 255, 1, 170, 66, 255, 134, 82, 226, 255, 218, 4, 57, 255, 38, 76, 18, 255, 189, 75,
        220, 255, 65, 21, 157, 255, 186, 20, 183, 255, 107, 127, 52, 255, 181, 208, 79, 255, 121,
        83, 90, 255, 211, 12, 91, 255, 170, 210, 127, 255, 136, 81, 55, 255, 195, 19, 240, 255,
        113, 102, 235, 255, 179, 156, 116, 255, 114, 12, 98, 255, 204, 168, 142, 255, 35, 142, 179,
        255, 204, 169, 14, 255, 59, 133, 91, 255, 135, 19, 55, 255, 222, 176, 160, 255, 223, 59,
        197, 255, 97, 130, 22, 255, 223, 0, 100, 255, 186, 220, 35, 255, 169, 160, 63, 255, 153,
        158, 209, 255, 167, 206, 151, 255, 65, 98, 215, 255, 194, 89, 154, 255, 207, 0, 155, 255,
        146, 107, 220, 255, 164, 238, 226, 255, 226, 109, 242, 255, 86, 43, 145, 255, 171, 47, 120,
        255, 158, 115, 101, 255, 75, 12, 23, 255, 125, 243, 37, 255, 233, 212, 99, 255, 196, 253,
        204, 255, 124, 75, 2, 255, 54, 217, 112, 255, 90, 237, 25, 255, 127, 62, 233, 255, 68, 237,
        162, 255, 226, 218, 228, 255, 81, 243, 230, 255, 132, 126, 141, 255, 248, 122, 140, 255,
        225, 39, 146, 255, 120, 139, 171, 255, 163, 41, 70, 255, 77, 118, 196, 255, 78, 109, 32,
        255, 212, 208, 169, 255, 238, 212, 31, 255, 105, 215, 199, 255, 10, 194, 244, 255, 3, 180,
        152, 255, 199, 214, 112, 255, 249, 112, 139, 255, 223, 248, 14, 255, 199, 172, 207, 255,
        84, 239, 65, 255, 13, 201, 13, 255, 42, 219, 69, 255, 236, 93, 25, 255, 133, 194, 167, 255,
        108, 232, 167, 255, 172, 194, 142, 255, 215, 129, 41, 255, 240, 9, 26, 255, 179, 114, 35,
        255, 20, 15, 126, 255, 102, 10, 78, 255, 122, 64, 242, 255, 58, 111, 238, 255, 131, 188,
        85, 255, 58, 83, 159, 255, 55, 13, 159, 255, 192, 203, 101, 255, 38, 124, 52, 255, 154, 61,
        21, 255, 177, 219, 189, 255, 35, 174, 6, 255, 215, 250, 54, 255, 221, 185, 235, 255, 78,
        222, 90, 255, 138, 247, 238, 255, 223, 137, 165, 255, 125, 44, 142, 255, 230, 124, 237,
        255, 194, 172, 14, 255, 253, 166, 93, 255, 249, 108, 181, 255, 132, 174, 143, 255, 141, 5,
        97, 255, 43, 123, 208, 255, 250, 123, 243, 255, 251, 229, 8, 255, 47, 150, 113, 255, 207,
        124, 156, 255, 188, 242, 176, 255, 217, 169, 180, 255, 232, 138, 156, 255, 128, 118, 61,
        255, 98, 161, 61, 255, 94, 98, 110, 255, 247, 141, 144, 255, 51, 99, 151, 255, 116, 184,
        91, 255, 154, 7, 64, 255, 140, 23, 27, 255, 149, 64, 251, 255, 52, 6, 145, 255, 240, 245,
        225, 255, 174, 94, 26, 255, 129, 244, 58, 255, 33, 205, 251, 255, 37, 27, 77, 255, 76, 155,
        43, 255, 127, 60, 213, 255, 115, 194, 230, 255, 226, 152, 219, 255, 156, 30, 50, 255, 106,
        108, 135, 255, 41, 80, 122, 255, 88, 38, 80, 255, 1, 209, 230, 255, 240, 149, 16, 255, 118,
        147, 144, 255, 232, 36, 119, 255, 135, 101, 217, 255, 58, 115, 76, 255, 136, 72, 36, 255,
        30, 84, 157, 255, 147, 224, 63, 255, 239, 155, 206, 255, 139, 252, 224, 255, 41, 20, 221,
        255, 165, 128, 13, 255, 46, 117, 10, 255, 137, 20, 89, 255, 240, 226, 142, 255, 92, 223,
        251, 255, 46, 240, 178, 255, 209, 170, 164, 255, 53, 82, 168, 255, 210, 253, 147, 255, 205,
        18, 232, 255, 45, 161, 129, 255, 165, 59, 206, 255, 0, 236, 211, 255, 27, 96, 185, 255,
        255, 226, 26, 255, 104, 136, 67, 255, 147, 224, 248, 255, 62, 14, 122, 255, 81, 159, 7,
        255, 208, 47, 115, 255, 58, 236, 60, 255, 78, 255, 149, 255, 139, 212, 247, 255, 241, 124,
        233, 255, 74, 196, 97, 255, 69, 35, 141, 255, 212, 174, 136, 255, 1, 144, 152, 255, 250,
        76, 228, 255, 247, 176, 170, 255, 193, 233, 164, 255, 96, 122, 196, 255, 119, 210, 22, 255,
        162, 242, 195, 255, 197, 77, 253, 255, 18, 64, 169, 255, 51, 225, 51, 255, 233, 7, 73, 255,
        209, 79, 38, 255, 240, 135, 173, 255, 203, 41, 168, 255, 194, 162, 249, 255, 18, 35, 120,
        255, 147, 116, 46, 255, 222, 50, 51, 255, 227, 85, 153, 255, 14, 23, 166, 255, 28, 150,
        183, 255, 191, 220, 74, 255, 125, 210, 92, 255, 87, 89, 40, 255, 195, 123, 254, 255, 73,
        118, 236, 255, 130, 235, 130, 255, 4, 238, 147, 255, 80, 37, 226, 255, 176, 153, 217, 255,
        128, 233, 154, 255, 101, 196, 247, 255, 54, 121, 195, 255, 183, 151, 151, 255, 11, 202,
        140, 255, 4, 25, 254, 255, 146, 117, 180, 255, 112, 97, 128, 255, 70, 49, 20, 255, 158,
        225, 17, 255, 186, 67, 46, 255, 151, 167, 212, 255, 89, 102, 67, 255, 187, 139, 84, 255,
        131, 246, 151, 255, 173, 58, 239, 255, 38, 72, 115, 255, 203, 187, 46, 255, 202, 7, 135,
        255, 63, 232, 188, 255, 134, 195, 190, 255, 55, 119, 241, 255, 12, 167, 113, 255, 32, 237,
        154, 255, 209, 59, 71, 255, 23, 19, 155, 255, 252, 59, 49, 255, 120, 69, 198, 255, 232,
        189, 214, 255, 79, 212, 50, 255, 250, 208, 143, 255, 16, 189, 111, 255, 227, 227, 120, 255,
        185, 50, 188, 255, 183, 31, 203, 255, 141, 97, 62, 255, 232, 46, 108, 255, 10, 25, 170,
        255, 124, 64, 105, 255, 35, 106, 110, 255, 119, 168, 75, 255, 1, 141, 74, 255, 66, 128, 89,
        255, 56, 13, 67, 255, 7, 183, 121, 255, 165, 8, 89, 255, 135, 26, 64, 255, 215, 58, 32,
        255, 243, 229, 185, 255, 55, 231, 113, 255, 22, 154, 234, 255, 15, 31, 245, 255, 205, 218,
        55, 255, 251, 227, 37, 255, 41, 164, 75, 255, 33, 64, 140, 255, 166, 195, 150, 255, 232,
        220, 50, 255, 58, 110, 220, 255, 231, 116, 211, 255, 173, 232, 204, 255, 212, 48, 160, 255,
        218, 160, 130, 255, 191, 78, 242, 255, 34, 46, 43, 255, 47, 221, 49, 255, 190, 66, 30, 255,
        168, 62, 210, 255, 181, 216, 26, 255, 147, 159, 180, 255, 53, 108, 79, 255, 246, 114, 55,
        255, 179, 188, 58, 255, 142, 115, 219, 255, 13, 136, 14, 255, 92, 139, 158, 255, 173, 179,
        3, 255, 92, 73, 205, 255, 35, 72, 15, 255, 46, 110, 192, 255, 214, 232, 174, 255, 80, 189,
        159, 255, 166, 43, 26, 255, 79, 80, 25, 255, 41, 139, 226, 255, 217, 248, 226, 255, 212,
        139, 110, 255, 58, 176, 220, 255, 56, 145, 249, 255, 157, 23, 112, 255, 202, 28, 3, 255,
        104, 154, 108, 255, 70, 130, 148, 255, 167, 61, 3, 255, 254, 220, 89, 255, 66, 194, 117,
        255, 181, 36, 203, 255, 21, 223, 9, 255, 235, 39, 160, 255, 219, 207, 213, 255, 148, 58,
        207, 255, 10, 166, 87, 255, 235, 185, 45, 255,
    ];

    fn decode_rgba(input: &[u8]) -> (u32, u32, Vec<u8>) {
        let image = decode(input, &ConvertOptions::default()).unwrap();
        (image.width, image.height, image.pixels)
    }

    #[test]
    fn decode_matches_an_independently_built_palette_webp_width_bits_3() {
        let (w, h, pixels) = decode_rgba(TINY_WEBP);
        assert_eq!((w, h), (2, 2));
        assert_eq!(pixels, TINY_WEBP_RGBA);
    }

    #[test]
    fn decode_matches_an_independently_built_palette_webp_width_bits_1() {
        let (w, h, pixels) = decode_rgba(PAL16_WEBP);
        assert_eq!((w, h), (5, 5));
        let mut expected = Vec::with_capacity(25 * 4);
        for y in 0..5u32 {
            for x in 0..5u32 {
                let c = (x + y * 5) % 16;
                expected.extend_from_slice(&[(c * 15) as u8, (c * 10) as u8, (c * 5) as u8, 255]);
            }
        }
        assert_eq!(pixels, expected);
    }

    #[test]
    fn decode_matches_an_independently_built_webp_with_heavy_backward_references() {
        let (w, h, pixels) = decode_rgba(PATTERN30_WEBP);
        assert_eq!((w, h), (30, 30));
        let mut expected = Vec::with_capacity(30 * 30 * 4);
        for y in 0..30u32 {
            for x in 0..30u32 {
                if (x / 3 + y / 3) % 2 == 0 {
                    expected.extend_from_slice(&[50, 60, 70, 255]);
                } else {
                    expected.extend_from_slice(&[200, 210, 220, 255]);
                }
            }
        }
        assert_eq!(pixels, expected);
    }

    #[test]
    fn decode_matches_an_independently_built_webp_using_predictor_and_meta_prefix() {
        let (w, h, pixels) = decode_rgba(RANDOM20_WEBP);
        assert_eq!((w, h), (20, 20));
        assert_eq!(pixels, RANDOM20_WEBP_RGBA);
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
        RawImage::new(width, height, pixels, Format::Webp).unwrap()
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
        image.pixels[3] = 128;
        let bytes = encode(&image, &ConvertOptions::default()).unwrap();
        let decoded = decode(&bytes, &ConvertOptions::default()).unwrap();
        assert_eq!(decoded.pixels, image.pixels);
    }

    #[test]
    fn decode_rejects_bad_riff_header() {
        let mut bytes = TINY_WEBP.to_vec();
        bytes[0] = b'X';
        assert!(matches!(
            decode(&bytes, &ConvertOptions::default()),
            Err(ConvertError::MalformedInput {
                format: Format::Webp
            })
        ));
    }

    #[test]
    fn decode_rejects_truncated_input() {
        let truncated = &TINY_WEBP[..TINY_WEBP.len() / 2];
        assert!(matches!(
            decode(truncated, &ConvertOptions::default()),
            Err(ConvertError::MalformedInput {
                format: Format::Webp
            })
        ));
    }

    #[test]
    fn encode_rejects_images_wider_than_vp8ls_14_bit_field_can_hold() {
        // A 1-pixel-tall image keeps this well under this crate's own MAX_PIXELS ceiling (so
        // RawImage::new itself accepts it) while still exceeding MAX_DIMENSION (16384) — the
        // limit VP8L's 14-bit width field can represent at all.
        let width = MAX_DIMENSION + 1;
        let image = RawImage::new(width, 1, vec![0u8; width as usize * 4], Format::Webp).unwrap();
        assert!(matches!(
            encode(&image, &ConvertOptions::default()),
            Err(ConvertError::UnsupportedFeature {
                format: Format::Webp,
                feature: "webp-size-too-large"
            })
        ));
    }

    /// Color transform correctness, checked directly against the bitstream spec's formulas
    /// rather than against a real file — see this module's header docs for why no real encoder
    /// tried here ever chose this transform.
    #[test]
    fn color_transform_delta_matches_spec_formula() {
        // t=64 (positive int8), c=32 (positive int8): (64*32)>>5 = 2048>>5 = 64.
        assert_eq!(color_transform_delta(64, 32), 64);
        // t=192 (=-64 as int8), c=32: (-64*32)>>5 = -2048>>5 = -64.
        assert_eq!(color_transform_delta(192, 32), -64);
        // t=0: delta is always 0 regardless of c.
        assert_eq!(color_transform_delta(0, 200), 0);
    }

    #[test]
    fn apply_color_transform_inverse_matches_a_hand_computed_example() {
        // green_to_red=64, green_to_blue=0, red_to_blue=32 packed as element = alpha(255) red
        // (=red_to_blue=32) green(=green_to_blue=0) blue(=green_to_red=64).
        let element: u32 = (255 << 24) | (32 << 16) | 64;
        let mut pixels = [combine([255, 100, 50, 10])]; // alpha, red, green, blue
        apply_color_transform_inverse(&mut pixels, 1, 1, 0, 1, &[element]);

        let green = 50u8;
        let expected_red = 100u8.wrapping_add(color_transform_delta(64, green) as u8);
        let expected_blue_step1 = 10u8.wrapping_add(color_transform_delta(0, green) as u8);
        let expected_blue =
            expected_blue_step1.wrapping_add(color_transform_delta(32, expected_red) as u8);
        assert_eq!(
            pixels[0],
            combine([255, expected_red, green, expected_blue])
        );
    }
}
