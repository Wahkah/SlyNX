//! The Sly 2 (USA) disc as far as sound needs it (sly2/disc.md, sly2/world-file.md 3): file records, the
//! executable's file table, the level table, and walking a world's header to its sound section.

use alloc::string::String;
use alloc::vec::Vec;

use formats::compress::decompress;
use formats::disc::boot_executable_name;
use formats::hash::{hex, sha256};
use formats::iso::{self, ReadAt};

use crate::Error;
use crate::sly2::sound::WorldSound;

/// SHA-256 of the supported executable, `SCUS_973.16`.
pub const EXECUTABLE_SHA256: &str = "a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73";
/// The executable's file table: virtual address and record count.
pub const FILE_TABLE: (u32, usize) = (0x002A_32A8, 29);
/// Index of the boot world (the intro hub) in the file table.
pub const BOOT_WORLD_RECORD: usize = 24;
/// Sector added to every decoded start sector.
pub const BASE_SECTOR: u32 = 0xCA0;
/// Size of a file record.
pub const RECORD_SIZE: usize = 64;
/// Bytes per sector.
pub const SECTOR: u64 = 2048;

/// A decoded 64-byte file record (sly2/disc.md 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileRecord {
    /// Start sector (base included).
    pub sector: u32,
    /// Size in bytes.
    pub size: u32,
    /// Identity.
    pub identity: u32,
}

impl FileRecord {
    /// Decodes the 64 bytes at the start of `raw`; `None` for an empty record (size 0).
    pub fn decode(raw: &[u8]) -> Result<Option<FileRecord>, Error> {
        let raw = raw.get(..RECORD_SIZE).ok_or(Error::Truncated { what: "file record", needed: 64, len: raw.len() })?;
        let w = |i: usize| u32::from_le_bytes([raw[i * 4], raw[i * 4 + 1], raw[i * 4 + 2], raw[i * 4 + 3]]);
        let size = w(1) ^ w(15);
        if size == 0 {
            return Ok(None);
        }
        Ok(Some(FileRecord { sector: (w(0) ^ w(13)).wrapping_add(BASE_SECTOR), size, identity: w(2) ^ w(14) }))
    }

    /// Byte offset in the image.
    pub fn offset(&self) -> u64 {
        self.sector as u64 * SECTOR
    }
}

/// A little-endian cursor for the world header.
pub(crate) struct Cur<'a> {
    pub(crate) b: &'a [u8],
    pub(crate) pos: usize,
}

impl<'a> Cur<'a> {
    pub(crate) fn new(b: &'a [u8], pos: usize) -> Self {
        Cur { b, pos }
    }

    pub(crate) fn bytes(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.b.len()).ok_or(Error::Truncated {
            what: "world header",
            needed: self.pos.saturating_add(n),
            len: self.b.len(),
        })?;
        let s = &self.b[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.bytes(1)?[0])
    }

    pub(crate) fn i8(&mut self) -> Result<i8, Error> {
        Ok(self.u8()? as i8)
    }

    pub(crate) fn u16(&mut self) -> Result<u16, Error> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub(crate) fn i16(&mut self) -> Result<i16, Error> {
        Ok(self.u16()? as i16)
    }

    pub(crate) fn u32(&mut self) -> Result<u32, Error> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub(crate) fn i32(&mut self) -> Result<i32, Error> {
        Ok(self.u32()? as i32)
    }

    pub(crate) fn f32(&mut self) -> Result<f32, Error> {
        Ok(f32::from_bits(self.u32()?))
    }

    pub(crate) fn record(&mut self) -> Result<Option<FileRecord>, Error> {
        FileRecord::decode(self.bytes(RECORD_SIZE)?)
    }

    /// A count that must be small enough to describe items of at least `item` bytes in the rest of the data.
    pub(crate) fn count(&self, n: i64, item: usize) -> Result<usize, Error> {
        if n < 0 || (n as u64).saturating_mul(item as u64) > (self.b.len() - self.pos) as u64 {
            return Err(Error::Invalid("count larger than the data"));
        }
        Ok(n as usize)
    }
}

/// One row of the level table (sly2/disc.md 6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelRow {
    /// The compressed world file (`None` for the empty row 18).
    pub file: Option<FileRecord>,
    /// Episode 0..8.
    pub episode: u8,
    /// Slot.
    pub slot: u8,
    /// Hub flag.
    pub hub: bool,
    /// Save path symbols.
    pub save_path: Vec<u16>,
}

