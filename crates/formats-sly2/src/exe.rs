//! The Sly 2 (USA) boot executable `SCUS_973.16`: identity, address mapping, the file table (disc.md 3, 5) and the
//! option table (object-stream.md 1.1).

use alloc::vec::Vec;

use formats::error::{Error, Result};
use formats::hash::{parse_hex32, sha256};
use formats::reader::le_u32;

use crate::record::FileRecord;

/// SHA-256 of the supported executable (disc.md 1).
pub const EXECUTABLE_SHA256: [u8; 32] =
    match parse_hex32("a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73") {
        Some(h) => h,
        None => [0; 32],
    };

/// Boot file name (disc.md 2).
pub const BOOT_FILE: &str = "SCUS_973.16";

/// Virtual address and entry count of the executable's file table (disc.md 5).
pub const FILE_TABLE: (u32, usize) = (0x002A_32A8, 29);
/// The file-table entry holding the boot world (the intro hub), disc.md 5.
pub const BOOT_WORLD_ENTRY: usize = 24;
/// Virtual address of the option table (object-stream.md 1.1).
pub const OPTION_TABLE_VA: u32 = 0x002A_C880;
/// Bytes per option-table entry.
pub const OPTION_ENTRY_SIZE: usize = 40;

/// One loadable segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub file_offset: u32,
    pub vaddr: u32,
    pub file_size: u32,
}

/// The parsed executable.
#[derive(Clone, Debug)]
pub struct Executable {
    pub data: Vec<u8>,
    pub segments: Vec<Segment>,
    pub sha256: [u8; 32],
}

impl Executable {
    /// Parses a 32-bit little-endian ELF and records its loadable segments.
    pub fn parse(data: Vec<u8>) -> Result<Executable> {
        if data.get(0..4) != Some(b"\x7fELF") || data.get(4) != Some(&1) || data.get(5) != Some(&1) {
            return Err(Error::BadExecutable("not a 32-bit little-endian ELF"));
        }
        let phoff = le_u32(&data, 0x1C)? as usize;
        let phentsize = u16::from_le_bytes([data[0x2A], data[0x2B]]) as usize;
        let phnum = u16::from_le_bytes([data[0x2C], data[0x2D]]) as usize;
        let mut segments = Vec::new();
        for i in 0..phnum {
            let o = phoff + i * phentsize;
            if le_u32(&data, o)? == 1 {
                segments.push(Segment {
                    file_offset: le_u32(&data, o + 4)?,
                    vaddr: le_u32(&data, o + 8)?,
                    file_size: le_u32(&data, o + 16)?,
                });
            }
        }
        if segments.is_empty() {
            return Err(Error::BadExecutable("no loadable segment"));
        }
        let sha256 = sha256(&data);
        Ok(Executable { data, segments, sha256 })
    }

    /// Whether this is the supported release.
    pub fn is_supported(&self) -> bool {
        self.sha256 == EXECUTABLE_SHA256
    }

    /// File offset of virtual address `va`, if it lies in a segment's file data.
    pub fn va_to_offset(&self, va: u32) -> Option<usize> {
        self.segments.iter().find_map(|s| {
            (va >= s.vaddr && va - s.vaddr < s.file_size).then(|| (va - s.vaddr + s.file_offset) as usize)
        })
    }

    /// `len` bytes at virtual address `va`.
    pub fn bytes_at(&self, va: u32, len: usize) -> Result<&[u8]> {
        let off = self.va_to_offset(va).ok_or(Error::BadExecutable("address outside the executable"))?;
        self.data.get(off..off + len).ok_or(Error::BadExecutable("table runs past the executable"))
    }

    /// The 29 file records of the file table.
    pub fn file_table(&self) -> Result<Vec<FileRecord>> {
        let (va, n) = FILE_TABLE;
        let raw = self.bytes_at(va, n * FileRecord::SIZE)?;
        Ok(raw.as_chunks::<{ FileRecord::SIZE }>().0.iter().map(|r| FileRecord::decode(r)).collect())
    }

    /// The option table, `count` entries from option ID 0.
    pub fn option_table(&self, count: usize) -> Result<OptionTable> {
        let raw = self.bytes_at(OPTION_TABLE_VA, count * OPTION_ENTRY_SIZE)?;
        Ok(OptionTable::from_raw(raw))
    }
}

