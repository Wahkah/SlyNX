//! The parts of ISO 9660 (ECMA-119) the Sly 1 disc needs: the primary volume descriptor and the root directory.
//!
//! The image is plain 2048-byte sectors. Reading goes through [`ReadAt`] so the same code works on a file (PC), a
//! byte slice (tests) or a platform file handle (Switch).

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Error, Result};
use crate::reader::le_u32;

/// Bytes per sector.
pub const SECTOR: u64 = 2048;

/// Random-access reads from a disc image.
pub trait ReadAt {
    /// Fills `buf` with the bytes at `offset`. Reading past the end is an error.
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()>;

    /// The size of the image in bytes, when the reader knows it. Used to refuse reads (and allocations) sized from
    /// corrupt fields before they happen.
    fn image_len(&mut self) -> Option<u64> {
        None
    }

    /// Reads `len` bytes at `offset` into a new vector. The read is checked against [`Self::image_len`] before
    /// anything is allocated; a reader that does not know its size accepts at most [`MAX_UNSIZED_READ`] bytes.
    fn read_vec(&mut self, offset: u64, len: usize) -> Result<Vec<u8>> {
        let past_end = match self.image_len() {
            Some(total) => offset.checked_add(len as u64).is_none_or(|end| end > total),
            None => len > MAX_UNSIZED_READ,
        };
        if past_end {
            let total = self.image_len().unwrap_or(0);
            return Err(Error::Eof { offset: offset as usize, needed: len, len: total as usize });
        }
        let mut v = vec![0u8; len];
        self.read_at(offset, &mut v)?;
        Ok(v)
    }
}

/// The largest single read accepted from a reader that does not know its size (the largest file on a PS2 disc
/// image is well below this).
pub const MAX_UNSIZED_READ: usize = 1 << 30;

impl ReadAt for &[u8] {
    fn image_len(&mut self) -> Option<u64> {
        Some(self.len() as u64)
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        let start = usize::try_from(offset).ok().filter(|&s| s <= self.len());
        let end = start.and_then(|s| s.checked_add(buf.len())).filter(|&e| e <= self.len());
        match (start, end) {
            (Some(s), Some(e)) => {
                buf.copy_from_slice(&self[s..e]);
                Ok(())
            }
            _ => Err(Error::Eof { offset: offset as usize, needed: buf.len(), len: self.len() }),
        }
    }
}

impl<R: ReadAt + ?Sized> ReadAt for &mut R {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        (**self).read_at(offset, buf)
    }

    fn image_len(&mut self) -> Option<u64> {
        (**self).image_len()
    }
}

#[cfg(feature = "std")]
impl ReadAt for std::fs::File {
    fn image_len(&mut self) -> Option<u64> {
        self.metadata().ok().map(|m| m.len())
    }

    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        use std::io::{Read, Seek, SeekFrom};
        self.seek(SeekFrom::Start(offset))?;
        self.read_exact(buf)?;
        Ok(())
    }
}

/// One record of a directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    /// The name as stored, including any `;1` version suffix. `"."` and `".."` for the special records.
    pub name: String,
    /// First sector of the extent.
    pub lba: u32,
    /// Data length in bytes.
    pub size: u32,
    /// Whether this is a directory.
    pub is_dir: bool,
}

impl DirEntry {
    /// The name without the `;version` suffix.
    pub fn base_name(&self) -> &str {
        self.name.split(';').next().unwrap_or("")
    }

    /// Byte offset of the extent in the image.
    pub fn offset(&self) -> u64 {
        self.lba as u64 * SECTOR
    }
}

/// Fields of the primary volume descriptor that identify a disc.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeInfo {
    /// System identifier (bytes 8..40), trailing spaces removed.
    pub system_id: String,
    /// Volume identifier (bytes 40..72), trailing spaces removed.
    pub volume_id: String,
    /// Volume creation date (bytes 813..829): 16 ASCII digits.
    pub creation_date: String,
    /// The root directory.
    pub root: DirEntry,
}

