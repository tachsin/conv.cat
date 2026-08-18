//! Baseline JPEG (ITU-T T.81) decode and encode — sequential DCT, Huffman entropy coding only.
//! No progressive scans, no arithmetic coding, no 12-bit precision, no CMYK — every one of those
//! is a real, separate encoding mode this module rejects as [`ConvertError::UnsupportedFeature`]
//! rather than silently mis-decoding.
//!
//! Unlike every other format in this hub, JPEG is genuinely **lossy**: the quantization step (see
//! [`quantize`]) throws away high-frequency detail on purpose, so there is no byte-exact golden
//! file for a JPEG encode the way there is for PNG or GIF — see `tests/golden.rs`'s JPEG section
//! for the tolerance-band style this uses instead.
//!
//! The decoder handles whatever chroma subsampling a real encoder chose (4:4:4, 4:2:2, 4:2:0, or
//! any other `Hi`/`Vi` combination up to 4, with nearest-neighbor chroma upsampling) — this isn't
//! optional, since 4:2:0 is what nearly every real-world JPEG (phone cameras, Pillow's default
//! `save`, libjpeg's default) actually uses. The **encoder** always emits 4:4:4 (no chroma
//! subsampling): the standard [`MIN_QUALITY`]..[`MAX_QUALITY`] IJG-style quality scaling ([`scale_quant_table`])
//! and the standard Annex K Huffman tables ([`STD_DC_LUMA_BITS`] and friends — the same tables
//! libjpeg ships as its non-optimized default) are what every basic encoder implementation uses;
//! subsampling on encode would additionally need edge-aware block padding and a box-filter
//! downsampler, extra machinery this module skips as an honest scope line, the same way
//! [`super::gif`]'s encoder only ever writes non-interlaced output despite decoding interlaced
//! files just fine.

use super::raster::{checked_pixel_count, checked_rgba_len, RawImage};
use crate::{ConvertError, ConvertOptions, Format};

fn malformed() -> ConvertError {
    ConvertError::MalformedInput {
        format: Format::Jpeg,
    }
}

fn unsupported(feature: &'static str) -> ConvertError {
    ConvertError::UnsupportedFeature {
        format: Format::Jpeg,
        feature,
    }
}

fn internal(detail: &'static str) -> ConvertError {
    ConvertError::Internal { detail }
}

/// Default JPEG encode quality (IJG `1..=100` scale) when [`ConvertOptions::jpeg_quality`] is
/// unset — the same default browsers and most image tools converge on for "no visible size vs.
/// quality complaint" output.
pub const DEFAULT_QUALITY: u8 = 75;
const MIN_QUALITY: u8 = 1;
const MAX_QUALITY: u8 = 100;

// ─── Zigzag scan order ────────────────────────────────────────────────────────
//
// `ZIGZAG[i]` is the natural (row-major, `row * 8 + col`) position of the coefficient that is the
// `i`-th one transmitted in a JPEG entropy-coded block — DC first, then AC coefficients in the
// diagonal zigzag order that groups low frequencies together so trailing runs of zero
// high-frequency coefficients compress well. Defined once here; both the coefficient stream and
// the quantization table (`DQT`, `parse_dqt`) are transmitted in this same order.
#[rustfmt::skip]
const ZIGZAG: [usize; 64] = [
     0,  1,  8, 16,  9,  2,  3, 10,
    17, 24, 32, 25, 18, 11,  4,  5,
    12, 19, 26, 33, 40, 48, 41, 34,
    27, 20, 13,  6,  7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46,
    53, 60, 61, 54, 47, 55, 62, 63,
];

// ─── Standard (Annex K) quantization and Huffman tables ──────────────────────
//
// These are the exact tables ITU-T T.81 Annex K publishes as informative examples, and that
// libjpeg (and therefore Pillow, and therefore most JPEGs on the web) ships as its default
// quality-50 quantization tables and its default ("not optimized") Huffman tables. Using them for
// this encoder is what makes a from-scratch encoder without an adaptive Huffman-table builder or
// a rate-distortion quantizer still produce standard-conformant, widely-interoperable output.
// Given in natural (row-major) order; converted to zigzag order once per encode via
// `natural_to_zigzag`, matching the order `DQT` segments are written in.
#[rustfmt::skip]
const STD_LUMA_QT_NATURAL: [u16; 64] = [
    16, 11, 10, 16,  24,  40,  51,  61,
    12, 12, 14, 19,  26,  58,  60,  55,
    14, 13, 16, 24,  40,  57,  69,  56,
    14, 17, 22, 29,  51,  87,  80,  62,
    18, 22, 37, 56,  68, 109, 103,  77,
    24, 35, 55, 64,  81, 104, 113,  92,
    49, 64, 78, 87, 103, 121, 120, 101,
    72, 92, 95, 98, 112, 100, 103,  99,
];

#[rustfmt::skip]
const STD_CHROMA_QT_NATURAL: [u16; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99,
    18, 21, 26, 66, 99, 99, 99, 99,
    24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
];

const STD_DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const STD_DC_LUMA_VALUES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

const STD_DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const STD_DC_CHROMA_VALUES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

const STD_AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
#[rustfmt::skip]
const STD_AC_LUMA_VALUES: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12,
    0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08,
    0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16,
    0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39,
    0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59,
    0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79,
    0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98,
    0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6,
    0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4,
    0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea,
    0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

const STD_AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
#[rustfmt::skip]
const STD_AC_CHROMA_VALUES: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21,
    0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91,
    0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34,
    0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38,
    0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58,
    0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78,
    0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96,
    0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4,
    0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2,
    0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9,
    0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

// ─── Markers ───────────────────────────────────────────────────────────────

const SOI: u8 = 0xd8;
const EOI: u8 = 0xd9;
const SOF0: u8 = 0xc0;
const SOF1: u8 = 0xc1;
const DHT: u8 = 0xc4;
const DAC: u8 = 0xcc;
const DQT: u8 = 0xdb;
const DRI: u8 = 0xdd;
const SOS: u8 = 0xda;

/// Every `SOFn` marker this module recognizes but does not support — progressive, lossless,
/// differential, and arithmetic-coded variants. Baseline (`SOF0`) and extended-sequential-Huffman
/// (`SOF1`) are handled identically here; the only difference between them the bitstream format
/// itself cares about (allowed sample precision, table-count limits) doesn't matter once this
/// module has already required 8-bit precision.
const UNSUPPORTED_SOF_MARKERS: [u8; 11] = [
    0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf,
];

// ─── Bit-level entropy coding: canonical Huffman, MSB-first, `0xFF00`-stuffed ─
//
// JPEG's Huffman codes are canonical (same construction as DEFLATE's — see `super::zlib`) and
// packed MSB-first within each byte (also like DEFLATE, unlike GIF's LSB-first LZW codes or
// VP8L's reversed-bit convention — every format in this hub picked its own bit order, worth
// stating plainly rather than trusting memory for). The one JPEG-specific wrinkle: inside
// entropy-coded scan data, a raw `0xFF` byte is always followed by a stuffing `0x00` (to keep any
// real `0xFF` byte from being misread as the start of a marker) — [`BitReader`]/[`BitWriter`]
// destuff/stuff transparently so nothing above them has to think about it.

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    acc: u32,
    nbits: u32,
    hit_marker: bool,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8], pos: usize) -> Self {
        BitReader {
            data,
            pos,
            acc: 0,
            nbits: 0,
            hit_marker: false,
        }
    }

    /// Returns the next destuffed data byte, or `None` once a real marker (or end of input) is
    /// hit — at which point this reader stops advancing `pos` so [`BitReader::byte_align`] can
    /// hand control back to marker-level parsing at the right offset.
    fn next_byte(&mut self) -> Option<u8> {
        if self.hit_marker {
            return None;
        }
        let b = *self.data.get(self.pos)?;
        if b == 0xff {
            match self.data.get(self.pos + 1).copied() {
                Some(0x00) => {
                    self.pos += 2;
                    Some(0xff)
                }
                _ => {
                    self.hit_marker = true;
                    None
                }
            }
        } else {
            self.pos += 1;
            Some(b)
        }
    }

    /// Reads `n` (`0..=24`) bits, MSB-first. Once real data runs out (a marker or truncated
    /// input), pads with `1`-bits — the standard JPEG convention for the last partial byte before
    /// a marker, and a safe, bounded fallback for truncated input: every block decode reads a
    /// fixed-bounded number of coefficients (see `decode_block`), so this can never spin forever
    /// even against adversarial input, it just eventually fails a Huffman/range check instead.
    fn get_bits(&mut self, n: u32) -> Result<u32, ConvertError> {
        if n == 0 {
            return Ok(0);
        }
        while self.nbits < n {
            let byte = self.next_byte().unwrap_or(0xff);
            self.acc = (self.acc << 8) | u32::from(byte);
            self.nbits += 8;
        }
        let shift = self.nbits - n;
        let value = (self.acc >> shift) & ((1u32 << n) - 1);
        self.nbits -= n;
        Ok(value)
    }

    /// Drops any buffered bits from the current (necessarily padding-only, per the restart-marker
    /// alignment rule) partial byte, so the next bytes read from `data[pos..]` are the restart
    /// marker itself.
    fn byte_align(&mut self) {
        self.acc = 0;
        self.nbits = 0;
        self.hit_marker = false;
    }

    /// Expects a restart marker (`0xFF` followed by a byte in `0xD0..=0xD7`) at the current
    /// (already byte-aligned) position, and advances past it. Does not validate the specific
    /// sequence number cycles 0..7 in order — some real encoders get this detail wrong, and the
    /// sequence number is redundant with just knowing "a restart happened here" for a
    /// single-scan, non-error-resilient decoder like this one.
    fn expect_restart_marker(&mut self) -> Result<(), ConvertError> {
        let marker = *self.data.get(self.pos).ok_or_else(malformed)?;
        let code = *self.data.get(self.pos + 1).ok_or_else(malformed)?;
        if marker != 0xff || !(0xd0..=0xd7).contains(&code) {
            return Err(malformed());
        }
        self.pos += 2;
        Ok(())
    }
}

