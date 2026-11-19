//! Variable-length code tables of ISO/IEC 13818-2 Annex B, and a two-level lookup decoder for them.
//!
//! The codes are written as the standard gives them (strings of `0` and `1`, spaces ignored). Sign bits that follow
//! a code (`s` in the standard) are not part of these strings; the decoder reads them separately.

use alloc::vec;
use alloc::vec::Vec;

use crate::bits::BitReader;

/// Bits indexed by the first-level table.
const ROOT_BITS: u32 = 8;

#[derive(Clone, Copy, Default, Debug)]
struct Entry {
    /// The symbol, or the subtable offset when `sub` is non-zero.
    value: i16,
    /// Total code length in bits; 0 marks an unused code.
    len: u8,
    /// Width of the subtable this entry points to (0 for a leaf).
    sub: u8,
}

/// A prefix-code decoder: a 256-entry first level, and subtables for the codes longer than 8 bits.
#[derive(Clone, Debug)]
pub struct Vlc {
    table: Vec<Entry>,
}

/// Parses a code string into (bits, length).
fn parse(code: &str) -> (u32, u32) {
    let mut bits = 0u32;
    let mut len = 0u32;
    for c in code.bytes() {
        match c {
            b'0' | b'1' => {
                bits = (bits << 1) | u32::from(c - b'0');
                len += 1;
            }
            b' ' => {}
            _ => panic!("bad character in a code table"),
        }
    }
    assert!((1..=24).contains(&len), "code length out of range");
    (bits, len)
}

impl Vlc {
    /// Builds a decoder from `(code, symbol)` pairs. Panics if the codes are not a prefix code (the tables are fixed,
    /// so this is a programming error caught by the unit tests).
    pub fn build(codes: &[(&str, i16)]) -> Vlc {
        let parsed: Vec<(u32, u32, i16)> = codes
            .iter()
            .map(|&(c, v)| {
                let (b, l) = parse(c);
                (b, l, v)
            })
            .collect();
        let mut table = vec![Entry::default(); 1 << ROOT_BITS];
        // First the short codes, directly in the root.
        for &(bits, len, value) in &parsed {
            if len <= ROOT_BITS {
                let shift = ROOT_BITS - len;
                let base = (bits << shift) as usize;
                for e in &mut table[base..base + (1 << shift)] {
                    assert!(e.len == 0, "codes overlap");
                    *e = Entry { value, len: len as u8, sub: 0 };
                }
            }
        }
        // Then a subtable per root prefix of the long codes.
        for prefix in 0..(1u32 << ROOT_BITS) {
            let long: Vec<&(u32, u32, i16)> =
                parsed.iter().filter(|&&(b, l, _)| l > ROOT_BITS && b >> (l - ROOT_BITS) == prefix).collect();
            if long.is_empty() {
                continue;
            }
            assert!(table[prefix as usize].len == 0, "a long code extends a short one");
            let width = long.iter().map(|&&(_, l, _)| l - ROOT_BITS).max().unwrap_or(0);
            let offset = table.len();
            assert!(offset + (1 << width) < i16::MAX as usize, "table too large");
            table.resize(offset + (1 << width), Entry::default());
            table[prefix as usize] = Entry { value: offset as i16, len: 0, sub: width as u8 };
            for &&(bits, len, value) in &long {
                let rest = bits & ((1 << (len - ROOT_BITS)) - 1);
                let shift = width - (len - ROOT_BITS);
                let base = offset + (rest << shift) as usize;
                for e in &mut table[base..base + (1 << shift)] {
                    assert!(e.len == 0, "codes overlap");
                    *e = Entry { value, len: len as u8, sub: 0 };
                }
            }
        }
        Vlc { table }
    }

