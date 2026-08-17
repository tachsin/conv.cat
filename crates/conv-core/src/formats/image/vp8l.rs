//! VP8L (WebP lossless) bitstream codec — bit reader/writer, canonical Huffman decode/encode,
//! backward-reference (LZ77) decoding with VP8L's own distance-mapping table, the color cache,
//! and the "meta prefix code" mechanism that lets different regions of an image use different
//! Huffman table sets. Pixel transforms (predictor, color, subtract-green, color-indexing) live
//! in [`super::webp`], since they're applied once around a fully-decoded pixel buffer rather than
//! being part of the bitstream's core symbol decode loop.
//!
//! ## Huffman codes here are packed differently than in [`super::zlib`]
//!
//! DEFLATE (RFC 1951) packs Huffman codes most-significant-bit-first — a deliberate exception to
//! its own general least-significant-bit-first field packing, which is why `zlib.rs`'s `decode`
//! builds a code by left-shifting and OR-ing in one new low bit per read. The VP8L spec states no
//! such exception ("bits of each byte are read in least-significant-bit-first order", with
//! nothing carved out for prefix codes), and libwebp's own `BuildHuffmanTable` confirms it: codes
//! are assigned canonically and then effectively bit-reversed (via an incremental "next reversed
//! key" trick, `GetNextKey`) so that decoding is a straight peek-N-bits-as-a-plain-integer table
//! lookup — no bit-by-bit accumulation needed. [`build_huffman`]/[`decode_symbol`] implement that
//! reversed-table approach.
//!
//! This module (bit order, the degenerate single-symbol 0-bit special case, meta-prefix-only-on-
//! the-ARGB-role rule, and the color-table pixel-bundling scheme) was validated by porting it
//! first to a small Python prototype and cross-checking pixel-exact decode output against
//! Pillow's (libwebp's) own decode of real files — covering the "simple" and "normal" code-length
//! encodings, the single-symbol table, meta-prefix groups, backward references, the color cache,
//! and (in `webp.rs`) all four pixel transforms — before writing a line of this file, because the
//! official bitstream spec deliberately omits these implementation details and getting any one of
//! them wrong would silently break every real-world file rather than fail one narrow test case.

use std::collections::{BTreeSet, HashMap};

use crate::ConvertOptions;

const MAXBITS: usize = 15;

/// How often the image-data decode/encode loops below poll cancellation and report progress —
/// same rationale as `qoi.rs`'s constant of the same name: frequent enough to stay responsive,
/// not so frequent that the cross-boundary `report_progress` call becomes the bottleneck.
pub(super) const PROGRESS_GRANULARITY: usize = 4096;

/// What stopped a VP8L decode partway through: either the input doesn't parse (maps to
/// [`crate::ConvertError::MalformedInput`] in `webp.rs`), or the caller cancelled (maps to
/// [`crate::ConvertError::Cancelled`]).
#[derive(Debug)]
pub(super) enum Stop {
    Malformed,
    Cancelled,
}

pub(super) type DResult<T> = Result<T, Stop>;

fn malformed<T>() -> DResult<T> {
    Err(Stop::Malformed)
}

fn required<T>(o: Option<T>) -> DResult<T> {
    o.ok_or(Stop::Malformed)
}

/// Bit reader over a byte slice, least-significant-bit-first within each byte — VP8L's uniform
/// bit-packing rule (see this module's docs for why that's different from `zlib.rs`'s DEFLATE
/// reader for Huffman codes specifically).
pub(super) struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    pub(super) fn new(data: &'a [u8]) -> Self {
        BitReader { data, pos: 0 }
    }

    fn total_bits(&self) -> usize {
        self.data.len() * 8
    }

    fn read_bit(&mut self) -> Option<u32> {
        if self.pos >= self.total_bits() {
            return None;
        }
        let byte = self.data[self.pos / 8];
        let bit = (byte >> (self.pos % 8)) & 1;
        self.pos += 1;
        Some(u32::from(bit))
    }

    pub(super) fn read_bits(&mut self, n: u32) -> Option<u32> {
        let mut v = 0u32;
        for i in 0..n {
            v |= self.read_bit()? << i;
        }
        Some(v)
    }

    /// Peeks up to `n` bits without consuming them, returning `(value, bits_actually_available)`
    /// — `bits_actually_available < n` only near the very end of the stream.
    fn peek_bits(&self, n: u32) -> (u32, u32) {
        let mut v = 0u32;
        let mut avail = 0u32;
        for i in 0..n {
            let p = self.pos + i as usize;
            if p >= self.total_bits() {
                break;
            }
            let byte = self.data[p / 8];
            let bit = (byte >> (p % 8)) & 1;
            v |= u32::from(bit) << i;
            avail += 1;
        }
        (v, avail)
    }

    fn consume(&mut self, n: u32) {
        self.pos += n as usize;
    }
}

