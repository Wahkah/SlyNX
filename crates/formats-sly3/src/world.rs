//! The outer layout of a decompressed Sly 3 world (world-file.md): header sections, then one body per group.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use formats::error::Result;
use formats::reader::Reader;
use formats_sly2::exe::OptionTable;
use formats_sly2::record::FileRecord;
pub use formats_sly2::world::TreeNode;

use crate::anim::DEFAULT_MULTIPLIERS;
use crate::filter::Context;
use crate::options::{self, OptionList, SET_COPY_COUNT};
use crate::stream::{BodyError, Record, Walk};
use crate::textures::MaterialTables;

/// Group names start with these four bytes.
pub const GROUP_PREFIX: &[u8] = b"FK$X";
/// Bytes per group-list entry.
pub const GROUP_ENTRY_SIZE: usize = 88;

/// A key-table entry (world-file.md 3.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEntry {
    pub key: i32,
    pub value: i8,
    pub list: Vec<i32>,
}

/// A group-list entry (world-file.md 3.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupEntry {
    /// The name without its `FK$X` prefix.
    pub name: String,
    pub shared: i32,
    pub streaming: i32,
    pub id: i32,
    pub second: i32,
    pub copies: i32,
    pub body_size: u32,
}

/// A string-table entry (world-file.md 3.5): its id and one string per language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WideString {
    pub id: i16,
    pub text: Vec<String>,
}

/// The option sets (world-file.md 3.7).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OptionSets {
    pub table_a: Vec<(u32, u32)>,
    pub table_b: Vec<(Vec<u16>, u32, u32)>,
    pub ids: Vec<i32>,
    pub sets: Vec<OptionList>,
}

/// A partition-tree set (world-file.md 3.8).
#[derive(Clone, Debug, PartialEq)]
pub struct PartitionSet {
    pub condition: i32,
    pub nodes: Vec<TreeNode>,
}

/// Start offsets of the header sections, in file order (world-file.md 7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sections {
    pub keys: usize,
    pub groups: usize,
    pub save_variables: usize,
    pub sound: usize,
    pub strings: usize,
    pub options: usize,
    pub option_sets: usize,
    pub partitions: usize,
    pub partitions_end: usize,
    pub first_body: usize,
}

/// One group body (world-file.md 4).
#[derive(Clone, Debug, PartialEq)]
pub struct GroupBody {
    pub offset: usize,
    pub end: usize,
    pub stamp: i32,
    pub debug: i32,
    /// Script modules in the script block.
    pub scripts: u32,
    pub materials: MaterialTables,
    /// The texture-data block: (absolute offset, size).
    pub texture_data: (usize, usize),
    /// Graph-block entry count (0 when absent).
    pub graph_entries: u32,
    pub objects: Vec<Record>,
}

impl GroupBody {
    pub fn texture_bytes<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        &data[self.texture_data.0..self.texture_data.0 + self.texture_data.1]
    }
}

/// A parsed world.
pub struct World<'a> {
    pub data: &'a [u8],
    pub keys: [Vec<KeyEntry>; 2],
    pub groups: Vec<GroupEntry>,
    pub sections: Sections,
    /// Save-variable names: (id, offset, length).
    pub save_variables: Vec<(u16, u32, u32)>,
    pub languages: Vec<u8>,
    pub strings: Vec<WideString>,
    /// Per language, the characters its strings use.
    pub characters: Vec<String>,
    pub options: OptionList,
    pub option_sets: OptionSets,
    pub partitions: Vec<PartitionSet>,
    pub bodies: Vec<GroupBody>,
    /// Bytes after the last group body.
    pub tail: usize,
    /// Bodies that did not parse field-exactly.
    pub errors: Vec<BodyError>,
    /// Object bodies read at every depth (effect components not included).
    pub object_bodies: u64,
    /// Effect component bodies read.
    pub component_bodies: u64,
}

fn file_record_skip(r: &mut Reader<'_>) -> Result<()> {
    r.skip(FileRecord::SIZE)
}

/// Steps over the sound tables (Sly 2's layout, world-file.md 3.4).
fn sound_tables(r: &mut Reader<'_>) -> Result<()> {
    file_record_skip(r)?;
    let b = r.i16()?.max(0) as usize;
    r.skip(b)?;
    r.skip(2)?;
    r.skip(b * 2)?;
    let m = r.i16()?;
    if m > 0 {
        r.skip(FileRecord::SIZE + m as usize * 2)?;
    }
    let tracks = r.u8()? as usize;
    r.skip(tracks)?;
    let e = r.i32()?;
    for _ in 0..e.max(0) {
        r.skip(4)?;
        r.skip(tracks * (FileRecord::SIZE + 1 + 4))?;
        let k = r.i32()?.max(0) as usize;
        r.skip(k * 8)?;
    }
    let records = r.i32()?;
    r.skip(records.max(0) as usize * (2 + FileRecord::SIZE))?;
    Ok(())
}

