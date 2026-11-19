//! The Sly 2 (USA) disc: boot executable, file table, and the level table found in the boot world (disc.md).

use alloc::vec::Vec;

use formats::compress;
use formats::disc::boot_executable_name;
use formats::error::{Error, Result};
use formats::iso::{self, ReadAt};

use crate::exe::{BOOT_WORLD_ENTRY, Executable, OptionTable};
use crate::record::FileRecord;
use crate::world::{LevelRow, read_header_start};

/// An opened Sly 2 disc image.
pub struct Disc<R: ReadAt> {
    src: R,
    pub volume: iso::VolumeInfo,
    pub executable: Executable,
    /// The executable's 29 file records (movies, boot worlds, the save icon).
    pub files: Vec<FileRecord>,
    /// The level table (44 rows; row 18 is empty).
    pub rows: Vec<LevelRow>,
    pub options: OptionTable,
}

impl<R: ReadAt> Disc<R> {
    /// Opens a disc: checks the executable's SHA-256, reads its file table and option table, and reads the level
    /// table from the boot world.
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
        let exe = iso::find_root_file(&mut src, &name)?;
        let executable = Executable::parse(iso::read_file(&mut src, &exe)?)?;
        if check && !executable.is_supported() {
            return Err(Error::UnknownExecutable);
        }
        let files = executable.file_table()?;
        let options = executable.option_table(OptionTable::ENTRIES)?;
        let boot = files.get(BOOT_WORLD_ENTRY).copied().ok_or(Error::BadExecutable("short file table"))?;
        let mut disc = Disc { src, volume, executable, files, rows: Vec::new(), options };
        let world = disc.read_file_decompressed(&boot)?;
        disc.rows = read_header_start(&world)?.level_rows;
        Ok(disc)
    }

    /// The underlying image.
    pub fn source(&mut self) -> &mut R {
        &mut self.src
    }

    /// The raw bytes of a file record.
    pub fn read_file(&mut self, f: &FileRecord) -> Result<Vec<u8>> {
        self.src.read_vec(f.offset(), f.size as usize)
    }

    /// A compressed world file, decompressed.
    pub fn read_file_decompressed(&mut self, f: &FileRecord) -> Result<Vec<u8>> {
        compress::decompress(&self.read_file(f)?)
    }

    /// The compressed bytes of level-table row `row`.
    pub fn read_world_compressed(&mut self, row: usize) -> Result<Vec<u8>> {
        let f = self.row(row)?.file;
        self.read_file(&f)
    }

    /// The decompressed world of level-table row `row`.
    pub fn read_world(&mut self, row: usize) -> Result<Vec<u8>> {
        compress::decompress(&self.read_world_compressed(row)?)
    }

    fn row(&self, row: usize) -> Result<&LevelRow> {
        let r = self.rows.get(row).ok_or(Error::OutOfRange { what: "level row", index: row, len: self.rows.len() })?;
        if r.file.is_empty() {
            return Err(Error::OutOfRange { what: "non-empty level row", index: row, len: self.rows.len() });
        }
        Ok(r)
    }

    /// Rows that hold a world.
    pub fn world_rows(&self) -> impl Iterator<Item = usize> + '_ {
        self.rows.iter().enumerate().filter(|(_, r)| !r.file.is_empty()).map(|(i, _)| i)
    }

    /// The row of the world with id `world_id`.
    pub fn find_world(&self, world_id: u32) -> Option<usize> {
        self.rows.iter().position(|r| !r.file.is_empty() && r.world_id() == world_id)
    }
}

#[cfg(feature = "std")]
impl Disc<std::fs::File> {
    /// Opens a disc image file.
    pub fn open_path(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Disc::open(std::fs::File::open(path)?)
    }
}