/// A canonical Huffman decode table, LSB-first/table-lookup style (see module docs).
enum HuffmanTable {
    /// No symbol has a nonzero code length — this table is never actually consulted by a
    /// well-formed stream (e.g. the distance alphabet when an image uses no backward
    /// references at all); [`decode_symbol`] treats using it as malformed input.
    Empty,
    /// Exactly one symbol has a nonzero length — decoded with **zero bits consumed**,
    /// regardless of what length value the "simple code" bookkeeping recorded for it. A real
    /// case `libwebp`'s own encoder emits, not just a theoretical edge — see module docs.
    Single(u16),
    /// General case: `table[peeked max_len-bit value] = (symbol, real code length)`, with every
    /// code's entries replicated across all higher-bit completions, exactly the scheme
    /// `zlib.rs::construct` uses for DEFLATE, minus the bit-reversal DEFLATE doesn't need.
    Multi { table: Vec<(u16, u8)>, max_len: u32 },
}

fn reverse_bits(value: u32, n: u32) -> u32 {
    let mut r = 0u32;
    let mut x = value;
    for _ in 0..n {
        r = (r << 1) | (x & 1);
        x >>= 1;
    }
    r
}

/// Builds a canonical [`HuffmanTable`] from code lengths (index = symbol). Returns `None` only
/// if the lengths are over-subscribed — see `zlib.rs::construct`'s docs for why an
/// *under*-subscribed (incomplete) code is accepted rather than rejected up front.
fn build_huffman(code_lengths: &[u8]) -> Option<HuffmanTable> {
    let nonzero: Vec<usize> = code_lengths
        .iter()
        .enumerate()
        .filter(|&(_, &l)| l > 0)
        .map(|(i, _)| i)
        .collect();
    if nonzero.is_empty() {
        return Some(HuffmanTable::Empty);
    }
    if nonzero.len() == 1 {
        return Some(HuffmanTable::Single(nonzero[0] as u16));
    }

    let max_len = *code_lengths.iter().max().unwrap() as usize;
    if max_len > MAXBITS {
        return None;
    }

    let mut bl_count = vec![0u32; max_len + 1];
    for &l in code_lengths {
        if l > 0 {
            bl_count[l as usize] += 1;
        }
    }

    let mut left: i64 = 1;
    for &c in &bl_count[1..=max_len] {
        left <<= 1;
        left -= i64::from(c);
        if left < 0 {
            return None;
        }
    }

    let mut next_code = vec![0u32; max_len + 1];
    let mut code = 0u32;
    for len in 1..=max_len {
        code = (code + bl_count[len - 1]) << 1;
        next_code[len] = code;
    }

    let table_size = 1usize << max_len;
    let mut table = vec![(0u16, 0u8); table_size];
    for (symbol, &len) in code_lengths.iter().enumerate() {
        if len == 0 {
            continue;
        }
        let l = len as usize;
        let c = next_code[l];
        next_code[l] += 1;
        let rev = reverse_bits(c, l as u32) as usize;
        let step = 1usize << l;
        let mut high = 0usize;
        while high < table_size {
            table[high | rev] = (symbol as u16, len);
            high += step;
        }
    }
    Some(HuffmanTable::Multi {
        table,
        max_len: max_len as u32,
    })
}

fn decode_symbol(reader: &mut BitReader, table: &HuffmanTable) -> Option<u16> {
    match table {
        HuffmanTable::Empty => None,
        HuffmanTable::Single(s) => Some(*s),
        HuffmanTable::Multi { table, max_len } => {
            let (peek, avail) = reader.peek_bits(*max_len);
            if avail == *max_len {
                let (symbol, len) = table[peek as usize];
                if len == 0 {
                    return None;
                }
                reader.consume(u32::from(len));
                return Some(symbol);
            }
            // Near the very end of the stream: try progressively shorter matches among
            // whatever bits are actually left.
            for use_len in (1..=avail).rev() {
                let mask = (1u32 << use_len) - 1;
                let key = (peek & mask) as usize;
                let step = 1usize << use_len;
                let mut idx = key;
                while idx < table.len() {
                    let (symbol, len) = table[idx];
                    if u32::from(len) == use_len {
                        reader.consume(use_len);
                        return Some(symbol);
                    }
                    idx += step;
                }
            }
            None
        }
    }
}

