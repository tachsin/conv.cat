//! Uncompressed 24-bit BMP (`BITMAPFILEHEADER` + 40-byte `BITMAPINFOHEADER`, `BI_RGB`) decode and
//! encode. This is the common case for a BMP file and enough to prove a real round trip; anything
//! else this format can legally contain (RLE compression, <24bpp palettised, top-down row order,
//! OS/2-style header variants) is an honest gap — [`decode`] returns
//! [`ConvertError::UnsupportedFeature`] rather than guessing.

use super::raster::{checked_rgba_len, RawImage};
use crate::{ConvertError, Format};

const FILE_HEADER_LEN: usize = 14;
const INFO_HEADER_LEN: usize = 40;
const BI_RGB: u32 = 0;

/// Decodes a 24-bit, uncompressed, bottom-up BMP into a [`RawImage`].
pub fn decode(input: &[u8]) -> Result<RawImage, ConvertError> {
    let malformed = || ConvertError::MalformedInput {
        format: Format::Bmp,
    };
    let unsupported = |feature| ConvertError::UnsupportedFeature {
        format: Format::Bmp,
        feature,
    };

    if input.len() < FILE_HEADER_LEN + INFO_HEADER_LEN {
        return Err(malformed());
    }
    if input[0..2] != *b"BM" {
        return Err(malformed());
    }

    let bf_off_bits = read_u32_le(input, 10);
    let bi_size = read_u32_le(input, 14);
    if bi_size != INFO_HEADER_LEN as u32 {
        // Every other DIB header variant (OS/2 12-byte, V4, V5, ...) has a different bi_size.
        return Err(unsupported("bmp-header-variant"));
    }

    let bi_width = read_i32_le(input, 18);
    let bi_height = read_i32_le(input, 22);
    let bi_planes = read_u16_le(input, 26);
    let bi_bit_count = read_u16_le(input, 28);
    let bi_compression = read_u32_le(input, 30);

    if bi_planes != 1 {
        return Err(malformed());
    }
    if bi_compression != BI_RGB {
        return Err(unsupported("bmp-compression"));
    }
    if bi_bit_count != 24 {
        return Err(unsupported("bmp-bit-depth"));
    }
    if bi_width <= 0 {
        return Err(malformed());
    }
    if bi_height < 0 {
        return Err(unsupported("bmp-top-down"));
    }
    if bi_height == 0 {
        return Err(malformed());
    }

    let width = bi_width as u32;
    let height = bi_height as u32;
    let rgba_len = checked_rgba_len(width, height, Format::Bmp)?;

    let row_bytes = u64::from(width).checked_mul(3).ok_or_else(malformed)?;
    let row_stride = row_bytes.div_ceil(4) * 4;
    let pixel_data_len = row_stride
        .checked_mul(u64::from(height))
        .ok_or_else(malformed)?;
    let end = u64::from(bf_off_bits)
        .checked_add(pixel_data_len)
        .ok_or_else(malformed)?;
    if end > input.len() as u64 {
        return Err(malformed());
    }

    let off_bits = bf_off_bits as usize;
    let row_stride = row_stride as usize;
    let mut pixels = vec![0u8; rgba_len as usize];

    for file_row in 0..height as usize {
        // Bottom-up storage: the first row on disk is the bottom scanline of the image.
        let image_row = height as usize - 1 - file_row;
        let row_start = off_bits + file_row * row_stride;
        let row = input
            .get(row_start..row_start + row_bytes as usize)
            .ok_or_else(malformed)?;

        for x in 0..width as usize {
            let src = x * 3;
            let (b, g, r) = (row[src], row[src + 1], row[src + 2]);
            let dst = (image_row * width as usize + x) * 4;
            pixels[dst] = r;
            pixels[dst + 1] = g;
            pixels[dst + 2] = b;
            pixels[dst + 3] = 255;
        }
    }

    RawImage::new(width, height, pixels, Format::Bmp)
}

/// Encodes a [`RawImage`] as a 24-bit, uncompressed, bottom-up BMP. Lossy only in the sense that
/// alpha is dropped (BMP's `BI_RGB` has no alpha channel) — otherwise byte-exact and deterministic.
pub fn encode(image: &RawImage) -> Vec<u8> {
    let width = image.width as u64;
    let height = image.height as u64;
    let row_bytes = width * 3;
    let row_stride = (row_bytes.div_ceil(4) * 4) as usize;
    let pixel_data_len = row_stride as u64 * height;
    let off_bits = (FILE_HEADER_LEN + INFO_HEADER_LEN) as u32;
    let file_size = off_bits as u64 + pixel_data_len;

    let mut out = Vec::with_capacity(file_size as usize);

    // BITMAPFILEHEADER
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // bfReserved1
    out.extend_from_slice(&0u16.to_le_bytes()); // bfReserved2
    out.extend_from_slice(&off_bits.to_le_bytes());

    // BITMAPINFOHEADER
    out.extend_from_slice(&(INFO_HEADER_LEN as u32).to_le_bytes());
    out.extend_from_slice(&image.width.to_le_bytes());
    out.extend_from_slice(&image.height.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&24u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&BI_RGB.to_le_bytes()); // biCompression
    out.extend_from_slice(&(pixel_data_len as u32).to_le_bytes()); // biSizeImage
    out.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    out.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    out.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

    let width = image.width as usize;
    let height = image.height as usize;
    let padding = row_stride - width * 3;

    for file_row in 0..height {
        // Bottom-up storage: emit the bottom image row first.
        let image_row = height - 1 - file_row;
        for x in 0..width {
            let src = (image_row * width + x) * 4;
            let (r, g, b) = (
                image.pixels[src],
                image.pixels[src + 1],
                image.pixels[src + 2],
            );
            out.push(b);
            out.push(g);
            out.push(r);
        }
        out.resize(out.len() + padding, 0);
    }

    out
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
