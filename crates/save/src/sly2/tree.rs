//! The save-variable section of a Sly 2 world (saving.md 1): the names, the record tree and where each value lives
//! in the progress block.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// Size of the progress block of a slot (saving.md 4: `0x5400` bytes).
pub const BLOCK_SIZE: usize = 0x5400;

/// The value kind of a record (saving.md 1.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// −1: a group with children.
    Group,
    /// 0 and 10: integers.
    Int,
    /// 2: an object ID.
    ObjectId,
    /// 3: a float.
    Float,
    /// 4: a flag; single flags take 4 bytes, arrays one bit per element.
    Flag,
    /// 9: a world ID (a level-table row).
    WorldId,
    /// A kind the game understands but the disc does not use (1, 5 to 8).
    Other(i8),
}

impl Kind {
    fn of(k: i8) -> Kind {
        match k {
            -1 => Kind::Group,
            0 | 10 => Kind::Int,
            2 => Kind::ObjectId,
            3 => Kind::Float,
            4 => Kind::Flag,
            9 => Kind::WorldId,
            k => Kind::Other(k),
        }
    }

    /// Whether the value is stored as a 32-bit integer.
    pub fn is_integer(self) -> bool {
        matches!(self, Kind::Int | Kind::ObjectId | Kind::WorldId)
    }
}

/// One record of the tree (saving.md 1.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    /// Name ID (−1 for the root and the terminator).
    pub name: i16,
    /// Value kind.
    pub kind: Kind,
    /// Element count: 0 a single value, else an array.
    pub count: u8,
    /// Byte offset of the value in the progress block (the used size, for the terminator).
    pub offset: u16,
    /// First child record (groups), 0 none.
    pub child: u16,
    /// Next sibling record, 0 none.
    pub next: u16,
}

/// A value's place in the block: a record, and the element for arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Var {
    /// Value kind.
    pub kind: Kind,
    /// Byte offset of the record's value.
    pub offset: u16,
    /// Element count (0 single).
    pub count: u8,
    /// The element, for arrays.
    pub index: Option<u8>,
}

/// The save-variable section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VarTree {
    names: BTreeMap<u16, String>,
    by_name: BTreeMap<String, u16>,
    records: Vec<Record>,
    /// Byte sum of the record table (the layout checksum, saving.md 3).
    checksum: u32,
}

fn u16_at(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*d.get(o)?, *d.get(o + 1)?]))
}

impl VarTree {
    /// Reads the section at the start of `data`. Returns the tree and the section's length.
    pub fn parse(data: &[u8]) -> Option<(VarTree, usize)> {
        let mut at = 0;
        let n = u16_at(data, at)? as usize;
        at += 2;
        let mut names = BTreeMap::new();
        let mut by_name = BTreeMap::new();
        for _ in 0..n {
            let id = u16_at(data, at)?;
            let len = u16_at(data, at + 2)? as usize;
            let bytes = data.get(at + 4..at + 4 + len)?;
            let name: String = bytes.iter().map(|&b| b as char).collect();
            by_name.entry(name.clone()).or_insert(id);
            names.insert(id, name);
            at += 4 + len;
        }
        let m = u16_at(data, at)? as usize;
        at += 2;
        let table = data.get(at..at + m * 10)?;
        let records = table
            .as_chunks::<10>()
            .0
            .iter()
            .map(|r| Record {
                name: i16::from_le_bytes([r[0], r[1]]),
                kind: Kind::of(r[2] as i8),
                count: r[3],
                offset: u16::from_le_bytes([r[4], r[5]]),
                child: u16::from_le_bytes([r[6], r[7]]),
                next: u16::from_le_bytes([r[8], r[9]]),
            })
            .collect::<Vec<_>>();
        if records.len() < 2 {
            return None;
        }
        let checksum = table.iter().fold(0u32, |s, &b| s.wrapping_add(b as u32));
        Some((VarTree { names, by_name, records, checksum }, at + m * 10))
    }

    /// The layout word: the used size of the block, held by the terminator record (saving.md 1.1).
    pub fn layout_word(&self) -> u32 {
        self.records.last().map_or(0, |r| r.offset as u32)
    }

    /// The layout checksum: the byte sum of the record table.
    pub fn checksum(&self) -> u32 {
        self.checksum
    }

    /// The records.
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// A name's ID.
    pub fn name_id(&self, name: &str) -> Option<u16> {
        self.by_name.get(name).copied()
    }

    /// The name of an ID.
    pub fn name(&self, id: u16) -> Option<&str> {
        self.names.get(&id).map(String::as_str)
    }