// The order code-length-alphabet lengths are themselves transmitted in — deliberately not
// ascending, same rationale as DEFLATE's own `CLEN_ORDER` in `zlib.rs`.
const CODE_LENGTH_CODE_ORDER: [usize; 19] = [
    17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];

fn read_huffman_code_lengths(reader: &mut BitReader, alphabet_size: usize) -> DResult<Vec<u8>> {
    let simple = required(reader.read_bits(1))?;
    let mut code_lengths = vec![0u8; alphabet_size];
    if simple == 1 {
        let num_symbols = required(reader.read_bits(1))? + 1;
        let is_first_8bits = required(reader.read_bits(1))?;
        let symbol0 = required(reader.read_bits(1 + 7 * is_first_8bits))? as usize;
        if symbol0 >= alphabet_size {
            return malformed();
        }
        code_lengths[symbol0] = 1;
        if num_symbols == 2 {
            let symbol1 = required(reader.read_bits(8))? as usize;
            if symbol1 >= alphabet_size {
                return malformed();
            }
            code_lengths[symbol1] = 1;
        }
        return Ok(code_lengths);
    }

    let num_code_lengths = 4 + required(reader.read_bits(4))?;
    let mut cl_lengths = [0u8; 19];
    for &slot in CODE_LENGTH_CODE_ORDER
        .iter()
        .take(num_code_lengths as usize)
    {
        cl_lengths[slot] = required(reader.read_bits(3))? as u8;
    }
    let cl_table = required(build_huffman(&cl_lengths))?;

    let use_max_symbol = required(reader.read_bits(1))?;
    let mut max_symbol = if use_max_symbol == 1 {
        let length_nbits = 2 + 2 * required(reader.read_bits(3))?;
        2 + required(reader.read_bits(length_nbits))?
    } else {
        alphabet_size as u32
    };

    let mut symbol = 0usize;
    let mut prev_code_length = 8u8;
    while symbol < alphabet_size {
        if max_symbol == 0 {
            break;
        }
        max_symbol -= 1;
        let code_len_sym = required(decode_symbol(reader, &cl_table))?;
        match code_len_sym {
            0..=15 => {
                code_lengths[symbol] = code_len_sym as u8;
                if code_len_sym != 0 {
                    prev_code_length = code_len_sym as u8;
                }
                symbol += 1;
            }
            16 => {
                let repeat = 3 + required(reader.read_bits(2))?;
                for _ in 0..repeat {
                    if symbol >= alphabet_size {
                        break;
                    }
                    code_lengths[symbol] = prev_code_length;
                    symbol += 1;
                }
            }
            17 => {
                let repeat = 3 + required(reader.read_bits(3))?;
                symbol = symbol.saturating_add(repeat as usize);
            }
            18 => {
                let repeat = 11 + required(reader.read_bits(7))?;
                symbol = symbol.saturating_add(repeat as usize);
            }
            _ => return malformed(),
        }
    }
    Ok(code_lengths)
}

struct PrefixGroup {
    green: HuffmanTable,
    red: HuffmanTable,
    blue: HuffmanTable,
    alpha: HuffmanTable,
    distance: HuffmanTable,
}

fn read_prefix_code_group(reader: &mut BitReader, color_cache_size: usize) -> DResult<PrefixGroup> {
    let green_alphabet = 256 + 24 + color_cache_size;
    let green = required(build_huffman(&read_huffman_code_lengths(
        reader,
        green_alphabet,
    )?))?;
    let red = required(build_huffman(&read_huffman_code_lengths(reader, 256)?))?;
    let blue = required(build_huffman(&read_huffman_code_lengths(reader, 256)?))?;
    let alpha = required(build_huffman(&read_huffman_code_lengths(reader, 256)?))?;
    let distance = required(build_huffman(&read_huffman_code_lengths(reader, 40)?))?;
    Ok(PrefixGroup {
        green,
        red,
        blue,
        alpha,
        distance,
    })
}

