//! The Sly 1 (USA) disc: boot files, the executable's address mapping, file descriptors and the world table
//! (disc.md).
//!
//! The game's data lives in sectors no directory record points at. The executable holds tables of scrambled
//! 32-byte [`Descriptor`]s that give each item's sector and size; the world table is one of them.

use alloc::string::String;
use alloc::vec::Vec;

use crate::compress;
use crate::error::{Error, Result};
use crate::hash::sha256;
use crate::iso::{self, ReadAt, SECTOR};
use crate::reader::{Reader, le_u32};

/// SHA-256 of the supported executable, `SCUS_971.98` (disc.md section 1).
pub const EXECUTABLE_SHA256: [u8; 32] =
    match crate::hash::parse_hex32("d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c") {
        Some(h) => h,
        None => [0; 32],
    };

/// Virtual address of the world table in the executable.
pub const WORLD_TABLE_VA: u32 = 0x0024_7AF0;
/// Number of worlds in the table.
pub const WORLD_COUNT: usize = 46;
/// Size of one world-table entry.
pub const WORLD_ENTRY_SIZE: usize = 44;
/// Expected file offset of the executable's loadable segment.
pub const SEGMENT_FILE_OFFSET: u32 = 0x1000;
/// Expected virtual address of the executable's loadable segment.
pub const SEGMENT_VADDR: u32 = 0x0010_0000;

/// Other descriptor tables in the executable (disc.md 4.3): virtual address and entry count. Stride 32.
pub const SOUND_BANK_TABLE: (u32, usize) = (0x0024_4F40, 7);
/// Movies table (disc.md 4.3).
pub const MOVIE_TABLE: (u32, usize) = (0x0024_8490, 17);
/// Ambient sounds table (disc.md 4.3).
pub const AMBIENT_TABLE: (u32, usize) = (0x0024_82D8, 6);

/// A decoded 32-byte file descriptor (disc.md 4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Descriptor {
    /// First sector (LBA) of the item in the image.
    pub sector: u32,
    /// Size in bytes.
    pub size: u32,
    /// Identity: a small number naming the item (for worlds, the search key).
    pub identity: u32,
    /// The two words whose meaning is unknown (offsets 12 and 16), kept as stored.
    pub unknown: [u32; 2],
}

impl Descriptor {
    /// Size of an encoded descriptor.
    pub const SIZE: usize = 32;

    /// Decodes the 32 bytes at the start of `raw`. All-zero means "none".
    ///
    /// Words: `A0 A1 A2 ? ? B0 B2 B1`; sector = `A0 ^ B0`, size = `A1 ^ B1`, identity = `A2 ^ B2`.
    pub fn decode(raw: &[u8]) -> Result<Option<Descriptor>> {
        let raw = raw.get(..Self::SIZE).ok_or(Error::Eof { offset: 0, needed: Self::SIZE, len: raw.len() })?;
        if raw.iter().all(|&b| b == 0) {
            return Ok(None);
        }
        let w = |i: usize| le_u32(raw, i * 4);
        Ok(Some(Descriptor {
            sector: w(0)? ^ w(5)?,
            size: w(1)? ^ w(7)?,
            identity: w(2)? ^ w(6)?,
            unknown: [w(3)?, w(4)?],
        }))
    }

    /// Reads and decodes a descriptor at the reader's position.
    pub fn read(r: &mut Reader<'_>) -> Result<Option<Descriptor>> {
        Descriptor::decode(r.bytes(Self::SIZE)?)
    }

    /// Byte offset of the item in the image.
    pub fn offset(&self) -> u64 {
        self.sector as u64 * SECTOR
    }
}

/// The boot executable with its loadable segment mapping.
#[derive(Clone, Debug)]
pub struct Executable {
    /// The whole file.
    pub data: Vec<u8>,
    /// File offset of the loadable segment.
    pub seg_offset: u32,
    /// Virtual address of the loadable segment.
    pub seg_vaddr: u32,
    /// Size of the segment in the file.
    pub seg_filesz: u32,
    /// Size of the segment in memory.
    pub seg_memsz: u32,
}