fn ascii_trimmed(b: &[u8]) -> String {
    let s: String = b.iter().map(|&c| if c.is_ascii() { c as char } else { '?' }).collect();
    String::from(s.trim_end_matches([' ', '\0']))
}

/// Parses one directory record at the start of `rec` (which holds at least `rec[0]` bytes).
fn parse_record(rec: &[u8]) -> Result<DirEntry> {
    let len = rec[0] as usize;
    if len < 34 || len > rec.len() {
        return Err(Error::BadIso("directory record too short"));
    }
    let name_len = rec[32] as usize;
    if 33 + name_len > len {
        return Err(Error::BadIso("directory record name overruns record"));
    }
    let raw = &rec[33..33 + name_len];
    let name = match raw {
        [0] => String::from("."),
        [1] => String::from(".."),
        _ => ascii_trimmed(raw),
    };
    Ok(DirEntry { name, lba: le_u32(rec, 2)?, size: le_u32(rec, 10)?, is_dir: rec[25] & 0x02 != 0 })
}

/// Reads the primary volume descriptor at sector 16.
pub fn read_volume<R: ReadAt + ?Sized>(src: &mut R) -> Result<VolumeInfo> {
    let mut pvd = [0u8; SECTOR as usize];
    src.read_at(16 * SECTOR, &mut pvd)?;
    if pvd[0] != 1 || &pvd[1..6] != b"CD001" || pvd[6] != 1 {
        return Err(Error::BadIso("no primary volume descriptor at sector 16"));
    }
    let root = parse_record(&pvd[156..190])?;
    if !root.is_dir {
        return Err(Error::BadIso("root record is not a directory"));
    }
    Ok(VolumeInfo {
        system_id: ascii_trimmed(&pvd[8..40]),
        volume_id: ascii_trimmed(&pvd[40..72]),
        creation_date: ascii_trimmed(&pvd[813..829]),
        root,
    })
}

/// Reads every record of directory `dir` (including `.` and `..`).
pub fn read_dir<R: ReadAt + ?Sized>(src: &mut R, dir: &DirEntry) -> Result<Vec<DirEntry>> {
    let sectors = (dir.size as u64).div_ceil(SECTOR);
    if sectors > 4096 {
        return Err(Error::BadIso("directory implausibly large"));
    }
    let data = src.read_vec(dir.offset(), (sectors * SECTOR) as usize)?;
    let mut out = Vec::new();
    for sector in data.as_chunks::<{ SECTOR as usize }>().0.iter() {
        let mut off = 0;
        while off < sector.len() {
            let len = sector[off] as usize;
            if len == 0 {
                break; // no more records in this sector
            }
            if off + len > sector.len() {
                return Err(Error::BadIso("directory record crosses a sector boundary"));
            }
            out.push(parse_record(&sector[off..off + len])?);
            off += len;
        }
    }
    Ok(out)
}

/// Finds `name` (case-insensitive, with or without `;1`) in the root directory.
pub fn find_root_file<R: ReadAt + ?Sized>(src: &mut R, name: &str) -> Result<DirEntry> {
    let vol = read_volume(src)?;
    let want = name.split(';').next().unwrap_or("");
    read_dir(src, &vol.root)?
        .into_iter()
        .find(|e| !e.is_dir && e.base_name().eq_ignore_ascii_case(want))
        .ok_or_else(|| Error::FileNotFound(String::from(name)))
}