/// The 120-entry backward-reference distance-mapping table (RFC-less, this is VP8L's own — see
/// the bitstream spec's "Distance Mapping" section): distance codes 1..=120 name a small 2D
/// neighborhood offset `(dx, dy)` rather than a raw scan-order pixel distance, so that "the pixel
/// just above" or "two pixels up and one left" are cheap to express regardless of image width.
const DIST_MAP: [(i32, i32); 120] = [
    (0, 1),
    (1, 0),
    (1, 1),
    (-1, 1),
    (0, 2),
    (2, 0),
    (1, 2),
    (-1, 2),
    (2, 1),
    (-2, 1),
    (2, 2),
    (-2, 2),
    (0, 3),
    (3, 0),
    (1, 3),
    (-1, 3),
    (3, 1),
    (-3, 1),
    (2, 3),
    (-2, 3),
    (3, 2),
    (-3, 2),
    (0, 4),
    (4, 0),
    (1, 4),
    (-1, 4),
    (4, 1),
    (-4, 1),
    (3, 3),
    (-3, 3),
    (2, 4),
    (-2, 4),
    (4, 2),
    (-4, 2),
    (0, 5),
    (3, 4),
    (-3, 4),
    (4, 3),
    (-4, 3),
    (5, 0),
    (1, 5),
    (-1, 5),
    (5, 1),
    (-5, 1),
    (2, 5),
    (-2, 5),
    (5, 2),
    (-5, 2),
    (4, 4),
    (-4, 4),
    (3, 5),
    (-3, 5),
    (5, 3),
    (-5, 3),
    (0, 6),
    (6, 0),
    (1, 6),
    (-1, 6),
    (6, 1),
    (-6, 1),
    (2, 6),
    (-2, 6),
    (6, 2),
    (-6, 2),
    (4, 5),
    (-4, 5),
    (5, 4),
    (-5, 4),
    (3, 6),
    (-3, 6),
    (6, 3),
    (-6, 3),
    (0, 7),
    (7, 0),
    (1, 7),
    (-1, 7),
    (5, 5),
    (-5, 5),
    (7, 1),
    (-7, 1),
    (4, 6),
    (-4, 6),
    (6, 4),
    (-6, 4),
    (2, 7),
    (-2, 7),
    (7, 2),
    (-7, 2),
    (3, 7),
    (-3, 7),
    (7, 3),
    (-7, 3),
    (5, 6),
    (-5, 6),
    (6, 5),
    (-6, 5),
    (8, 0),
    (4, 7),
    (-4, 7),
    (7, 4),
    (-7, 4),
    (8, 1),
    (8, 2),
    (6, 6),
    (-6, 6),
    (8, 3),
    (5, 7),
    (-5, 7),
    (7, 5),
    (-7, 5),
    (8, 4),
    (6, 7),
    (-6, 7),
    (7, 6),
    (-7, 6),
    (8, 5),
    (7, 7),
    (-7, 7),
    (8, 6),
    (8, 7),
];

/// Decodes a length/distance prefix code (0-23 for lengths, 0-39 for distances — both share this
/// one formula) into its actual value: codes 0..4 map directly to 1..4, and every pair of codes
/// after that doubles the range width, à la DEFLATE's own length/distance extra-bits scheme (see
/// `zlib.rs`'s `LENGTH_BASE`/`DIST_BASE`) but computed rather than tabulated, since VP8L's ranges
/// follow a single regular pattern instead of needing per-code tuning.
fn prefix_decode_value(reader: &mut BitReader, prefix_code: u32) -> DResult<u32> {
    if prefix_code < 4 {
        return Ok(prefix_code + 1);
    }
    let extra_bits = (prefix_code - 2) >> 1;
    let offset = (2 + (prefix_code & 1)) << extra_bits;
    Ok(offset + required(reader.read_bits(extra_bits))? + 1)
}

fn color_cache_index(argb: u32, code_bits: u32) -> usize {
    (argb.wrapping_mul(0x1e35_a7bd) >> (32 - code_bits)) as usize
}