impl Executable {
    /// Parses a 32-bit little-endian MIPS ELF and finds its single loadable program header.
    pub fn parse(data: Vec<u8>) -> Result<Executable> {
        if data.len() < 52 || &data[0..4] != b"\x7fELF" {
            return Err(Error::BadExecutable("not an ELF file"));
        }
        if data[4] != 1 || data[5] != 1 {
            return Err(Error::BadExecutable("not a 32-bit little-endian ELF"));
        }
        let mut r = Reader::at(&data, 0x1C);
        let phoff = r.u32()? as usize;
        r.set_pos(0x2A);
        let phentsize = r.u16()? as usize;
        let phnum = r.u16()? as usize;
        if phentsize < 32 {
            return Err(Error::BadExecutable("program header entries too small"));
        }
        let mut load = None;
        for i in 0..phnum {
            let at = i
                .checked_mul(phentsize)
                .and_then(|o| o.checked_add(phoff))
                .ok_or(Error::BadExecutable("program header offset overflows"))?;
            let mut p = Reader::at(&data, at);
            let p_type = p.u32()?;
            let offset = p.u32()?;
            let vaddr = p.u32()?;
            let _paddr = p.u32()?;
            let filesz = p.u32()?;
            let memsz = p.u32()?;
            if p_type == 1 {
                if load.is_some() {
                    return Err(Error::BadExecutable("more than one loadable segment"));
                }
                load = Some((offset, vaddr, filesz, memsz));
            }
        }
        let (seg_offset, seg_vaddr, seg_filesz, seg_memsz) = load.ok_or(Error::BadExecutable("no loadable segment"))?;
        if (seg_offset as usize).checked_add(seg_filesz as usize).is_none_or(|end| end > data.len()) {
            return Err(Error::BadExecutable("segment runs past the end of the file"));
        }
        Ok(Executable { data, seg_offset, seg_vaddr, seg_filesz, seg_memsz })
    }

    /// File offset of virtual address `va`, when it is inside the file part of the segment.
    pub fn va_to_offset(&self, va: u32) -> Option<usize> {
        let rel = va.checked_sub(self.seg_vaddr)?;
        (rel < self.seg_filesz).then(|| (self.seg_offset + rel) as usize)
    }

    /// `len` bytes at virtual address `va`.
    pub fn bytes_at(&self, va: u32, len: usize) -> Result<&[u8]> {
        let off = self.va_to_offset(va).ok_or(Error::BadExecutable("address outside the loaded segment"))?;
        self.data.get(off..off + len).ok_or(Error::BadExecutable("read past the end of the segment"))
    }

    /// The NUL-terminated ASCII string at `va`.
    pub fn c_string(&self, va: u32) -> Result<String> {
        let off = self.va_to_offset(va).ok_or(Error::BadExecutable("string pointer outside the segment"))?;
        let rest = &self.data[off..];
        let end = rest.iter().position(|&b| b == 0).ok_or(Error::BadExecutable("unterminated string"))?;
        Ok(rest[..end].iter().map(|&b| b as char).collect())
    }

    /// Decodes a table of `count` descriptors with a 32-byte stride at `va`.
    pub fn descriptor_table(&self, va: u32, count: usize) -> Result<Vec<Option<Descriptor>>> {
        let raw = self.bytes_at(va, count * Descriptor::SIZE)?;
        raw.as_chunks::<{ Descriptor::SIZE }>().0.iter().map(|c| Descriptor::decode(c)).collect()
    }

    /// Whether this is the supported executable (by SHA-256).
    pub fn is_supported(&self) -> bool {
        sha256(&self.data) == EXECUTABLE_SHA256
    }
}

