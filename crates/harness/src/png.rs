//! A self-contained PNG encoder and decoder (no external crates).
//!
//! Written from the public specifications: PNG (ISO/IEC 15948, W3C PNG Third Edition), zlib (RFC 1950) and DEFLATE
//! (RFC 1951).
//!
//! - **Encoder:** writes 8-bit RGBA, or 8-bit RGB when every pixel is opaque. Each row gets the adaptive filter with
//!   the smallest sum of absolute differences; the data is compressed with LZ77 and the fixed Huffman code.
//! - **Decoder:** reads every standard PNG: greyscale, RGB, palette, greyscale+alpha and RGBA, bit depths 1 to 16,
//!   Adam7 interlacing, and `tRNS` transparency. Output is always 8-bit RGBA (16-bit samples keep their high byte).
//!   Gamma and colour-space chunks are ignored. Every chunk CRC and the zlib Adler-32 are checked.

use std::fmt;
use std::path::Path;

use crate::image::Image;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];

/// Largest width or height accepted by the decoder, to keep a corrupt header from requesting gigabytes.
pub const MAX_DIMENSION: u32 = 1 << 14;

/// A PNG read or write error.
#[derive(Debug)]
pub enum PngError {
    Io(std::io::Error),
    Format(String),
}

impl fmt::Display for PngError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PngError::Io(e) => write!(f, "{e}"),
            PngError::Format(m) => write!(f, "invalid PNG: {m}"),
        }
    }
}

impl std::error::Error for PngError {}

impl From<std::io::Error> for PngError {
    fn from(e: std::io::Error) -> Self {
        PngError::Io(e)
    }
}

fn bad<T>(m: impl Into<String>) -> Result<T, PngError> {
    Err(PngError::Format(m.into()))
}

// ---------------------------------------------------------------------------------------------------------------
// Checksums

fn crc_table() -> &'static [u32; 256] {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (n, slot) in t.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *slot = c;
        }
        t
    })
}