/// Decodes the `color_cache_info` + `meta_prefix` + `prefix_codes` + `lz77-coded-image` section
/// for an image of the given `width`/`height` (the caller has already consumed any transform
/// header), per the bitstream spec's image-data section. Returns ARGB pixels, row-major.
///
/// `is_argb_role` gates the meta-prefix (entropy image) bit: it exists **only** for the
/// top-level pixel data, never for a transform's own sub-image, the color table, or the entropy
/// image's own data — confirmed against the spec's explicit text ("meta prefix codes may be used
/// only when the image is being used in the role of an ARGB image") after an earlier, wrong
/// assumption (reading this bit unconditionally) produced runaway nested "entropy image inside
/// an entropy image" parsing on every real file tested against Pillow.
pub(super) fn decode_image_stream(
    reader: &mut BitReader,
    width: usize,
    height: usize,
    is_argb_role: bool,
    options: &ConvertOptions,
    progress: &dyn Fn(f32),
) -> DResult<Vec<u32>> {
    let use_color_cache = required(reader.read_bits(1))?;
    let mut color_cache_size = 0usize;
    let mut cache_bits = 0u32;
    if use_color_cache == 1 {
        cache_bits = required(reader.read_bits(4))?;
        if cache_bits == 0 || cache_bits > 11 {
            return malformed();
        }
        color_cache_size = 1usize << cache_bits;
    }
    let mut cache = vec![0u32; color_cache_size];

    let use_entropy_image = if is_argb_role {
        required(reader.read_bits(1))?
    } else {
        0
    };
    let mut prefix_bits = 0u32;
    let mut entropy_width = 0usize;
    let mut entropy_image: Vec<u32> = Vec::new();
    if use_entropy_image == 1 {
        prefix_bits = 2 + required(reader.read_bits(3))?;
        entropy_width = width.div_ceil(1usize << prefix_bits);
        let entropy_height = height.div_ceil(1usize << prefix_bits);
        entropy_image = decode_image_stream(
            reader,
            entropy_width,
            entropy_height,
            false,
            options,
            &|_| {},
        )?;
    }

    let num_groups = if use_entropy_image == 1 {
        entropy_image
            .iter()
            .map(|&argb| (((argb >> 16) & 0xff) << 8) | ((argb >> 8) & 0xff))
            .max()
            .map(|m| m as usize + 1)
            .unwrap_or(1)
    } else {
        1
    };
    let mut groups = Vec::with_capacity(num_groups);
    for _ in 0..num_groups {
        groups.push(read_prefix_code_group(reader, color_cache_size)?);
    }

    let total_pixels = width * height;
    let mut pixels = Vec::with_capacity(total_pixels);
    let mut since_check = PROGRESS_GRANULARITY;

    while pixels.len() < total_pixels {
        if since_check >= PROGRESS_GRANULARITY {
            if options.is_cancelled() {
                return Err(Stop::Cancelled);
            }
            progress(pixels.len() as f32 / total_pixels.max(1) as f32);
            since_check = 0;
        }

        let group = if use_entropy_image == 1 {
            let x = pixels.len() % width;
            let y = pixels.len() / width;
            let meta = entropy_image[(y >> prefix_bits) * entropy_width + (x >> prefix_bits)];
            let idx = (((meta >> 16) & 0xff) << 8) | ((meta >> 8) & 0xff);
            groups.get(idx as usize).ok_or(Stop::Malformed)?
        } else {
            &groups[0]
        };

        let sym = required(decode_symbol(reader, &group.green))?;
        if sym < 256 {
            let green = sym;
            let red = required(decode_symbol(reader, &group.red))?;
            let blue = required(decode_symbol(reader, &group.blue))?;
            let alpha = required(decode_symbol(reader, &group.alpha))?;
            let argb = (u32::from(alpha) << 24)
                | (u32::from(red) << 16)
                | (u32::from(green) << 8)
                | u32::from(blue);
            pixels.push(argb);
            if color_cache_size > 0 {
                cache[color_cache_index(argb, cache_bits)] = argb;
            }
            since_check += 1;
        } else if sym < 256 + 24 {
            let length = prefix_decode_value(reader, u32::from(sym) - 256)?;
            let dist_sym = required(decode_symbol(reader, &group.distance))?;
            let dist_code = prefix_decode_value(reader, u32::from(dist_sym))?;
            let dist = if dist_code > 120 {
                (dist_code - 120) as usize
            } else {
                let (dx, dy) = DIST_MAP[(dist_code - 1) as usize];
                let d = dx + dy * width as i32;
                if d < 1 {
                    1
                } else {
                    d as usize
                }
            };
            if dist == 0 || dist > pixels.len() {
                return malformed();
            }
            if pixels.len() + length as usize > total_pixels {
                return malformed();
            }
            let start = pixels.len() - dist;
            for i in 0..length as usize {
                let argb = pixels[start + i];
                pixels.push(argb);
                if color_cache_size > 0 {
                    cache[color_cache_index(argb, cache_bits)] = argb;
                }
            }
            since_check += length as usize;
        } else {
            let cache_idx = (sym - 256 - 24) as usize;
            let argb = *cache.get(cache_idx).ok_or(Stop::Malformed)?;
            pixels.push(argb);
            cache[cache_idx] = argb;
            since_check += 1;
        }
    }

    Ok(pixels)
}

