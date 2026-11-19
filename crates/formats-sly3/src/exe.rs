//! The Sly 3 (USA) boot executable `SCUS_974.64` (disc.md 1, 3, 5; object-stream.md 1.1 and 3.1). The ELF mapping
//! and the option-table entry format are Sly 2's ([`formats_sly2::exe`]); the addresses and the hash are Sly 3's.

use alloc::string::String;
use alloc::vec::Vec;

use formats::error::{Error, Result};
use formats::hash::parse_hex32;
use formats_sly2::exe::{OPTION_ENTRY_SIZE, OptionTable};
use formats_sly2::record::FileRecord;

pub use formats_sly2::exe::Executable;

/// SHA-256 of the supported executable (disc.md 1).
pub const EXECUTABLE_SHA256: [u8; 32] =
    match parse_hex32("ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338") {
        Some(h) => h,
        None => [0; 32],
    };

/// Number of world slots (disc.md 5.1).
pub const WORLD_SLOTS: usize = 40;
/// World name pointers.
pub const WORLD_NAMES_VA: u32 = 0x002E_DE48;
/// World episodes.
pub const WORLD_EPISODES_VA: u32 = 0x002E_DF10;
/// World file records.
pub const WORLD_RECORDS_VA: u32 = 0x002E_DFD8;
/// The movie table: address and record count (disc.md 5.2).
pub const MOVIE_TABLE: (u32, usize) = (0x0031_79F8, 23);
/// The option table: address and entry count (object-stream.md 1.1).
pub const OPTION_TABLE: (u32, usize) = (0x0032_CA80, 2044);
/// The class table: address and entry count (object-stream.md 3.1).
pub const CLASS_TABLE: (u32, usize) = (0x0034_09E0, 249);
/// Bytes per class-table entry.
pub const CLASS_ENTRY_SIZE: usize = 20;
/// The three packed-tangent multipliers (animation.md 2.1).
pub const TANGENT_MULTIPLIERS_VA: u32 = 0x0031_F690;

/// One world slot of the executable's tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldSlot {
    pub slot: usize,
    pub name: String,
    pub episode: i32,
    /// The compressed world file; empty (size 0) for slot 1.
    pub file: FileRecord,
}

impl WorldSlot {
    /// The world id (the record's identity).
    pub fn world_id(&self) -> u32 {
        self.file.identity
    }
}

/// One class-table entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassEntry {
    /// Address of the base class's entry (0 for the root).
    pub base: u32,
    pub class: i32,
    pub last_derived: i32,
    pub size: u32,
    pub constructor: u32,
}

/// Whether `exe` is the supported Sly 3 executable.
pub fn is_supported(exe: &Executable) -> bool {
    exe.sha256 == EXECUTABLE_SHA256
}

fn word(exe: &Executable, va: u32) -> Result<u32> {
    let b = exe.bytes_at(va, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn c_string(exe: &Executable, va: u32) -> Result<String> {
    let off = exe.va_to_offset(va).ok_or(Error::BadExecutable("string outside the executable"))?;
    let rest = &exe.data[off..];
    let end = rest.iter().position(|&b| b == 0).ok_or(Error::BadExecutable("unterminated string"))?;
    Ok(rest[..end].iter().map(|&b| b as char).collect())
}

/// The 40 world slots (disc.md 5.1).
pub fn world_slots(exe: &Executable) -> Result<Vec<WorldSlot>> {
    (0..WORLD_SLOTS)
        .map(|slot| {
            let s = slot as u32;
            let name = c_string(exe, word(exe, WORLD_NAMES_VA + s * 4)?)?;
            let episode = word(exe, WORLD_EPISODES_VA + s * 4)? as i32;
            let file = FileRecord::decode(exe.bytes_at(WORLD_RECORDS_VA + s * 64, 64)?);
            Ok(WorldSlot { slot, name, episode, file })
        })
        .collect()
}

/// The 23 movie records (disc.md 5.2).
pub fn movie_table(exe: &Executable) -> Result<Vec<FileRecord>> {
    let (va, n) = MOVIE_TABLE;
    let raw = exe.bytes_at(va, n * FileRecord::SIZE)?;
    Ok(raw.as_chunks::<{ FileRecord::SIZE }>().0.iter().map(|r| FileRecord::decode(r)).collect())
}

/// The option table (2,044 entries).
pub fn option_table(exe: &Executable) -> Result<OptionTable> {
    let (va, n) = OPTION_TABLE;
    Ok(OptionTable::from_raw(exe.bytes_at(va, n * OPTION_ENTRY_SIZE)?))
}

/// The class table (249 entries).
pub fn class_table(exe: &Executable) -> Result<Vec<ClassEntry>> {
    let (va, n) = CLASS_TABLE;
    let raw = exe.bytes_at(va, n * CLASS_ENTRY_SIZE)?;
    Ok(raw
        .as_chunks::<CLASS_ENTRY_SIZE>()
        .0
        .iter()
        .map(|e| {
            let w = |o: usize| u32::from_le_bytes([e[o], e[o + 1], e[o + 2], e[o + 3]]);
            ClassEntry { base: w(0), class: w(4) as i32, last_derived: w(8) as i32, size: w(12), constructor: w(16) }
        })
        .collect())
}

/// The three packed-tangent multipliers (60, 10, 10 on the disc).
pub fn tangent_multipliers(exe: &Executable) -> Result<[f32; 3]> {
    let b = exe.bytes_at(TANGENT_MULTIPLIERS_VA, 12)?;
    let f = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    Ok([f(0), f(4), f(8)])
}