/// The outer sections of a decompressed world up to and including the level table.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldHeader {
    /// Package names (without the `FK$X` prefix), in order; the last is the level.
    pub packages: Vec<String>,
    /// Byte range of the sound section.
    pub sound_range: core::ops::Range<usize>,
    /// The sound section.
    pub sound: WorldSound,
    /// The level table.
    pub levels: Vec<LevelRow>,
    /// The string table (world-file.md 3.5), first language: (string id, bytes). Subtitles are the strings whose
    /// id is the line number (dialogue-and-voice.md 4).
    pub strings: Vec<(i16, Vec<u8>)>,
}

impl WorldHeader {
    /// Walks a decompressed world from its start through the level table.
    pub fn parse(world: &[u8]) -> Result<WorldHeader, Error> {
        let mut c = Cur::new(world, 0);
        let n = c.i16()? as i64;
        let n = c.count(n, 84)?;
        let mut packages = Vec::with_capacity(n);
        for _ in 0..n {
            let e = c.bytes(84)?;
            let name = &e[..64];
            let end = name.iter().position(|&b| b == 0).unwrap_or(64);
            let name = name[..end].strip_prefix(b"FK$X").unwrap_or(&name[..end]);
            packages.push(name.iter().map(|&b| b as char).collect());
        }
        let start = c.pos;
        let sound = WorldSound::read(&mut c)?;
        let sound_range = start..c.pos;
        // Save variables: u16 n, n x (u16 id, string); u16 m, m x 10 bytes.
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
        // Level table.
        let rows = c.u32()? as i64;
        let rows = c.count(rows, 68)?;
        c.u32()?;
        let mut levels = Vec::with_capacity(rows);
        for _ in 0..rows {
            let file = c.record()?;
            let episode = c.u8()?;
            let slot = c.u8()?;
            let hub = c.u8()? != 0;
            let k = c.u8()? as usize;
            let mut save_path = Vec::with_capacity(k);
            for _ in 0..k {
                save_path.push(c.u16()?);
            }
            levels.push(LevelRow { file, episode, slot, hub, save_path });
        }
        // String table: u8 language count L, L codes, s16 count, per string s16 id and L strings.
        let langs = c.u8()? as usize;
        c.bytes(langs)?;
        let n = c.i16()? as i64;
        let n = c.count(n, 2 + 2 * langs)?;
        let mut strings = Vec::with_capacity(n);
        for _ in 0..n {
            let id = c.i16()?;
            let mut first = Vec::new();
            for l in 0..langs {
                let len = c.u16()? as usize;
                let b = c.bytes(len)?;
                if l == 0 {
                    first = b.to_vec();
                }
            }
            strings.push((id, first));
        }
        Ok(WorldHeader { packages, sound_range, sound, levels, strings })
    }

    /// The string with id `id` (first language), as stored.
    pub fn string(&self, id: i16) -> Option<&[u8]> {
        self.strings.iter().find(|s| s.0 == id).map(|s| s.1.as_slice())
    }

    /// The level package's name.
    pub fn level_name(&self) -> &str {
        self.packages.last().map_or("", |s| s.as_str())
    }
}

/// An opened Sly 2 disc image.
pub struct Sly2Disc<R: ReadAt> {
    src: R,
    /// The boot executable.
    pub executable: Vec<u8>,
    /// Loadable segments: (file offset, virtual address, size in file).
    pub segments: Vec<(u32, u32, u32)>,
    /// The executable's file table.
    pub files: Vec<Option<FileRecord>>,
    /// The level table (from the boot world).
    pub levels: Vec<LevelRow>,
}

pub(crate) fn elf_segments(exe: &[u8]) -> Result<Vec<(u32, u32, u32)>, Error> {
    if exe.len() < 52 || &exe[..4] != b"\x7fELF" || exe[4] != 1 || exe[5] != 1 {
        return Err(Error::Invalid("not a 32-bit little-endian ELF"));
    }
    let mut c = Cur::new(exe, 0x1C);
    let phoff = c.u32()? as usize;
    c.pos = 0x2A;
    let phentsize = c.u16()? as usize;
    let phnum = c.u16()? as usize;
    let mut segs = Vec::new();
    for i in 0..phnum {
        let mut p = Cur::new(exe, phoff + i * phentsize);
        let kind = p.u32()?;
        let off = p.u32()?;
        let va = p.u32()?;
        p.u32()?;
        let filesz = p.u32()?;
        if kind == 1 {
            segs.push((off, va, filesz));
        }
    }
    Ok(segs)
}