/// How an option's value is stored (object-stream.md 1.1 to 1.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptionEntry {
    pub value_type: u32,
    pub flags: u32,
    pub arg_count: i32,
}

impl OptionEntry {
    /// The value is a typed argument list.
    pub fn is_typed(&self) -> bool {
        self.flags & 0x400 != 0
    }
}

/// The executable's option table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OptionTable {
    pub entries: Vec<OptionEntry>,
}

/// Option IDs the readers need to know (object-stream.md 1.4, classes/effects-logic-ui.md 1).
pub mod option_ids {
    /// Remove the object when the level starts.
    pub const REMOVE_AT_START: i16 = 26;
    /// Emitter origin kind, class 29.
    pub const EMITTER_ORIGIN_29: i16 = 490;
    /// Class 29: the origin mesh is present exactly when this option is 3 (classes/effects-logic-ui.md 1).
    pub const EMITTER_MESH_KIND_29: i16 = 483;
    /// Present on class 29: the emitter has an origin mesh.
    pub const EMITTER_MESH_29: i16 = 501;
    /// Emitter origin kind, class 33.
    pub const EMITTER_ORIGIN_33: i16 = 542;
}

impl OptionTable {
    /// Entries option IDs needs (the highest on the disc is 1,448).
    pub const ENTRIES: usize = 1600;

    pub fn from_raw(raw: &[u8]) -> OptionTable {
        let entries = raw
            .as_chunks::<OPTION_ENTRY_SIZE>()
            .0
            .iter()
            .map(|e| OptionEntry {
                value_type: u32::from_le_bytes(e[0..4].try_into().unwrap()),
                flags: u32::from_le_bytes(e[4..8].try_into().unwrap()),
                arg_count: i32::from_le_bytes(e[24..28].try_into().unwrap()),
            })
            .collect();
        OptionTable { entries }
    }

    pub fn get(&self, id: i16) -> Option<&OptionEntry> {
        usize::try_from(id).ok().and_then(|i| self.entries.get(i))
    }
}

/// Bytes of one fixed-size option value of `value_type` (object-stream.md 1.2), `None` for a type that does not occur.
pub fn value_size(value_type: u32) -> Option<usize> {
    Some(match value_type {
        0 => 1,
        1 | 5 | 6 | 13 => 4,
        2 | 9 => 12,
        7 | 14 | 15 | 16 => 2,
        10 => 8,
        0x1000..=0x2FFF => 4,
        _ => return None,
    })
}

/// Bytes of one typed-argument value with tag `tag` (low 15 bits; object-stream.md 1.3; tag 15 occurs only in
/// mission options).
pub fn arg_size(tag: u16) -> Option<usize> {
    Some(match tag {
        0 => 1,
        1 | 5 | 6 => 4,
        2 => 12,
        7 | 14 | 15 | 16 | 19 | 20 | 21 => 2,
        10 => 8,
        0x1000..=0x1FFF => 4,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(value_size(0), Some(1));
        assert_eq!(value_size(9), Some(12));
        assert_eq!(value_size(0x2ABC), Some(4));
        assert_eq!(value_size(3), None);
        assert_eq!(arg_size(21), Some(2));
        assert_eq!(arg_size(0x1010), Some(4));
        assert_eq!(arg_size(0x2000), None);
    }

    #[test]
    fn option_table_entries() {
        let mut raw = alloc::vec![0u8; 3 * OPTION_ENTRY_SIZE];
        raw[40..44].copy_from_slice(&2u32.to_le_bytes());
        raw[80 + 4..80 + 8].copy_from_slice(&0x400u32.to_le_bytes());
        raw[80 + 24..80 + 28].copy_from_slice(&3i32.to_le_bytes());
        let t = OptionTable::from_raw(&raw);
        assert_eq!(t.get(1).unwrap().value_type, 2);
        assert!(t.get(2).unwrap().is_typed());
        assert_eq!(t.get(2).unwrap().arg_count, 3);
        assert_eq!(t.get(-1), None);
        assert_eq!(t.get(3), None);
    }
}