struct BitWriter {
    out: Vec<u8>,
    acc: u32,
    nbits: u32,
}

impl BitWriter {
    fn new() -> Self {
        BitWriter {
            out: Vec::new(),
            acc: 0,
            nbits: 0,
        }
    }

    fn push_byte(&mut self, b: u8) {
        self.out.push(b);
        if b == 0xff {
            self.out.push(0x00);
        }
    }

    fn put_bits(&mut self, value: u32, len: u8) {
        if len == 0 {
            return;
        }
        let len = u32::from(len);
        self.acc = (self.acc << len) | (value & ((1u32 << len) - 1));
        self.nbits += len;
        while self.nbits >= 8 {
            let byte = ((self.acc >> (self.nbits - 8)) & 0xff) as u8;
            self.push_byte(byte);
            self.nbits -= 8;
        }
    }

    /// Flushes any partial trailing byte, padding with `1`-bits (the same convention
    /// [`BitReader::get_bits`] assumes on the way in), and returns the finished byte stream.
    fn finish(mut self) -> Vec<u8> {
        if self.nbits > 0 {
            let pad = 8 - self.nbits;
            let data = self.acc & ((1u32 << self.nbits) - 1);
            let byte = ((data << pad) | ((1u32 << pad) - 1)) as u8;
            self.push_byte(byte);
        }
        self.out
    }
}

/// A decode-side canonical Huffman table, built from a `DHT` segment's 16 code-length counts plus
/// its symbol values, using the classic `mincode`/`maxcode`/`valptr`-per-length structure (ITU-T
/// T.81 Annex F / the algorithm every baseline JPEG decoder implements this same way) rather than
/// a hash map — this is JPEG's hot loop (one lookup per coefficient, dozens of coefficients per
/// block, up to hundreds of thousands of blocks for a large image), so an O(code length) array
/// walk matters more here than it did for the small, one-shot Huffman decodes elsewhere in this
/// crate.
struct HuffTable {
    values: Vec<u8>,
    mincode: [i32; 17],
    maxcode: [i32; 17],
    valptr: [i32; 17],
}

fn build_huff_table(counts: &[u8; 16], values: Vec<u8>) -> Result<HuffTable, ConvertError> {
    let total: usize = counts.iter().map(|&c| c as usize).sum();
    if total != values.len() || total > 256 {
        return Err(malformed());
    }

    let mut huffsize = Vec::with_capacity(total);
    for (i, &count) in counts.iter().enumerate() {
        for _ in 0..count {
            huffsize.push((i + 1) as u8);
        }
    }

    let mut huffcode = vec![0u16; huffsize.len()];
    let mut code: u32 = 0;
    let mut k = 0;
    for len in 1..=16u8 {
        while k < huffsize.len() && huffsize[k] == len {
            if code > 0xffff {
                return Err(malformed()); // overlong canonical code — an invalid DHT
            }
            huffcode[k] = code as u16;
            code += 1;
            k += 1;
        }
        code <<= 1;
    }

    let mut mincode = [0i32; 17];
    let mut maxcode = [-1i32; 17];
    let mut valptr = [0i32; 17];
    let mut p = 0usize;
    for len in 1..=16usize {
        let count = counts[len - 1] as usize;
        if count > 0 {
            valptr[len] = p as i32;
            mincode[len] = i32::from(huffcode[p]);
            p += count;
            maxcode[len] = i32::from(huffcode[p - 1]);
        }
    }

    Ok(HuffTable {
        values,
        mincode,
        maxcode,
        valptr,
    })
}

fn decode_huff(table: &HuffTable, reader: &mut BitReader) -> Result<u8, ConvertError> {
    let mut code: i32 = 0;
    for len in 1..=16usize {
        code = (code << 1) | reader.get_bits(1)? as i32;
        if table.maxcode[len] != -1 && code <= table.maxcode[len] {
            let index = (table.valptr[len] + (code - table.mincode[len])) as usize;
            return table.values.get(index).copied().ok_or_else(malformed);
        }
    }
    Err(malformed())
}

/// An encode-side Huffman table: symbol value (`0..=255`) -> `(code, code length)`, built with
/// the same canonical assignment `build_huff_table` uses for decode, just indexed the other way.
struct EncodeHuff {
    codes: [Option<(u16, u8)>; 256],
}

fn build_encode_huff(counts: &[u8; 16], values: &[u8]) -> EncodeHuff {
    let mut huffsize = Vec::with_capacity(values.len());
    for (i, &count) in counts.iter().enumerate() {
        for _ in 0..count {
            huffsize.push((i + 1) as u8);
        }
    }
    let mut huffcode = vec![0u16; huffsize.len()];
    let mut code: u32 = 0;
    let mut k = 0;
    for len in 1..=16u8 {
        while k < huffsize.len() && huffsize[k] == len {
            huffcode[k] = code as u16;
            code += 1;
            k += 1;
        }
        code <<= 1;
    }

    let mut codes = [None; 256];
    for (i, &value) in values.iter().enumerate() {
        codes[value as usize] = Some((huffcode[i], huffsize[i]));
    }
    EncodeHuff { codes }
}

/// JPEG's `RECEIVE`-then-`EXTEND` step: turns the `size`-bit magnitude field following a Huffman
/// symbol into a signed coefficient value. `size == 0` always means `0` (used for a zero DC diff).
fn extend(bits: u32, size: u8) -> i32 {
    if size == 0 {
        return 0;
    }
    let half = 1i32 << (size - 1);
    let bits = bits as i32;
    if bits < half {
        bits - ((1 << size) - 1)
    } else {
        bits
    }
}

/// The inverse of [`extend`]: the `(size, bits)` a signed coefficient encodes as. `size` is the
/// coefficient's bit length (`0` only for `v == 0`).
fn category_and_bits(v: i32) -> (u8, u32) {
    if v == 0 {
        return (0, 0);
    }
    let mag = v.unsigned_abs();
    let size = (32 - mag.leading_zeros()) as u8;
    let bits = if v > 0 {
        mag
    } else {
        (v + (1i32 << size) - 1) as u32
    };
    (size, bits)
}

// ─── DCT ───────────────────────────────────────────────────────────────────
//
// A direct (not fast-algorithm) separable 8-point DCT-II/DCT-III, using a precomputed cosine
// table shared by both the forward (encode) and inverse (decode) transforms — they're transposes
// of the same underlying matrix (see `fdct_1d`/`idct_1d`'s doc comments), so one table serves
// both directions. This is `O(8)` per output sample per axis (two passes of `O(64)` each per
// 8x8 block) rather than the `O(64)` naive direct-2D-sum-per-sample approach, which would be
// prohibitively slow at this crate's `4096x4096` pixel ceiling (tens of thousands of blocks).

type CosTable = [[f32; 8]; 8];

/// `table[x][u] = cos((2x + 1) * u * pi / 16)`.
fn build_cos_table() -> CosTable {
    let mut table = [[0f32; 8]; 8];
    for (x, row) in table.iter_mut().enumerate() {
        for (u, cell) in row.iter_mut().enumerate() {
            *cell = ((2 * x + 1) as f32 * u as f32 * std::f32::consts::PI / 16.0).cos();
        }
    }
    table
}

const INV_SQRT_2: f32 = std::f32::consts::FRAC_1_SQRT_2;

fn c(u: usize) -> f32 {
    if u == 0 {
        INV_SQRT_2
    } else {
        1.0
    }
}

/// 1D inverse DCT: frequency-domain `input` (already `C(u)`-unweighted, i.e. raw coefficients) to
/// spatial-domain output. Used along both axes of [`idct_block`].
fn idct_1d(input: &[f32; 8], cos_table: &CosTable) -> [f32; 8] {
    let mut out = [0f32; 8];
    for (x, out_x) in out.iter_mut().enumerate() {
        let mut sum = 0f32;
        for (u, &value) in input.iter().enumerate() {
            sum += c(u) * value * cos_table[x][u];
        }
        *out_x = 0.5 * sum;
    }
    out
}

