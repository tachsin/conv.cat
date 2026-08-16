//! [`RasterConverter`]: one [`Converter`] handling every registered raster pair in this module, by
//! decoding `from` to a [`RawImage`] and encoding it as `to` — the pattern
//! `crate::Converter::convert`'s own rustdoc names as the expected shape for a shared raster
//! implementation.

use super::{bmp, qoi, raster::RawImage};
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
            _ => Err(ConvertError::UnsupportedPair { from, to }),
        }
    }
}

fn decode(input: &[u8], from: Format, options: &ConvertOptions) -> Result<RawImage, ConvertError> {
    match from {
        Format::Bmp => bmp::decode(input, options),
        Format::Qoi => qoi::decode(input, options),
        _ => Err(ConvertError::UnsupportedPair { from, to: from }),
    }
}
