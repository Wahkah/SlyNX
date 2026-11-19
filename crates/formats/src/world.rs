//! The outer layout of a decompressed world file (world-file.md section 4).
//!
//! [`WorldLayout::parse`] reads everything up to the start of the object stream. The object stream has no length
//! field; [`crate::stream::ObjectStream::walk`] walks it, and [`texture_base`] then gives the start of the texture
//! payload. [`World::load`] does all of it.

use core::ops::Range;

use crate::disc::Descriptor;
use crate::error::{Error, Result};
use crate::options::{OptionList, OptionTable};
use crate::reader::Reader;
use crate::stream::ObjectStream;
use crate::textures::{MaterialTables, TexturePayload};

/// The 8-byte header (world-file.md 4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    /// Size of the texture payload covered by the material tables.
    pub texture_size: u32,
    /// Probably a build timestamp (seconds since 1970).
    pub timestamp: u32,
}

/// The script table's counts and extent (world-file.md 4.3). The scripts themselves are stepped over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptTable {
    /// File range of the whole table.
    pub range: Range<usize>,
    /// Script count.
    pub count: u32,
    /// Symbol count.
    pub symbols: u32,
    /// Extra-string count.
    pub extra: u32,
    /// File offset of each script's first node.
    pub scripts: alloc::vec::Vec<usize>,
}

/// Payload size of script list node kinds (world-file.md 4.3); kind 30 holds a nested node instead.
fn node_payload(kind: u8) -> Option<usize> {
    Some(match kind {
        20 | 21 | 27 | 28 | 29 | 32 => 4,
        22 | 24 | 26 => 12,
        23 => 64,
        25 => 8,
        30 => 0,
        0..=36 => 0,
        _ => return None,
    })
}

/// Steps over one script (a list node and everything it chains to), iteratively.
fn skip_script(r: &mut Reader<'_>, debug: bool) -> Result<()> {
    // Each node is self-delimiting once its tag is known, so a count of nodes still to read is enough.
    let mut pending: usize = 1;
    while pending > 0 {
        pending -= 1;
        let tag = r.u8()?;
        if tag == 0xFF {
            continue;
        }
        let kind = tag & 0x7F;
        if debug {
            r.skip(10)?;
        }
        let n = node_payload(kind).ok_or_else(|| r.corrupt("script node kind above 36"))?;
        r.skip(n)?;
        if kind == 30 {
            pending += 1;
        }
        if tag & 0x80 != 0 {
            pending += 1;
        }
    }
    Ok(())
}

/// Everything before the object stream, plus where the stream starts.
#[derive(Clone, Debug)]
pub struct WorldLayout {
    /// Header.
    pub header: Header,
    /// Debug flag (0 on every retail world).
    pub debug: u8,
    /// Sound bank index, or -1.
    pub sound_bank: i16,
    /// Name-table count.
    pub name_count: u32,
    /// Script table.
    pub scripts: ScriptTable,
    /// The world option list.
    pub world_options: OptionList,
    /// Range of the descriptors and string of world-file.md 4.5.
    pub descriptors: Range<usize>,
    /// The audio descriptor (empty on every USA world).
    pub audio: Option<Descriptor>,
    /// The speaker string (empty on every USA world).
    pub speakers: Range<usize>,
    /// The music track.
    pub music: Option<Descriptor>,
    /// Range of the material tables.
    pub materials_range: Range<usize>,
    /// The material tables.
    pub materials: MaterialTables,
    /// File offset of the object stream (its `u16` count).
    pub object_stream: usize,
}

impl WorldLayout {
    /// Reads the outer sections of `data` up to the object stream.
    pub fn parse(data: &[u8], options: &OptionTable) -> Result<WorldLayout> {
        let mut r = Reader::new(data);
        let header = Header { texture_size: r.u32()?, timestamp: r.u32()? };
        let debug = r.u8()?;
        let sound_bank = r.i16()?;
        let name_count = r.u32()?;
        if debug != 0 {
            for _ in 1..name_count {
                r.string()?;
            }
        }
        let start = r.pos();
        let count = r.u32()?;
        let symbols = r.u32()?;
        let extra = r.u32()?;
        let mut scripts = alloc::vec::Vec::with_capacity((count as usize).min(r.remaining()));
        for _ in 0..count {
            scripts.push(r.pos());
            skip_script(&mut r, debug != 0)?;
            if debug != 0 {
                for _ in 0..3 {
                    r.string()?;
                }
            }
        }
        if debug != 0 {
            for _ in 0..symbols as u64 + extra as u64 {
                r.string()?;
            }
        }
        let scripts = ScriptTable { range: start..r.pos(), count, symbols, extra, scripts };
        let world_options = OptionList::read(&mut r, options)?;
        let dstart = r.pos();
        let audio = Descriptor::read(&mut r)?;
        let s = r.string()?;
        let speakers = r.pos() - s.len()..r.pos();
        let music = Descriptor::read(&mut r)?;
        let descriptors = dstart..r.pos();
        let mstart = r.pos();
        let materials = MaterialTables::read(&mut r)?;
        let materials_range = mstart..r.pos();
        Ok(WorldLayout {
            header,
            debug,
            sound_bank,
            name_count,
            scripts,
            world_options,
            descriptors,
            audio,
            speakers,
            music,
            materials_range,
            materials,
            object_stream: r.pos(),
        })
    }
}