    /// The symbol and code length at the start of the left-aligned word `p`, or `None` for a pattern that is not
    /// a code. Nothing is consumed.
    #[inline(always)]
    pub(crate) fn lookup(&self, p: u32) -> Option<(i16, u32)> {
        let mut e = *self.table.get((p >> (32 - ROOT_BITS)) as usize)?;
        if e.sub != 0 {
            let idx = e.value as usize + ((p << ROOT_BITS) >> (32 - u32::from(e.sub))) as usize;
            e = *self.table.get(idx)?;
        }
        if e.len == 0 { None } else { Some((e.value, u32::from(e.len))) }
    }

    /// Decodes one symbol, or `None` for a bit pattern that is not a code.
    #[inline(always)]
    pub(crate) fn decode(&self, br: &mut BitReader<'_>) -> Option<i16> {
        let (v, len) = self.lookup(br.peek32())?;
        br.skip(len);
        Some(v)
    }

    /// Decodes from a left-aligned 32-bit word; returns (symbol, length). For tests.
    pub fn decode_word(&self, word: u32) -> Option<(i16, u32)> {
        let bytes = word.to_be_bytes();
        let mut br = BitReader::new(&bytes);
        let v = self.decode(&mut br)?;
        Some((v, br.position() as u32))
    }
}

/// Macroblock type flags (Tables B.2 to B.4).
pub mod mb {
    pub const QUANT: i16 = 1;
    pub const FORWARD: i16 = 2;
    pub const BACKWARD: i16 = 4;
    pub const PATTERN: i16 = 8;
    pub const INTRA: i16 = 16;
}

/// Symbol of `macroblock_escape` in [`MACROBLOCK_ADDRESS_INCREMENT`].
pub const MB_ESCAPE: i16 = -1;

/// Table B.1, macroblock_address_increment.
pub const MACROBLOCK_ADDRESS_INCREMENT: &[(&str, i16)] = &[
    ("1", 1),
    ("011", 2),
    ("010", 3),
    ("0011", 4),
    ("0010", 5),
    ("0001 1", 6),
    ("0001 0", 7),
    ("0000 111", 8),
    ("0000 110", 9),
    ("0000 1011", 10),
    ("0000 1010", 11),
    ("0000 1001", 12),
    ("0000 1000", 13),
    ("0000 0111", 14),
    ("0000 0110", 15),
    ("0000 0101 11", 16),
    ("0000 0101 10", 17),
    ("0000 0101 01", 18),
    ("0000 0101 00", 19),
    ("0000 0100 11", 20),
    ("0000 0100 10", 21),
    ("0000 0100 011", 22),
    ("0000 0100 010", 23),
    ("0000 0100 001", 24),
    ("0000 0100 000", 25),
    ("0000 0011 111", 26),
    ("0000 0011 110", 27),
    ("0000 0011 101", 28),
    ("0000 0011 100", 29),
    ("0000 0011 011", 30),
    ("0000 0011 010", 31),
    ("0000 0011 001", 32),
    ("0000 0011 000", 33),
    ("0000 0001 000", MB_ESCAPE),
];

use mb::{BACKWARD as B, FORWARD as F, INTRA as I, PATTERN as P, QUANT as Q};

/// Table B.2, macroblock_type in I pictures.
pub const MB_TYPE_I: &[(&str, i16)] = &[("1", I), ("01", I | Q)];

/// Table B.3, macroblock_type in P pictures.
pub const MB_TYPE_P: &[(&str, i16)] =
    &[("1", F | P), ("01", P), ("001", F), ("0001 1", I), ("0001 0", Q | F | P), ("0000 1", Q | P), ("0000 01", Q | I)];

/// Table B.4, macroblock_type in B pictures.
pub const MB_TYPE_B: &[(&str, i16)] = &[
    ("10", F | B),
    ("11", F | B | P),
    ("010", B),
    ("011", B | P),
    ("0010", F),
    ("0011", F | P),
    ("0001 1", I),
    ("0001 0", Q | F | B | P),
    ("0000 11", Q | F | P),
    ("0000 10", Q | B | P),
    ("0000 01", Q | I),
];

