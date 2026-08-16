//! [QOI](https://qoiformat.org/) ("Quite OK Image") decode and encode — a small, fully-specified,
//! deterministic lossless format. This is `docs/adding-a-format.md`'s own worked example; this
//! module is the real implementation the doc's `todo!()` placeholder stands in for.
//!
//! Wire format: a 14-byte header (`"qoif"`, big-endian `width`/`height`, `channels`,
//! `colorspace`), a stream of variable-length chunks describing each pixel relative to a running
//! previous-pixel/64-slot index cache, and an 8-byte end marker (seven `0x00` bytes then `0x01`).
//! See the [spec](https://qoiformat.org/qoi-specification.pdf) for the chunk encoding this module
//! implements byte-for-byte.

use super::raster::{checked_rgba_len, RawImage};
use crate::{ConvertError, Format};

const MAGIC: &[u8; 4] = b"qoif";
const HEADER_LEN: usize = 14;
const END_MARKER: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];

const OP_RGB: u8 = 0b1111_1110;
const OP_RGBA: u8 = 0b1111_1111;
const OP_INDEX: u8 = 0b0000_0000;
const OP_DIFF: u8 = 0b0100_0000;
const OP_LUMA: u8 = 0b1000_0000;
const OP_RUN: u8 = 0b1100_0000;
const TAG_MASK: u8 = 0b1100_0000;

type Pixel = [u8; 4];

fn hash(pixel: Pixel) -> usize {
    let [r, g, b, a] = pixel;
    (usize::from(r) * 3 + usize::from(g) * 5 + usize::from(b) * 7 + usize::from(a) * 11) % 64
}

/// Encodes a [`RawImage`] as QOI. Always writes `channels = 4` (RGBA) and `colorspace = 0`
/// (sRGB with linear alpha) — [`RawImage`] always carries alpha, and the previous pixel a decoder
/// starts from is `(0, 0, 0, 255)` regardless of the `channels` byte, so there's no information
/// loss in always declaring 4.
pub fn encode(image: &RawImage) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + image.pixels.len() + END_MARKER.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&image.width.to_be_bytes());
    out.extend_from_slice(&image.height.to_be_bytes());
    out.push(4); // channels
    out.push(0); // colorspace

    let mut index = [[0u8; 4]; 64];
    let mut prev: Pixel = [0, 0, 0, 255];
    let mut run: u8 = 0;

    let pixel_count = image.pixels.len() / 4;
    for i in 0..pixel_count {
        let base = i * 4;
        let pixel: Pixel = [
            image.pixels[base],
            image.pixels[base + 1],
            image.pixels[base + 2],
            image.pixels[base + 3],
        ];

        if pixel == prev {
            run += 1;
            if run == 62 || i == pixel_count - 1 {
                out.push(OP_RUN | (run - 1));
                run = 0;
            }
            continue;
        }
        if run > 0 {
            out.push(OP_RUN | (run - 1));
            run = 0;
        }

        let idx = hash(pixel);
        if index[idx] == pixel {
            out.push(OP_INDEX | idx as u8);
        } else {
            index[idx] = pixel;

            if pixel[3] == prev[3] {
                let dr = pixel[0].wrapping_sub(prev[0]) as i8;
                let dg = pixel[1].wrapping_sub(prev[1]) as i8;
                let db = pixel[2].wrapping_sub(prev[2]) as i8;

                if (-2..=1).contains(&dr) && (-2..=1).contains(&dg) && (-2..=1).contains(&db) {
                    out.push(
                        OP_DIFF
                            | (((dr + 2) as u8) << 4)
                            | (((dg + 2) as u8) << 2)
                            | (db + 2) as u8,
                    );
                } else {
                    let dr_dg = dr.wrapping_sub(dg);
                    let db_dg = db.wrapping_sub(dg);
                    if (-32..=31).contains(&dg)
                        && (-8..=7).contains(&dr_dg)
                        && (-8..=7).contains(&db_dg)
                    {
                        out.push(OP_LUMA | (dg + 32) as u8);
                        out.push((((dr_dg + 8) as u8) << 4) | (db_dg + 8) as u8);
                    } else {
                        out.push(OP_RGB);
                        out.extend_from_slice(&pixel[0..3]);
                    }
                }
            } else {
                out.push(OP_RGBA);
                out.extend_from_slice(&pixel);
            }
        }

        prev = pixel;
    }

    out.extend_from_slice(&END_MARKER);
    out
}

