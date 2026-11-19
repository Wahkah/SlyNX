//! Option lists and the option-id encoding table (object-stream.md section 4, option-encodings.md).

use crate::error::{Error, Result};
use crate::reader::Reader;

/// How an option's value is stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionEncoding {
    /// `b`: 1 byte.
    Byte,
    /// `h`: `s16`.
    Short,
    /// `f`: `f32`.
    Float,
    /// `i`: `s32`.
    Int,
    /// `c`: packed colour R, G, B, A.
    Color,
    /// `p`: two `f32`.
    Pair,
    /// `v`: three `f32`.
    Vector,
    /// `q`: four `f32`.
    Quad,
    /// `0`..`6`: a typed argument list with that many arguments.
    Args(u8),
}

impl OptionEncoding {
    /// From the one-character code of option-encodings.md (`.` and unknown characters give `None`).
    pub fn from_code(c: u8) -> Option<OptionEncoding> {
        Some(match c {
            b'b' => OptionEncoding::Byte,
            b'h' => OptionEncoding::Short,
            b'f' => OptionEncoding::Float,
            b'i' => OptionEncoding::Int,
            b'c' => OptionEncoding::Color,
            b'p' => OptionEncoding::Pair,
            b'v' => OptionEncoding::Vector,
            b'q' => OptionEncoding::Quad,
            b'0'..=b'6' => OptionEncoding::Args(c - b'0'),
            _ => return None,
        })
    }

    /// Fixed value size, or `None` for typed argument lists.
    pub fn fixed_size(self) -> Option<usize> {
        Some(match self {
            OptionEncoding::Byte => 1,
            OptionEncoding::Short => 2,
            OptionEncoding::Float | OptionEncoding::Int | OptionEncoding::Color => 4,
            OptionEncoding::Pair => 8,
            OptionEncoding::Vector => 12,
            OptionEncoding::Quad => 16,
            OptionEncoding::Args(_) => return None,
        })
    }
}

/// Number of option ids (0..1184).
pub const OPTION_ID_COUNT: usize = 1184;

/// The encoding grid of option-encodings.md section 2: 64 codes per line, in four groups of 16. Only the 653 ids that
/// occur on the USA disc have a code; `.` marks an id that does not occur (corrupt input).
const GRID: [&str; 19] = [
    "..0...........22 22222vvhf....... .........ffbhfff f...bhbhfvf.bhif",
    ".fvf.f...hh.hfvp vp......3..hfb.c .hvvfhffffp..fhh hhhh...b...b.b..",
    "hvbhhfffb.bbbb.h .h1.b........bbh f4.h..h.bbbbbf.. ..hbbifvp....b..",
    "...........v.... ..bbhhhhbf.pffhh hbbi..bhfiv.hfff hbfhh.hff.bhhh.h",
    ".fh.bb.b4..h.f.b bbbh4b.h.b...h.. bhh......4.hh.h. ffh......hf.....",
    ".iifhiifhiifhiff fhfhhfbfhhb..bhp ffp.bhhhhh....h. vvhhv.vphpfvffv.",
    "piffppvhh.p.h.hv .v.fbhhhfhpb6b.. .fff.h.vvh.hv.vp hpfpifp.phhhfbvf",
    "f..fpvhh.pvh.hv. v.fbh.hfhpb.bhff f..hffffb..hvfff ff..bbpff.......",
    ".b..........bfff cffffffc.ff..... .....hpp2....... ............b...",
    "....1........... ..........hvvv.. bhh...fbhfbh.h.f .ffh...f.hbhh.fb",
    "f..vhhh.....hppf 2..f422.2ib.p.h. .bfvh.iv.hhffb.b .pbfbbh.ff...h.2",
    "bbhhhfffffhhb.fp f..b.hhffffb.... ..........hhhh3. ffffff.....ffff.",
    "vbvhhhhhffvffvpp vppvffvff.vf.... ihivf.i..v...... h...h..f..b.1ff.",
    "fff..hf..ffh.p.h ...fv.v.v.ffffff fvbhbbib.ff.fbfb .vp.hffh......h.",
    "h.hffh.ff..4bh.. ......h.......pf .h..vhvvpvp.h... ..h..h..........",
    ".......h.b.002.. h.b...b.bp..c.ff fv.ffh..h.i0.vff fvvpp...........",
    ".....bfbbfffb... bh.hppi2hhb....f ...h.f.f.f....ff .ff.i...ff......",
    "....hi...f.vpvp. .....ff.f.....11 1112433321b0.ccc fffh........i.i.",
    ".........332222. ................ ................ ................",
];