/// Table B.9, coded_block_pattern (4:2:0).
pub const CODED_BLOCK_PATTERN: &[(&str, i16)] = &[
    ("111", 60),
    ("1101", 4),
    ("1100", 8),
    ("1011", 16),
    ("1010", 32),
    ("1001 1", 12),
    ("1001 0", 48),
    ("1000 1", 20),
    ("1000 0", 40),
    ("0111 1", 28),
    ("0111 0", 44),
    ("0110 1", 52),
    ("0110 0", 56),
    ("0101 1", 1),
    ("0101 0", 61),
    ("0100 1", 2),
    ("0100 0", 62),
    ("0011 11", 24),
    ("0011 10", 36),
    ("0011 01", 3),
    ("0011 00", 63),
    ("0010 111", 5),
    ("0010 110", 9),
    ("0010 101", 17),
    ("0010 100", 33),
    ("0010 011", 6),
    ("0010 010", 10),
    ("0010 001", 18),
    ("0010 000", 34),
    ("0001 1111", 7),
    ("0001 1110", 11),
    ("0001 1101", 19),
    ("0001 1100", 35),
    ("0001 1011", 13),
    ("0001 1010", 49),
    ("0001 1001", 21),
    ("0001 1000", 41),
    ("0001 0111", 14),
    ("0001 0110", 50),
    ("0001 0101", 22),
    ("0001 0100", 42),
    ("0001 0011", 15),
    ("0001 0010", 51),
    ("0001 0001", 23),
    ("0001 0000", 43),
    ("0000 1111", 25),
    ("0000 1110", 37),
    ("0000 1101", 26),
    ("0000 1100", 38),
    ("0000 1011", 29),
    ("0000 1010", 45),
    ("0000 1001", 53),
    ("0000 1000", 57),
    ("0000 0111", 30),
    ("0000 0110", 46),
    ("0000 0101", 54),
    ("0000 0100", 58),
    ("0000 0011 1", 31),
    ("0000 0011 0", 47),
    ("0000 0010 1", 55),
    ("0000 0010 0", 59),
    ("0000 0001 1", 27),
    ("0000 0001 0", 39),
    ("0000 0000 1", 0),
];

/// Table B.10, motion_code magnitudes (a sign bit follows every non-zero code).
pub const MOTION_CODE: &[(&str, i16)] = &[
    ("1", 0),
    ("01", 1),
    ("001", 2),
    ("0001", 3),
    ("0000 11", 4),
    ("0000 101", 5),
    ("0000 100", 6),
    ("0000 011", 7),
    ("0000 0101 1", 8),
    ("0000 0101 0", 9),
    ("0000 0100 1", 10),
    ("0000 0100 01", 11),
    ("0000 0100 00", 12),
    ("0000 0011 11", 13),
    ("0000 0011 10", 14),
    ("0000 0011 01", 15),
    ("0000 0011 00", 16),
];

/// Table B.11, dmvector.
pub const DMVECTOR: &[(&str, i16)] = &[("0", 0), ("10", 1), ("11", -1)];

/// Table B.12, dct_dc_size_luminance.
pub const DC_SIZE_LUMA: &[(&str, i16)] = &[
    ("100", 0),
    ("00", 1),
    ("01", 2),
    ("101", 3),
    ("110", 4),
    ("1110", 5),
    ("1111 0", 6),
    ("1111 10", 7),
    ("1111 110", 8),
    ("1111 1110", 9),
    ("1111 1111 0", 10),
    ("1111 1111 1", 11),
];

/// Table B.13, dct_dc_size_chrominance.
pub const DC_SIZE_CHROMA: &[(&str, i16)] = &[
    ("00", 0),
    ("01", 1),
    ("10", 2),
    ("110", 3),
    ("1110", 4),
    ("1111 0", 5),
    ("1111 10", 6),
    ("1111 110", 7),
    ("1111 1110", 8),
    ("1111 1111 0", 9),
    ("1111 1111 10", 10),
    ("1111 1111 11", 11),
];