fn wide_string(r: &mut Reader<'_>) -> Result<String> {
    let n = r.u16()? as usize;
    let units = (0..n).map(|_| r.u16()).collect::<Result<Vec<_>>>()?;
    Ok(char::decode_utf16(units).map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER)).collect())
}

/// Script nodes (world-file.md 4.1): steps over one node chain.
fn script_node(r: &mut Reader<'_>, depth: u32) -> Result<()> {
    if depth > 64 {
        return Err(r.corrupt("script nodes nested too deeply"));
    }
    loop {
        let tag = r.u8()?;
        if tag == 0xFF {
            return Ok(());
        }
        match tag & 0x7F {
            19..=23 | 31 => r.skip(4)?,
            24 | 26 | 28 => r.skip(12)?,
            25 => r.skip(64)?,
            27 => r.skip(8)?,
            29 => script_node(r, depth + 1)?,
            _ => {}
        }
        if tag & 0x80 == 0 {
            return Ok(());
        }
    }
}

fn key_table(r: &mut Reader<'_>) -> Result<Vec<KeyEntry>> {
    let n = r.u8()?;
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let key = r.i32()?;
        let value = r.i8()?;
        let c = r.u8()?;
        let list = (0..c).map(|_| r.i32()).collect::<Result<Vec<_>>>()?;
        out.push(KeyEntry { key, value, list });
    }
    Ok(out)
}

/// Reads the group list (world-file.md 3.2) at the reader's position.
fn group_list(r: &mut Reader<'_>) -> Result<Vec<GroupEntry>> {
    let n = r.i16()?;
    if n <= 0 {
        return Err(r.corrupt("group count"));
    }
    let mut groups = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let name = r.bytes(64)?;
        let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(64)];
        let name = name.strip_prefix(GROUP_PREFIX).ok_or_else(|| r.corrupt("group name without FK$X"))?;
        groups.push(GroupEntry {
            name: name.iter().map(|&b| b as char).collect(),
            shared: r.i32()?,
            streaming: r.i32()?,
            id: r.i32()?,
            second: r.i32()?,
            copies: r.i32()?,
            body_size: r.u32()?,
        });
    }
    Ok(groups)
}

