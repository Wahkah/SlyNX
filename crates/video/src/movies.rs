//! The movie lists: Sly 1's movie and title tables (specs/sly1/movies.md 1), Sly 2's file-table records
//! (specs/sly2/disc.md 5) and Sly 3's movie table (specs/sly3/disc.md 5.2, read by `formats-sly3`).

use alloc::string::String;
use alloc::vec::Vec;

use formats::disc::{Descriptor, Executable, MOVIE_TABLE};
use formats::iso::ReadAt;

use crate::Error;

/// Virtual address of the 16 title pointers (movies.md 1.2). Title `i` belongs to movie table entry `i + 1`.
pub const TITLE_TABLE: (u32, usize) = (0x0026_99C0, 16);

/// One entry of the movie table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovieEntry {
    /// Position in the movie table (0 to 16).
    pub index: usize,
    pub descriptor: Descriptor,
    /// The game's title for the movie (none for entry 0, the start-up logo).
    pub title: Option<String>,
}

impl MovieEntry {
    /// The movie's identity (the name the game uses for it).
    pub fn identity(&self) -> u32 {
        self.descriptor.identity
    }
}

/// Reads the movie table and the titles from the executable.
pub fn movie_table(exe: &Executable) -> Result<Vec<MovieEntry>, Error> {
    let descriptors = exe.descriptor_table(MOVIE_TABLE.0, MOVIE_TABLE.1)?;
    let mut out = Vec::with_capacity(descriptors.len());
    for (index, d) in descriptors.into_iter().enumerate() {
        let descriptor = d.ok_or(Error::Container { offset: 0, what: "empty movie table entry" })?;
        let title = if index == 0 || index > TITLE_TABLE.1 {
            None
        } else {
            let p = exe.bytes_at(TITLE_TABLE.0 + 4 * (index as u32 - 1), 4)?;
            Some(exe.c_string(u32::from_le_bytes([p[0], p[1], p[2], p[3]]))?)
        };
        out.push(MovieEntry { index, descriptor, title });
    }
    Ok(out)
}

/// The Sly 2 file-table records that hold movies (specs/sly2/disc.md 5): 24 records, 23 distinct (23 and 27 are
/// the same file).
pub const SLY2_MOVIE_RECORDS: [usize; 24] =
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 23, 27, 28];

/// The distinct movies among `records` (by start sector, the first record kept), with their record index.
pub fn distinct_movies(
    records: &[formats_sly2::record::FileRecord],
    indices: impl IntoIterator<Item = usize>,
) -> Vec<(usize, formats_sly2::record::FileRecord)> {
    let mut out: Vec<(usize, formats_sly2::record::FileRecord)> = Vec::new();
    for i in indices {
        if let Some(r) = records.get(i).filter(|r| !r.is_empty())
            && !out.iter().any(|(_, o)| o.sector == r.sector)
        {
            out.push((i, *r));
        }
    }
    out
}

/// Sly 2's 23 distinct movies from the executable's file table (`formats_sly2::Disc::files`).
pub fn sly2_movies(files: &[formats_sly2::record::FileRecord]) -> Vec<(usize, formats_sly2::record::FileRecord)> {
    distinct_movies(files, SLY2_MOVIE_RECORDS)
}

/// Sly 3's 21 distinct movies from the executable's movie table (`formats_sly3::Disc::movies`, 23 records).
pub fn sly3_movies(table: &[formats_sly2::record::FileRecord]) -> Vec<(usize, formats_sly2::record::FileRecord)> {
    distinct_movies(table, 0..table.len())
}

/// A byte range of a disc image (one movie file), readable at offsets relative to its start.
pub struct Region<R: ReadAt> {
    src: R,
    start: u64,
    len: u64,
}

impl<R: ReadAt> Region<R> {
    pub fn new(src: R, start: u64, len: u64) -> Self {
        Region { src, start, len }
    }

    /// The region a Sly 2 or Sly 3 file record names.
    pub fn of_file_record(src: R, f: &formats_sly2::record::FileRecord) -> Self {
        Region::new(src, f.offset(), u64::from(f.size))
    }

    /// The region a descriptor names.
    pub fn of_descriptor(src: R, d: &Descriptor) -> Self {
        Region::new(src, d.offset(), u64::from(d.size))
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Reads up to `buf.len()` bytes at `offset`; returns how many (0 at the end).
    pub fn read(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, Error> {
        let n = (self.len.saturating_sub(offset)).min(buf.len() as u64) as usize;
        if n > 0 {
            self.src.read_at(self.start + offset, &mut buf[..n])?;
        }
        Ok(n)
    }
}