/// Reads a whole file.
pub fn read_file<R: ReadAt + ?Sized>(src: &mut R, entry: &DirEntry) -> Result<Vec<u8>> {
    src.read_vec(entry.offset(), entry.size as usize)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn record(name: &[u8], lba: u32, size: u32, dir: bool) -> Vec<u8> {
        let mut len = 33 + name.len();
        len += len % 2;
        let mut r = vec![0u8; len];
        r[0] = len as u8;
        r[2..6].copy_from_slice(&lba.to_le_bytes());
        r[6..10].copy_from_slice(&lba.to_be_bytes());
        r[10..14].copy_from_slice(&size.to_le_bytes());
        r[14..18].copy_from_slice(&size.to_be_bytes());
        r[25] = if dir { 2 } else { 0 };
        r[32] = name.len() as u8;
        r[33..33 + name.len()].copy_from_slice(name);
        r
    }

    /// Builds a small image: PVD at 16, root directory at 20 (two sectors, the second file record in sector 2),
    /// and the given files' data at the sectors given.
    pub(crate) fn build_image(files: &[(&str, u32, &[u8])], total_sectors: usize) -> Vec<u8> {
        let mut img = vec![0u8; total_sectors * SECTOR as usize];
        let pvd = 16 * SECTOR as usize;
        img[pvd] = 1;
        img[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        img[pvd + 6] = 1;
        img[pvd + 8..pvd + 40].fill(b' ');
        img[pvd + 8..pvd + 19].copy_from_slice(b"PLAYSTATION");
        img[pvd + 40..pvd + 72].fill(b' ');
        img[pvd + 40..pvd + 43].copy_from_slice(b"SLY");
        img[pvd + 813..pvd + 829].copy_from_slice(b"2002082412000000");
        let root = record(&[0], 20, 2 * SECTOR as u32, true);
        img[pvd + 156..pvd + 156 + root.len()].copy_from_slice(&root);
        let mut dir = Vec::new();
        dir.extend(record(&[0], 20, 2 * SECTOR as u32, true));
        dir.extend(record(&[1], 20, 2 * SECTOR as u32, true));
        let dir_off = 20 * SECTOR as usize;
        let mut off = dir_off;
        for (i, (name, lba, data)) in files.iter().enumerate() {
            let r = record(name.as_bytes(), *lba, data.len() as u32, false);
            if i == files.len() - 1 && files.len() > 1 {
                off = dir_off + SECTOR as usize; // put the last record in the second directory sector
            }
            if off == dir_off {
                img[off..off + dir.len()].copy_from_slice(&dir);
                off += dir.len();
            }
            img[off..off + r.len()].copy_from_slice(&r);
            off += r.len();
            let d = *lba as usize * SECTOR as usize;
            img[d..d + data.len()].copy_from_slice(data);
        }
        img
    }

    #[test]
    fn reads_volume_and_root_directory() {
        let img = build_image(&[("SYSTEM.CNF;1", 30, b"hello"), ("GAME.ELF;1", 31, b"elf!")], 40);
        let mut src: &[u8] = &img;
        let vol = read_volume(&mut src).unwrap();
        assert_eq!(vol.system_id, "PLAYSTATION");
        assert_eq!(vol.volume_id, "SLY");
        assert_eq!(vol.creation_date, "2002082412000000");
        let entries = read_dir(&mut src, &vol.root).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, [".", "..", "SYSTEM.CNF;1", "GAME.ELF;1"]);
        let f = find_root_file(&mut src, "game.elf").unwrap();
        assert_eq!((f.lba, f.size), (31, 4));
        assert_eq!(read_file(&mut src, &f).unwrap(), b"elf!");
        assert!(matches!(find_root_file(&mut src, "NOPE"), Err(Error::FileNotFound(_))));
    }

    #[test]
    fn rejects_non_iso() {
        let img = vec![0u8; 20 * SECTOR as usize];
        let mut src: &[u8] = &img;
        assert!(matches!(read_volume(&mut src), Err(Error::BadIso(_))));
        let mut short: &[u8] = &img[..100];
        assert!(read_volume(&mut short).is_err());
    }

    #[test]
    fn reads_are_checked_against_the_image_size_before_allocating() {
        let img = [0u8; 16];
        let mut src: &[u8] = &img;
        assert!(src.read_vec(8, 8).is_ok());
        assert!(matches!(src.read_vec(8, 9), Err(Error::Eof { .. })));
        assert!(src.read_vec(0, usize::MAX).is_err());
        let entry = DirEntry { name: String::from("BIG;1"), lba: 0, size: u32::MAX, is_dir: false };
        assert!(read_file(&mut src, &entry).is_err());
    }
}