    /// The children of record `r`, in order. A sibling chain that loops back (a damaged file) ends after as many
    /// steps as there are records.
    pub fn children(&self, r: usize) -> impl Iterator<Item = usize> + '_ {
        let mut c = self.records.get(r).map_or(0, |x| x.child as usize);
        let mut left = self.records.len();
        core::iter::from_fn(move || {
            if c == 0 || c >= self.records.len() || left == 0 {
                return None;
            }
            left -= 1;
            let here = c;
            c = self.records[c].next as usize;
            Some(here)
        })
    }

    /// The record a path of name IDs reaches (saving.md 1.3), root first; `None` when missing.
    pub fn record(&self, path: &[u16]) -> Option<usize> {
        let mut at = 0usize;
        for &name in path {
            at = self.children(at).find(|&c| self.records[c].name == name as i16)?;
        }
        (!path.is_empty()).then_some(at)
    }

    /// The value a path reaches: names, then for an array one more element, the index.
    pub fn var(&self, path: &[u16], index: Option<usize>) -> Option<Var> {
        let r = self.records[self.record(path)?];
        if r.kind == Kind::Group {
            return None;
        }
        let index = match (r.count, index) {
            (0, None) => None,
            (n, Some(i)) if n > 0 && i < n as usize => Some(i as u8),
            _ => return None,
        };
        Some(Var { kind: r.kind, offset: r.offset, count: r.count, index })
    }

    /// [`VarTree::var`] by names.
    pub fn var_named(&self, path: &[&str], index: Option<usize>) -> Option<Var> {
        let ids: Option<Vec<u16>> = path.iter().map(|n| self.name_id(n)).collect();
        self.var(&ids?, index)
    }

    /// The top-level record named `name`.
    pub fn top(&self, name: &str) -> Option<usize> {
        self.record(&[self.name_id(name)?])
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A small section: root → (ccoin int, flags group → (lang int), owned 10-flag array, t1 group → (state int,
    /// from_memcard flag)), terminator at 40.
    pub(crate) fn sample() -> Vec<u8> {
        let names: &[(u16, &str)] = &[
            (1, "ccoin"),
            (2, "flags"),
            (3, "lang"),
            (4, "owned"),
            (5, "t1"),
            (6, "state"),
            (7, "from_memcard"),
            (8, "dt"),
        ];
        let mut d = Vec::new();
        d.extend_from_slice(&(names.len() as u16).to_le_bytes());
        for (id, n) in names {
            d.extend_from_slice(&id.to_le_bytes());
            d.extend_from_slice(&(n.len() as u16).to_le_bytes());
            d.extend_from_slice(n.as_bytes());
        }
        // (name, kind, count, offset, child, next)
        let recs: &[(i16, i8, u8, u16, u16, u16)] = &[
            (-1, -1, 0, 0, 1, 0),
            (1, 0, 0, 0, 0, 2),
            (2, -1, 0, 0, 3, 4),
            (3, 0, 0, 4, 0, 0),
            (4, 4, 10, 8, 0, 5),
            (5, -1, 0, 0, 6, 8),
            (6, 0, 0, 12, 0, 7),
            (7, 4, 0, 16, 0, 0),
            (8, 3, 0, 20, 0, 0),
            (-1, -1, 0, 40, 0, 0),
        ];
        d.extend_from_slice(&(recs.len() as u16).to_le_bytes());
        for &(n, k, c, o, ch, nx) in recs {
            d.extend_from_slice(&n.to_le_bytes());
            d.push(k as u8);
            d.push(c);
            d.extend_from_slice(&o.to_le_bytes());
            d.extend_from_slice(&ch.to_le_bytes());
            d.extend_from_slice(&nx.to_le_bytes());
        }
        d
    }

    #[test]
    fn paths() {
        let data = sample();
        let (t, len) = VarTree::parse(&data).unwrap();
        assert_eq!(len, data.len());
        assert_eq!(t.layout_word(), 40);
        assert_eq!(t.var_named(&["ccoin"], None).unwrap().offset, 0);
        assert_eq!(t.var_named(&["flags", "lang"], None).unwrap().offset, 4);
        assert_eq!(t.var_named(&["owned"], Some(9)).unwrap().index, Some(9));
        // An array needs its index, and only within its count; a group is no value; a missing name is none.
        assert!(t.var_named(&["owned"], None).is_none());
        assert!(t.var_named(&["owned"], Some(10)).is_none());
        assert!(t.var_named(&["flags"], None).is_none());
        assert!(t.var_named(&["lang"], None).is_none());
        assert_eq!(t.var_named(&["t1", "state"], None).unwrap().kind, Kind::Int);
        assert_eq!(t.children(0).count(), 5);
    }

    #[test]
    fn a_looping_sibling_chain_ends() {
        let (mut t, _) = VarTree::parse(&sample()).unwrap();
        // The last top-level record's next points back at the first: the walk stops instead of looping.
        let tops: Vec<usize> = t.children(0).collect();
        let (first, last) = (tops[0], *tops.last().unwrap());
        t.records[last].next = first as u16;
        assert!(t.children(0).count() <= t.records().len());
        assert!(t.var_named(&["missing"], None).is_none(), "a name lookup through the loop ends");
        assert_eq!(t.var_named(&["ccoin"], None).unwrap().offset, 0);
    }
}