/// Maps option ids to their value encodings.
#[derive(Clone, Debug)]
pub struct OptionTable {
    codes: [u8; OPTION_ID_COUNT],
}

impl Default for OptionTable {
    fn default() -> Self {
        Self::sly1()
    }
}

impl OptionTable {
    /// The Sly 1 table of option-encodings.md.
    pub fn sly1() -> Self {
        let mut codes = [b'.'; OPTION_ID_COUNT];
        let mut id = 0;
        for c in GRID.iter().flat_map(|line| line.bytes()).filter(|&c| c != b' ') {
            // The grid's last line runs past id 1183 with '-' entries.
            if let Some(slot) = codes.get_mut(id) {
                *slot = c;
            }
            id += 1;
        }
        debug_assert_eq!(id, GRID.len() * 64);
        OptionTable { codes }
    }

    /// An empty table (every id invalid), for building custom tables in tests.
    pub fn empty() -> Self {
        OptionTable { codes: [b'.'; OPTION_ID_COUNT] }
    }

    /// Sets the code of `id`.
    pub fn set(&mut self, id: u16, code: u8) {
        if let Some(c) = self.codes.get_mut(id as usize) {
            *c = code;
        }
    }

    /// The encoding of `id`, if valid.
    pub fn get(&self, id: i16) -> Option<OptionEncoding> {
        let c = *self.codes.get(usize::try_from(id).ok()?)?;
        OptionEncoding::from_code(c)
    }
}

/// Size of a typed argument's value by its tag (object-stream.md 4.3).
pub fn typed_arg_size(tag: i16) -> Option<usize> {
    Some(match tag {
        0 => 1,
        1 | 5 | 13 => 4,
        2 | 4 | 9 | 11 => 12,
        3 | 12 => 16,
        6 | 7 | 14 | 15 | 16 | 17 | 20 => 2,
        10 => 8,
        4096..=8191 => 2,
        _ => return None,
    })
}

/// Option ids whose value (the emitter origin kind) changes a later part of the same body.
pub const ORIGIN_KIND_OPTIONS: [i16; 2] = [366, 421];
/// The origin kind that adds an emitter mesh.
pub const ORIGIN_KIND_MESH: i16 = 4;

/// An option list that was stepped over.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OptionList {
    /// File offset of the first option id.
    pub start: usize,
    /// File offset just past the terminator.
    pub end: usize,
    /// Number of options (the terminator not counted).
    pub count: u32,
    /// Value of option 366 or 421 if the list sets it (the last one wins).
    pub origin_kind: Option<i16>,
}

impl OptionList {
    /// Steps over an option list at the reader's position.
    pub fn read(r: &mut Reader<'_>, table: &OptionTable) -> Result<OptionList> {
        let start = r.pos();
        let mut list = OptionList { start, ..Default::default() };
        loop {
            let at = r.pos();
            let id = r.i16()?;
            if id < 0 {
                break;
            }
            let enc = table.get(id).ok_or(Error::UnknownOption { offset: at, id })?;
            match enc {
                OptionEncoding::Short if ORIGIN_KIND_OPTIONS.contains(&id) => {
                    list.origin_kind = Some(r.i16()?);
                }
                OptionEncoding::Args(n) => {
                    for _ in 0..n {
                        let tag_at = r.pos();
                        let tag = r.i16()?;
                        let size = typed_arg_size(tag)
                            .ok_or(Error::Corrupt { offset: tag_at, what: "unknown typed-argument tag" })?;
                        r.skip(size)?;
                    }
                }
                fixed => r.skip(fixed.fixed_size().unwrap_or(0))?,
            }
            list.count += 1;
        }
        list.end = r.pos();
        Ok(list)
    }

    /// Iterates the (id, value bytes) pairs of this list again. Typed argument lists yield their raw bytes.
    pub fn iter<'a>(&self, data: &'a [u8], table: &'a OptionTable) -> OptionIter<'a> {
        OptionIter { r: Reader::at(data, self.start), table, done: false }
    }
}

/// Iterator over an option list's entries.
pub struct OptionIter<'a> {
    r: Reader<'a>,
    table: &'a OptionTable,
    done: bool,
}

