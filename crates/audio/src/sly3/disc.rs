//! The Sly 3 (USA) disc as far as sound needs it (sly3/disc.md, sly3/world-file.md 3): the executable's world and
//! movie tables, and walking a world's header to its sound section. File records, the sound section, effect banks
//! (version 3), `VPK ` streams, voice lines and movie soundtracks are Sly 2's ([`crate::sly2`]).

use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Range;

use formats::compress::decompress;
use formats::disc::boot_executable_name;
use formats::hash::{hex, sha256};
use formats::iso::{self, ReadAt};

use crate::Error;
use crate::sly2::disc::{Cur, FileRecord, RECORD_SIZE, elf_segments};
use crate::sly2::sound::WorldSound;

/// SHA-256 of the supported executable, `SCUS_974.64`.
pub const EXECUTABLE_SHA256: &str = "ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338";
/// Number of world slots.
pub const WORLD_SLOTS: usize = 40;
/// World name pointers (`u32` each).
pub const WORLD_NAMES: u32 = 0x002E_DE48;
/// World episodes (`s32` each).
pub const WORLD_EPISODES: u32 = 0x002E_DF10;
/// World file records (64 bytes each).
pub const WORLD_RECORDS: u32 = 0x002E_DFD8;
/// The movie table: address and record count.
pub const MOVIE_TABLE: (u32, usize) = (0x0031_79F8, 23);

/// One world slot of the executable's tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldSlot {
    /// Slot number.
    pub slot: usize,
    /// Name (for example `v_apt`).
    pub name: String,
    /// Episode: 1 menus, 2 prologue, 3..8 the six episodes.
    pub episode: i32,
    /// The compressed world file (`None` for the empty slot 1); its identity is the world id.
    pub file: Option<FileRecord>,
}

/// One group-list entry (sly3/world-file.md 3.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// Name without the `FK$X` prefix.
    pub name: String,
    /// Shared flag.
    pub shared: i32,
    /// Streaming flag.
    pub streaming: i32,
    /// Group id.
    pub id: i32,
    /// Second number.
    pub second: i32,
    /// Copy count.
    pub copies: i32,
    /// Body size in bytes.
    pub body_size: u32,
}

/// The outer sections of a decompressed Sly 3 world up to and including the sound section.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldHeader {
    /// Byte range of the two key tables.
    pub key_tables: Range<usize>,
    /// The group list; the level group is the shared group named after the world.
    pub groups: Vec<Group>,
    /// Byte range of the save variables.
    pub save_range: Range<usize>,
    /// Byte range of the sound section.
    pub sound_range: Range<usize>,
    /// The sound section (Sly 2 layout).
    pub sound: WorldSound,
    /// The string table (world-file.md 3.5), first language: (string id, text).
    pub strings: Vec<(i16, String)>,
}

impl WorldHeader {
    /// Walks a decompressed world from its start through the sound section.
    pub fn parse(world: &[u8]) -> Result<WorldHeader, Error> {
        let mut c = Cur::new(world, 0);
        for _ in 0..2 {
            let n = c.u8()?;
            for _ in 0..n {
                c.i32()?;
                c.i8()?;
                let k = c.u8()? as usize;
                c.bytes(k * 4)?;
            }
        }
        let key_tables = 0..c.pos;
        let n = c.i16()? as i64;
        let n = c.count(n, 88)?;
        let mut groups = Vec::with_capacity(n);
        for _ in 0..n {
            let e = c.bytes(88)?;
            let raw = &e[..64];
            let end = raw.iter().position(|&b| b == 0).unwrap_or(64);
            let name = raw[..end].strip_prefix(b"FK$X").unwrap_or(&raw[..end]);
            let w = |o: usize| i32::from_le_bytes([e[o], e[o + 1], e[o + 2], e[o + 3]]);
            groups.push(Group {
                name: name.iter().map(|&b| b as char).collect(),
                shared: w(64),
                streaming: w(68),
                id: w(72),
                second: w(76),
                copies: w(80),
                body_size: w(84) as u32,
            });
        }
        let save_start = c.pos;
        let n = c.u16()? as i64;
        let n = c.count(n, 4)?;
        for _ in 0..n {
            c.u16()?;
            let len = c.u16()? as usize;
            c.bytes(len)?;
        }
        let m = c.u16()? as i64;
        let m = c.count(m, 10)?;
        c.bytes(m * 10)?;
        let save_range = save_start..c.pos;
        let start = c.pos;
        let sound = WorldSound::read(&mut c)?;
        let sound_range = start..c.pos;
        // String table: u8 L, L codes, s16 count, per string s16 id and L UTF-16 strings, then L character lists.
        let langs = c.u8()? as usize;
        c.bytes(langs)?;
        let n = c.i16()? as i64;
        let n = c.count(n, 2 + 2 * langs)?;
        let mut strings = Vec::with_capacity(n);
        for _ in 0..n {
            let id = c.i16()?;
            let mut first = String::new();
            for l in 0..langs {
                let len = c.u16()? as usize;
                let b = c.bytes(len * 2)?;
                if l == 0 {
                    let units = b.as_chunks::<2>().0.iter().map(|u| u16::from_le_bytes(*u));
                    first = char::decode_utf16(units).map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER)).collect();
                }
            }
            strings.push((id, first));
        }
        Ok(WorldHeader { key_tables, groups, save_range, sound_range, sound, strings })
    }

    /// The string with id `id` (first language).
    pub fn string(&self, id: i16) -> Option<&str> {
        self.strings.iter().find(|s| s.0 == id).map(|s| s.1.as_str())
    }
}