// ─── Encode: minimal but valid — literal-only, no transforms, no backward references, no color
// cache, a single Huffman group. See this crate's `png.rs`/`zlib.rs` precedent for the same
// "not size-optimal, just correct" trade: RFC 1951 gave DEFLATE a free "stored block" escape
// hatch for this; VP8L has none (every pixel must go through real prefix coding), so a minimal
// VP8L encoder still needs genuine canonical Huffman *construction*, not just parsing. ───

/// Least-significant-bit-first bit writer — the encode-side mirror of [`BitReader`].
pub(super) struct BitWriter {
    out: Vec<u8>,
    cur: u8,
    nbits: u32,
}

impl BitWriter {
    pub(super) fn new() -> Self {
        BitWriter {
            out: Vec::new(),
            cur: 0,
            nbits: 0,
        }
    }

    fn write_bit(&mut self, bit: u32) {
        self.cur |= ((bit & 1) as u8) << self.nbits;
        self.nbits += 1;
        if self.nbits == 8 {
            self.out.push(self.cur);
            self.cur = 0;
            self.nbits = 0;
        }
    }

    pub(super) fn write_bits(&mut self, value: u32, n: u32) {
        for i in 0..n {
            self.write_bit((value >> i) & 1);
        }
    }

    pub(super) fn finish(mut self) -> Vec<u8> {
        if self.nbits > 0 {
            self.out.push(self.cur);
        }
        self.out
    }
}

fn bits_for_count(n: usize) -> u32 {
    if n <= 1 {
        0
    } else {
        usize::BITS - (n - 1).leading_zeros()
    }
}

/// How to write occurrences of a symbol from one channel's alphabet — the encode-side mirror of
/// [`HuffmanTable`], with the same "single symbol needs zero bits" special case.
enum EncodeTable {
    Single,
    Multi(HashMap<u16, (u32, u8)>),
}

/// Assigns every symbol in `used` (sorted ascending — canonical order) the same code length, the
/// simplest scheme that is always a valid (if not optimally short) prefix code: for `n` used
/// symbols, `ceil(log2(n))` bits each stays under the Kraft-inequality budget (`n * 2^-len <= 1`)
/// with equality only when `n` is a power of two, and strictly under otherwise — never
/// over-subscribed. Returns the `alphabet_size`-length code_lengths array (for the bitstream
/// header) and the [`EncodeTable`] (for writing actual occurrences).
fn build_uniform_table(alphabet_size: usize, used: &BTreeSet<u16>) -> (Vec<u8>, EncodeTable) {
    let mut code_lengths = vec![0u8; alphabet_size];
    if used.len() <= 1 {
        if let Some(&s) = used.iter().next() {
            code_lengths[s as usize] = 1;
        }
        return (code_lengths, EncodeTable::Single);
    }

    let len = bits_for_count(used.len());
    let mut codes = HashMap::with_capacity(used.len());
    for (code, &s) in used.iter().enumerate() {
        code_lengths[s as usize] = len as u8;
        codes.insert(s, (reverse_bits(code as u32, len), len as u8));
    }
    (code_lengths, EncodeTable::Multi(codes))
}

fn write_symbol(writer: &mut BitWriter, table: &EncodeTable, symbol: u16) {
    match table {
        EncodeTable::Single => {}
        EncodeTable::Multi(codes) => {
            let &(code, len) = codes.get(&symbol).expect(
                "write_symbol is only ever called with a symbol this table's caller collected \
                 into `used` before building it",
            );
            writer.write_bits(code, u32::from(len));
        }
    }
}

