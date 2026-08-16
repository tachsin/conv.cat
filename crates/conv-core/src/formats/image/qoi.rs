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
use crate::{ConvertError, ConvertOptions, Format};

/// How often (in pixels) the encode/decode loops below poll cancellation and report progress.
/// Checking every single pixel would be wasted work at this crate's multi-million-pixel size
/// ceiling (see `raster::MAX_PIXELS`); this is frequent enough to keep a cancel request
/// responsive and progress visibly moving without paying a `report_progress` call (a callback
/// across the wasm boundary, in the browser build) per pixel.
const PROGRESS_GRANULARITY: usize = 4096;

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
///
/// Polls `options` for cancellation and reports progress (as the `0.5..=1.0` half of a full
/// decode-then-encode conversion — see [`super::converter::RasterConverter`]) every
/// [`PROGRESS_GRANULARITY`] pixels, so this stays responsive on a large image.
pub fn encode(image: &RawImage, options: &ConvertOptions) -> Result<Vec<u8>, ConvertError> {
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
        if i.is_multiple_of(PROGRESS_GRANULARITY) {
            if options.is_cancelled() {
                return Err(ConvertError::Cancelled);
            }
            options.report_progress(0.5 + 0.5 * (i as f32 / pixel_count.max(1) as f32));
        }

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
    options.report_progress(1.0);
    Ok(out)
}

/// Decodes a QOI byte stream into a [`RawImage`].
///
/// Polls `options` for cancellation and reports progress (the `0.0..=0.5` half — see
/// [`super::converter::RasterConverter`]) every [`PROGRESS_GRANULARITY`] pixels. Validates the
/// mandatory 8-byte end marker and rejects any trailing bytes after it, in addition to the
/// per-chunk bounds checks below — a stream that decodes the right pixel count but doesn't
/// actually end where the format says it must is malformed, not just "good enough".
pub fn decode(input: &[u8], options: &ConvertOptions) -> Result<RawImage, ConvertError> {
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
    // Pixels decoded since the last cancellation/progress check. A `RUN` chunk can advance
    // `decoded` by up to 62 in a single loop iteration, so checking only on an exact multiple of
    // `PROGRESS_GRANULARITY` (as the encode loop below can, since it advances by exactly 1 pixel
    // per iteration) could jump straight past every checkpoint and never poll again after the
    // first one — this accumulator guarantees a check at least every `PROGRESS_GRANULARITY`
    // pixels of real progress regardless of how the stream is chunked.
    let mut since_check = PROGRESS_GRANULARITY;

    while decoded < pixel_count {
        if since_check >= PROGRESS_GRANULARITY {
            if options.is_cancelled() {
                return Err(ConvertError::Cancelled);
            }
            options.report_progress(0.5 * (decoded as f32 / pixel_count.max(1) as f32));
            since_check = 0;
        }

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
                    since_check += take;
                    continue;
                }
            }
        };

        index[hash(pixel)] = pixel;
        pixels.extend_from_slice(&pixel);
        prev = pixel;
        decoded += 1;
        since_check += 1;
    }

    let marker = body
        .get(pos..pos + END_MARKER.len())
        .ok_or_else(malformed)?;
    if marker != END_MARKER || pos + END_MARKER.len() != body.len() {
        return Err(malformed());
    }

    options.report_progress(0.5);
    RawImage::new(width, height, pixels, Format::Qoi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProgressSink;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Reports "not cancelled" for its first `cancel_after` polls, then "cancelled" forever
    /// after — lets a test prove the loop checks cancellation more than once (at a real
    /// `PROGRESS_GRANULARITY` checkpoint), not just at its very first opportunity.
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

    // Two full granularity windows, so there's a real second checkpoint to reach.
    const TEST_PIXEL_COUNT: usize = PROGRESS_GRANULARITY * 2;

    fn solid_white_image() -> RawImage {
        RawImage::new(
            TEST_PIXEL_COUNT as u32,
            1,
            vec![255u8; TEST_PIXEL_COUNT * 4],
            Format::Qoi,
        )
        .unwrap()
    }

    #[test]
    fn decode_checks_cancellation_past_the_first_granularity_window() {
        let bytes = encode(&solid_white_image(), &ConvertOptions::default()).unwrap();

        let (options, sink) = options_cancelling_after(1);
        let result = decode(&bytes, &options);

        assert!(matches!(result, Err(ConvertError::Cancelled)));
        // The first poll (decoded == 0) must have returned false, or the loop would never have
        // decoded far enough to reach the second `PROGRESS_GRANULARITY` checkpoint at all.
        assert!(sink.calls.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn encode_checks_cancellation_past_the_first_granularity_window() {
        let (options, sink) = options_cancelling_after(1);
        let result = encode(&solid_white_image(), &options);

        assert!(matches!(result, Err(ConvertError::Cancelled)));
        assert!(sink.calls.load(Ordering::SeqCst) >= 2);
    }
}