/// DCT coefficient symbol for end of block.
pub const DCT_EOB: i16 = -1;
/// DCT coefficient symbol for escape (6-bit run and 12-bit level follow).
pub const DCT_ESCAPE: i16 = -2;

/// Packs a (run, level) pair into a DCT table symbol.
pub const fn rl(run: i16, level: i16) -> i16 {
    (level << 5) | run
}

/// Run of a packed DCT symbol.
#[inline(always)]
pub const fn run_of(sym: i16) -> i16 {
    sym & 31
}

/// Level of a packed DCT symbol.
#[inline(always)]
pub const fn level_of(sym: i16) -> i16 {
    sym >> 5
}

/// The (run, level) codes shared by Tables B.14 and B.15 (14 bits and longer, plus most 12- and 13-bit codes).
const DCT_COMMON: &[(&str, i16)] = &[
    ("0000 01", DCT_ESCAPE),
    ("0000 0001 1100", rl(3, 3)),
    ("0000 0001 0010", rl(4, 3)),
    ("0000 0001 1110", rl(6, 2)),
    ("0000 0001 0101", rl(7, 2)),
    ("0000 0001 0001", rl(8, 2)),
    ("0000 0001 1111", rl(17, 1)),
    ("0000 0001 1010", rl(18, 1)),
    ("0000 0001 1001", rl(19, 1)),
    ("0000 0001 0111", rl(20, 1)),
    ("0000 0001 0110", rl(21, 1)),
    ("0000 0000 1011 0", rl(1, 6)),
    ("0000 0000 1010 1", rl(1, 7)),
    ("0000 0000 1010 0", rl(2, 5)),
    ("0000 0000 1001 1", rl(3, 4)),
    ("0000 0000 1001 0", rl(5, 3)),
    ("0000 0000 1000 1", rl(9, 2)),
    ("0000 0000 1000 0", rl(10, 2)),
    ("0000 0000 1111 1", rl(22, 1)),
    ("0000 0000 1111 0", rl(23, 1)),
    ("0000 0000 1110 1", rl(24, 1)),
    ("0000 0000 1110 0", rl(25, 1)),
    ("0000 0000 1101 1", rl(26, 1)),
    ("0000 0000 0111 11", rl(0, 16)),
    ("0000 0000 0111 10", rl(0, 17)),
    ("0000 0000 0111 01", rl(0, 18)),
    ("0000 0000 0111 00", rl(0, 19)),
    ("0000 0000 0110 11", rl(0, 20)),
    ("0000 0000 0110 10", rl(0, 21)),
    ("0000 0000 0110 01", rl(0, 22)),
    ("0000 0000 0110 00", rl(0, 23)),
    ("0000 0000 0101 11", rl(0, 24)),
    ("0000 0000 0101 10", rl(0, 25)),
    ("0000 0000 0101 01", rl(0, 26)),
    ("0000 0000 0101 00", rl(0, 27)),
    ("0000 0000 0100 11", rl(0, 28)),
    ("0000 0000 0100 10", rl(0, 29)),
    ("0000 0000 0100 01", rl(0, 30)),
    ("0000 0000 0100 00", rl(0, 31)),
    ("0000 0000 0011 000", rl(0, 32)),
    ("0000 0000 0010 111", rl(0, 33)),
    ("0000 0000 0010 110", rl(0, 34)),
    ("0000 0000 0010 101", rl(0, 35)),
    ("0000 0000 0010 100", rl(0, 36)),
    ("0000 0000 0010 011", rl(0, 37)),
    ("0000 0000 0010 010", rl(0, 38)),
    ("0000 0000 0010 001", rl(0, 39)),
    ("0000 0000 0010 000", rl(0, 40)),
    ("0000 0000 0011 111", rl(1, 8)),
    ("0000 0000 0011 110", rl(1, 9)),
    ("0000 0000 0011 101", rl(1, 10)),
    ("0000 0000 0011 100", rl(1, 11)),
    ("0000 0000 0011 011", rl(1, 12)),
    ("0000 0000 0011 010", rl(1, 13)),
    ("0000 0000 0011 001", rl(1, 14)),
    ("0000 0000 0001 0011", rl(1, 15)),
    ("0000 0000 0001 0010", rl(1, 16)),
    ("0000 0000 0001 0001", rl(1, 17)),
    ("0000 0000 0001 0000", rl(1, 18)),
    ("0000 0000 0001 0100", rl(6, 3)),
    ("0000 0000 0001 1010", rl(11, 2)),
    ("0000 0000 0001 1001", rl(12, 2)),
    ("0000 0000 0001 1000", rl(13, 2)),
    ("0000 0000 0001 0111", rl(14, 2)),
    ("0000 0000 0001 0110", rl(15, 2)),
    ("0000 0000 0001 0101", rl(16, 2)),
    ("0000 0000 0001 1111", rl(27, 1)),
    ("0000 0000 0001 1110", rl(28, 1)),
    ("0000 0000 0001 1101", rl(29, 1)),
    ("0000 0000 0001 1100", rl(30, 1)),
    ("0000 0000 0001 1011", rl(31, 1)),
];