/// Writes one channel's Huffman header via the "normal" code-length-code path (see
/// [`read_huffman_code_lengths`]) — always, even when the ["simple" path](read_huffman_code_lengths)
/// could represent it more compactly, to keep this encoder to one code path rather than two.
/// `code_lengths` here only ever has at most two distinct values in practice (`0` for unused
/// symbols, one uniform length for used ones — see [`build_uniform_table`]), so the code-length
/// alphabet this writes is itself tiny, but the logic below handles any number of distinct values
/// generally rather than assuming exactly two.
fn write_huffman_code_lengths(writer: &mut BitWriter, code_lengths: &[u8]) {
    writer.write_bits(0, 1); // simple = 0

    let mut distinct: BTreeSet<u8> = code_lengths.iter().copied().collect();
    distinct.insert(0);
    let n = distinct.len();
    let level2_len = if n <= 1 { 0 } else { bits_for_count(n) } as u8;

    let mut cl_lengths = [0u8; 19];
    for &v in &distinct {
        cl_lengths[v as usize] = level2_len.max(1);
    }
    writer.write_bits(19 - 4, 4); // num_code_lengths = 19: always transmit every slot
    for &sym in &CODE_LENGTH_CODE_ORDER {
        writer.write_bits(u32::from(cl_lengths[sym]), 3);
    }

    writer.write_bits(0, 1); // use_max_symbol = 0: header covers the full alphabet

    let mut level2_codes: HashMap<u8, (u32, u8)> = HashMap::with_capacity(n);
    if n <= 1 {
        if let Some(&v) = distinct.iter().next() {
            level2_codes.insert(v, (0, 0));
        }
    } else {
        for (code, &v) in distinct.iter().enumerate() {
            level2_codes.insert(
                v,
                (reverse_bits(code as u32, u32::from(level2_len)), level2_len),
            );
        }
    }
    for &len in code_lengths {
        let &(code, bits) = level2_codes.get(&len).expect(
            "every value in code_lengths was inserted into `distinct` above, so it always has \
             a level-2 code",
        );
        if bits > 0 {
            writer.write_bits(code, u32::from(bits));
        }
    }
}

