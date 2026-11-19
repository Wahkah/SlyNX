//! The Sly 3 (USA) disc (disc.md): the boot executable with its world, movie, option and class tables. The game
//! data is in `SLY3.WAD`, addressed by file records with the Sly 2 sector base, so it is read by sector.

use alloc::vec::Vec;

use formats::compress;
use formats::disc::boot_executable_name;
use formats::error::{Error, Result};
use formats::iso::{self, ReadAt};
use formats_sly2::exe::OptionTable;
use formats_sly2::record::FileRecord;

use crate::exe::{self, ClassEntry, Executable, WorldSlot};

/// An opened Sly 3 disc image.
pub struct Disc<R: ReadAt> {
    src: R,
    pub volume: iso::VolumeInfo,
    pub executable: Executable,
    /// The 40 world slots (slot 1 is empty).
    pub worlds: Vec<WorldSlot>,
    /// The 23 movie records.
    pub movies: Vec<FileRecord>,
    pub options: OptionTable,
    pub classes: Vec<ClassEntry>,
    /// The packed-tangent multipliers (animation.md 2.1).
    pub tangent_multipliers: [f32; 3],
}

impl<R: ReadAt> Disc<R> {
    /// Opens a disc: checks the executable's SHA-256 and reads its tables.
    pub fn open(src: R) -> Result<Self> {
        Self::open_inner(src, true)
    }

    /// As [`Disc::open`] without the hash check.
    pub fn open_unchecked(src: R) -> Result<Self> {
        Self::open_inner(src, false)
    }

    fn open_inner(mut src: R, check: bool) -> Result<Self> {
        let volume = iso::read_volume(&mut src)?;
        let cnf = iso::find_root_file(&mut src, "SYSTEM.CNF")?;
        let cnf = iso::read_file(&mut src, &cnf)?;
        let name = boot_executable_name(&cnf)?;
        let entry = iso::find_root_file(&mut src, &name)?;
        let executable = Executable::parse(iso::read_file(&mut src, &entry)?)?;
        if check && !exe::is_supported(&executable) {
            return Err(Error::UnknownExecutable);
        }
        Ok(Disc {
            volume,
            worlds: exe::world_slots(&executable)?,
            movies: exe::movie_table(&executable)?,
            options: exe::option_table(&executable)?,
            classes: exe::class_table(&executable)?,
            tangent_multipliers: exe::tangent_multipliers(&executable)?,
            executable,
            src,
        })
    }

    /// The underlying image.
    pub fn source(&mut self) -> &mut R {
        &mut self.src
    }

    /// The raw bytes of a file record.
    pub fn read_file(&mut self, f: &FileRecord) -> Result<Vec<u8>> {
        self.src.read_vec(f.offset(), f.size as usize)
    }

    fn slot(&self, slot: usize) -> Result<&WorldSlot> {
        let w = self.worlds.get(slot).ok_or(Error::OutOfRange { what: "world slot", index: slot, len: 40 })?;
        if w.file.is_empty() {
            return Err(Error::OutOfRange { what: "non-empty world slot", index: slot, len: 40 });
        }
        Ok(w)
    }

    /// The compressed world of `slot`.
    pub fn read_world_compressed(&mut self, slot: usize) -> Result<Vec<u8>> {
        let f = self.slot(slot)?.file;
        self.read_file(&f)
    }

    /// The decompressed world of `slot`.
    pub fn read_world(&mut self, slot: usize) -> Result<Vec<u8>> {
        compress::decompress(&self.read_world_compressed(slot)?)
    }

    /// Slots that hold a world.
    pub fn world_slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.worlds.iter().filter(|w| !w.file.is_empty()).map(|w| w.slot)
    }

    /// The slot named `name`.
    pub fn find_world(&self, name: &str) -> Option<usize> {
        self.worlds.iter().position(|w| w.name == name && !w.file.is_empty())
    }
}

#[cfg(feature = "std")]
impl Disc<std::fs::File> {
    /// Opens a disc image file.
    pub fn open_path(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Disc::open(std::fs::File::open(path)?)
    }
}
