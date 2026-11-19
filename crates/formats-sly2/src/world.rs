//! The outer layout of a decompressed world (world-file.md): header sections, then one body per package.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use formats::error::Result;
use formats::reader::Reader;

use crate::exe::OptionTable;
use crate::options::OptionList;
use crate::record::FileRecord;
use crate::stream::{BodyError, Record, Walk};
use crate::textures::MaterialTables;

/// Package names start with these four bytes (world-file.md 3.1).
pub const PACKAGE_PREFIX: &[u8] = b"FK$X";

/// A package entry (84 bytes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageEntry {
    /// The name without its `FK$X` prefix.
    pub name: String,
    pub keep: i32,
    pub shared: i32,
    pub set_id: i32,
    pub second_id: i32,
    pub copies: i32,
}

/// A level-table row (disc.md 6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelRow {
    pub file: FileRecord,
    pub episode: u8,
    pub slot: u8,
    pub hub: bool,
    pub save_path: Vec<u16>,
}

impl LevelRow {
    /// The world id (the file record's identity).
    pub fn world_id(&self) -> u32 {
        self.file.identity
    }
}

/// A partition-tree node (world-file.md 3.6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreeNode {
    pub normal: [f32; 3],
    pub distance: f32,
    pub front: i16,
    pub back: i16,
}

/// Start offsets of the header sections, in file order (world-file.md 6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sections {
    pub sound: usize,
    pub save_variables: usize,
    pub level_table: usize,
    pub strings: usize,
    pub tree: usize,
    pub options: usize,
    pub options_end: usize,
    pub first_body: usize,
}

/// The header sections up to and including the level table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeaderStart {
    pub packages: Vec<PackageEntry>,
    pub level_rows: Vec<LevelRow>,
    /// The level table's second word (30 on the disc; unused by the game).
    pub level_table_word: u32,
    pub sections: Sections,
}

fn file_record(r: &mut Reader<'_>) -> Result<FileRecord> {
    Ok(FileRecord::decode(r.bytes(FileRecord::SIZE)?))
}

/// Reads the package list, sound tables, save variables and level table: everything needed to list the worlds,
/// without the option table.
pub fn read_header_start(data: &[u8]) -> Result<HeaderStart> {
    let mut r = Reader::new(data);
    let n = r.i16()?;
    if n <= 0 {
        return Err(r.corrupt("package count"));
    }
    let mut packages = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let name = r.bytes(64)?;
        let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(64)];
        let name = name.strip_prefix(PACKAGE_PREFIX).ok_or_else(|| r.corrupt("package name without FK$X"))?;
        packages.push(PackageEntry {
            name: name.iter().map(|&b| b as char).collect(),
            keep: r.i32()?,
            shared: r.i32()?,
            set_id: r.i32()?,
            second_id: r.i32()?,
            copies: r.i32()?,
        });
    }
    let mut sections = Sections { sound: r.pos(), ..Sections::default() };
    sound_tables(&mut r)?;
    sections.save_variables = r.pos();
    let n = r.u16()?;
    for _ in 0..n {
        r.skip(2)?;
        r.string()?;
    }
    let m = r.u16()? as usize;
    r.skip(m * 10)?;
    sections.level_table = r.pos();
    let rows = r.u32()?;
    if rows > 1024 {
        return Err(r.corrupt("level-table row count"));
    }
    let level_table_word = r.u32()?;
    let mut level_rows = Vec::with_capacity(rows as usize);
    for _ in 0..rows {
        let file = file_record(&mut r)?;
        let episode = r.u8()?;
        let slot = r.u8()?;
        let hub = r.u8()? != 0;
        let n = r.u8()?;
        let save_path = (0..n).map(|_| r.u16()).collect::<Result<Vec<_>>>()?;
        level_rows.push(LevelRow { file, episode, slot, hub, save_path });
    }
    sections.strings = r.pos();
    Ok(HeaderStart { packages, level_rows, level_table_word, sections })
}

/// Steps over the sound tables (world-file.md 3.2).
fn sound_tables(r: &mut Reader<'_>) -> Result<()> {
    r.skip(FileRecord::SIZE)?;
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

/// One package body (world-file.md 4).
#[derive(Clone, Debug, PartialEq)]
pub struct PackageBody {
    pub offset: usize,
    pub end: usize,
    pub stamp: i32,
    pub debug: i32,
    /// Script modules in the script block.
    pub scripts: u32,
    pub materials: MaterialTables,
    /// The texture-data block: (absolute offset, size).
    pub texture_data: (usize, usize),
    /// Navigation-graph entry count (0 when absent).
    pub graph_entries: u32,
    pub objects: Vec<Record>,
}

impl PackageBody {
    pub fn texture_bytes<'a>(&self, data: &'a [u8]) -> &'a [u8] {
        &data[self.texture_data.0..self.texture_data.0 + self.texture_data.1]
    }
}