/// Writes the `color_cache_info` + `meta_prefix` + `prefix_codes` + `lz77-coded-image` section
/// for `pixels` (row-major ARGB, `width * height` long) into `writer` — no transforms, no color
/// cache, no backward references, a single Huffman group — see this section's header comment for
/// why that's a deliberate, spec-valid minimal encoder rather than an oversight.
///
/// Takes an already-open [`BitWriter`] (continuing the caller's in-progress bitstream) rather
/// than returning its own finished byte vector: VP8L has no byte-alignment points before the end
/// of the whole stream, so finishing a `BitWriter` early (padding to a byte boundary) and later
/// concatenating byte vectors would silently insert padding bits in the middle of the real
/// bitstream — this must all be one continuous bit sequence.
pub(super) fn encode_image_stream(
    writer: &mut BitWriter,
    pixels: &[u32],
    options: &ConvertOptions,
    progress: &dyn Fn(f32),
) -> DResult<()> {
    let mut green_used = BTreeSet::new();
    let mut red_used = BTreeSet::new();
    let mut blue_used = BTreeSet::new();
    let mut alpha_used = BTreeSet::new();
    for &argb in pixels {
        green_used.insert(((argb >> 8) & 0xff) as u16);
        red_used.insert(((argb >> 16) & 0xff) as u16);
        blue_used.insert((argb & 0xff) as u16);
        alpha_used.insert(((argb >> 24) & 0xff) as u16);
    }

    // Green shares its alphabet with length/color-cache codes (256..280 here, all unused since
    // this encoder never emits either) — sized 280 to match what a decoder always expects for
    // color_cache_size == 0.
    let (green_lengths, green_table) = build_uniform_table(256 + 24, &green_used);
    let (red_lengths, red_table) = build_uniform_table(256, &red_used);
    let (blue_lengths, blue_table) = build_uniform_table(256, &blue_used);
    let (alpha_lengths, alpha_table) = build_uniform_table(256, &alpha_used);
    // Distance codes are never emitted (no backward references) — a single dummy symbol keeps
    // the alphabet non-empty (an all-unused table is valid too, but this is simpler to write).
    let mut distance_used = BTreeSet::new();
    distance_used.insert(0u16);
    let (distance_lengths, _unused_distance_table) = build_uniform_table(40, &distance_used);

    writer.write_bits(0, 1); // use_color_cache = 0
    writer.write_bits(0, 1); // use_entropy_image = 0 (ARGB role, single group)

    write_huffman_code_lengths(writer, &green_lengths);
    write_huffman_code_lengths(writer, &red_lengths);
    write_huffman_code_lengths(writer, &blue_lengths);
    write_huffman_code_lengths(writer, &alpha_lengths);
    write_huffman_code_lengths(writer, &distance_lengths);

    let total = pixels.len();
    let mut since_check = PROGRESS_GRANULARITY;
    for (i, &argb) in pixels.iter().enumerate() {
        if since_check >= PROGRESS_GRANULARITY {
            if options.is_cancelled() {
                return Err(Stop::Cancelled);
            }
            progress(i as f32 / total.max(1) as f32);
            since_check = 0;
        }
        let alpha = ((argb >> 24) & 0xff) as u16;
        let red = ((argb >> 16) & 0xff) as u16;
        let green = ((argb >> 8) & 0xff) as u16;
        let blue = (argb & 0xff) as u16;

        write_symbol(writer, &green_table, green);
        write_symbol(writer, &red_table, red);
        write_symbol(writer, &blue_table, blue);
        write_symbol(writer, &alpha_table, alpha);
        since_check += 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huffman_single_symbol_round_trips_with_zero_bits() {
        let mut lengths = vec![0u8; 10];
        lengths[3] = 1;
        let table = build_huffman(&lengths).unwrap();
        let mut reader = BitReader::new(&[]);
        // No bits available at all -- must still succeed, since a Single table consumes none.
        assert_eq!(decode_symbol(&mut reader, &table), Some(3));
    }

    #[test]
    fn huffman_empty_table_never_decodes() {
        let lengths = vec![0u8; 10];
        let table = build_huffman(&lengths).unwrap();
        let mut reader = BitReader::new(&[0xff]);
        assert!(decode_symbol(&mut reader, &table).is_none());
    }

    #[test]
    fn huffman_rejects_an_over_subscribed_code() {
        // Three symbols all claiming the single available 1-bit code.
        assert!(build_huffman(&[1, 1, 1]).is_none());
    }

    #[test]
    fn bit_writer_round_trips_through_bit_reader() {
        let mut writer = BitWriter::new();
        writer.write_bits(0b101, 3);
        writer.write_bits(0b1, 1);
        writer.write_bits(0b11001, 5);
        let bytes = writer.finish();

        let mut reader = BitReader::new(&bytes);
        assert_eq!(reader.read_bits(3), Some(0b101));
        assert_eq!(reader.read_bits(1), Some(0b1));
        assert_eq!(reader.read_bits(5), Some(0b11001));
    }

    #[test]
    fn prefix_decode_value_matches_known_ranges() {
        assert_eq!(prefix_decode_value(&mut BitReader::new(&[]), 0).unwrap(), 1);
        assert_eq!(prefix_decode_value(&mut BitReader::new(&[]), 3).unwrap(), 4);
        // code 4: extra_bits=1, offset=(2+0)<<1=4, +extra(0..=1)+1 -> range 5..=6
        assert_eq!(
            prefix_decode_value(&mut BitReader::new(&[0b0]), 4).unwrap(),
            5
        );
        assert_eq!(
            prefix_decode_value(&mut BitReader::new(&[0b1]), 4).unwrap(),
            6
        );
    }

    #[test]
    fn encode_then_decode_round_trips_a_small_image() {
        let width = 4usize;
        let height = 3usize;
        let mut pixels = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                let r = (x * 37 + y * 11) as u32 % 256;
                let g = (x * 7 + y * 53) as u32 % 256;
                let b = (x * x + y) as u32 % 256;
                let a = (200 + x * 3 + y * 5) as u32 % 256;
                pixels.push((a << 24) | (r << 16) | (g << 8) | b);
            }
        }

        let options = ConvertOptions::default();
        let mut writer = BitWriter::new();
        encode_image_stream(&mut writer, &pixels, &options, &|_| {})
            .unwrap_or_else(|_| panic!("encoding a valid pixel buffer should never fail"));
        let encoded = writer.finish();

        let mut reader = BitReader::new(&encoded);
        let decoded = decode_image_stream(&mut reader, width, height, true, &options, &|_| {})
            .unwrap_or_else(|_| panic!("decoding this encoder's own output should never fail"));

        assert_eq!(decoded, pixels);
    }
}