/// The texture base T for an object stream ending at `stream_end`: the zero padding to the next multiple of 16 is
/// checked, and so is `T + header word 0 <= file size` (world-file.md 4.8).
pub fn texture_base(data: &[u8], stream_end: usize, header: &Header) -> Result<usize> {
    let mut r = Reader::at(data, stream_end);
    r.align_zero(16)?;
    let t = r.pos();
    if t as u64 + header.texture_size as u64 > data.len() as u64 {
        return Err(Error::Corrupt { offset: t, what: "texture payload runs past the end of the file" });
    }
    Ok(t)
}

/// A fully parsed world: outer layout, object stream and texture base.
#[derive(Clone, Debug)]
pub struct World<'a> {
    /// The decompressed world file.
    pub data: &'a [u8],
    /// Outer layout and material tables.
    pub layout: WorldLayout,
    /// The walked object stream.
    pub objects: ObjectStream,
    /// The texture base T.
    pub texture_base: usize,
}

impl<'a> World<'a> {
    /// Parses a whole decompressed world with the Sly 1 option table.
    pub fn load(data: &'a [u8]) -> Result<World<'a>> {
        Self::load_with(data, &OptionTable::sly1())
    }

    /// Parses a whole decompressed world with a given option table.
    pub fn load_with(data: &'a [u8], options: &OptionTable) -> Result<World<'a>> {
        let layout = WorldLayout::parse(data, options)?;
        let objects = ObjectStream::walk(data, layout.object_stream, options)?;
        let texture_base = texture_base(data, objects.end, &layout.header)?;
        Ok(World { data, layout, objects, texture_base })
    }

    /// The texture payload (from T to the end of the file).
    pub fn payload(&self) -> TexturePayload<'a> {
        TexturePayload { bytes: &self.data[self.texture_base..] }
    }

    /// Bytes after the texture payload that nothing points at.
    pub fn tail_bytes(&self) -> usize {
        self.data.len() - self.texture_base - self.layout.header.texture_size as usize
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::geometry::tests::W;

    /// Builds the outer sections of a synthetic world with an empty material table, returning the bytes so far.
    pub(crate) fn outer(texture_size: u32) -> W {
        let mut w = W::default();
        w.u32(texture_size).u32(0x3D68_0000).u8(0).i16(1).u32(5);
        // Script table: 2 scripts, 3 symbols, 0 extra.
        w.u32(2).u32(3).u32(0);
        // Script 1: kind 30 (nested) with "next" bit, nested: kind 20 int, then next: kind 23 matrix.
        w.u8(0x80 | 30).u8(20).u32(7).u8(23).zeros(64);
        // Script 2: empty.
        w.u8(0xFF);
        // World options: 528 colour, 530 float.
        w.i16(528).u32(0x8080_8080).i16(530).f32(70.).i16(-1);
        // Audio descriptor (empty), empty string, music descriptor (non-empty).
        w.zeros(32).u16(0);
        for v in [0x10u32, 0x20, 0x30, 0, 0, 0x11, 0x33, 0x22] {
            w.u32(v);
        }
        // Material tables: no palettes, no bitmaps, no shaders, no fonts.
        w.u16(0).u16(0).u16(0).u16(0).u16(0);
        w
    }

    #[test]
    fn parses_outer_layout() {
        let w = outer(0);
        let layout = WorldLayout::parse(&w.0, &OptionTable::sly1()).unwrap();
        assert_eq!(layout.header.texture_size, 0);
        assert_eq!((layout.debug, layout.sound_bank, layout.name_count), (0, 1, 5));
        assert_eq!(layout.scripts.range, 0xF..0xF + 12 + 1 + 1 + 4 + 1 + 64 + 1);
        assert_eq!(layout.scripts.scripts.len(), 2);
        assert_eq!(layout.world_options.count, 2);
        assert_eq!(layout.audio, None);
        let m = layout.music.unwrap();
        assert_eq!((m.sector, m.size, m.identity), (0x01, 0x02, 0x03));
        assert_eq!(layout.object_stream, w.0.len());
    }

    #[test]
    fn script_kinds_above_36_are_corrupt() {
        let mut w = W::default();
        w.u8(37);
        assert!(skip_script(&mut Reader::new(&w.0), false).is_err());
    }

    #[test]
    fn texture_base_aligns_and_checks_size() {
        let mut d = alloc::vec![0u8; 40];
        let h = Header { texture_size: 8, timestamp: 0 };
        assert_eq!(texture_base(&d, 17, &h).unwrap(), 32);
        assert_eq!(texture_base(&d, 32, &h).unwrap(), 32);
        assert!(texture_base(&d, 33, &h).is_err());
        d[20] = 1;
        assert!(texture_base(&d, 17, &h).is_err());
    }
}