/// One entry of the world table (disc.md 4.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldEntry {
    /// Index in the table (0..46).
    pub index: usize,
    /// Where the compressed world file is. Its identity is the world's search key.
    pub file: Descriptor,
    /// World id: high byte = episode, low byte = level within the episode.
    pub id: u32,
    /// Display name, as stored in the executable.
    pub name: String,
    /// Task bitmask (not needed for loading).
    pub task_mask: u32,
}

impl WorldEntry {
    /// Episode number (high byte of the id): 0 intro and hub, 1..5 the five episodes.
    pub fn episode(&self) -> u32 {
        self.id >> 8
    }

    /// Level within the episode (low byte of the id).
    pub fn level(&self) -> u32 {
        self.id & 0xFF
    }

    /// The search key (the descriptor's identity), used by other worlds to refer to this one.
    pub fn key(&self) -> u32 {
        self.file.identity
    }

    /// The display name made safe for a host file name (anything but letters, digits, space, `-`, `_`, `!` and `'`
    /// becomes `_`).
    pub fn file_name(&self) -> String {
        self.name.chars().map(|c| if c.is_ascii_alphanumeric() || " -_!'".contains(c) { c } else { '_' }).collect()
    }
}

/// Reads the world table out of the executable.
pub fn parse_world_table(exe: &Executable) -> Result<Vec<WorldEntry>> {
    let raw = exe.bytes_at(WORLD_TABLE_VA, WORLD_COUNT * WORLD_ENTRY_SIZE)?;
    let mut out = Vec::with_capacity(WORLD_COUNT);
    for (index, e) in raw.as_chunks::<{ WORLD_ENTRY_SIZE }>().0.iter().enumerate() {
        let file = Descriptor::decode(e)?.ok_or(Error::BadExecutable("empty world descriptor"))?;
        let mut r = Reader::at(e, 32);
        let id = r.u32()?;
        let name_va = r.u32()?;
        let task_mask = r.u32()?;
        out.push(WorldEntry { index, file, id, name: exe.c_string(name_va)?, task_mask });
    }
    Ok(out)
}

/// Extracts the executable name from `SYSTEM.CNF` (the `BOOT2` line, for example `cdrom0:\SCUS_971.98;1`).
pub fn boot_executable_name(system_cnf: &[u8]) -> Result<String> {
    let text: String = system_cnf.iter().map(|&b| b as char).collect();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        if key.trim().eq_ignore_ascii_case("BOOT2") {
            let path = value.trim();
            let name = path.rsplit(['\\', '/', ':']).next().unwrap_or(path);
            let name = name.split(';').next().unwrap_or(name);
            if !name.is_empty() {
                return Ok(String::from(name));
            }
        }
    }
    Err(Error::BadIso("SYSTEM.CNF has no BOOT2 line"))
}

/// An opened Sly 1 disc image.
pub struct Disc<R: ReadAt> {
    src: R,
    /// The volume descriptor fields.
    pub volume: iso::VolumeInfo,
    /// The boot executable.
    pub executable: Executable,
    /// The 46 worlds.
    pub worlds: Vec<WorldEntry>,
}

impl<R: ReadAt> Disc<R> {
    /// Opens a disc: reads `SYSTEM.CNF`, the executable it names, checks the executable's hash, and reads the
    /// world table.
    pub fn open(mut src: R) -> Result<Self> {
        let disc = Self::open_unchecked_inner(&mut src)?;
        if !disc.1.is_supported() {
            return Err(Error::UnknownExecutable);
        }
        Self::finish(src, disc)
    }

    /// Like [`Disc::open`] but without the executable hash check (the segment layout is still checked).
    pub fn open_unchecked(mut src: R) -> Result<Self> {
        let disc = Self::open_unchecked_inner(&mut src)?;
        Self::finish(src, disc)
    }