/// A parsed world.
pub struct World<'a> {
    pub data: &'a [u8],
    pub header: HeaderStart,
    /// String count of the string table.
    pub strings: usize,
    pub tree: Vec<TreeNode>,
    pub options: [OptionList; 2],
    pub bodies: Vec<PackageBody>,
    /// Bytes after the last package body.
    pub tail: usize,
    /// Bodies that did not parse field-exactly.
    pub errors: Vec<BodyError>,
    /// Object bodies read at every depth.
    pub object_bodies: u64,
    /// Bodies the spec's rules could not decide (see `stream::Walk::undecided`).
    pub undecided: Vec<(i16, u32, &'static str)>,
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
            21..=25 | 33 => r.skip(4)?,
            26 | 28 | 30 => r.skip(12)?,
            27 => r.skip(64)?,
            29 => r.skip(8)?,
            31 => script_node(r, depth + 1)?,
            _ => {}
        }
        if tag & 0x80 == 0 {
            return Ok(());
        }
    }
}

impl<'a> World<'a> {
    /// Parses a decompressed world with the executable's option table.
    pub fn parse(data: &'a [u8], table: &OptionTable) -> Result<World<'a>> {
        let mut header = read_header_start(data)?;
        let mut r = Reader::at(data, header.sections.strings);
        let langs = r.u8()? as usize;
        r.skip(langs)?;
        let strings = r.i16()?.max(0) as usize;
        for _ in 0..strings {
            r.skip(2)?;
            for _ in 0..langs {
                r.string()?;
            }
        }
        header.sections.tree = r.pos();
        let n = r.u16()?;
        let mut tree = Vec::with_capacity(n as usize);
        for _ in 0..n {
            tree.push(TreeNode { normal: r.vec3()?, distance: r.f32()?, front: r.i16()?, back: r.i16()? });
        }
        header.sections.options = r.pos();
        let options = [OptionList::read(&mut r, table)?, OptionList::read(&mut r, table)?];
        header.sections.options_end = r.pos();
        r.align_zero(16)?;
        header.sections.first_body = r.pos();

        let shared: BTreeMap<i32, bool> = header.packages.iter().map(|p| (p.set_id, p.shared != 0)).collect();
        let mut walk = Walk::new(data, table, &shared);
        let mut bodies = Vec::with_capacity(header.packages.len());
        for _ in 0..header.packages.len() {
            bodies.push(Self::package_body(&mut r, &mut walk)?);
        }
        let tail = data.len() - r.pos();
        let (errors, object_bodies, undecided) = (walk.errors, walk.bodies, walk.undecided);
        Ok(World { data, header, strings, tree, options, bodies, tail, errors, object_bodies, undecided })
    }

    fn package_body(r: &mut Reader<'a>, walk: &mut Walk<'a, '_>) -> Result<PackageBody> {
        let offset = r.pos();
        let g = r.i32()?;
        if g < 0 {
            return Err(r.corrupt("negative texture-data size"));
        }
        let stamp = r.i32()?;
        let debug = r.i32()?;
        if debug != 0 {
            return Err(r.corrupt("package debug flag set (not supported)"));
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
        Ok(PackageBody {
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

    /// The level package (the last one).
    pub fn level_package(&self) -> (&PackageEntry, &PackageBody) {
        (self.header.packages.last().unwrap(), self.bodies.last().unwrap())
    }

    /// The package whose set ID is `set_id`.
    pub fn package_by_set(&self, set_id: i32) -> Option<usize> {
        self.header.packages.iter().position(|p| p.set_id == set_id)
    }

    /// Error if any body failed to parse.
    pub fn check(&self) -> Result<()> {
        match self.errors.first() {
            None => Ok(()),
            Some(e) => Err(e.error.clone()),
        }
    }
}

/// Partition-tree leaf regions: negative children numbered 0, 1, 2, ... in node order, front before back
/// (world-file.md 3.6). Returns `(node, is_back, region)`.
pub fn leaf_regions(tree: &[TreeNode]) -> Vec<(usize, bool, usize)> {
    let mut out = Vec::new();
    for (i, n) in tree.iter().enumerate() {
        for (back, child) in [(false, n.front), (true, n.back)] {
            if child < 0 {
                let region = out.len();
                out.push((i, back, region));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::error::Error;

    #[test]
    fn script_nodes() {
        // Kind 21 (4 bytes) followed (bit 7) by kind 31 holding a nested kind 27 (64 bytes), then the empty node.
        let mut d = alloc::vec![0x80 | 21, 1, 2, 3, 4, 0x80 | 31, 27];
        d.extend_from_slice(&[0u8; 64]);
        d.push(0xFF);
        let mut r = Reader::new(&d);
        script_node(&mut r, 0).unwrap();
        assert_eq!(r.pos(), d.len());
        assert_eq!(Error::Eof { offset: 0, needed: 1, len: 0 }, script_node(&mut Reader::new(&[]), 0).unwrap_err());
    }

    #[test]
    fn regions() {
        let n = |front, back| TreeNode { normal: [0.0, 1.0, 0.0], distance: 0.0, front, back };
        assert_eq!(leaf_regions(&[n(1, -1), n(-1, -1)]), [(0, true, 0), (1, false, 1), (1, true, 2)]);
    }
}