impl<'a> Iterator for OptionIter<'a> {
    type Item = Result<(i16, &'a [u8])>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let step = (|| {
            let at = self.r.pos();
            let id = self.r.i16()?;
            if id < 0 {
                return Ok(None);
            }
            let enc = self.table.get(id).ok_or(Error::UnknownOption { offset: at, id })?;
            let vstart = self.r.pos();
            match enc {
                OptionEncoding::Args(n) => {
                    for _ in 0..n {
                        let tag = self.r.i16()?;
                        let size = typed_arg_size(tag)
                            .ok_or(Error::Corrupt { offset: self.r.pos() - 2, what: "unknown typed-argument tag" })?;
                        self.r.skip(size)?;
                    }
                }
                e => self.r.skip(e.fixed_size().unwrap_or(0))?,
            }
            Ok(Some((id, &self.r.data()[vstart..self.r.pos()])))
        })();
        match step {
            Ok(Some(v)) => Some(Ok(v)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::tests::W;
    use alloc::vec::Vec;

    #[test]
    fn table_matches_spec_spot_checks() {
        let t = OptionTable::sly1();
        assert_eq!(t.get(2), Some(OptionEncoding::Args(0)));
        for id in 14..=20 {
            assert_eq!(t.get(id), Some(OptionEncoding::Args(2)));
        }
        assert_eq!(t.get(21), Some(OptionEncoding::Vector));
        assert_eq!(t.get(366), Some(OptionEncoding::Short));
        assert_eq!(t.get(421), Some(OptionEncoding::Short));
        for id in [95, 528, 535, 988, 1133, 1134, 1135] {
            assert_eq!(t.get(id), Some(OptionEncoding::Color), "id {id}");
        }
        for id in 529..=534 {
            assert_eq!(t.get(id), Some(OptionEncoding::Float));
        }
        assert_eq!(t.get(96), None);
        assert_eq!(t.get(0), None);
        assert_eq!(t.get(1183), None);
        assert_eq!(t.get(1166), Some(OptionEncoding::Args(2)));
        assert_eq!(t.get(1167), None);
        // Section 3: 653 ids occur; 200 f, 166 h, 97 b, 62 v, 46 p, 25 i, 7 c, 50 typed lists.
        let mut hist = alloc::collections::BTreeMap::new();
        for id in 0..OPTION_ID_COUNT as i16 {
            if let Some(e) = t.get(id) {
                let k = match e {
                    OptionEncoding::Args(_) => b'a',
                    OptionEncoding::Byte => b'b',
                    OptionEncoding::Short => b'h',
                    OptionEncoding::Float => b'f',
                    OptionEncoding::Int => b'i',
                    OptionEncoding::Color => b'c',
                    OptionEncoding::Pair => b'p',
                    OptionEncoding::Vector => b'v',
                    OptionEncoding::Quad => b'q',
                };
                *hist.entry(k).or_insert(0) += 1;
            }
        }
        let want = [(b'a', 50), (b'b', 97), (b'c', 7), (b'f', 200), (b'h', 166), (b'i', 25), (b'p', 46), (b'v', 62)];
        assert_eq!(hist.into_iter().collect::<Vec<_>>(), want);
        assert_eq!(t.get(1184), None);
        assert_eq!(t.get(-1), None);
        assert_eq!(t.get(549), Some(OptionEncoding::Short));
        assert_eq!(t.get(550), Some(OptionEncoding::Pair));
        assert_eq!(t.get(524), Some(OptionEncoding::Byte));
    }

    #[test]
    fn steps_over_every_encoding() {
        let mut t = OptionTable::empty();
        for (id, c) in [(1u16, b'b'), (2, b'h'), (3, b'q'), (4, b'2'), (5, b'0'), (366, b'h')] {
            t.set(id, c);
        }
        let mut w = W::default();
        w.i16(1).u8(9).i16(2).i16(-5).i16(3).zeros(16);
        w.i16(4).i16(4096).i16(7).i16(10).zeros(8);
        w.i16(5).i16(366).i16(4).i16(-1).u8(0xAA);
        let mut r = Reader::new(&w.0);
        let l = OptionList::read(&mut r, &t).unwrap();
        assert_eq!(l.count, 6);
        assert_eq!(l.origin_kind, Some(4));
        assert_eq!(w.0[l.end], 0xAA);
        let entries: Vec<_> = l.iter(&w.0, &t).map(|e| e.unwrap()).collect();
        assert_eq!(entries.iter().map(|e| e.0).collect::<Vec<_>>(), [1, 2, 3, 4, 5, 366]);
        assert_eq!(entries[1].1, (-5i16).to_le_bytes());
        assert_eq!(entries[3].1.len(), 14);

        let mut bad = W::default();
        bad.i16(7).i16(-1);
        assert!(matches!(OptionList::read(&mut Reader::new(&bad.0), &t), Err(Error::UnknownOption { id: 7, .. })));
        let mut bad = W::default();
        bad.i16(4).i16(8).i16(-1);
        assert!(OptionList::read(&mut Reader::new(&bad.0), &t).is_err());
    }
}
