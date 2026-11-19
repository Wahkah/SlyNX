//! The per-class mapping from script option IDs to stream option IDs (scripts.md section 8), read from the
//! player's own executable.

use alloc::vec::Vec;
use core::fmt;

use formats::disc::Executable;

/// Address of the per-class table in the supported executable.
pub const TABLE_VA: u32 = 0x0024_4950;
/// File offset of the table in the supported executable (a check on the address mapping).
pub const TABLE_FILE_OFFSET: usize = 0x14_5950;
/// Number of class records in the table.
pub const CLASS_COUNT: usize = 148;
/// Address of the first option descriptor.
pub const DESCRIPTOR_VA: u32 = 0x0025_05D8;
/// Size of one option descriptor.
pub const DESCRIPTOR_SIZE: u32 = 32;
/// The first custom property ID; script option IDs below it name built-in options.
pub const CUSTOM_BASE: u32 = 1173;

/// Why the mapping could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapError {
    /// The executable is not the supported one (SHA-256 mismatch).
    UnsupportedExecutable,
    /// An address outside the executable's loaded segment.
    BadAddress(u32),
    /// A descriptor address that is not a whole descriptor from the base.
    BadDescriptor(u32),
    /// A script option ID of 1173 or more in a class list.
    BadScriptOption(u32),
    /// A class list longer than [`MAX_CLASS_OPTIONS`].
    BadCount(u32),
}

impl fmt::Display for MapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapError::UnsupportedExecutable => f.write_str("unsupported executable (SHA-256 mismatch)"),
            MapError::BadAddress(a) => write!(f, "address {a:#x} outside the executable"),
            MapError::BadDescriptor(a) => write!(f, "option descriptor address {a:#x} is not a descriptor"),
            MapError::BadScriptOption(o) => write!(f, "script option ID {o} out of range"),
            MapError::BadCount(n) => write!(f, "class option count {n} out of range"),
        }
    }
}

/// Where an executable keeps its per-class option map, and how to read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapLayout {
    /// Address of the class records (`u32` count, `u32` list address each).
    pub table_va: u32,
    /// Number of class records.
    pub class_count: usize,
    /// Address of stream option 0's descriptor (or option-table entry).
    pub descriptor_va: u32,
    /// Bytes per descriptor.
    pub descriptor_size: u32,
    /// The first custom property ID: script option IDs in the lists must be below it.
    pub custom_base: u32,
}

impl MapLayout {
    /// Sly 1 (scripts.md 8).
    pub const SLY1: MapLayout = MapLayout {
        table_va: TABLE_VA,
        class_count: CLASS_COUNT,
        descriptor_va: DESCRIPTOR_VA,
        descriptor_size: DESCRIPTOR_SIZE,
        custom_base: CUSTOM_BASE,
    };

    /// Sly 2 (specs/sly2/scripts.md 5): 199 class records at `0x279AD0`, entries of the 40-byte option table at
    /// `0x2AC880`; custom properties from 1,233.
    pub const SLY2: MapLayout = MapLayout {
        table_va: 0x0027_9AD0,
        class_count: 199,
        descriptor_va: 0x002A_C880,
        descriptor_size: 40,
        custom_base: SLY2_CUSTOM_BASE,
    };

    /// Sly 3 (specs/sly3/scripts.md 5): 249 class records at `0x2ED518`, entries of the 40-byte option table at
    /// `0x32CA80`; custom properties from 1,816.
    pub const SLY3: MapLayout = MapLayout {
        table_va: 0x002E_D518,
        class_count: 249,
        descriptor_va: 0x0032_CA80,
        descriptor_size: 40,
        custom_base: SLY3_CUSTOM_BASE,
    };
}

/// The first Sly 2 custom property ID, a fixed threshold in the game (`0x4D1`; specs/sly2/scripts.md 5): getting or
/// setting an ID below it goes through the class map, from it on to the object's property store.
pub const SLY2_CUSTOM_BASE: u32 = 1233;

/// The first Sly 3 custom property ID (IDs up to 1,815, `0x717`, go through the class map; specs/sly3/scripts.md 5).
pub const SLY3_CUSTOM_BASE: u32 = 1816;

/// Script option ID to stream option ID, per class.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OptionMap {
    classes: Vec<Vec<(u16, u16)>>,
    /// List records read from the executable (duplicates included).
    pub records: usize,
}

/// Longest per-class list accepted (every script option ID below 1173 at most once; layouts with a higher custom
/// base allow up to that base).
pub const MAX_CLASS_OPTIONS: usize = CUSTOM_BASE as usize;

fn word(b: &[u8], i: usize) -> u32 {
    match b.get(i..i + 4) {
        Some(w) => u32::from_le_bytes([w[0], w[1], w[2], w[3]]),
        None => 0,
    }
}

/// `read(address, length)`, refusing a short answer.
fn read_exact<'a>(read: &impl Fn(u32, usize) -> Option<&'a [u8]>, va: u32, len: usize) -> Result<&'a [u8], MapError> {
    read(va, len).filter(|b| b.len() >= len).ok_or(MapError::BadAddress(va))
}