/// CRC-32 (ISO 3309 / ITU-T V.42), as used by PNG chunks.
pub fn crc32(parts: &[&[u8]]) -> u32 {
    let t = crc_table();
    let mut c = 0xFFFF_FFFFu32;
    for part in parts {
        for &b in *part {
            c = t[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
        }
    }
    c ^ 0xFFFF_FFFF
}

/// Adler-32, as used by zlib.
pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

// ---------------------------------------------------------------------------------------------------------------
// DEFLATE tables (RFC 1951 section 3.2.5)

const LEN_BASE: [u16; 29] =
    [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145,
    8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] =
    [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
/// Order of the code-length code lengths in a dynamic block header.
const CLEN_ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

fn fixed_lit_lengths() -> [u8; 288] {
    let mut l = [0u8; 288];
    l[..144].fill(8);
    l[144..256].fill(9);
    l[256..280].fill(7);
    l[280..].fill(8);
    l
}

// ---------------------------------------------------------------------------------------------------------------
// Compression

struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl BitWriter {
    fn put(&mut self, bits: u32, count: u32) {
        self.acc |= (bits as u64) << self.n;
        self.n += count;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }

    /// Writes a Huffman code, which DEFLATE stores most-significant bit first.
    fn put_code(&mut self, code: u32, len: u32) {
        self.put(code.reverse_bits() >> (32 - len), len);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

/// Canonical Huffman codes from code lengths (RFC 1951 section 3.2.2).
fn canonical_codes(lengths: &[u8]) -> Vec<u32> {
    let mut bl_count = [0u32; 16];
    for &l in lengths {
        bl_count[l as usize] += 1;
    }
    bl_count[0] = 0;
    let mut next = [0u32; 16];
    let mut code = 0;
    for bits in 1..16 {
        code = (code + bl_count[bits - 1]) << 1;
        next[bits] = code;
    }
    lengths
        .iter()
        .map(|&l| {
            if l == 0 {
                0
            } else {
                let c = next[l as usize];
                next[l as usize] += 1;
                c
            }
        })
        .collect()
}

fn length_symbol(len: usize) -> usize {
    LEN_BASE.iter().rposition(|&b| b as usize <= len).unwrap()
}

fn dist_symbol(dist: usize) -> usize {
    DIST_BASE.iter().rposition(|&b| b as usize <= dist).unwrap()
}

/// Compresses `data` as a zlib stream (LZ77 with hash chains, fixed Huffman code, one block).
pub fn zlib_compress(data: &[u8]) -> Vec<u8> {
    const WINDOW: usize = 32768;
    const HASH_BITS: u32 = 15;
    const MAX_CHAIN: usize = 48;
    const MAX_MATCH: usize = 258;

    let lit_lens = fixed_lit_lengths();
    let lit_codes = canonical_codes(&lit_lens);
    let mut w = BitWriter { out: vec![0x78, 0x01], acc: 0, n: 0 };
    w.put(1, 1); // final block
    w.put(1, 2); // fixed Huffman

    let hash = |i: usize| -> usize {
        let v = (data[i] as u32) | (data[i + 1] as u32) << 8 | (data[i + 2] as u32) << 16;
        (v.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
    };
    let mut head = vec![usize::MAX; 1 << HASH_BITS];
    let mut prev = vec![usize::MAX; WINDOW];
    let insert = |i: usize, head: &mut [usize], prev: &mut [usize]| {
        if i + 2 < data.len() {
            let h = hash(i);
            prev[i % WINDOW] = head[h];
            head[h] = i;
        }
    };

    let mut i = 0;
    while i < data.len() {
        let mut best_len = 0;
        let mut best_dist = 0;
        if i + 2 < data.len() {
            let mut cand = head[hash(i)];
            let max_len = MAX_MATCH.min(data.len() - i);
            let mut chain = 0;
            while cand != usize::MAX && i - cand <= WINDOW && chain < MAX_CHAIN {
                let mut l = 0;
                while l < max_len && data[cand + l] == data[i + l] {
                    l += 1;
                }
                if l > best_len {
                    best_len = l;
                    best_dist = i - cand;
                    if l == max_len {
                        break;
                    }
                }
                let p = prev[cand % WINDOW];
                if p == usize::MAX || p >= cand {
                    break;
                }
                cand = p;
                chain += 1;
            }
        }
        if best_len >= 3 {
            let ls = length_symbol(best_len);
            let sym = 257 + ls;
            w.put_code(lit_codes[sym], lit_lens[sym] as u32);
            w.put((best_len - LEN_BASE[ls] as usize) as u32, LEN_EXTRA[ls] as u32);
            let ds = dist_symbol(best_dist);
            w.put_code(ds as u32, 5);
            w.put((best_dist - DIST_BASE[ds] as usize) as u32, DIST_EXTRA[ds] as u32);
            for k in i..i + best_len {
                insert(k, &mut head, &mut prev);
            }
            i += best_len;
        } else {
            let sym = data[i] as usize;
            w.put_code(lit_codes[sym], lit_lens[sym] as u32);
            insert(i, &mut head, &mut prev);
            i += 1;
        }
    }
    w.put_code(lit_codes[256], lit_lens[256] as u32);
    let mut out = w.finish();
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Decompression

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    acc: u64,
    n: u32,
    /// Zero bytes fed past the end, so a peek near the end works; consuming them is an error.
    overrun: u32,
}

impl BitReader<'_> {
    fn refill(&mut self) {
        while self.n <= 56 {
            let b = match self.data.get(self.pos) {
                Some(&b) => b,
                None => {
                    self.overrun += 8;
                    0
                }
            };
            self.pos += 1;
            self.acc |= (b as u64) << self.n;
            self.n += 8;
        }
    }

    fn bits(&mut self, count: u32) -> Result<u32, PngError> {
        if count == 0 {
            return Ok(0);
        }
        if self.n < count {
            self.refill();
        }
        let v = (self.acc & ((1u64 << count) - 1)) as u32;
        self.consume(count)?;
        Ok(v)
    }

    fn consume(&mut self, count: u32) -> Result<(), PngError> {
        self.acc >>= count;
        self.n -= count;
        if self.overrun > self.n {
            return bad("compressed data ends early");
        }
        Ok(())
    }

    fn align_to_byte(&mut self) {
        let drop = self.n % 8;
        self.acc >>= drop;
        self.n -= drop;
    }

    /// Byte position of the next unread whole byte.
    fn byte_pos(&self) -> usize {
        self.pos - (self.n / 8) as usize
    }
}

/// A Huffman decoding table indexed by the next `max_len` input bits (least-significant first).
struct Huffman {
    table: Vec<u16>,
    max_len: u32,
}

impl Huffman {
    fn new(lengths: &[u8]) -> Result<Huffman, PngError> {
        let max_len = lengths.iter().copied().max().unwrap_or(0) as u32;
        if max_len == 0 {
            // An empty code is legal (a block with no distance codes); any use of it is an error.
            return Ok(Huffman { table: vec![0; 2], max_len: 1 });
        }
        // Reject over-subscribed codes.
        let mut left: i32 = 1;
        for len in 1..=15 {
            left = (left << 1) - lengths.iter().filter(|&&l| l as u32 == len).count() as i32;
            if left < 0 {
                return bad("over-subscribed Huffman code");
            }
        }
        let codes = canonical_codes(lengths);
        let mut table = vec![0u16; 1 << max_len];
        for (sym, (&len, &code)) in lengths.iter().zip(&codes).enumerate() {
            if len == 0 {
                continue;
            }
            let rev = code.reverse_bits() >> (32 - len as u32);
            let entry = ((sym as u16) << 4) | len as u16;
            let mut idx = rev as usize;
            while idx < table.len() {
                table[idx] = entry;
                idx += 1 << len;
            }
        }
        Ok(Huffman { table, max_len })
    }

    fn decode(&self, r: &mut BitReader<'_>) -> Result<usize, PngError> {
        if r.n < self.max_len {
            r.refill();
        }
        let entry = self.table[(r.acc & ((1u64 << self.max_len) - 1)) as usize];
        let len = (entry & 0xF) as u32;
        if len == 0 {
            return bad("invalid Huffman code");
        }
        r.consume(len)?;
        Ok((entry >> 4) as usize)
    }
}

fn inflate_block(
    r: &mut BitReader<'_>,
    out: &mut Vec<u8>,
    lit: &Huffman,
    dist: &Huffman,
    limit: usize,
) -> Result<(), PngError> {
    loop {
        let sym = lit.decode(r)?;
        match sym {
            0..=255 => out.push(sym as u8),
            256 => return Ok(()),
            257..=285 => {
                let li = sym - 257;
                let len = LEN_BASE[li] as usize + r.bits(LEN_EXTRA[li] as u32)? as usize;
                let ds = dist.decode(r)?;
                if ds >= 30 {
                    return bad("invalid distance code");
                }
                let d = DIST_BASE[ds] as usize + r.bits(DIST_EXTRA[ds] as u32)? as usize;
                if d > out.len() {
                    return bad("distance before start of data");
                }
                let start = out.len() - d;
                for k in 0..len {
                    let b = out[start + k];
                    out.push(b);
                }
            }
            _ => return bad("invalid literal/length code"),
        }
        if out.len() > limit {
            return bad("decompressed data larger than the image");
        }
    }
}

/// Decompresses a zlib stream. `limit` caps the output size.
pub fn zlib_decompress(data: &[u8], limit: usize) -> Result<Vec<u8>, PngError> {
    if data.len() < 6 {
        return bad("zlib stream too short");
    }
    let (cmf, flg) = (data[0], data[1]);
    if cmf & 0x0F != 8 || cmf >> 4 > 7 || ((cmf as u16) << 8 | flg as u16) % 31 != 0 {
        return bad("bad zlib header");
    }
    if flg & 0x20 != 0 {
        return bad("zlib preset dictionary not supported");
    }
    let mut r = BitReader { data: &data[2..], pos: 0, acc: 0, n: 0, overrun: 0 };
    let mut out = Vec::new();
    loop {
        let last = r.bits(1)? == 1;
        match r.bits(2)? {
            0 => {
                r.align_to_byte();
                let len = r.bits(16)? as usize;
                let nlen = r.bits(16)? as usize;
                if len != !nlen & 0xFFFF {
                    return bad("stored block length mismatch");
                }
                for _ in 0..len {
                    out.push(r.bits(8)? as u8);
                }
                if out.len() > limit {
                    return bad("decompressed data larger than the image");
                }
            }
            1 => {
                let lit = Huffman::new(&fixed_lit_lengths())?;
                let dist = Huffman::new(&[5u8; 30])?;
                inflate_block(&mut r, &mut out, &lit, &dist, limit)?;
            }
            2 => {
                let hlit = r.bits(5)? as usize + 257;
                let hdist = r.bits(5)? as usize + 1;
                let hclen = r.bits(4)? as usize + 4;
                let mut clen = [0u8; 19];
                for &idx in &CLEN_ORDER[..hclen] {
                    clen[idx] = r.bits(3)? as u8;
                }
                let clen_code = Huffman::new(&clen)?;
                let mut lengths = Vec::with_capacity(hlit + hdist);
                while lengths.len() < hlit + hdist {
                    let sym = clen_code.decode(&mut r)?;
                    let (value, repeat) = match sym {
                        0..=15 => (sym as u8, 1),
                        16 => {
                            let &p = lengths.last().ok_or(PngError::Format("repeat with no previous length".into()))?;
                            (p, 3 + r.bits(2)? as usize)
                        }
                        17 => (0, 3 + r.bits(3)? as usize),
                        18 => (0, 11 + r.bits(7)? as usize),
                        _ => return bad("invalid code length code"),
                    };
                    if lengths.len() + repeat > hlit + hdist {
                        return bad("code lengths overflow");
                    }
                    lengths.extend(std::iter::repeat_n(value, repeat));
                }
                if lengths[256] == 0 {
                    return bad("no end-of-block code");
                }
                let lit = Huffman::new(&lengths[..hlit])?;
                let dist = Huffman::new(&lengths[hlit..])?;
                inflate_block(&mut r, &mut out, &lit, &dist, limit)?;
            }
            _ => return bad("invalid block type"),
        }
        if last {
            break;
        }
    }
    r.align_to_byte();
    let p = 2 + r.byte_pos();
    let Some(tail) = data.get(p..p + 4) else {
        return bad("missing Adler-32");
    };
    if u32::from_be_bytes([tail[0], tail[1], tail[2], tail[3]]) != adler32(&out) {
        return bad("Adler-32 mismatch");
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------------------------
// Filters (PNG section 9)

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let (pa, pb, pc) = ((p - a as i16).abs(), (p - b as i16).abs(), (p - c as i16).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn unfilter(kind: u8, row: &mut [u8], prev: &[u8], bpp: usize) -> Result<(), PngError> {
    match kind {
        0 => {}
        1 => {
            for i in bpp..row.len() {
                row[i] = row[i].wrapping_add(row[i - bpp]);
            }
        }
        2 => {
            for i in 0..row.len() {
                row[i] = row[i].wrapping_add(prev[i]);
            }
        }
        3 => {
            for i in 0..row.len() {
                let a = if i >= bpp { row[i - bpp] } else { 0 };
                row[i] = row[i].wrapping_add(((a as u16 + prev[i] as u16) / 2) as u8);
            }
        }
        4 => {
            for i in 0..row.len() {
                let (a, c) = if i >= bpp { (row[i - bpp], prev[i - bpp]) } else { (0, 0) };
                row[i] = row[i].wrapping_add(paeth(a, prev[i], c));
            }
        }
        _ => return bad(format!("unknown filter type {kind}")),
    }
    Ok(())
}

fn filter_row(kind: u8, row: &[u8], prev: &[u8], bpp: usize, out: &mut Vec<u8>) {
    out.push(kind);
    for i in 0..row.len() {
        let a = if i >= bpp { row[i - bpp] } else { 0 };
        let c = if i >= bpp { prev[i - bpp] } else { 0 };
        let b = prev[i];
        let pred = match kind {
            0 => 0,
            1 => a,
            2 => b,
            3 => ((a as u16 + b as u16) / 2) as u8,
            _ => paeth(a, b, c),
        };
        out.push(row[i].wrapping_sub(pred));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Encoding

fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    out.extend_from_slice(&crc32(&[kind, data]).to_be_bytes());
}

/// Encodes an image as PNG: RGB when every pixel is opaque, RGBA otherwise.
pub fn encode(img: &Image) -> Vec<u8> {
    let opaque = img.is_opaque();
    let channels = if opaque { 3 } else { 4 };
    let (w, h) = (img.width() as usize, img.height() as usize);
    let stride = w * channels;

    let mut raw = Vec::with_capacity((stride + 1) * h);
    let mut prev = vec![0u8; stride];
    let mut row = vec![0u8; stride];
    let mut trial = Vec::with_capacity(stride + 1);
    for y in 0..h {
        let src = &img.pixels()[y * w * 4..(y + 1) * w * 4];
        if opaque {
            for (d, s) in row.as_chunks_mut::<3>().0.iter_mut().zip(src.as_chunks::<4>().0) {
                d.copy_from_slice(&s[..3]);
            }
        } else {
            row.copy_from_slice(src);
        }
        // Pick the filter with the smallest sum of absolute (signed) residuals.
        let mut best = (u64::MAX, 0u8);
        for kind in 0..5u8 {
            trial.clear();
            filter_row(kind, &row, &prev, channels, &mut trial);
            let score: u64 = trial[1..].iter().map(|&v| (v as i8).unsigned_abs() as u64).sum();
            if score < best.0 {
                best = (score, kind);
            }
        }
        filter_row(best.1, &row, &prev, channels, &mut raw);
        std::mem::swap(&mut prev, &mut row);
    }

    let mut out = Vec::new();
    out.extend_from_slice(&SIGNATURE);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&img.width().to_be_bytes());
    ihdr.extend_from_slice(&img.height().to_be_bytes());
    ihdr.extend_from_slice(&[8, if opaque { 2 } else { 6 }, 0, 0, 0]);
    write_chunk(&mut out, b"IHDR", &ihdr);
    write_chunk(&mut out, b"IDAT", &zlib_compress(&raw));
    write_chunk(&mut out, b"IEND", &[]);
    out
}

/// Encodes and writes a PNG file, creating parent directories.
pub fn write_file(path: impl AsRef<Path>, img: &Image) -> Result<(), PngError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, encode(img))?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Decoding

struct Header {
    width: u32,
    height: u32,
    depth: u8,
    color: u8,
    interlaced: bool,
}

impl Header {
    fn channels(&self) -> usize {
        match self.color {
            0 | 3 => 1,
            2 => 3,
            4 => 2,
            _ => 4,
        }
    }

    fn bits_per_pixel(&self) -> usize {
        self.channels() * self.depth as usize
    }

    fn row_bytes(&self, width: usize) -> usize {
        (width * self.bits_per_pixel()).div_ceil(8)
    }
}

/// Decodes a PNG to RGBA8.
pub fn decode(bytes: &[u8]) -> Result<Image, PngError> {
    if bytes.len() < 8 || bytes[..8] != SIGNATURE {
        return bad("missing PNG signature");
    }
    let mut pos = 8;
    let mut header: Option<Header> = None;
    let mut palette: Vec<[u8; 4]> = Vec::new();
    let mut trns: Option<Vec<u8>> = None;
    let mut idat = Vec::new();
    let mut seen_iend = false;
    while pos < bytes.len() {
        let Some(len) = bytes.get(pos..pos + 4) else { return bad("truncated chunk") };
        let len = u32::from_be_bytes([len[0], len[1], len[2], len[3]]) as usize;
        let Some(kind) = bytes.get(pos + 4..pos + 8) else { return bad("truncated chunk") };
        let kind: [u8; 4] = [kind[0], kind[1], kind[2], kind[3]];
        let Some(data) = bytes.get(pos + 8..pos + 8 + len) else { return bad("truncated chunk") };
        let Some(crc) = bytes.get(pos + 8 + len..pos + 12 + len) else { return bad("truncated chunk") };
        if u32::from_be_bytes([crc[0], crc[1], crc[2], crc[3]]) != crc32(&[&kind, data]) {
            return bad(format!("CRC mismatch in {} chunk", String::from_utf8_lossy(&kind)));
        }
        pos += 12 + len;
        if header.is_none() && &kind != b"IHDR" {
            return bad("first chunk is not IHDR");
        }
        match &kind {
            b"IHDR" => {
                if header.is_some() || len != 13 {
                    return bad("bad IHDR");
                }
                let width = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
                let height = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                let (depth, color) = (data[8], data[9]);
                if data[10] != 0 || data[11] != 0 || data[12] > 1 {
                    return bad("unsupported compression, filter or interlace method");
                }
                let ok = match color {
                    0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
                    3 => matches!(depth, 1 | 2 | 4 | 8),
                    2 | 4 | 6 => matches!(depth, 8 | 16),
                    _ => false,
                };
                if !ok {
                    return bad(format!("invalid colour type {color} with bit depth {depth}"));
                }
                if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
                    return bad(format!("unsupported size {width}x{height}"));
                }
                header = Some(Header { width, height, depth, color, interlaced: data[12] == 1 });
            }
            b"PLTE" => {
                if len % 3 != 0 || len / 3 > 256 || len == 0 {
                    return bad("bad PLTE");
                }
                palette = data.as_chunks::<3>().0.iter().map(|c| [c[0], c[1], c[2], 255]).collect();
            }
            b"tRNS" => trns = Some(data.to_vec()),
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => {
                seen_iend = true;
                break;
            }
            _ => {
                if kind[0] & 0x20 == 0 {
                    return bad(format!("unknown critical chunk {}", String::from_utf8_lossy(&kind)));
                }
            }
        }
    }
    if !seen_iend {
        return bad("missing IEND");
    }
    let Some(h) = header else { return bad("missing IHDR") };
    if h.color == 3 {
        if palette.is_empty() {
            return bad("palette image without PLTE");
        }
        if let Some(t) = &trns {
            for (entry, &a) in palette.iter_mut().zip(t) {
                entry[3] = a;
            }
        }
    }

    let passes: Vec<(usize, usize, usize, usize)> = if h.interlaced {
        // (x start, y start, x step, y step) for the seven Adam7 passes.
        vec![(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)]
    } else {
        vec![(0, 0, 1, 1)]
    };
    let (w, hgt) = (h.width as usize, h.height as usize);
    let pass_dims = |&(x0, y0, dx, dy): &(usize, usize, usize, usize)| {
        let pw = if w > x0 { (w - x0).div_ceil(dx) } else { 0 };
        let ph = if hgt > y0 { (hgt - y0).div_ceil(dy) } else { 0 };
        (pw, ph)
    };
    let expected: usize = passes
        .iter()
        .map(|p| {
            let (pw, ph) = pass_dims(p);
            if pw == 0 || ph == 0 { 0 } else { (h.row_bytes(pw) + 1) * ph }
        })
        .sum();
    let raw = zlib_decompress(&idat, expected)?;
    if raw.len() < expected {
        return bad("image data too short");
    }

    let trns_key = |c: usize| -> Option<u16> {
        let t = trns.as_ref()?;
        t.get(c * 2..c * 2 + 2).map(|b| u16::from_be_bytes([b[0], b[1]]))
    };
    let mut img = Image::new(h.width, h.height, [0, 0, 0, 0]);
    let bpp = h.bits_per_pixel().div_ceil(8);
    let mut offset = 0;
    for pass in &passes {
        let (pw, ph) = pass_dims(pass);
        if pw == 0 || ph == 0 {
            continue;
        }
        let rb = h.row_bytes(pw);
        let mut prev = vec![0u8; rb];
        for py in 0..ph {
            let kind = raw[offset];
            let mut row = raw[offset + 1..offset + 1 + rb].to_vec();
            offset += rb + 1;
            unfilter(kind, &mut row, &prev, bpp)?;
            for px in 0..pw {
                let sample = |c: usize| -> u16 {
                    let idx = px * h.channels() + c;
                    match h.depth {
                        16 => u16::from_be_bytes([row[idx * 2], row[idx * 2 + 1]]),
                        8 => row[idx] as u16,
                        d => {
                            let bit = idx * d as usize;
                            let byte = row[bit / 8];
                            let shift = 8 - d as usize - bit % 8;
                            ((byte >> shift) & ((1u8 << d) - 1)) as u16
                        }
                    }
                };
                let to8 = |v: u16| -> u8 {
                    match h.depth {
                        16 => (v >> 8) as u8,
                        8 => v as u8,
                        d => (v as u32 * 255 / ((1u32 << d) - 1)) as u8,
                    }
                };
                let rgba = match h.color {
                    0 => {
                        let v = sample(0);
                        let a = if trns_key(0) == Some(v) { 0 } else { 255 };
                        let g = to8(v);
                        [g, g, g, a]
                    }
                    2 => {
                        let (r, g, b) = (sample(0), sample(1), sample(2));
                        let key = (trns_key(0), trns_key(1), trns_key(2));
                        let a = if key == (Some(r), Some(g), Some(b)) { 0 } else { 255 };
                        [to8(r), to8(g), to8(b), a]
                    }
                    3 => {
                        let i = sample(0) as usize;
                        *palette.get(i).ok_or(PngError::Format(format!("palette index {i} out of range")))?
                    }
                    4 => {
                        let g = to8(sample(0));
                        [g, g, g, to8(sample(1))]
                    }
                    _ => [to8(sample(0)), to8(sample(1)), to8(sample(2)), to8(sample(3))],
                };
                let (x0, y0, dx, dy) = *pass;
                img.set((x0 + px * dx) as u32, (y0 + py * dy) as u32, rgba);
            }
            prev = row;
        }
    }
    Ok(img)
}

/// Reads and decodes a PNG file.
pub fn read_file(path: impl AsRef<Path>) -> Result<Image, PngError> {
    decode(&std::fs::read(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    fn noisy_image(w: u32, h: u32, seed: u64, alpha: bool) -> Image {
        let mut rng = Rng::new(seed);
        let mut img = Image::new(w, h, [0, 0, 0, 255]);
        for y in 0..h {
            for x in 0..w {
                // Mix smooth gradients (filters and matches help) with noise (they don't).
                let n = rng.next_u32();
                let c = if (x / 4 + y / 4) % 3 == 0 {
                    [n as u8, (n >> 8) as u8, (n >> 16) as u8, if alpha { (n >> 24) as u8 } else { 255 }]
                } else {
                    [(x * 3) as u8, (y * 5) as u8, ((x + y) * 2) as u8, if alpha { 200 } else { 255 }]
                };
                img.set(x, y, c);
            }
        }
        img
    }

    #[test]
    fn crc_and_adler_known_vectors() {
        assert_eq!(crc32(&[b"123456789"]), 0xCBF4_3926);
        assert_eq!(crc32(&[b"1234", b"56789"]), 0xCBF4_3926);
        assert_eq!(crc32(&[b"IEND"]), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(b""), 1);
        let big = vec![0xFFu8; 100_000];
        let (mut a, mut b) = (1u64, 0u64);
        for &x in &big {
            a = (a + x as u64) % 65521;
            b = (b + a) % 65521;
        }
        assert_eq!(adler32(&big), ((b << 16) | a) as u32);
    }

    #[test]
    fn zlib_round_trip() {
        let mut rng = Rng::new(3);
        let cases: Vec<Vec<u8>> = vec![
            vec![],
            vec![7],
            b"abcabcabcabcabcabcabcabc".to_vec(),
            vec![0u8; 100_000],
            (0..70_000).map(|_| rng.below(4) as u8).collect(),
            (0..50_000).map(|_| rng.next_u32() as u8).collect(),
        ];
        for data in cases {
            let z = zlib_compress(&data);
            assert_eq!(zlib_decompress(&z, data.len()).unwrap(), data);
        }
        let z = zlib_compress(&[0u8; 100_000]);
        assert!(z.len() < 1000, "runs compress: {} bytes", z.len());
    }

    #[test]
    fn inflates_stored_and_dynamic_blocks() {
        // A stored block holding "hello", written by hand.
        let mut s = vec![0x78, 0x01, 0x01, 5, 0, !5u8, 0xFF];
        s.extend_from_slice(b"hello");
        s.extend_from_slice(&adler32(b"hello").to_be_bytes());
        assert_eq!(zlib_decompress(&s, 100).unwrap(), b"hello");

        // A dynamic-Huffman block, written by the small test writer below.
        let data = b"the quick brown fox jumps over the lazy dog, the quick brown fox";
        let z = dynamic_block_for_test(data);
        assert_eq!(zlib_decompress(&z, 1000).unwrap(), data);
    }

    /// Writes a single dynamic-Huffman block that codes every literal with 8 bits except a few (a valid, if silly,
    /// code), to exercise the dynamic-header path of the decoder.
    fn dynamic_block_for_test(data: &[u8]) -> Vec<u8> {
        // Literal/length code: every symbol 9 bits except `o` (4 bits). The code is incomplete, which the decoder
        // accepts as long as it is not over-subscribed.
        let mut lit_lens = vec![9u8; 257];
        lit_lens[b'o' as usize] = 4;
        let dist_lens = vec![1u8];
        let mut w = BitWriter { out: vec![0x78, 0x01], acc: 0, n: 0 };
        w.put(1, 1);
        w.put(2, 2);
        w.put((lit_lens.len() - 257) as u32, 5);
        w.put((dist_lens.len() - 1) as u32, 5);
        // Code-length alphabet: only lengths 1, 4 and 9 occur; give each a 2-bit code.
        let mut clen = [0u8; 19];
        clen[1] = 2;
        clen[4] = 2;
        clen[9] = 2;
        w.put(15, 4); // HCLEN = 19 entries
        for &idx in &CLEN_ORDER {
            w.put(clen[idx] as u32, 3);
        }
        let clen_codes = canonical_codes(&clen);
        for &l in lit_lens.iter().chain(&dist_lens) {
            w.put_code(clen_codes[l as usize], clen[l as usize] as u32);
        }
        let lit_codes = canonical_codes(&lit_lens);
        for &b in data {
            w.put_code(lit_codes[b as usize], lit_lens[b as usize] as u32);
        }
        w.put_code(lit_codes[256], lit_lens[256] as u32);
        let mut out = w.finish();
        out.extend_from_slice(&adler32(data).to_be_bytes());
        out
    }

    #[test]
    fn png_round_trip_rgb_and_rgba() {
        for (w, h, alpha) in [(1, 1, false), (17, 9, false), (64, 48, true), (3, 100, true), (200, 2, false)] {
            let img = noisy_image(w, h, w as u64 * 31 + h as u64, alpha);
            let bytes = encode(&img);
            assert_eq!(decode(&bytes).unwrap(), img, "{w}x{h} alpha={alpha}");
        }
    }

    #[test]
    fn png_file_round_trip() {
        let dir = std::env::temp_dir().join(format!("slynx-harness-png-{}", std::process::id()));
        let path = dir.join("sub").join("t.png");
        let img = noisy_image(33, 21, 1, true);
        write_file(&path, &img).unwrap();
        assert_eq!(read_file(&path).unwrap(), img);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Builds a PNG from raw (already filtered) scanlines and an optional palette / tRNS, for decoder tests of
    /// formats our encoder never writes.
    fn build_png(
        w: u32,
        h: u32,
        depth: u8,
        color: u8,
        interlace: u8,
        raw: &[u8],
        extra: &[(&[u8; 4], Vec<u8>)],
    ) -> Vec<u8> {
        let mut out = SIGNATURE.to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[depth, color, 0, 0, interlace]);
        write_chunk(&mut out, b"IHDR", &ihdr);
        for (k, d) in extra {
            write_chunk(&mut out, k, d);
        }
        // Split the data over two IDAT chunks to check they are joined.
        let z = zlib_compress(raw);
        let mid = z.len() / 2;
        write_chunk(&mut out, b"IDAT", &z[..mid]);
        write_chunk(&mut out, b"tEXt", b"Comment\0ignored");
        write_chunk(&mut out, b"IDAT", &z[mid..]);
        write_chunk(&mut out, b"IEND", &[]);
        out
    }

    #[test]
    fn decodes_greyscale_palette_and_16_bit() {
        // 4x2 greyscale, 2 bits per pixel: values 0,1,2,3 / 3,2,1,0.
        let raw = [0, 0b00_01_10_11, 0, 0b11_10_01_00];
        let img = decode(&build_png(4, 2, 2, 0, 0, &raw, &[])).unwrap();
        assert_eq!(img.get(0, 0), [0, 0, 0, 255]);
        assert_eq!(img.get(1, 0), [85, 85, 85, 255]);
        assert_eq!(img.get(3, 0), [255, 255, 255, 255]);
        assert_eq!(img.get(0, 1), [255, 255, 255, 255]);

        // 3x1 palette, 1 bit per pixel, with tRNS making entry 1 half transparent.
        let raw = [0, 0b0100_0000];
        let plte = vec![10, 20, 30, 40, 50, 60];
        let img = decode(&build_png(3, 1, 1, 3, 0, &raw, &[(b"PLTE", plte), (b"tRNS", vec![255, 128])])).unwrap();
        assert_eq!(img.get(0, 0), [10, 20, 30, 255]);
        assert_eq!(img.get(1, 0), [40, 50, 60, 128]);
        assert_eq!(img.get(2, 0), [10, 20, 30, 255]);

        // 2x1 RGB 16-bit with a tRNS colour key on the second pixel, and the Sub filter.
        let px0 = [0x12u8, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let px1 = [0x01u8, 0x00, 0x02, 0x00, 0x03, 0x00];
        let mut row = vec![1u8];
        row.extend_from_slice(&px0);
        for i in 0..6 {
            row.push(px1[i].wrapping_sub(px0[i]));
        }
        let key = vec![0x01, 0x00, 0x02, 0x00, 0x03, 0x00];
        let img = decode(&build_png(2, 1, 16, 2, 0, &row, &[(b"tRNS", key)])).unwrap();
        assert_eq!(img.get(0, 0), [0x12, 0x56, 0x9A, 255]);
        assert_eq!(img.get(1, 0), [1, 2, 3, 0]);

        // Grey+alpha 8-bit with the Up filter on the second row.
        let raw = [0, 100, 200, 2, 5, 5];
        let img = decode(&build_png(1, 2, 8, 4, 0, &raw, &[])).unwrap();
        assert_eq!(img.get(0, 0), [100, 100, 100, 200]);
        assert_eq!(img.get(0, 1), [105, 105, 105, 205]);
    }

    #[test]
    fn decodes_adam7_interlace() {
        // Build an interlaced RGBA image by hand from a known image, then decode it.
        let src = noisy_image(11, 7, 9, true);
        let passes = [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)];
        let mut raw = Vec::new();
        for (x0, y0, dx, dy) in passes {
            let mut prev: Vec<u8> = Vec::new();
            let mut y = y0;
            while y < 7 {
                let mut row = Vec::new();
                let mut x = x0;
                while x < 11 {
                    row.extend_from_slice(&src.get(x, y));
                    x += dx;
                }
                if row.is_empty() {
                    break;
                }
                if prev.is_empty() {
                    prev = vec![0; row.len()];
                }
                filter_row(4, &row, &prev, 4, &mut raw);
                prev = row;
                y += dy;
            }
        }
        let img = decode(&build_png(11, 7, 8, 6, 1, &raw, &[])).unwrap();
        assert_eq!(img, src);
    }

    #[test]
    fn rejects_corrupt_files() {
        let img = noisy_image(8, 8, 2, false);
        let good = encode(&img);
        assert!(decode(&good[..good.len() - 5]).is_err(), "truncated");
        let mut bad_crc = good.clone();
        bad_crc[20] ^= 1;
        assert!(decode(&bad_crc).is_err(), "IHDR CRC");
        let mut bad_sig = good.clone();
        bad_sig[0] = 0;
        assert!(decode(&bad_sig).is_err(), "signature");
        assert!(decode(&[]).is_err());
        // Garbage zlib data inside a valid chunk structure.
        let garbage = build_png(4, 4, 8, 6, 0, &[0; 4], &[]);
        assert!(decode(&garbage).is_err(), "data too short for 4x4");
        let mut z = zlib_compress(b"abcdef");
        let n = z.len();
        z[n - 1] ^= 1;
        assert!(zlib_decompress(&z, 100).is_err(), "Adler mismatch");
        assert!(zlib_decompress(&zlib_compress(&[0; 1000]), 10).is_err(), "limit");
    }
}