/// 1D forward DCT: spatial-domain `input` to frequency-domain output. The transpose of
/// [`idct_1d`] — same `cos_table[x][u]` values, `C(u)` applied to the *output* frequency bin
/// instead of the input. Used along both axes of [`fdct_block`].
fn fdct_1d(input: &[f32; 8], cos_table: &CosTable) -> [f32; 8] {
    let mut out = [0f32; 8];
    for (u, out_u) in out.iter_mut().enumerate() {
        let mut sum = 0f32;
        for (x, &value) in input.iter().enumerate() {
            sum += value * cos_table[x][u];
        }
        *out_u = 0.5 * c(u) * sum;
    }
    out
}

/// Full 2D IDCT of a dequantized, natural-order (`row * 8 + col`) coefficient block, level-shifted
/// back to `0..=255` spatial samples.
fn idct_block(natural: &[i32; 64], cos_table: &CosTable) -> [u8; 64] {
    let mut rows = [[0f32; 8]; 8];
    for (v, row_out) in rows.iter_mut().enumerate() {
        let mut row = [0f32; 8];
        for (u, cell) in row.iter_mut().enumerate() {
            *cell = natural[v * 8 + u] as f32;
        }
        *row_out = idct_1d(&row, cos_table);
    }

    let mut spatial = [0u8; 64];
    for x in 0..8 {
        let mut col = [0f32; 8];
        for (v, cell) in col.iter_mut().enumerate() {
            *cell = rows[v][x];
        }
        let out = idct_1d(&col, cos_table);
        for (y, &value) in out.iter().enumerate() {
            spatial[y * 8 + x] = (value + 128.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    spatial
}

/// Full 2D FDCT of a level-shifted (`-128..=127`) spatial block, producing natural-order
/// (`row * 8 + col`) frequency coefficients — not yet quantized or zigzag-reordered.
fn fdct_block(spatial: &[i32; 64], cos_table: &CosTable) -> [f32; 64] {
    let mut rows = [[0f32; 8]; 8];
    for (y, row_out) in rows.iter_mut().enumerate() {
        let mut row = [0f32; 8];
        for (x, cell) in row.iter_mut().enumerate() {
            *cell = spatial[y * 8 + x] as f32;
        }
        *row_out = fdct_1d(&row, cos_table);
    }

    let mut freq = [0f32; 64];
    for u in 0..8 {
        let mut col = [0f32; 8];
        for (y, cell) in col.iter_mut().enumerate() {
            *cell = rows[y][u];
        }
        let out = fdct_1d(&col, cos_table);
        for (v, &value) in out.iter().enumerate() {
            freq[v * 8 + u] = value;
        }
    }
    freq
}

// ─── Decode ────────────────────────────────────────────────────────────────

struct Component {
    id: u8,
    h: u8,
    v: u8,
    qt: u8,
}

struct SofInfo {
    width: usize,
    height: usize,
    components: Vec<Component>,
}

struct ScanComponent {
    comp_index: usize,
    dc_table: u8,
    ac_table: u8,
}

fn read_u16_be(input: &[u8], pos: usize) -> Option<u16> {
    let bytes = input.get(pos..pos + 2)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

/// Reads a generic marker segment's 2-byte big-endian length (which counts itself) and skips past
/// the segment — used for `APPn`/`COM`/any marker this decoder doesn't need the contents of.
fn skip_segment(input: &[u8], pos: &mut usize) -> Result<(), ConvertError> {
    let len = read_u16_be(input, *pos).ok_or_else(malformed)? as usize;
    if len < 2 {
        return Err(malformed());
    }
    let end = pos.checked_add(len).ok_or_else(malformed)?;
    if end > input.len() {
        return Err(malformed());
    }
    *pos = end;
    Ok(())
}

fn parse_dqt(
    input: &[u8],
    pos: &mut usize,
    tables: &mut [Option<[u16; 64]>; 4],
) -> Result<(), ConvertError> {
    let len = read_u16_be(input, *pos).ok_or_else(malformed)? as usize;
    if len < 2 {
        return Err(malformed());
    }
    let seg_end = pos.checked_add(len).ok_or_else(malformed)?;
    if seg_end > input.len() {
        return Err(malformed());
    }
    let mut p = *pos + 2;
    while p < seg_end {
        let pq_tq = *input.get(p).ok_or_else(malformed)?;
        p += 1;
        let precision = pq_tq >> 4;
        let id = (pq_tq & 0x0f) as usize;
        if id >= 4 {
            return Err(malformed());
        }
        let mut table = [0u16; 64];
        match precision {
            0 => {
                let bytes = input.get(p..p + 64).ok_or_else(malformed)?;
                for (dst, &src) in table.iter_mut().zip(bytes) {
                    *dst = src as u16;
                }
                p += 64;
            }
            1 => {
                for slot in table.iter_mut() {
                    *slot = read_u16_be(input, p).ok_or_else(malformed)?;
                    p += 2;
                }
            }
            _ => return Err(malformed()),
        }
        tables[id] = Some(table);
    }
    if p != seg_end {
        return Err(malformed());
    }
    *pos = seg_end;
    Ok(())
}

fn parse_dht(
    input: &[u8],
    pos: &mut usize,
    dc_tables: &mut [Option<HuffTable>; 4],
    ac_tables: &mut [Option<HuffTable>; 4],
) -> Result<(), ConvertError> {
    let len = read_u16_be(input, *pos).ok_or_else(malformed)? as usize;
    if len < 2 {
        return Err(malformed());
    }
    let seg_end = pos.checked_add(len).ok_or_else(malformed)?;
    if seg_end > input.len() {
        return Err(malformed());
    }
    let mut p = *pos + 2;
    while p < seg_end {
        let tc_th = *input.get(p).ok_or_else(malformed)?;
        p += 1;
        let class = tc_th >> 4;
        let id = (tc_th & 0x0f) as usize;
        if id >= 4 || class > 1 {
            return Err(malformed());
        }
        let counts_bytes = input.get(p..p + 16).ok_or_else(malformed)?;
        let mut counts = [0u8; 16];
        counts.copy_from_slice(counts_bytes);
        p += 16;
        let total: usize = counts.iter().map(|&x| x as usize).sum();
        let values = input.get(p..p + total).ok_or_else(malformed)?.to_vec();
        p += total;
        let table = build_huff_table(&counts, values)?;
        if class == 0 {
            dc_tables[id] = Some(table);
        } else {
            ac_tables[id] = Some(table);
        }
    }
    if p != seg_end {
        return Err(malformed());
    }
    *pos = seg_end;
    Ok(())
}

fn parse_sof(input: &[u8], pos: &mut usize) -> Result<SofInfo, ConvertError> {
    let len = read_u16_be(input, *pos).ok_or_else(malformed)? as usize;
    if len < 8 {
        return Err(malformed());
    }
    let seg_end = pos.checked_add(len).ok_or_else(malformed)?;
    if seg_end > input.len() {
        return Err(malformed());
    }
    let mut p = *pos + 2;
    let precision = *input.get(p).ok_or_else(malformed)?;
    p += 1;
    if precision != 8 {
        return Err(unsupported("jpeg-sample-precision"));
    }
    let height = read_u16_be(input, p).ok_or_else(malformed)? as usize;
    p += 2;
    let width = read_u16_be(input, p).ok_or_else(malformed)? as usize;
    p += 2;
    let nf = *input.get(p).ok_or_else(malformed)? as usize;
    p += 1;
    if nf != 1 && nf != 3 {
        return Err(unsupported("jpeg-component-count"));
    }
    if seg_end - p != nf * 3 {
        return Err(malformed());
    }
    let mut components = Vec::with_capacity(nf);
    for _ in 0..nf {
        let id = *input.get(p).ok_or_else(malformed)?;
        p += 1;
        let hv = *input.get(p).ok_or_else(malformed)?;
        p += 1;
        let h = hv >> 4;
        let v = hv & 0x0f;
        if h == 0 || h > 4 || v == 0 || v > 4 {
            return Err(malformed());
        }
        let qt = *input.get(p).ok_or_else(malformed)?;
        p += 1;
        if qt >= 4 {
            return Err(malformed());
        }
        components.push(Component { id, h, v, qt });
    }
    checked_pixel_count(width as u32, height as u32, Format::Jpeg)?;
    *pos = seg_end;
    Ok(SofInfo {
        width,
        height,
        components,
    })
}

fn parse_dri(input: &[u8], pos: &mut usize) -> Result<u16, ConvertError> {
    let len = read_u16_be(input, *pos).ok_or_else(malformed)? as usize;
    if len != 4 {
        return Err(malformed());
    }
    let seg_end = pos.checked_add(len).ok_or_else(malformed)?;
    if seg_end > input.len() {
        return Err(malformed());
    }
    let interval = read_u16_be(input, *pos + 2).ok_or_else(malformed)?;
    *pos = seg_end;
    Ok(interval)
}

fn parse_sos_header(
    input: &[u8],
    pos: &mut usize,
    sof: &SofInfo,
) -> Result<Vec<ScanComponent>, ConvertError> {
    let len = read_u16_be(input, *pos).ok_or_else(malformed)? as usize;
    if len < 6 {
        return Err(malformed());
    }
    let seg_end = pos.checked_add(len).ok_or_else(malformed)?;
    if seg_end > input.len() {
        return Err(malformed());
    }
    let mut p = *pos + 2;
    let ns = *input.get(p).ok_or_else(malformed)? as usize;
    p += 1;
    if ns != sof.components.len() {
        // Baseline JPEG's single scan always covers every component declared in SOF; a scan
        // covering a subset is a hallmark of progressive/multi-scan encoding this module doesn't
        // implement.
        return Err(unsupported("jpeg-progressive"));
    }
    let mut scan_components = Vec::with_capacity(ns);
    for _ in 0..ns {
        let cs = *input.get(p).ok_or_else(malformed)?;
        p += 1;
        let td_ta = *input.get(p).ok_or_else(malformed)?;
        p += 1;
        let comp_index = sof
            .components
            .iter()
            .position(|comp| comp.id == cs)
            .ok_or_else(malformed)?;
        scan_components.push(ScanComponent {
            comp_index,
            dc_table: td_ta >> 4,
            ac_table: td_ta & 0x0f,
        });
    }
    let ss = *input.get(p).ok_or_else(malformed)?;
    p += 1;
    let se = *input.get(p).ok_or_else(malformed)?;
    p += 1;
    let ah_al = *input.get(p).ok_or_else(malformed)?;
    if ss != 0 || se != 63 || ah_al != 0 {
        return Err(unsupported("jpeg-progressive"));
    }
    *pos = seg_end;
    Ok(scan_components)
}

fn decode_block(
    reader: &mut BitReader,
    dc_table: &HuffTable,
    ac_table: &HuffTable,
    dc_pred: &mut i32,
) -> Result<[i32; 64], ConvertError> {
    let mut coeffs = [0i32; 64];

    let dc_size = decode_huff(dc_table, reader)?;
    if dc_size > 11 {
        return Err(malformed());
    }
    let diff = extend(reader.get_bits(dc_size as u32)?, dc_size);
    *dc_pred += diff;
    coeffs[0] = *dc_pred;

    let mut k = 1usize;
    while k < 64 {
        let rs = decode_huff(ac_table, reader)?;
        let run = rs >> 4;
        let size = rs & 0x0f;
        if size == 0 {
            if run == 15 {
                k += 16; // ZRL: 16 zero coefficients, no value follows
                continue;
            }
            break; // EOB: rest of the block is zero
        }
        k += run as usize;
        if k >= 64 {
            return Err(malformed());
        }
        coeffs[k] = extend(reader.get_bits(size as u32)?, size);
        k += 1;
    }

    Ok(coeffs)
}

fn dequantize_and_idct(coeffs: &[i32; 64], qt: &[u16; 64], cos_table: &CosTable) -> [u8; 64] {
    let mut natural = [0i32; 64];
    for (i, &coeff) in coeffs.iter().enumerate() {
        natural[ZIGZAG[i]] = coeff * qt[i] as i32;
    }
    idct_block(&natural, cos_table)
}

/// Bilinear sample of a chroma plane at a fractional (sub-pixel) coordinate, clamped to the
/// plane's edges — the chroma-upsampling step in [`decode_scan`]'s color conversion.
fn sample_bilinear(plane: &[u8], plane_w: usize, plane_h: usize, fx: f32, fy: f32) -> f32 {
    let fx = fx.clamp(0.0, (plane_w - 1) as f32);
    let fy = fy.clamp(0.0, (plane_h - 1) as f32);
    let x0 = fx.floor() as usize;
    let y0 = fy.floor() as usize;
    let x1 = (x0 + 1).min(plane_w - 1);
    let y1 = (y0 + 1).min(plane_h - 1);
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let top = plane[y0 * plane_w + x0] as f32 * (1.0 - tx) + plane[y0 * plane_w + x1] as f32 * tx;
    let bottom =
        plane[y1 * plane_w + x0] as f32 * (1.0 - tx) + plane[y1 * plane_w + x1] as f32 * tx;
    top * (1.0 - ty) + bottom * ty
}

#[allow(clippy::too_many_arguments)]
fn decode_scan(
    input: &[u8],
    scan_start: usize,
    sof: &SofInfo,
    scan_components: &[ScanComponent],
    quant_tables: &[Option<[u16; 64]>; 4],
    dc_tables: &[Option<HuffTable>; 4],
    ac_tables: &[Option<HuffTable>; 4],
    restart_interval: u16,
    options: &ConvertOptions,
) -> Result<RawImage, ConvertError> {
    let h_max = sof.components.iter().map(|c| c.h).max().unwrap_or(1) as usize;
    let v_max = sof.components.iter().map(|c| c.v).max().unwrap_or(1) as usize;
    let mcus_per_row = sof.width.div_ceil(8 * h_max);
    let mcus_per_col = sof.height.div_ceil(8 * v_max);

    let plane_dims: Vec<(usize, usize)> = sof
        .components
        .iter()
        .map(|comp| {
            (
                mcus_per_row * 8 * comp.h as usize,
                mcus_per_col * 8 * comp.v as usize,
            )
        })
        .collect();
    let mut planes: Vec<Vec<u8>> = plane_dims.iter().map(|&(w, h)| vec![0u8; w * h]).collect();

    let mut reader = BitReader::new(input, scan_start);
    let mut dc_pred = vec![0i32; sof.components.len()];
    let cos_table = build_cos_table();
    let mut mcus_since_restart: u32 = 0;

    let total_mcus = mcus_per_row * mcus_per_col;
    for mcu_index in 0..total_mcus {
        if options.is_cancelled() {
            return Err(ConvertError::Cancelled);
        }
        let mcu_x = mcu_index % mcus_per_row;
        let mcu_y = mcu_index / mcus_per_row;

        for scan_c in scan_components {
            let comp = &sof.components[scan_c.comp_index];
            let dc_table = dc_tables[scan_c.dc_table as usize]
                .as_ref()
                .ok_or_else(malformed)?;
            let ac_table = ac_tables[scan_c.ac_table as usize]
                .as_ref()
                .ok_or_else(malformed)?;
            let qt = quant_tables[comp.qt as usize]
                .as_ref()
                .ok_or_else(malformed)?;
            let (plane_w, _) = plane_dims[scan_c.comp_index];

            for by in 0..comp.v as usize {
                for bx in 0..comp.h as usize {
                    let coeffs = decode_block(
                        &mut reader,
                        dc_table,
                        ac_table,
                        &mut dc_pred[scan_c.comp_index],
                    )?;
                    let spatial = dequantize_and_idct(&coeffs, qt, &cos_table);

                    let origin_x = (mcu_x * comp.h as usize + bx) * 8;
                    let origin_y = (mcu_y * comp.v as usize + by) * 8;
                    let plane = &mut planes[scan_c.comp_index];
                    for row in 0..8 {
                        let dst = (origin_y + row) * plane_w + origin_x;
                        plane[dst..dst + 8].copy_from_slice(&spatial[row * 8..row * 8 + 8]);
                    }
                }
            }
        }

        mcus_since_restart += 1;
        if restart_interval > 0
            && mcus_since_restart == restart_interval as u32
            && mcu_index + 1 < total_mcus
        {
            reader.byte_align();
            reader.expect_restart_marker()?;
            mcus_since_restart = 0;
            dc_pred.iter_mut().for_each(|d| *d = 0);
        }
    }

    options.report_progress(1.0);

    let rgba_len = checked_rgba_len(sof.width as u32, sof.height as u32, Format::Jpeg)?;
    let mut rgba = vec![0u8; rgba_len as usize];

    if sof.components.len() == 1 {
        let (plane_w, _) = plane_dims[0];
        let plane = &planes[0];
        for y in 0..sof.height {
            for x in 0..sof.width {
                let value = plane[y * plane_w + x];
                let o = (y * sof.width + x) * 4;
                rgba[o] = value;
                rgba[o + 1] = value;
                rgba[o + 2] = value;
                rgba[o + 3] = 255;
            }
        }
    } else {
        let (yw, _) = plane_dims[0];
        let (cbw, cbh) = plane_dims[1];
        let (crw, crh_dim) = plane_dims[2];
        let ch = sof.components[1].h as usize;
        let cv = sof.components[1].v as usize;
        let crh = sof.components[2].h as usize;
        let crv = sof.components[2].v as usize;
        for y in 0..sof.height {
            for x in 0..sof.width {
                let yv = planes[0][y * yw + x] as f32;
                // Chroma is subsampled relative to luma whenever `Hi`/`Vi` < `h_max`/`v_max` (the
                // common case — see module docs) — bilinear-interpolate rather than
                // nearest-neighbor duplicate, matching the "fancy upsampling" every mainstream
                // decoder (libjpeg included) actually does; nearest-neighbor is spec-legal but
                // visibly blockier on real photos, confirmed empirically against Pillow-decoded
                // real files during this module's development.
                let cb_fx = (x as f32 + 0.5) * ch as f32 / h_max as f32 - 0.5;
                let cb_fy = (y as f32 + 0.5) * cv as f32 / v_max as f32 - 0.5;
                let cr_fx = (x as f32 + 0.5) * crh as f32 / h_max as f32 - 0.5;
                let cr_fy = (y as f32 + 0.5) * crv as f32 / v_max as f32 - 0.5;
                let cb = sample_bilinear(&planes[1], cbw, cbh, cb_fx, cb_fy) - 128.0;
                let cr = sample_bilinear(&planes[2], crw, crh_dim, cr_fx, cr_fy) - 128.0;
                let r = yv + 1.402 * cr;
                let g = yv - 0.344_136 * cb - 0.714_136 * cr;
                let b = yv + 1.772 * cb;
                let o = (y * sof.width + x) * 4;
                rgba[o] = r.round().clamp(0.0, 255.0) as u8;
                rgba[o + 1] = g.round().clamp(0.0, 255.0) as u8;
                rgba[o + 2] = b.round().clamp(0.0, 255.0) as u8;
                rgba[o + 3] = 255;
            }
        }
    }

    RawImage::new(sof.width as u32, sof.height as u32, rgba, Format::Jpeg)
}

/// Decodes a baseline JPEG into a [`RawImage`]. Alpha is always `255` — JPEG has no alpha
/// channel.
pub fn decode(input: &[u8], options: &ConvertOptions) -> Result<RawImage, ConvertError> {
    if options.is_cancelled() {
        return Err(ConvertError::Cancelled);
    }
    if input.len() < 4 || input[0] != 0xff || input[1] != SOI {
        return Err(malformed());
    }

    let mut pos = 2usize;
    let mut quant_tables: [Option<[u16; 64]>; 4] = [None, None, None, None];
    let mut dc_tables: [Option<HuffTable>; 4] = [None, None, None, None];
    let mut ac_tables: [Option<HuffTable>; 4] = [None, None, None, None];
    let mut sof: Option<SofInfo> = None;
    let mut restart_interval: u16 = 0;

    loop {
        if *input.get(pos).ok_or_else(malformed)? != 0xff {
            return Err(malformed());
        }
        pos += 1;
        while *input.get(pos).ok_or_else(malformed)? == 0xff {
            pos += 1;
        }
        let marker = input[pos];
        pos += 1;

        if marker == EOI {
            return Err(malformed()); // reached the end with no scan at all
        }
        if marker == SOF0 || marker == SOF1 {
            sof = Some(parse_sof(input, &mut pos)?);
            continue;
        }
        if UNSUPPORTED_SOF_MARKERS.contains(&marker) {
            return Err(unsupported("jpeg-non-baseline"));
        }
        if marker == DAC {
            return Err(unsupported("jpeg-arithmetic-coding"));
        }
        if marker == DHT {
            parse_dht(input, &mut pos, &mut dc_tables, &mut ac_tables)?;
            continue;
        }
        if marker == DQT {
            parse_dqt(input, &mut pos, &mut quant_tables)?;
            continue;
        }
        if marker == DRI {
            restart_interval = parse_dri(input, &mut pos)?;
            continue;
        }
        if marker == SOS {
            let sof = sof.as_ref().ok_or_else(malformed)?;
            let scan_components = parse_sos_header(input, &mut pos, sof)?;
            return decode_scan(
                input,
                pos,
                sof,
                &scan_components,
                &quant_tables,
                &dc_tables,
                &ac_tables,
                restart_interval,
                options,
            );
        }
        skip_segment(input, &mut pos)?;
    }
}

// ─── Encode ────────────────────────────────────────────────────────────────

/// IJG-style quality-to-scale-factor mapping (ITU-T T.81 Annex K.1's example scaling algorithm,
/// the same one libjpeg uses): quality `50` reproduces the base table unscaled, `< 50` scales it
/// up (coarser, smaller file), `> 50` scales it down (finer, larger file), floored at `1`.
fn scale_quant_table(base: &[u16; 64], quality: u8) -> [u16; 64] {
    let q = i32::from(quality.clamp(MIN_QUALITY, MAX_QUALITY));
    let scale = if q < 50 { 5000 / q } else { 200 - q * 2 };
    let mut out = [0u16; 64];
    for (dst, &src) in out.iter_mut().zip(base) {
        let scaled = (i32::from(src) * scale + 50) / 100;
        *dst = scaled.clamp(1, 255) as u16;
    }
    out
}

fn natural_to_zigzag(natural: &[u16; 64]) -> [u16; 64] {
    let mut out = [0u16; 64];
    for (i, dst) in out.iter_mut().enumerate() {
        *dst = natural[ZIGZAG[i]];
    }
    out
}

/// The standard AC Huffman tables (see module docs) top out at category `10` (run `0xFA`'s low
/// nibble) — no code exists for a size-`11`+ AC coefficient. DC's table does cover size `11`.
/// Clamping here means only genuinely extreme, adversarial-contrast 8x8 blocks (far beyond normal
/// photographic content) lose a negligible amount of additional precision, in exchange for the
/// encoder never failing to find a Huffman code for a value it just produced.
const MAX_AC_CATEGORY: u8 = 10;
const MAX_DC_CATEGORY: u8 = 11;

fn clamp_to_category(v: i32, max_category: u8) -> i32 {
    let limit = (1i32 << max_category) - 1;
    v.clamp(-limit, limit)
}

fn write_marker_with_len(out: &mut Vec<u8>, marker: u8, payload_len: usize) {
    out.extend_from_slice(&[0xff, marker]);
    out.extend_from_slice(&((payload_len + 2) as u16).to_be_bytes());
}

fn write_app0(out: &mut Vec<u8>) {
    write_marker_with_len(out, 0xe0, 14);
    out.extend_from_slice(b"JFIF\0");
    out.extend_from_slice(&[0x01, 0x01]); // version 1.1
    out.push(0x00); // no density units, aspect ratio only
    out.extend_from_slice(&[0x00, 0x01]); // xdensity
    out.extend_from_slice(&[0x00, 0x01]); // ydensity
    out.extend_from_slice(&[0x00, 0x00]); // no embedded thumbnail
}

fn write_dqt(out: &mut Vec<u8>, id: u8, table_zigzag: &[u16; 64]) {
    write_marker_with_len(out, DQT, 1 + 64);
    out.push(id); // precision nibble 0 (8-bit) << 4 | id
    for &v in table_zigzag {
        out.push(v as u8);
    }
}

fn write_sof0(out: &mut Vec<u8>, width: u16, height: u16) {
    write_marker_with_len(out, SOF0, 1 + 2 + 2 + 1 + 3 * 3);
    out.push(8); // precision
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&width.to_be_bytes());
    out.push(3); // component count: Y, Cb, Cr
    out.extend_from_slice(&[1, 0x11, 0]); // Y:  id 1, H1V1, quant table 0
    out.extend_from_slice(&[2, 0x11, 1]); // Cb: id 2, H1V1, quant table 1
    out.extend_from_slice(&[3, 0x11, 1]); // Cr: id 3, H1V1, quant table 1
}

fn write_dht(out: &mut Vec<u8>, class: u8, id: u8, bits: &[u8; 16], values: &[u8]) {
    write_marker_with_len(out, DHT, 1 + 16 + values.len());
    out.push((class << 4) | id);
    out.extend_from_slice(bits);
    out.extend_from_slice(values);
}

fn write_sos(out: &mut Vec<u8>) {
    write_marker_with_len(out, SOS, 1 + 3 * 2 + 3);
    out.push(3);
    out.extend_from_slice(&[1, 0x00]); // Y:  DC table 0, AC table 0
    out.extend_from_slice(&[2, 0x11]); // Cb: DC table 1, AC table 1
    out.extend_from_slice(&[3, 0x11]); // Cr: DC table 1, AC table 1
    out.extend_from_slice(&[0, 63, 0]); // spectral selection / successive approx: full baseline scan
}

#[allow(clippy::too_many_arguments)]
fn encode_block(
    writer: &mut BitWriter,
    plane: &[u8],
    plane_w: usize,
    origin_x: usize,
    origin_y: usize,
    qt_zigzag: &[u16; 64],
    dc_huff: &EncodeHuff,
    ac_huff: &EncodeHuff,
    dc_pred: &mut i32,
    cos_table: &CosTable,
    max_ac_category: u8,
) -> Result<(), ConvertError> {
    let mut spatial = [0i32; 64];
    for row in 0..8 {
        let src = (origin_y + row) * plane_w + origin_x;
        for col in 0..8 {
            spatial[row * 8 + col] = plane[src + col] as i32 - 128;
        }
    }
    let freq = fdct_block(&spatial, cos_table);

    let mut zz = [0i32; 64];
    for (i, dst) in zz.iter_mut().enumerate() {
        let q = qt_zigzag[i] as f32;
        *dst = (freq[ZIGZAG[i]] / q).round() as i32;
    }

    let dc_value = clamp_to_category(zz[0], MAX_DC_CATEGORY);
    let diff = clamp_to_category(dc_value - *dc_pred, MAX_DC_CATEGORY);
    *dc_pred = dc_value;
    let (size, bits) = category_and_bits(diff);
    let (code, len) =
        dc_huff.codes[size as usize].ok_or_else(|| internal("jpeg-dc-huffman-gap"))?;
    writer.put_bits(code as u32, len);
    if size > 0 {
        writer.put_bits(bits, size);
    }

    let mut run: u32 = 0;
    for &raw in &zz[1..64] {
        let coeff = clamp_to_category(raw, max_ac_category);
        if coeff == 0 {
            run += 1;
            continue;
        }
        while run >= 16 {
            let (code, len) = ac_huff.codes[0xf0].ok_or_else(|| internal("jpeg-ac-huffman-gap"))?;
            writer.put_bits(code as u32, len);
            run -= 16;
        }
        let (size, bits) = category_and_bits(coeff);
        let rs = ((run as u8) << 4) | size;
        let (code, len) =
            ac_huff.codes[rs as usize].ok_or_else(|| internal("jpeg-ac-huffman-gap"))?;
        writer.put_bits(code as u32, len);
        writer.put_bits(bits, size);
        run = 0;
    }
    if run > 0 {
        let (code, len) = ac_huff.codes[0x00].ok_or_else(|| internal("jpeg-ac-huffman-gap"))?;
        writer.put_bits(code as u32, len);
    }

    Ok(())
}

/// Encodes a [`RawImage`] as baseline JPEG, 4:4:4 (no chroma subsampling — see module docs),
/// using [`ConvertOptions::jpeg_quality`] (default [`DEFAULT_QUALITY`]). Alpha is discarded — JPEG
/// has no alpha channel, the same lossy-to-this-format choice [`super::bmp`]'s encoder makes.
pub fn encode(image: &RawImage, options: &ConvertOptions) -> Result<Vec<u8>, ConvertError> {
    if options.is_cancelled() {
        return Err(ConvertError::Cancelled);
    }

    let quality = options
        .jpeg_quality
        .unwrap_or(DEFAULT_QUALITY)
        .clamp(MIN_QUALITY, MAX_QUALITY);
    let luma_qt = natural_to_zigzag(&scale_quant_table(&STD_LUMA_QT_NATURAL, quality));
    let chroma_qt = natural_to_zigzag(&scale_quant_table(&STD_CHROMA_QT_NATURAL, quality));

    let width = image.width as usize;
    let height = image.height as usize;
    let mcus_w = width.div_ceil(8);
    let mcus_h = height.div_ceil(8);
    let padded_w = mcus_w * 8;
    let padded_h = mcus_h * 8;

    let mut y_plane = vec![0u8; padded_w * padded_h];
    let mut cb_plane = vec![0u8; padded_w * padded_h];
    let mut cr_plane = vec![0u8; padded_w * padded_h];
    for py in 0..padded_h {
        let sy = py.min(height - 1);
        for px in 0..padded_w {
            let sx = px.min(width - 1);
            let o = (sy * width + sx) * 4;
            let r = image.pixels[o] as f32;
            let g = image.pixels[o + 1] as f32;
            let b = image.pixels[o + 2] as f32;
            let yv = 0.299 * r + 0.587 * g + 0.114 * b;
            let cb = -0.168_736 * r - 0.331_264 * g + 0.5 * b + 128.0;
            let cr = 0.5 * r - 0.418_688 * g - 0.081_312 * b + 128.0;
            let idx = py * padded_w + px;
            y_plane[idx] = yv.round().clamp(0.0, 255.0) as u8;
            cb_plane[idx] = cb.round().clamp(0.0, 255.0) as u8;
            cr_plane[idx] = cr.round().clamp(0.0, 255.0) as u8;
        }
    }

    let cos_table = build_cos_table();
    let dc_luma_huff = build_encode_huff(&STD_DC_LUMA_BITS, &STD_DC_LUMA_VALUES);
    let ac_luma_huff = build_encode_huff(&STD_AC_LUMA_BITS, &STD_AC_LUMA_VALUES);
    let dc_chroma_huff = build_encode_huff(&STD_DC_CHROMA_BITS, &STD_DC_CHROMA_VALUES);
    let ac_chroma_huff = build_encode_huff(&STD_AC_CHROMA_BITS, &STD_AC_CHROMA_VALUES);

    let planes: [&[u8]; 3] = [&y_plane, &cb_plane, &cr_plane];
    let qts = [&luma_qt, &chroma_qt, &chroma_qt];
    let dc_huffs = [&dc_luma_huff, &dc_chroma_huff, &dc_chroma_huff];
    let ac_huffs = [&ac_luma_huff, &ac_chroma_huff, &ac_chroma_huff];
    let mut dc_pred = [0i32; 3];

    let mut writer = BitWriter::new();
    for mcu_y in 0..mcus_h {
        if options.is_cancelled() {
            return Err(ConvertError::Cancelled);
        }
        for mcu_x in 0..mcus_w {
            for c in 0..3 {
                encode_block(
                    &mut writer,
                    planes[c],
                    padded_w,
                    mcu_x * 8,
                    mcu_y * 8,
                    qts[c],
                    dc_huffs[c],
                    ac_huffs[c],
                    &mut dc_pred[c],
                    &cos_table,
                    MAX_AC_CATEGORY,
                )?;
            }
        }
    }
    let entropy_data = writer.finish();

    let mut out = Vec::new();
    out.extend_from_slice(&[0xff, SOI]);
    write_app0(&mut out);
    write_dqt(&mut out, 0, &luma_qt);
    write_dqt(&mut out, 1, &chroma_qt);
    write_sof0(&mut out, width as u16, height as u16);
    write_dht(&mut out, 0, 0, &STD_DC_LUMA_BITS, &STD_DC_LUMA_VALUES);
    write_dht(&mut out, 1, 0, &STD_AC_LUMA_BITS, &STD_AC_LUMA_VALUES);
    write_dht(&mut out, 0, 1, &STD_DC_CHROMA_BITS, &STD_DC_CHROMA_VALUES);
    write_dht(&mut out, 1, 1, &STD_AC_CHROMA_BITS, &STD_AC_CHROMA_VALUES);
    write_sos(&mut out);
    out.extend_from_slice(&entropy_data);
    out.extend_from_slice(&[0xff, EOI]);

    options.report_progress(1.0);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real JPEG built by Pillow (`Image.save(..., quality=85)`, libjpeg under the hood) — an
    /// independent encoder, not this module's own. 8x8 (a single MCU), default 4:2:0 chroma
    /// subsampling. Ground truth is Pillow's own decode of this exact file (`Image.open` +
    /// `getdata`), not a synthetic idealization — JPEG is lossy (see module docs), so "matches
    /// what a real independent decoder produces from these exact bytes" is the correctness bar,
    /// not "matches the pixels originally fed into the encoder". Chroma here is flat (the source
    /// was grayscale, R=G=B everywhere), which is what makes an exact-match assertion meaningful
    /// despite this decoder's chroma upsampling algorithm differing from libjpeg's (see
    /// [`sample_bilinear`]'s call site) — a flat plane upsamples identically under any filter.
    const CHECKER8_JPG: &[u8] = &[
        0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00,
        0x01, 0x00, 0x01, 0x00, 0x00, 0xff, 0xdb, 0x00, 0x43, 0x00, 0x05, 0x03, 0x04, 0x04, 0x04,
        0x03, 0x05, 0x04, 0x04, 0x04, 0x05, 0x05, 0x05, 0x06, 0x07, 0x0c, 0x08, 0x07, 0x07, 0x07,
        0x07, 0x0f, 0x0b, 0x0b, 0x09, 0x0c, 0x11, 0x0f, 0x12, 0x12, 0x11, 0x0f, 0x11, 0x11, 0x13,
        0x16, 0x1c, 0x17, 0x13, 0x14, 0x1a, 0x15, 0x11, 0x11, 0x18, 0x21, 0x18, 0x1a, 0x1d, 0x1d,
        0x1f, 0x1f, 0x1f, 0x13, 0x17, 0x22, 0x24, 0x22, 0x1e, 0x24, 0x1c, 0x1e, 0x1f, 0x1e, 0xff,
        0xdb, 0x00, 0x43, 0x01, 0x05, 0x05, 0x05, 0x07, 0x06, 0x07, 0x0e, 0x08, 0x08, 0x0e, 0x1e,
        0x14, 0x11, 0x14, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e,
        0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e,
        0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e,
        0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x08,
        0x00, 0x08, 0x03, 0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01, 0xff, 0xc4, 0x00,
        0x1f, 0x00, 0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b,
        0xff, 0xc4, 0x00, 0xb5, 0x10, 0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03, 0x05, 0x05,
        0x04, 0x04, 0x00, 0x00, 0x01, 0x7d, 0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21,
        0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08,
        0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a,
        0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35, 0x36, 0x37,
        0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56,
        0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75,
        0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93,
        0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9,
        0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6,
        0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
        0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7,
        0xf8, 0xf9, 0xfa, 0xff, 0xc4, 0x00, 0x1f, 0x01, 0x00, 0x03, 0x01, 0x01, 0x01, 0x01, 0x01,
        0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
        0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0xff, 0xc4, 0x00, 0xb5, 0x11, 0x00, 0x02, 0x01, 0x02,
        0x04, 0x04, 0x03, 0x04, 0x07, 0x05, 0x04, 0x04, 0x00, 0x01, 0x02, 0x77, 0x00, 0x01, 0x02,
        0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71, 0x13, 0x22,
        0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0, 0x15,
        0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
        0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47,
        0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66,
        0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84,
        0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a,
        0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7,
        0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4,
        0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea,
        0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xff, 0xda, 0x00, 0x0c, 0x03, 0x01,
        0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3f, 0x00, 0x4f, 0xf8, 0xf8, 0xff, 0x00, 0xa7, 0x8f,
        0x3f, 0xfe, 0xda, 0xf9, 0xbb, 0xff, 0x00, 0xef, 0xf7, 0x99, 0xbf, 0xed, 0x5f, 0xf4, 0xdf,
        0x7f, 0xda, 0xbf, 0xe5, 0xeb, 0xed, 0x7f, 0xf1, 0x3c, 0x28, 0xa2, 0x80, 0x3f, 0xff, 0xd9,
    ];
    const CHECKER8_RGBA: &[u8] = &[
        255, 255, 255, 255, 0, 0, 0, 255, 254, 254, 254, 255, 0, 0, 0, 255, 255, 255, 255, 255, 1,
        1, 1, 255, 255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0,
        255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255,
        255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255,
        255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255, 1,
        1, 1, 255, 247, 247, 247, 255, 8, 8, 8, 255, 254, 254, 254, 255, 0, 0, 0, 255, 255, 255,
        255, 255, 255, 255, 255, 255, 0, 0, 0, 255, 254, 254, 254, 255, 8, 8, 8, 255, 247, 247,
        247, 255, 1, 1, 1, 255, 255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255,
        0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255,
        255, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255,
        255, 255, 0, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255, 255,
        1, 1, 1, 255, 255, 255, 255, 255, 0, 0, 0, 255, 254, 254, 254, 255, 0, 0, 0, 255, 255, 255,
        255, 255,
    ];

    /// Same provenance as [`CHECKER8_JPG`], 12x10 (non-multiple-of-8 dimensions, forcing edge
    /// blocks) with a restart marker after every MCU (`restart_marker_blocks=1`) — the one real
    /// fixture here that exercises `DRI`/`expect_restart_marker` against an independent encoder's
    /// actual restart-marker placement, not just this module's own. A smooth RGB gradient, so
    /// real (non-flat) chroma detail — the bilinear-vs-fancy chroma upsampling difference (see
    /// module docs) means this one is checked with a small tolerance, not exact equality.
    const RESTART_JPG: &[u8] = &[
        0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x00, 0x00,
        0x01, 0x00, 0x01, 0x00, 0x00, 0xff, 0xdb, 0x00, 0x43, 0x00, 0x03, 0x02, 0x02, 0x03, 0x02,
        0x02, 0x03, 0x03, 0x03, 0x03, 0x04, 0x03, 0x03, 0x04, 0x05, 0x08, 0x05, 0x05, 0x04, 0x04,
        0x05, 0x0a, 0x07, 0x07, 0x06, 0x08, 0x0c, 0x0a, 0x0c, 0x0c, 0x0b, 0x0a, 0x0b, 0x0b, 0x0d,
        0x0e, 0x12, 0x10, 0x0d, 0x0e, 0x11, 0x0e, 0x0b, 0x0b, 0x10, 0x16, 0x10, 0x11, 0x13, 0x14,
        0x15, 0x15, 0x15, 0x0c, 0x0f, 0x17, 0x18, 0x16, 0x14, 0x18, 0x12, 0x14, 0x15, 0x14, 0xff,
        0xdb, 0x00, 0x43, 0x01, 0x03, 0x04, 0x04, 0x05, 0x04, 0x05, 0x09, 0x05, 0x05, 0x09, 0x14,
        0x0d, 0x0b, 0x0d, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14,
        0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14,
        0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14,
        0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0x14, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x0a,
        0x00, 0x0c, 0x03, 0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01, 0xff, 0xc4, 0x00,
        0x1f, 0x00, 0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b,
        0xff, 0xc4, 0x00, 0xb5, 0x10, 0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03, 0x05, 0x05,
        0x04, 0x04, 0x00, 0x00, 0x01, 0x7d, 0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21,
        0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08,
        0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a,
        0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35, 0x36, 0x37,
        0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56,
        0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75,
        0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93,
        0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9,
        0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6,
        0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
        0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7,
        0xf8, 0xf9, 0xfa, 0xff, 0xc4, 0x00, 0x1f, 0x01, 0x00, 0x03, 0x01, 0x01, 0x01, 0x01, 0x01,
        0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05,
        0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0xff, 0xc4, 0x00, 0xb5, 0x11, 0x00, 0x02, 0x01, 0x02,
        0x04, 0x04, 0x03, 0x04, 0x07, 0x05, 0x04, 0x04, 0x00, 0x01, 0x02, 0x77, 0x00, 0x01, 0x02,
        0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71, 0x13, 0x22,
        0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0, 0x15,
        0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
        0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47,
        0x48, 0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66,
        0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84,
        0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a,
        0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7,
        0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4,
        0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea,
        0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9, 0xfa, 0xff, 0xdd, 0x00, 0x04, 0x00, 0x01,
        0xff, 0xda, 0x00, 0x0c, 0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3f, 0x00, 0xf0,
        0x8f, 0x87, 0x3f, 0xb3, 0x5f, 0xfa, 0xaf, 0xf4, 0x5f, 0x4f, 0xe1, 0xaf, 0xa2, 0xf4, 0x3f,
        0xd9, 0xaf, 0xfe, 0x25, 0xd1, 0xff, 0x00, 0xa2, 0x7f, 0xe3, 0xb5, 0xeb, 0xff, 0x00, 0x0e,
        0x6d, 0x20, 0xfd, 0xd7, 0xee, 0x63, 0xed, 0xfc, 0x02, 0xbe, 0x89, 0xd1, 0x2d, 0x20, 0xfe,
        0xce, 0x8f, 0xf7, 0x31, 0xff, 0x00, 0xdf, 0x02, 0xa7, 0x2e, 0xc7, 0xd5, 0xe5, 0x39, 0xbc,
        0x37, 0xf1, 0x03, 0x35, 0xfa, 0xa6, 0xfd, 0x3b, 0x9f, 0xff, 0xd9,
    ];
    const RESTART_RGBA: &[u8] = &[
        0, 0, 0, 255, 14, 5, 10, 255, 35, 4, 19, 255, 54, 4, 29, 255, 79, 4, 43, 255, 100, 3, 54,
        255, 119, 3, 66, 255, 142, 4, 79, 255, 163, 3, 91, 255, 183, 3, 102, 255, 206, 4, 112, 255,
        218, 7, 120, 255, 7, 20, 13, 255, 21, 25, 24, 255, 42, 25, 33, 255, 62, 25, 43, 255, 86,
        25, 58, 255, 107, 24, 68, 255, 127, 24, 80, 255, 150, 25, 93, 255, 172, 25, 106, 255, 192,
        25, 117, 255, 215, 26, 128, 255, 226, 29, 135, 255, 7, 46, 27, 255, 20, 51, 36, 255, 42,
        51, 46, 255, 62, 51, 57, 255, 86, 51, 71, 255, 107, 50, 82, 255, 127, 50, 94, 255, 149, 50,
        105, 255, 172, 51, 120, 255, 192, 51, 130, 255, 215, 52, 141, 255, 226, 55, 149, 255, 5,
        70, 38, 255, 20, 76, 47, 255, 41, 75, 58, 255, 60, 75, 68, 255, 86, 75, 81, 255, 106, 75,
        93, 255, 127, 74, 104, 255, 148, 75, 118, 255, 170, 76, 129, 255, 190, 75, 140, 255, 213,
        77, 153, 255, 225, 80, 159, 255, 6, 98, 51, 255, 20, 103, 59, 255, 42, 103, 70, 255, 61,
        103, 81, 255, 87, 103, 93, 255, 107, 103, 104, 255, 128, 101, 116, 255, 148, 102, 130, 255,
        170, 102, 141, 255, 190, 102, 152, 255, 213, 103, 164, 255, 225, 106, 170, 255, 4, 122, 61,
        255, 20, 128, 70, 255, 40, 128, 80, 255, 61, 127, 89, 255, 85, 127, 103, 255, 106, 126,
        114, 255, 126, 126, 126, 255, 148, 128, 139, 255, 169, 127, 151, 255, 190, 126, 160, 255,
        211, 128, 174, 255, 223, 131, 180, 255, 6, 148, 72, 255, 19, 153, 80, 255, 40, 154, 92,
        255, 61, 154, 101, 255, 86, 154, 115, 255, 107, 153, 125, 255, 126, 153, 138, 255, 148,
        153, 149, 255, 170, 153, 163, 255, 190, 153, 171, 255, 211, 154, 186, 255, 224, 157, 192,
        255, 6, 173, 83, 255, 20, 178, 93, 255, 41, 179, 102, 255, 62, 178, 113, 255, 85, 179, 126,
        255, 106, 179, 136, 255, 127, 177, 148, 255, 147, 178, 162, 255, 172, 178, 174, 255, 191,
        178, 185, 255, 213, 180, 197, 255, 224, 182, 204, 255, 8, 199, 96, 255, 22, 203, 106, 255,
        42, 205, 116, 255, 63, 204, 125, 255, 88, 204, 139, 255, 109, 203, 150, 255, 128, 203, 162,
        255, 149, 204, 173, 255, 172, 203, 187, 255, 192, 203, 197, 255, 214, 205, 208, 255, 224,
        207, 215, 255, 16, 219, 111, 255, 30, 225, 120, 255, 51, 225, 130, 255, 71, 225, 139, 255,
        97, 224, 153, 255, 116, 224, 164, 255, 137, 223, 176, 255, 159, 225, 189, 255, 180, 224,
        201, 255, 200, 224, 211, 255, 223, 225, 222, 255, 234, 228, 230, 255,
    ];

    fn mean_abs_diff_u8(a: &[u8], b: &[u8]) -> f64 {
        assert_eq!(a.len(), b.len());
        let total: u64 = a
            .iter()
            .zip(b)
            .map(|(&x, &y)| (x as i32 - y as i32).unsigned_abs() as u64)
            .sum();
        total as f64 / a.len() as f64
    }

    #[test]
    fn decode_matches_an_independently_built_jpeg_exactly_when_chroma_is_flat() {
        let image = decode(CHECKER8_JPG, &ConvertOptions::default()).expect("decode");
        assert_eq!((image.width, image.height), (8, 8));
        assert_eq!(image.pixels, CHECKER8_RGBA);
    }

    #[test]
    fn decode_matches_an_independently_built_jpeg_with_restart_markers_closely() {
        let image = decode(RESTART_JPG, &ConvertOptions::default()).expect("decode");
        assert_eq!((image.width, image.height), (12, 10));
        // See RESTART_JPG's doc comment: real chroma detail here, so this decoder's bilinear
        // upsampling and libjpeg's "fancy" upsampling diverge slightly — tight enough to catch a
        // real bug (restart-marker desync would produce garbage, not a small diff), loose enough
        // not to fail on the upsampling algorithm choice itself.
        assert!(mean_abs_diff_u8(&image.pixels, RESTART_RGBA) < 2.0);
    }

    fn checkerboard(width: u32, height: u32) -> RawImage {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let on = (x + y) % 2 == 0;
                let v = if on { 255 } else { 0 };
                let o = ((y * width + x) * 4) as usize;
                pixels[o] = v;
                pixels[o + 1] = v;
                pixels[o + 2] = v;
                pixels[o + 3] = 255;
            }
        }
        RawImage::new(width, height, pixels, Format::Bmp).expect("valid checkerboard")
    }

    fn gradient(width: u32, height: u32) -> RawImage {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let o = ((y * width + x) * 4) as usize;
                pixels[o] = (x * 255 / width.max(1)) as u8;
                pixels[o + 1] = (y * 255 / height.max(1)) as u8;
                pixels[o + 2] = ((x + y) * 255 / (width + height).max(1)) as u8;
                pixels[o + 3] = 255;
            }
        }
        RawImage::new(width, height, pixels, Format::Bmp).expect("valid gradient")
    }

    fn mean_abs_diff(a: &RawImage, b: &RawImage) -> f64 {
        assert_eq!((a.width, a.height), (b.width, b.height));
        let mut total = 0u64;
        for (&x, &y) in a.pixels.iter().zip(b.pixels.iter()) {
            total += (x as i32 - y as i32).unsigned_abs() as u64;
        }
        total as f64 / a.pixels.len() as f64
    }

    #[test]
    fn round_trip_a_checkerboard_stays_visually_close() {
        let original = checkerboard(16, 16);
        let encoded = encode(&original, &ConvertOptions::default()).expect("encode");
        assert!(encoded.starts_with(&[0xff, 0xd8, 0xff]));
        let decoded = decode(&encoded, &ConvertOptions::default()).expect("decode");
        assert_eq!((decoded.width, decoded.height), (16, 16));
        // Lossy by design (see module docs) — a high-contrast checkerboard is close to the worst
        // case for DCT-based compression, so allow real slack, just prove it's not garbage.
        assert!(mean_abs_diff(&original, &decoded) < 60.0);
    }

    #[test]
    fn round_trip_a_gradient_is_nearly_exact() {
        // Smooth content compresses close to losslessly even at moderate quality — a much
        // tighter bound than the checkerboard case, catching gross transform/quantization bugs.
        let original = gradient(32, 24);
        let encoded = encode(&original, &ConvertOptions::default()).expect("encode");
        let decoded = decode(&encoded, &ConvertOptions::default()).expect("decode");
        assert!(mean_abs_diff(&original, &decoded) < 8.0);
    }

    #[test]
    fn round_trip_survives_non_multiple_of_8_dimensions() {
        let original = gradient(13, 21);
        let encoded = encode(&original, &ConvertOptions::default()).expect("encode");
        let decoded = decode(&encoded, &ConvertOptions::default()).expect("decode");
        assert_eq!((decoded.width, decoded.height), (13, 21));
        assert!(mean_abs_diff(&original, &decoded) < 10.0);
    }

    #[test]
    fn higher_quality_option_produces_a_closer_round_trip() {
        let original = checkerboard(16, 16);
        let low = {
            let opts = ConvertOptions {
                jpeg_quality: Some(10),
                ..ConvertOptions::default()
            };
            let encoded = encode(&original, &opts).expect("encode");
            decode(&encoded, &ConvertOptions::default()).expect("decode")
        };
        let high = {
            let opts = ConvertOptions {
                jpeg_quality: Some(95),
                ..ConvertOptions::default()
            };
            let encoded = encode(&original, &opts).expect("encode");
            decode(&encoded, &ConvertOptions::default()).expect("decode")
        };
        assert!(mean_abs_diff(&original, &high) < mean_abs_diff(&original, &low));
    }

    #[test]
    fn decode_rejects_bad_signature() {
        let input = b"NOTAJPEGFILE";
        let err = decode(input, &ConvertOptions::default()).unwrap_err();
        assert!(matches!(err, ConvertError::MalformedInput { .. }));
    }

    #[test]
    fn decode_rejects_truncated_input() {
        let original = checkerboard(8, 8);
        let mut encoded = encode(&original, &ConvertOptions::default()).expect("encode");
        encoded.truncate(20);
        let err = decode(&encoded, &ConvertOptions::default()).unwrap_err();
        assert!(matches!(err, ConvertError::MalformedInput { .. }));
    }

    #[test]
    fn decode_rejects_progressive_scan_marker() {
        let original = checkerboard(8, 8);
        let mut encoded = encode(&original, &ConvertOptions::default()).expect("encode");
        // Flip the SOF0 marker byte (0xC0) to SOF2 (0xC2), the progressive-DCT marker — the
        // second byte after the [0xFF, 0xD8, 0xFF, 0xE0, ...APP0...] header's own SOF0 marker.
        let sof_marker_pos = encoded
            .windows(2)
            .position(|w| w == [0xff, SOF0])
            .expect("encoded output has a SOF0 marker");
        encoded[sof_marker_pos + 1] = 0xc2;
        let err = decode(&encoded, &ConvertOptions::default()).unwrap_err();
        assert!(matches!(err, ConvertError::UnsupportedFeature { .. }));
    }

    #[test]
    fn category_and_bits_round_trips_through_extend() {
        for v in -2000..=2000 {
            let (size, bits) = category_and_bits(v);
            assert_eq!(extend(bits, size), v, "failed for v={v}");
        }
    }
}