impl<'a> World<'a> {
    /// Parses a decompressed world with the executable's option table and the default tangent multipliers.
    pub fn parse(data: &'a [u8], table: &OptionTable) -> Result<World<'a>> {
        Self::parse_with(data, table, DEFAULT_MULTIPLIERS)
    }

    /// Parses a decompressed world with the executable's option table and tangent multipliers.
    pub fn parse_with(data: &'a [u8], table: &OptionTable, multipliers: [f32; 3]) -> Result<World<'a>> {
        let mut r = Reader::new(data);
        let mut s = Sections::default();
        let keys = [key_table(&mut r)?, key_table(&mut r)?];
        s.groups = r.pos();
        let groups = group_list(&mut r)?;
        s.save_variables = r.pos();
        let n = r.u16()?;
        let mut save_variables = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let id = r.u16()?;
            let len = r.u16()? as u32;
            save_variables.push((id, r.pos() as u32, len));
            r.skip(len as usize)?;
        }
        let m = r.u16()? as usize;
        r.skip(m * 10)?;
        s.sound = r.pos();
        sound_tables(&mut r)?;
        s.strings = r.pos();
        let l = r.u8()? as usize;
        let languages = r.bytes(l)?.to_vec();
        let count = r.i16()?.max(0) as usize;
        let mut strings = Vec::with_capacity(count);
        for _ in 0..count {
            let id = r.i16()?;
            let text = (0..l).map(|_| wide_string(&mut r)).collect::<Result<Vec<_>>>()?;
            strings.push(WideString { id, text });
        }
        let characters = (0..l).map(|_| wide_string(&mut r)).collect::<Result<Vec<_>>>()?;
        s.options = r.pos();
        let options = options::read(&mut r, table)?;
        s.option_sets = r.pos();
        let option_sets = Self::option_sets(&mut r, table)?;
        s.partitions = r.pos();
        let mut partitions = Vec::new();
        for _ in 0..r.u8()? {
            let condition = r.i32()?;
            let n = r.u16()?;
            let mut nodes = Vec::with_capacity(n as usize);
            for _ in 0..n {
                nodes.push(TreeNode { normal: r.vec3()?, distance: r.f32()?, front: r.i16()?, back: r.i16()? });
            }
            partitions.push(PartitionSet { condition, nodes });
        }
        s.partitions_end = r.pos();
        r.align_zero(16)?;
        s.first_body = r.pos();

        let streaming: BTreeMap<i32, bool> = groups.iter().map(|g| (g.id, g.streaming != 0)).collect();
        let mut walk = Walk::new(data, table, &streaming, multipliers);
        let mut bodies = Vec::with_capacity(groups.len());
        for g in &groups {
            let start = r.pos();
            let end = start + g.body_size as usize;
            if end > data.len() {
                return Err(r.corrupt("group body runs past the end of the world"));
            }
            let mut b = Reader::at(&data[..end], start);
            let body = Self::group_body(&mut b, &mut walk)?;
            if b.pos() != end {
                return Err(formats::error::Error::Corrupt {
                    offset: b.pos(),
                    what: "group body does not end at its stated size",
                });
            }
            bodies.push(body);
            r.set_pos(end);
        }
        let tail = data.len() - r.pos();
        Ok(World {
            data,
            keys,
            groups,
            sections: s,
            save_variables,
            languages,
            strings,
            characters,
            options,
            option_sets,
            partitions,
            bodies,
            tail,
            errors: walk.errors,
            object_bodies: walk.bodies,
            component_bodies: walk.components,
        })
    }

    fn option_sets(r: &mut Reader<'_>, table: &OptionTable) -> Result<OptionSets> {
        let mut o = OptionSets::default();
        for _ in 0..r.u8()? {
            o.table_a.push((r.u32()?, r.u32()?));
        }
        for _ in 0..r.u8()? {
            let n = r.u16()?;
            let list = (0..n).map(|_| r.u16()).collect::<Result<Vec<_>>>()?;
            o.table_b.push((list, r.u32()?, r.u32()?));
        }
        let n = r.u8()?;
        o.ids = (0..n).map(|_| r.i32()).collect::<Result<Vec<_>>>()?;
        for _ in 0..n {
            let size = r.i32()?;
            let start = r.pos();
            let list = options::read(r, table)?;
            if size < 0 || r.pos() - start != size as usize {
                return Err(formats::error::Error::Corrupt {
                    offset: start,
                    what: "option set does not end at its size",
                });
            }
            o.sets.push(list);
        }
        Ok(o)
    }

    fn group_body(r: &mut Reader<'a>, walk: &mut Walk<'a, '_>) -> Result<GroupBody> {
        let offset = r.pos();
        let g = r.i32()?;
        if g < 0 {
            return Err(r.corrupt("negative texture-data size"));
        }
        let stamp = r.i32()?;
        let debug = r.i32()?;
        if debug != 0 {
            return Err(r.corrupt("group debug flag set (not supported)"));
        }
        let scripts = r.u32()?;
        for _ in 0..scripts {
            script_node(r, 0)?;
        }
        let materials = MaterialTables::read(r)?;
        r.align_zero(16)?;
        let texture_data = (r.pos(), g as usize);
        r.skip(g as usize)?;
        let a = r.u32()?;
        if a != 0 {
            let b = r.u32()? as usize;
            let c = r.u32()? as usize;
            r.skip(a as usize * 6 + b * 16 + c * 4)?;
        }
        let objects = walk.object_list(r)?;
        r.align_zero(16)?;
        Ok(GroupBody {
            offset,
            end: r.pos(),
            stamp,
            debug,
            scripts,
            materials,
            texture_data,
            graph_entries: a,
            objects,
        })
    }

    /// The group named `name`.
    pub fn group(&self, name: &str) -> Option<usize> {
        self.groups.iter().position(|g| g.name == name)
    }

    /// The group whose id is `id`.
    pub fn group_by_id(&self, id: i32) -> Option<usize> {
        self.groups.iter().position(|g| g.id == id)
    }

    /// The string with id `id` (language 0).
    pub fn string(&self, id: i16) -> Option<&str> {
        self.strings.iter().find(|s| s.id == id).and_then(|s| s.text.first()).map(String::as_str)
    }

    /// The save-variable name of entry `i` (raw bytes).
    pub fn save_variable(&self, i: usize) -> Option<(u16, &'a [u8])> {
        let &(id, at, len) = self.save_variables.get(i)?;
        Some((id, &self.data[at as usize..(at + len) as usize]))
    }

    /// The selection a fresh load uses (world-file.md 3.7): the current world slot and set 0's id as the condition.
    pub fn fresh_context(&self, slot: Option<i16>) -> Context {
        Context { slot, condition: self.option_sets.ids.first().copied().unwrap_or(-1), extra: Vec::new() }
    }

    /// Copy counts after option set `set` (option 1032: group id, count).
    pub fn copy_counts(&self, set: usize) -> BTreeMap<i32, i32> {
        let mut out: BTreeMap<i32, i32> = self.groups.iter().map(|g| (g.id, g.copies)).collect();
        if let Some(list) = self.option_sets.sets.get(set) {
            for o in list.options.iter().filter(|o| o.id == SET_COPY_COUNT) {
                let (at, len) = (o.value.0 as usize, o.value.1 as usize);
                if let Some(v) = typed_ints(&self.data[at..at + len])
                    && let [group, count, ..] = v[..]
                {
                    out.insert(group, count);
                }
            }
        }
        out
    }

    /// The level group: the shared group named after the world, else the largest shared group.
    pub fn level_group(&self, name: &str) -> Option<usize> {
        self.group(name).or_else(|| {
            (0..self.groups.len()).filter(|&i| self.groups[i].shared != 0).max_by_key(|&i| self.groups[i].body_size)
        })
    }

    /// Error if any body failed to parse.
    pub fn check(&self) -> Result<()> {
        match self.errors.first() {
            None => Ok(()),
            Some(e) => Err(e.error.clone()),
        }
    }
}

/// The integer arguments of a typed argument list (1-, 2- and 4-byte values; lists expanded).
pub fn typed_ints(mut v: &[u8]) -> Option<Vec<i32>> {
    let mut out = Vec::new();
    while v.len() >= 2 {
        let tag = u16::from_le_bytes([v[0], v[1]]);
        v = &v[2..];
        let size = options::arg_size(tag & 0x7FFF)?;
        let count = if tag & 0x8000 != 0 {
            let c = *v.first()? as usize;
            v = &v[1..];
            c
        } else {
            1
        };
        for _ in 0..count {
            let b = v.get(..size)?;
            out.push(match size {
                1 => b[0] as i8 as i32,
                2 => i16::from_le_bytes([b[0], b[1]]) as i32,
                4 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                _ => 0,
            });
            v = &v[size..];
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_nodes_renumbered() {
        // Kind 19 (4 bytes), more; kind 29 holding kind 25 (64 bytes); then the empty node.
        let mut d = alloc::vec![0x80 | 19, 1, 2, 3, 4, 0x80 | 29, 25];
        d.extend_from_slice(&[0u8; 64]);
        d.push(0xFF);
        let mut r = Reader::new(&d);
        script_node(&mut r, 0).unwrap();
        assert_eq!(r.pos(), d.len());
    }

    #[test]
    fn key_tables_vector() {
        // world-file.md 7: the v_apt key tables.
        let hex = "02eb0800 00010106 00000069 09000002 00061307 00000200 ec080000 01010600 00001207 00000101 06000000 f8080000 01010600 00000009 00000101 06000000 10090000 01010600 0000";
        let hex: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
        let d: Vec<u8> = (0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap()).collect();
        assert_eq!(d.len(), 74);
        let mut r = Reader::new(&d);
        let a = key_table(&mut r).unwrap();
        let b = key_table(&mut r).unwrap();
        assert_eq!(r.pos(), 74);
        assert_eq!(a[0], KeyEntry { key: 2283, value: 1, list: alloc::vec![6] });
        assert_eq!(a[1], KeyEntry { key: 2409, value: 2, list: alloc::vec![] });
        assert_eq!((b[0].key, b[1].key, b[2].key), (1811, 2284, 1810));
    }

    #[test]
    fn typed_arguments() {
        let mut v = alloc::vec![];
        v.extend_from_slice(&6u16.to_le_bytes());
        v.extend_from_slice(&149054i32.to_le_bytes());
        v.extend_from_slice(&0x8007u16.to_le_bytes());
        v.push(2);
        v.extend_from_slice(&[3, 0, 0xFF, 0xFF]);
        assert_eq!(typed_ints(&v), Some(alloc::vec![149054, 3, -1]));
    }
}