/// An opened Sly 3 disc image.
pub struct Sly3Disc<R: ReadAt> {
    src: R,
    /// The boot executable.
    pub executable: Vec<u8>,
    /// Loadable segments: (file offset, virtual address, size in file).
    pub segments: Vec<(u32, u32, u32)>,
    /// The 40 world slots.
    pub worlds: Vec<WorldSlot>,
    /// The 23 movie records.
    pub movies: Vec<Option<FileRecord>>,
}

impl<R: ReadAt> Sly3Disc<R> {
    /// Opens a disc: reads `SYSTEM.CNF` and the executable it names, checks the executable's SHA-256, and reads
    /// the world and movie tables.
    pub fn open(mut src: R) -> Result<Self, Error> {
        let cnf = iso::find_root_file(&mut src, "SYSTEM.CNF")?;
        let cnf = iso::read_file(&mut src, &cnf)?;
        let name = boot_executable_name(&cnf)?;
        let exe = iso::find_root_file(&mut src, &name)?;
        let executable = iso::read_file(&mut src, &exe)?;
        if hex(&sha256(&executable)) != EXECUTABLE_SHA256 {
            return Err(Error::Invalid("not the supported Sly 3 executable (SHA-256 mismatch)"));
        }
        let segments = elf_segments(&executable)?;
        let mut disc = Sly3Disc { src, executable, segments, worlds: Vec::new(), movies: Vec::new() };
        let mut worlds = Vec::with_capacity(WORLD_SLOTS);
        for slot in 0..WORLD_SLOTS {
            let ptr = disc.u32_at(WORLD_NAMES + slot as u32 * 4)?;
            let episode = disc.u32_at(WORLD_EPISODES + slot as u32 * 4)? as i32;
            let rec = disc.bytes_at(WORLD_RECORDS + (slot * RECORD_SIZE) as u32, RECORD_SIZE)?;
            let file = FileRecord::decode(rec)?;
            worlds.push(WorldSlot { slot, name: disc.c_string(ptr)?, episode, file });
        }
        let (va, n) = MOVIE_TABLE;
        let table = disc.bytes_at(va, n * RECORD_SIZE)?.to_vec();
        disc.movies =
            table.as_chunks::<RECORD_SIZE>().0.iter().map(|r| FileRecord::decode(r)).collect::<Result<_, _>>()?;
        disc.worlds = worlds;
        Ok(disc)
    }

    /// Bytes of the executable at virtual address `va`.
    pub fn bytes_at(&self, va: u32, len: usize) -> Result<&[u8], Error> {
        for &(off, base, size) in &self.segments {
            if va >= base && (va - base) as usize + len <= size as usize {
                let o = (off + (va - base)) as usize;
                return self.executable.get(o..o + len).ok_or(Error::Invalid("segment past the file"));
            }
        }
        Err(Error::Invalid("address outside the executable"))
    }