/// The codes of Table B.14 (DCT coefficients table zero) that are not in [`DCT_COMMON`]. `11s` is (0, 1) for
/// coefficients after the first; the first coefficient of a non-intra block uses `1s` (handled by the decoder).
const DCT_ZERO_OWN: &[(&str, i16)] = &[
    ("10", DCT_EOB),
    ("11", rl(0, 1)),
    ("011", rl(1, 1)),
    ("0100", rl(0, 2)),
    ("0101", rl(2, 1)),
    ("0010 1", rl(0, 3)),
    ("0011 1", rl(3, 1)),
    ("0011 0", rl(4, 1)),
    ("0001 10", rl(1, 2)),
    ("0001 11", rl(5, 1)),
    ("0001 01", rl(6, 1)),
    ("0001 00", rl(7, 1)),
    ("0000 110", rl(0, 4)),
    ("0000 100", rl(2, 2)),
    ("0000 111", rl(8, 1)),
    ("0000 101", rl(9, 1)),
    ("0010 0110", rl(0, 5)),
    ("0010 0001", rl(0, 6)),
    ("0010 0101", rl(1, 3)),
    ("0010 0100", rl(3, 2)),
    ("0010 0111", rl(10, 1)),
    ("0010 0011", rl(11, 1)),
    ("0010 0010", rl(12, 1)),
    ("0010 0000", rl(13, 1)),
    ("0000 0010 10", rl(0, 7)),
    ("0000 0011 00", rl(1, 4)),
    ("0000 0010 11", rl(2, 3)),
    ("0000 0011 11", rl(4, 2)),
    ("0000 0010 01", rl(5, 2)),
    ("0000 0011 10", rl(14, 1)),
    ("0000 0011 01", rl(15, 1)),
    ("0000 0010 00", rl(16, 1)),
    ("0000 0001 1101", rl(0, 8)),
    ("0000 0001 1000", rl(0, 9)),
    ("0000 0001 0011", rl(0, 10)),
    ("0000 0001 0000", rl(0, 11)),
    ("0000 0001 1011", rl(1, 5)),
    ("0000 0001 0100", rl(2, 4)),
    ("0000 0000 1101 0", rl(0, 12)),
    ("0000 0000 1100 1", rl(0, 13)),
    ("0000 0000 1100 0", rl(0, 14)),
    ("0000 0000 1011 1", rl(0, 15)),
];