impl OptionMap {
    /// Reads the mapping from the supported executable. Refuses any other executable.
    pub fn from_executable(exe: &Executable) -> Result<OptionMap, MapError> {
        if !exe.is_supported() {
            return Err(MapError::UnsupportedExecutable);
        }
        Self::read(|va, len| exe.bytes_at(va, len).ok())
    }

    /// Reads the mapping through `read(address, length)`, which gives bytes of the executable's image. No hash
    /// check: callers must have identified the executable.
    pub fn read<'a>(read: impl Fn(u32, usize) -> Option<&'a [u8]>) -> Result<OptionMap, MapError> {
        Self::read_layout(read, &MapLayout::SLY1)
    }

    /// Reads the Sly 2 mapping from the supported Sly 2 executable. Refuses any other executable.
    pub fn from_sly2_executable(exe: &formats_sly2::exe::Executable) -> Result<OptionMap, MapError> {
        if !exe.is_supported() {
            return Err(MapError::UnsupportedExecutable);
        }
        Self::read_layout(|va, len| exe.bytes_at(va, len).ok(), &MapLayout::SLY2)
    }

    /// Reads the Sly 3 mapping from the supported Sly 3 executable. Refuses any other executable.
    pub fn from_sly3_executable(exe: &formats_sly3::exe::Executable) -> Result<OptionMap, MapError> {
        if !formats_sly3::exe::is_supported(exe) {
            return Err(MapError::UnsupportedExecutable);
        }
        Self::read_layout(|va, len| exe.bytes_at(va, len).ok(), &MapLayout::SLY3)
    }

    /// Reads a mapping laid out as `layout` through `read(address, length)`. No hash check.
    pub fn read_layout<'a>(
        read: impl Fn(u32, usize) -> Option<&'a [u8]>,
        layout: &MapLayout,
    ) -> Result<OptionMap, MapError> {
        let table = read_exact(&read, layout.table_va, layout.class_count * 8)?;
        let mut classes = Vec::with_capacity(layout.class_count);
        let mut records = 0;
        for c in 0..layout.class_count {
            let count = word(table, c * 8);
            let addr = word(table, c * 8 + 4);
            if count as usize > MAX_CLASS_OPTIONS.max(layout.custom_base as usize) {
                return Err(MapError::BadCount(count));
            }
            let count = count as usize;
            let mut list = Vec::with_capacity(count);
            if count > 0 {
                let raw = read_exact(&read, addr, count * 8)?;
                for i in 0..count {
                    let script = word(raw, i * 8);
                    let desc = word(raw, i * 8 + 4);
                    if script >= layout.custom_base {
                        return Err(MapError::BadScriptOption(script));
                    }
                    let rel = desc.checked_sub(layout.descriptor_va).ok_or(MapError::BadDescriptor(desc))?;
                    if rel % layout.descriptor_size != 0 || rel / layout.descriptor_size > u16::MAX as u32 {
                        return Err(MapError::BadDescriptor(desc));
                    }
                    list.push((script as u16, (rel / layout.descriptor_size) as u16));
                }
            }
            records += count;
            classes.push(list);
        }
        let mut map = Self::from_lists(classes);
        map.records = records;
        Ok(map)
    }

    /// A mapping from explicit per-class lists of (script option ID, stream option ID). Lists are sorted here
    /// (stably, so entries naming the same ID keep their order); duplicates are kept for [`OptionMap::lookup`].
    pub fn from_lists(mut classes: Vec<Vec<(u16, u16)>>) -> OptionMap {
        let mut records = 0;
        for l in &mut classes {
            records += l.len();
            l.sort_by_key(|e| e.0);
        }
        OptionMap { classes, records }
    }

    /// Number of classes in the table.
    pub fn class_count(&self) -> usize {
        self.classes.len()
    }

    /// The (script option, stream option) pairs of a class, sorted by script option ID (duplicates included).
    pub fn class(&self, class: i16) -> &[(u16, u16)] {
        usize::try_from(class).ok().and_then(|c| self.classes.get(c)).map_or(&[], |v| v.as_slice())
    }

    /// The stream option a script option ID names for a class: the game's binary search (specs/sly2/scripts.md 5),
    /// halving between the first and last record with the midpoint rounded down and taking the first matching
    /// record it reaches. When a list names an ID twice this is sometimes the second entry (Sly 2 class 98, IDs 112
    /// and 113).
    pub fn lookup(&self, class: i16, script_option: u32) -> Option<u16> {
        let list = self.class(class);
        let key = u16::try_from(script_option).ok()?;
        let (mut lo, mut hi) = (0usize, list.len());
        // `hi` is one past the last record still in range.
        while lo < hi {
            let mid = (lo + hi - 1) / 2;
            match list[mid].0.cmp(&key) {
                core::cmp::Ordering::Equal => return Some(list[mid].1),
                core::cmp::Ordering::Greater => hi = mid,
                core::cmp::Ordering::Less => lo = mid + 1,
            }
        }
        None
    }
}