    fn open_unchecked_inner(src: &mut R) -> Result<(iso::VolumeInfo, Executable)> {
        let volume = iso::read_volume(src)?;
        let cnf = iso::find_root_file(src, "SYSTEM.CNF")?;
        let cnf = iso::read_file(src, &cnf)?;
        let exe_name = boot_executable_name(&cnf)?;
        let exe = iso::find_root_file(src, &exe_name)?;
        let exe = Executable::parse(iso::read_file(src, &exe)?)?;
        Ok((volume, exe))
    }

    fn finish(src: R, (volume, executable): (iso::VolumeInfo, Executable)) -> Result<Self> {
        if executable.seg_offset != SEGMENT_FILE_OFFSET || executable.seg_vaddr != SEGMENT_VADDR {
            return Err(Error::BadExecutable("segment mapping differs from the supported release"));
        }
        let worlds = parse_world_table(&executable)?;
        Ok(Disc { src, volume, executable, worlds })
    }

    /// The underlying image reader.
    pub fn source(&mut self) -> &mut R {
        &mut self.src
    }

    /// Reads the bytes an arbitrary descriptor points at.
    pub fn read_descriptor(&mut self, d: &Descriptor) -> Result<Vec<u8>> {
        self.src.read_vec(d.offset(), d.size as usize)
    }

    /// The compressed bytes of world `index`.
    pub fn read_world_compressed(&mut self, index: usize) -> Result<Vec<u8>> {
        let d = self.worlds.get(index).ok_or(Error::OutOfRange { what: "world", index, len: WORLD_COUNT })?.file;
        self.read_descriptor(&d)
    }

    /// The decompressed world file of world `index`.
    pub fn read_world(&mut self, index: usize) -> Result<Vec<u8>> {
        compress::decompress(&self.read_world_compressed(index)?)
    }

    /// Finds a world by its display name (exact, case-insensitive) or its id.
    pub fn find_world(&self, name_or_id: &str) -> Option<usize> {
        let id = name_or_id
            .strip_prefix("0x")
            .and_then(|h| u32::from_str_radix(h, 16).ok())
            .or_else(|| name_or_id.parse::<u32>().ok().filter(|&i| (i as usize) >= WORLD_COUNT));
        self.worlds.iter().position(|w| Some(w.id) == id || w.name.eq_ignore_ascii_case(name_or_id))
    }
}