/// The codes of Table B.15 (DCT coefficients table one) that are not in [`DCT_COMMON`].
const DCT_ONE_OWN: &[(&str, i16)] = &[
    ("0110", DCT_EOB),
    ("10", rl(0, 1)),
    ("010", rl(1, 1)),
    ("110", rl(0, 2)),
    ("0010 1", rl(2, 1)),
    ("0111", rl(0, 3)),
    ("0011 1", rl(3, 1)),
    ("0001 10", rl(4, 1)),
    ("0011 0", rl(1, 2)),
    ("0001 11", rl(5, 1)),
    ("0000 110", rl(6, 1)),
    ("0000 100", rl(7, 1)),
    ("1110 0", rl(0, 4)),
    ("0000 111", rl(2, 2)),
    ("0000 101", rl(8, 1)),
    ("1111 000", rl(9, 1)),
    ("1110 1", rl(0, 5)),
    ("0001 01", rl(0, 6)),
    ("1111 001", rl(1, 3)),
    ("0010 0110", rl(3, 2)),
    ("1111 010", rl(10, 1)),
    ("0010 0001", rl(11, 1)),
    ("0010 0101", rl(12, 1)),
    ("0010 0100", rl(13, 1)),
    ("0001 00", rl(0, 7)),
    ("0010 0111", rl(1, 4)),
    ("1111 1100", rl(2, 3)),
    ("1111 1101", rl(4, 2)),
    ("0000 0010 0", rl(5, 2)),
    ("0000 0010 1", rl(14, 1)),
    ("0000 0011 1", rl(15, 1)),
    ("0000 0011 01", rl(16, 1)),
    ("1111 011", rl(0, 8)),
    ("1111 100", rl(0, 9)),
    ("0010 0011", rl(0, 10)),
    ("0010 0010", rl(0, 11)),
    ("0010 0000", rl(1, 5)),
    ("0000 0011 00", rl(2, 4)),
    ("1111 1010", rl(0, 12)),
    ("1111 1011", rl(0, 13)),
    ("1111 1110", rl(0, 14)),
    ("1111 1111", rl(0, 15)),
];

/// Table B.14 in full.
pub fn dct_table_zero() -> Vec<(&'static str, i16)> {
    DCT_ZERO_OWN.iter().chain(DCT_COMMON).copied().collect()
}

/// Table B.15 in full.
pub fn dct_table_one() -> Vec<(&'static str, i16)> {
    DCT_ONE_OWN.iter().chain(DCT_COMMON).copied().collect()
}

/// Every table, built once per decoder.
#[derive(Clone, Debug)]
pub struct Tables {
    pub mba: Vlc,
    pub mb_type: [Vlc; 3],
    pub cbp: Vlc,
    pub motion: Vlc,
    pub dmv: Vlc,
    pub dc_luma: Vlc,
    pub dc_chroma: Vlc,
    pub dct_zero: Vlc,
    pub dct_one: Vlc,
}

impl Tables {
    pub fn new() -> Tables {
        Tables {
            mba: Vlc::build(MACROBLOCK_ADDRESS_INCREMENT),
            mb_type: [Vlc::build(MB_TYPE_I), Vlc::build(MB_TYPE_P), Vlc::build(MB_TYPE_B)],
            cbp: Vlc::build(CODED_BLOCK_PATTERN),
            motion: Vlc::build(MOTION_CODE),
            dmv: Vlc::build(DMVECTOR),
            dc_luma: Vlc::build(DC_SIZE_LUMA),
            dc_chroma: Vlc::build(DC_SIZE_CHROMA),
            dct_zero: Vlc::build(&dct_table_zero()),
            dct_one: Vlc::build(&dct_table_one()),
        }
    }
}