impl<R: ReadAt> Sly2Disc<R> {
    /// Opens a disc: reads `SYSTEM.CNF` and the executable it names, checks the executable's SHA-256, reads the
    /// file table, and reads the level table from the boot world.
    pub fn open(mut src: R) -> Result<Self, Error> {
        let cnf = iso::find_root_file(&mut src, "SYSTEM.CNF")?;
        let cnf = iso::read_file(&mut src, &cnf)?;
        let name = boot_executable_name(&cnf)?;
        let exe = iso::find_root_file(&mut src, &name)?;
        let executable = iso::read_file(&mut src, &exe)?;
        if hex(&sha256(&executable)) != EXECUTABLE_SHA256 {
            return Err(Error::Invalid("not the supported Sly 2 executable (SHA-256 mismatch)"));
        }
        let segments = elf_segments(&executable)?;
        let mut disc = Sly2Disc { src, executable, segments, files: Vec::new(), levels: Vec::new() };
        let (va, n) = FILE_TABLE;
        let table = disc.bytes_at(va, n * RECORD_SIZE)?.to_vec();
        disc.files =
            table.as_chunks::<RECORD_SIZE>().0.iter().map(|r| FileRecord::decode(r)).collect::<Result<_, _>>()?;
        let boot = disc.files.get(BOOT_WORLD_RECORD).copied().flatten().ok_or(Error::Invalid("no boot world"))?;
        let world = disc.read_world_file(&boot)?;
        disc.levels = WorldHeader::parse(&world)?.levels;
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

    /// The raw bytes a record points at.
    pub fn read(&mut self, r: &FileRecord) -> Result<Vec<u8>, Error> {
        Ok(self.src.read_vec(r.offset(), r.size as usize)?)
    }

    /// Reads and decompresses a world file.
    pub fn read_world_file(&mut self, r: &FileRecord) -> Result<Vec<u8>, Error> {
        Ok(decompress(&self.read(r)?)?)
    }

    /// Reads and decompresses the world of level-table row `row`.
    pub fn read_world(&mut self, row: usize) -> Result<Vec<u8>, Error> {
        let r = self.levels.get(row).and_then(|l| l.file).ok_or(Error::Invalid("empty or missing level row"))?;
        self.read_world_file(&r)
    }

    /// Reads and parses an effect bank (version 3).
    pub fn read_bank(&mut self, r: &FileRecord) -> Result<crate::bank::EffectBank, Error> {
        crate::bank::EffectBank::parse(self.read(r)?)
    }

    /// Reads a `VPK ` stream file and splits it into channels.
    pub fn read_vpk(&mut self, r: &FileRecord) -> Result<crate::sly2::vpk::VpkStream, Error> {
        crate::sly2::vpk::VpkStream::parse(&self.read(r)?)
    }

    /// Reads music stream `n` of a world's sound section (`None` when it names no file).
    pub fn read_music_stream(
        &mut self,
        sound: &crate::sly2::WorldSound,
        n: i16,
    ) -> Result<Option<crate::sly2::vpk::VpkStream>, Error> {
        match sound.music_stream(n) {
            Some(r) => self.read_vpk(&r).map(Some),
            None => Ok(None),
        }
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
impl Sly2Disc<std::fs::File> {
    /// Opens a disc image file.
    pub fn open_path(path: impl AsRef<std::path::Path>) -> Result<Self, Error> {
        let f = std::fs::File::open(path).map_err(|e| Error::Disc(formats::error::Error::Io(alloc::format!("{e}"))))?;
        Sly2Disc::open(f)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Encodes a record with scrambling keys.
    pub(crate) fn record(sector: u32, size: u32, identity: u32) -> [u8; 64] {
        let mut w = [0u32; 16];
        for (i, x) in w.iter_mut().enumerate() {
            *x = 0x9E37_79B9u32.wrapping_mul(i as u32 + 7);
        }
        w[0] = (sector - BASE_SECTOR) ^ w[13];
        w[1] = size ^ w[15];
        w[2] = identity ^ w[14];
        let mut b = [0u8; 64];
        for (i, x) in w.iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&x.to_le_bytes());
        }
        b
    }

    #[test]
    fn records() {
        let r = FileRecord::decode(&record(380_864, 1_330_700, 58)).unwrap().unwrap();
        assert_eq!((r.sector, r.size, r.identity), (380_864, 1_330_700, 58));
        assert_eq!(r.offset(), 380_864 * 2048);
        let mut empty = record(0xCA0, 1, 0);
        let key: [u8; 4] = empty[60..64].try_into().unwrap();
        empty[4..8].copy_from_slice(&key);
        assert_eq!(FileRecord::decode(&empty).unwrap(), None);
        assert!(FileRecord::decode(&[0; 10]).is_err());
    }
}