#[cfg(feature = "std")]
impl Disc<std::fs::File> {
    /// Opens a disc image file.
    pub fn open_path(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Disc::open(std::fs::File::open(path)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn scramble(sector: u32, size: u32, identity: u32, keys: [u32; 3]) -> [u8; 32] {
        let words = [sector ^ keys[0], size ^ keys[1], identity ^ keys[2], 0x1111, 0x2222, keys[0], keys[2], keys[1]];
        let mut out = [0u8; 32];
        for (o, w) in out.as_chunks_mut::<4>().0.iter_mut().zip(words) {
            o.copy_from_slice(&w.to_le_bytes());
        }
        out
    }

    #[test]
    fn descriptor_pairs_words_as_specified() {
        let raw = scramble(820_372, 150_849, 21, [0xDEAD_BEEF, 0x0BAD_F00D, 0x1234_5678]);
        let d = Descriptor::decode(&raw).unwrap().unwrap();
        assert_eq!((d.sector, d.size, d.identity), (820_372, 150_849, 21));
        assert_eq!(d.unknown, [0x1111, 0x2222]);
        assert_eq!(d.offset(), 820_372 * 2048);
        assert_eq!(Descriptor::decode(&[0u8; 32]).unwrap(), None);
    }

    #[test]
    fn boot_name_from_system_cnf() {
        let cnf = b"BOOT2 = cdrom0:\\GAME_123.45;1\r\nVER = 1.00\r\nVMODE = NTSC\r\n";
        assert_eq!(boot_executable_name(cnf).unwrap(), "GAME_123.45");
        assert!(boot_executable_name(b"VER=1\r\n").is_err());
    }

    /// A minimal ELF with one loadable segment at file offset 0x1000, address 0x100000, holding a world table.
    fn synthetic_elf() -> Vec<u8> {
        let table_off = (WORLD_TABLE_VA - SEGMENT_VADDR + SEGMENT_FILE_OFFSET) as usize;
        let names_off = table_off + WORLD_COUNT * WORLD_ENTRY_SIZE;
        let mut data = vec![0u8; names_off + WORLD_COUNT * 16];
        data[0..4].copy_from_slice(b"\x7fELF");
        data[4] = 1;
        data[5] = 1;
        data[0x1C..0x20].copy_from_slice(&52u32.to_le_bytes());
        data[0x2A..0x2C].copy_from_slice(&32u16.to_le_bytes());
        data[0x2C..0x2E].copy_from_slice(&1u16.to_le_bytes());
        let filesz = (data.len() - 0x1000) as u32;
        for (i, w) in [1u32, 0x1000, SEGMENT_VADDR, SEGMENT_VADDR, filesz, filesz + 100].iter().enumerate() {
            data[52 + i * 4..56 + i * 4].copy_from_slice(&w.to_le_bytes());
        }
        for i in 0..WORLD_COUNT {
            let e = table_off + i * WORLD_ENTRY_SIZE;
            let raw = scramble(1000 + i as u32, 10 + i as u32, 50 + i as u32, [0xA5A5_0000 + i as u32, 7, 9]);
            data[e..e + 32].copy_from_slice(&raw);
            data[e + 32..e + 36].copy_from_slice(&(0x100 + i as u32).to_le_bytes());
            let name_va = SEGMENT_VADDR + (names_off + i * 16 - 0x1000) as u32;
            data[e + 36..e + 40].copy_from_slice(&name_va.to_le_bytes());
            let name = alloc::format!("World {i}, x");
            data[names_off + i * 16..names_off + i * 16 + name.len()].copy_from_slice(name.as_bytes());
        }
        data
    }

    #[test]
    fn executable_mapping_and_world_table() {
        let exe = Executable::parse(synthetic_elf()).unwrap();
        assert_eq!(exe.va_to_offset(0x0010_0000), Some(0x1000));
        assert_eq!(exe.va_to_offset(WORLD_TABLE_VA), Some(0x148AF0));
        assert_eq!(exe.va_to_offset(0x000F_FFFF), None);
        let worlds = parse_world_table(&exe).unwrap();
        assert_eq!(worlds.len(), WORLD_COUNT);
        let w = &worlds[3];
        assert_eq!((w.file.sector, w.file.size, w.key(), w.id), (1003, 13, 53, 0x103));
        assert_eq!((w.episode(), w.level()), (1, 3));
        assert_eq!(w.name, "World 3, x");
        assert_eq!(w.file_name(), "World 3_ x");
        assert!(!exe.is_supported());
    }

    #[test]
    fn opens_synthetic_disc_and_reads_compressed_world() {
        let elf = synthetic_elf();
        let cnf = b"BOOT2=cdrom0:\\GAME.ELF;1\r\n";
        // World 0 lives at sector 1000 with 10 bytes: a compressed stream of literals.
        let mut img = iso::tests::build_image(&[("SYSTEM.CNF;1", 30, cnf), ("GAME.ELF;1", 31, &elf)], 1100);
        let w0 = 1000 * SECTOR as usize;
        img[w0..w0 + 10].copy_from_slice(&[0xFF, 1, 2, 3, 4, 5, 6, 7, 8, 0x01]);
        let mut src: &[u8] = &img;
        assert!(matches!(Disc::open(&mut src), Err(Error::UnknownExecutable)));
        let mut disc = Disc::open_unchecked(&mut src).unwrap();
        assert_eq!(disc.worlds.len(), WORLD_COUNT);
        assert_eq!(disc.read_world_compressed(0).unwrap().len(), 10);
        assert_eq!(disc.read_world(0).unwrap(), [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(disc.find_world("world 2, x"), Some(2));
        assert_eq!(disc.find_world("0x105"), Some(5));
    }
}