impl Default for Tables {
    fn default() -> Self {
        Tables::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeSet;

    /// Sum of 2^-len over the codes. A code followed by a sign bit stands for two symbols of one more bit each,
    /// which adds the same amount.
    fn kraft(codes: &[(&str, i16)]) -> f64 {
        codes.iter().map(|&(c, _)| 1.0 / f64::from(1u32 << parse(c).1)).sum()
    }

    fn check_all_codes_decode(codes: &[(&str, i16)]) {
        let vlc = Vlc::build(codes);
        for &(c, v) in codes {
            let (bits, len) = parse(c);
            // Pad with alternating bits to check the decoder ignores what follows.
            let word = (bits << (32 - len)) | (0x5555_5555 >> len);
            assert_eq!(vlc.decode_word(word), Some((v, len)), "code {c}");
        }
    }

    #[test]
    fn tables_are_prefix_codes_and_round_trip() {
        let all: [(&str, &[(&str, i16)]); 10] = [
            ("mba", MACROBLOCK_ADDRESS_INCREMENT),
            ("I", MB_TYPE_I),
            ("P", MB_TYPE_P),
            ("B", MB_TYPE_B),
            ("cbp", CODED_BLOCK_PATTERN),
            ("motion", MOTION_CODE),
            ("dmv", DMVECTOR),
            ("dc luma", DC_SIZE_LUMA),
            ("dc chroma", DC_SIZE_CHROMA),
            ("common", DCT_COMMON),
        ];
        for (name, t) in all {
            check_all_codes_decode(t);
            let symbols: BTreeSet<i16> = t.iter().map(|e| e.1).collect();
            assert_eq!(symbols.len(), t.len(), "{name}: duplicate symbols");
        }
        check_all_codes_decode(&dct_table_zero());
        check_all_codes_decode(&dct_table_one());
    }

    #[test]
    fn table_sizes_and_kraft_sums() {
        assert_eq!(MACROBLOCK_ADDRESS_INCREMENT.len(), 34);
        assert_eq!(CODED_BLOCK_PATTERN.len(), 64);
        let cbps: BTreeSet<i16> = CODED_BLOCK_PATTERN.iter().map(|e| e.1).collect();
        assert_eq!(cbps, (0..64).collect());
        // Complete codes (every bit pattern decodes) except for the patterns the standard leaves unused.
        assert_eq!(kraft(MB_TYPE_I), 0.75);
        assert_eq!(kraft(MB_TYPE_P), 1.0 - 1.0 / 64.0);
        assert_eq!(kraft(MB_TYPE_B), 1.0 - 1.0 / 64.0);
        assert_eq!(kraft(CODED_BLOCK_PATTERN), 1.0 - 1.0 / 512.0);
        assert_eq!(kraft(DMVECTOR), 1.0);
        assert_eq!(kraft(DC_SIZE_LUMA), 1.0);
        assert_eq!(kraft(DC_SIZE_CHROMA), 1.0);
        let motion = kraft(MOTION_CODE);
        assert_eq!(motion, 1.0 - 3.0 / 256.0);
        // 111 (run, level) codes plus EOB and escape in each DCT table.
        let zero = dct_table_zero();
        let one = dct_table_one();
        assert_eq!(zero.len(), 113);
        assert_eq!(one.len(), 113);
        let pairs = |t: &[(&str, i16)]| -> BTreeSet<i16> { t.iter().map(|e| e.1).filter(|&v| v >= 0).collect() };
        assert_eq!(pairs(&zero), pairs(&one), "both tables code the same (run, level) pairs");
        // Table zero is complete except for the 12-zero prefix.
        assert_eq!(kraft(&zero), 1.0 - 1.0 / 4096.0);
        // Table one leaves the table-zero codes of the pairs it codes shorter unused.
        assert!(kraft(&one) < 1.0);
    }

    #[test]
    fn dct_symbol_packing() {
        for run in 0..32 {
            for level in 1..41 {
                let s = rl(run, level);
                assert_eq!((run_of(s), level_of(s)), (run, level));
            }
        }
    }
}
