//! [`RasterConverter`]: one [`Converter`] handling every registered raster pair in this module, by
//! decoding `from` to a [`RawImage`] and encoding it as `to` — the pattern
//! `crate::Converter::convert`'s own rustdoc names as the expected shape for a shared raster
//! implementation.

use super::{bmp, png, qoi, raster::RawImage};
use crate::{ConvertError, ConvertOptions, Converter, Format};

/// Decodes `from`, then encodes as `to`. Registered for every raster `(from, to)` pair this crate
/// currently supports (see `crates/conv-core/src/lib.rs`'s `default_registry`); an unrecognized
/// pair falls through to [`ConvertError::UnsupportedPair`] as a defensive fallback, though the
/// registry itself is what actually keeps unregistered pairs from reaching this type.
#[derive(Debug, Default, Clone, Copy)]
pub struct RasterConverter;

impl Converter for RasterConverter {
    fn convert(
        &self,
        input: &[u8],
        from: Format,
        to: Format,
        options: &ConvertOptions,
    ) -> Result<Vec<u8>, ConvertError> {
        if options.is_cancelled() {
            return Err(ConvertError::Cancelled);
        }

        let image = decode(input, from, options)?;

        match to {
            Format::Bmp => bmp::encode(&image, options),
            Format::Qoi => qoi::encode(&image, options),
            Format::Png => png::encode(&image, options),
            _ => Err(ConvertError::UnsupportedPair { from, to }),
        }
    }
}

fn decode(input: &[u8], from: Format, options: &ConvertOptions) -> Result<RawImage, ConvertError> {
    match from {
        Format::Bmp => bmp::decode(input, options),
        Format::Qoi => qoi::decode(input, options),
        Format::Png => png::decode(input, options),
        _ => Err(ConvertError::UnsupportedPair { from, to: from }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::image::FORMATS;

    /// Encodes `image` as `format`, for test setup only — a small, deliberately parallel
    /// dispatch to [`convert`]'s own `match to { .. }` above, so a format missing from *either*
    /// match is visible right next to the other in this same file.
    fn encode_as(format: Format, image: &RawImage, options: &ConvertOptions) -> Vec<u8> {
        match format {
            Format::Bmp => bmp::encode(image, options),
            Format::Qoi => qoi::encode(image, options),
            _ => panic!(
                "encode_as has no encoder wired up for {format:?} — it was added to \
                 `formats::image::FORMATS` but this test helper wasn't taught how to produce \
                 sample bytes for it"
            ),
        }
        .expect("encoding a valid 1x1 RawImage should never fail")
    }

    /// Guards against [`FORMATS`] drifting from [`RasterConverter`]'s own `decode`/`convert`
    /// match arms — the mismatch Sourcery's review of the PR that introduced `FORMATS` flagged:
    /// if a format lands in the list but its match arm is forgotten, nothing fails to compile;
    /// `RasterConverter::convert` just falls through to `ConvertError::UnsupportedPair` at
    /// runtime for a pair that superficially looks registered. This test round-trips every
    /// ordered pair `FORMATS` claims to support through the real `RasterConverter` and fails
    /// loudly if any of them hit that fallback instead of actually converting.
    #[test]
    fn every_pair_in_formats_is_wired_up_in_decode_and_convert() {
        let options = ConvertOptions::default();
        let image = RawImage::new(1, 1, vec![10, 20, 30, 255], Format::Bmp)
            .expect("a 1x1 RGBA image is always valid");
        let converter = RasterConverter;

        for &from in FORMATS {
            let encoded = encode_as(from, &image, &options);
            for &to in FORMATS {
                if from == to {
                    continue; // self-pairs are never registered — see `default_registry`
                }
                let result = converter.convert(&encoded, from, to, &options);
                assert!(
                    !matches!(result, Err(ConvertError::UnsupportedPair { .. })),
                    "{from:?} -> {to:?} is listed in FORMATS but RasterConverter's decode/convert \
                     match arms don't handle it"
                );
            }
        }
    }
}