/// Decodes a QOI byte stream into a [`RawImage`].
pub fn decode(input: &[u8]) -> Result<RawImage, ConvertError> {
    let malformed = || ConvertError::MalformedInput {
        format: Format::Qoi,
    };

    if input.len() < HEADER_LEN + END_MARKER.len() || &input[0..4] != MAGIC {
        return Err(malformed());
    }
    let width = u32::from_be_bytes([input[4], input[5], input[6], input[7]]);
    let height = u32::from_be_bytes([input[8], input[9], input[10], input[11]]);
    let channels = input[12];
    let colorspace = input[13];
    if !(3..=4).contains(&channels) || colorspace > 1 {
        return Err(malformed());
    }

    let rgba_len = checked_rgba_len(width, height, Format::Qoi)?;
    let pixel_count = (rgba_len / 4) as usize;

    let mut pixels = Vec::with_capacity(rgba_len as usize);
    let mut index = [[0u8; 4]; 64];
    let mut prev: Pixel = [0, 0, 0, 255];

    let body = &input[HEADER_LEN..];
    let mut pos = 0usize;
    let mut decoded = 0usize;

    while decoded < pixel_count {
        let byte = *body.get(pos).ok_or_else(malformed)?;
        pos += 1;

        let pixel = if byte == OP_RGB {
            let chunk = body.get(pos..pos + 3).ok_or_else(malformed)?;
            pos += 3;
            [chunk[0], chunk[1], chunk[2], prev[3]]
        } else if byte == OP_RGBA {
            let chunk = body.get(pos..pos + 4).ok_or_else(malformed)?;
            pos += 4;
            [chunk[0], chunk[1], chunk[2], chunk[3]]
        } else {
            match byte & TAG_MASK {
                OP_INDEX => index[(byte & 0x3F) as usize],
                OP_DIFF => {
                    let dr = ((byte >> 4) & 0x03) as i8 - 2;
                    let dg = ((byte >> 2) & 0x03) as i8 - 2;
                    let db = (byte & 0x03) as i8 - 2;
                    [
                        prev[0].wrapping_add(dr as u8),
                        prev[1].wrapping_add(dg as u8),
                        prev[2].wrapping_add(db as u8),
                        prev[3],
                    ]
                }
                OP_LUMA => {
                    let dg = (byte & 0x3F) as i8 - 32;
                    let byte2 = *body.get(pos).ok_or_else(malformed)?;
                    pos += 1;
                    let dr_dg = ((byte2 >> 4) & 0x0F) as i8 - 8;
                    let db_dg = (byte2 & 0x0F) as i8 - 8;
                    let dr = dg.wrapping_add(dr_dg);
                    let db = dg.wrapping_add(db_dg);
                    [
                        prev[0].wrapping_add(dr as u8),
                        prev[1].wrapping_add(dg as u8),
                        prev[2].wrapping_add(db as u8),
                        prev[3],
                    ]
                }
                _ => {
                    // OP_RUN
                    let run = (byte & 0x3F) as usize + 1;
                    let remaining = pixel_count - decoded;
                    let take = run.min(remaining);
                    for _ in 0..take {
                        pixels.extend_from_slice(&prev);
                    }
                    decoded += take;
                    continue;
                }
            }
        };

        index[hash(pixel)] = pixel;
        pixels.extend_from_slice(&pixel);
        prev = pixel;
        decoded += 1;
    }

    RawImage::new(width, height, pixels, Format::Qoi)
}