    fn u32_at(&self, va: u32) -> Result<u32, Error> {
        let b = self.bytes_at(va, 4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn c_string(&self, va: u32) -> Result<String, Error> {
        let mut s = String::new();
        for i in 0..256u32 {
            let b = self.bytes_at(va + i, 1)?[0];
            if b == 0 {
                return Ok(s);
            }
            s.push(b as char);
        }
        Err(Error::Invalid("world name too long"))
    }

    /// The slot named `name`.
    pub fn find_world(&self, name: &str) -> Option<&WorldSlot> {
        self.worlds.iter().find(|w| w.name == name)
    }

    /// The raw bytes a record points at.
    pub fn read(&mut self, r: &FileRecord) -> Result<Vec<u8>, Error> {
        Ok(self.src.read_vec(r.offset(), r.size as usize)?)
    }

    /// Reads and decompresses the world of `slot`.
    pub fn read_world(&mut self, slot: usize) -> Result<Vec<u8>, Error> {
        let r = self.worlds.get(slot).and_then(|w| w.file).ok_or(Error::Invalid("empty or missing world slot"))?;
        Ok(decompress(&self.read(&r)?)?)
    }

    /// Reads and parses an effect bank (version 3).
    pub fn read_bank(&mut self, r: &FileRecord) -> Result<crate::bank::EffectBank, Error> {
        crate::bank::EffectBank::parse(self.read(r)?)
    }

    /// Reads a `VPK ` stream file and splits it into channels.
    pub fn read_vpk(&mut self, r: &FileRecord) -> Result<crate::sly2::vpk::VpkStream, Error> {
        crate::sly2::vpk::VpkStream::parse(&self.read(r)?)
    }

    /// Reads a VAG file (a voice line).
    pub fn read_vag(&mut self, r: &FileRecord) -> Result<crate::vag::VagSound, Error> {
        crate::vag::VagSound::new(self.read(r)?)
    }

    /// The underlying reader.
    pub fn source(&mut self) -> &mut R {
        &mut self.src
    }
}

#[cfg(feature = "std")]
impl Sly3Disc<std::fs::File> {
    /// Opens a disc image file.
    pub fn open_path(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        let f = std::fs::File::open(path).map_err(|e| Error::Disc(formats::error::Error::Io(alloc::format!("{e}"))))?;
        Sly3Disc::open(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sly2::sound::tests::section;
    use alloc::vec;

    #[test]
    fn walks_a_synthetic_header() {
        let mut w = vec![];
        // Key tables: one entry (key 2283, value 1, [6]), then an empty table.
        w.push(1);
        w.extend_from_slice(&2283i32.to_le_bytes());
        w.push(1);
        w.push(1);
        w.extend_from_slice(&6i32.to_le_bytes());
        w.push(0);
        // One group.
        w.extend_from_slice(&1i16.to_le_bytes());
        let mut e = vec![0u8; 88];
        e[..9].copy_from_slice(b"FK$Xv_apt");
        e[64..68].copy_from_slice(&1i32.to_le_bytes());
        e[72..76].copy_from_slice(&9900i32.to_le_bytes());
        e[80..84].copy_from_slice(&1i32.to_le_bytes());
        e[84..88].copy_from_slice(&95_552u32.to_le_bytes());
        w.extend_from_slice(&e);
        // Save variables: one name, one 10-byte record.
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&256u16.to_le_bytes());
        w.extend_from_slice(&3u16.to_le_bytes());
        w.extend_from_slice(b"abc");
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&[0u8; 10]);
        let sound_start = w.len();
        w.extend_from_slice(&section());
        let sound_end = w.len();
        // String table: one language, one string (id 7, "hi" in UTF-16).
        w.extend_from_slice(&[1, 0]);
        w.extend_from_slice(&1i16.to_le_bytes());
        w.extend_from_slice(&7i16.to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&[b'h', 0, b'i', 0]);
        let h = WorldHeader::parse(&w).unwrap();
        assert_eq!(h.string(7), Some("hi"));
        assert_eq!(h.key_tables, 0..12);
        assert_eq!(
            h.groups[0],
            Group { name: "v_apt".into(), shared: 1, streaming: 0, id: 9900, second: 0, copies: 1, body_size: 95_552 }
        );
        assert_eq!(h.save_range, 102..sound_start);
        assert_eq!(h.sound_range, sound_start..sound_end);
        assert_eq!(h.sound.bank.unwrap().identity, 58);
        assert!(WorldHeader::parse(&w[..50]).is_err());
    }
}
