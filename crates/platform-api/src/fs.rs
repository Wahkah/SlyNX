//! File access: read-only game data and a writable user folder.
//!
//! Paths are always relative to a [`Root`] and use `/` on every platform. [`validate`] rejects anything that could
//! escape the root, so callers can build paths from data without worrying about `..` tricks.

use alloc::vec::Vec;
use core::fmt;

/// Where a path is resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Root {
    /// Data packed into the program (Switch: the `.nro`'s RomFS at `romfs:/`; PC: a folder, `romfs/` by default).
    RomFs,
    /// The SD card, read-only through this API (Switch: `sdmc:/`; PC: a folder, `sdmc/` by default). Converted game
    /// data lives here.
    Sd,
    /// The writable user folder for saves and settings (Switch: `sdmc:/switch/SlyNX/user/`; PC: the user's data
    /// folder, see `docs/platform.md`).
    User,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FsError {
    NotFound,
    /// The path is not relative, contains `..`, `.`, empty parts, `\` or `:`, or is longer than [`MAX_PATH`].
    BadPath,
    /// The root is not available (Switch: the program has no RomFS).
    NoRoot,
    PermissionDenied,
    /// The file ended before the requested range.
    UnexpectedEof,
    /// The file does not fit in memory or in `usize`.
    TooLarge,
    /// Any other error; the code is the backend's (`errno` on both).
    Io(i32),
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsError::NotFound => f.write_str("file not found"),
            FsError::BadPath => f.write_str("invalid path"),
            FsError::NoRoot => f.write_str("storage root not available"),
            FsError::PermissionDenied => f.write_str("permission denied"),
            FsError::UnexpectedEof => f.write_str("unexpected end of file"),
            FsError::TooLarge => f.write_str("file too large"),
            FsError::Io(c) => write!(f, "I/O error {c}"),
        }
    }
}

/// Longest accepted relative path, in bytes.
pub const MAX_PATH: usize = 512;

/// Checks that `path` is a plain relative path: non-empty `/`-separated parts, none of them `.` or `..`, no `\`,
/// `:` or NUL, no leading or trailing `/`, at most [`MAX_PATH`] bytes.
pub fn validate(path: &str) -> Result<(), FsError> {
    if path.is_empty() || path.len() > MAX_PATH {
        return Err(FsError::BadPath);
    }
    if path.bytes().any(|b| b == b'\\' || b == b':' || b == 0) {
        return Err(FsError::BadPath);
    }
    for part in path.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(FsError::BadPath);
        }
    }
    Ok(())
}

/// A file opened for reading. Reads are positioned (`read_at`), so one handle can serve out-of-order requests and
/// a sequential reader just advances its own offset.
pub trait DataFile {
    /// File size in bytes.
    fn len(&self) -> u64;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Reads up to `buf.len()` bytes at `offset`. Returns the number read; 0 means end of file.
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, FsError>;

    /// Fills `buf` from `offset`, or fails with [`FsError::UnexpectedEof`].
    fn read_exact_at(&mut self, mut offset: u64, mut buf: &mut [u8]) -> Result<(), FsError> {
        while !buf.is_empty() {
            let n = self.read_at(offset, buf)?;
            if n == 0 {
                return Err(FsError::UnexpectedEof);
            }
            offset += n as u64;
            buf = &mut buf[n..];
        }
        Ok(())
    }
}

/// Access to the three roots. Implementations are cheap handles that can be cloned and sent to other threads.
pub trait FileSystem {
    /// A read-only file.
    type File: DataFile + Send;

    /// Opens `path` (relative, `/`-separated, see [`validate`]) under `root` for reading.
    fn open(&self, root: Root, path: &str) -> Result<Self::File, FsError>;

    /// True if `path` exists under `root` (file or directory).
    fn exists(&self, root: Root, path: &str) -> bool;

    /// Reads a whole file in one sequential pass.
    fn read(&self, root: Root, path: &str) -> Result<Vec<u8>, FsError> {
        let mut file = self.open(root, path)?;
        let len = usize::try_from(file.len()).map_err(|_| FsError::TooLarge)?;
        let mut data = Vec::new();
        data.try_reserve_exact(len).map_err(|_| FsError::TooLarge)?;
        data.resize(len, 0);
        file.read_exact_at(0, &mut data)?;
        Ok(data)
    }

    /// Writes `data` to `path` in the writable user folder ([`Root::User`]), creating parent folders. The data goes
    /// to `<path>.tmp` first, which then replaces `path`; if the program dies between the two steps, `open` of
    /// `path` in [`Root::User`] falls back to the temporary file, so a reader sees either the old or the new
    /// contents, never a mix.
    fn write_user(&self, path: &str, data: &[u8]) -> Result<(), FsError>;

    /// Deletes a file in the user folder. Deleting a file that does not exist is not an error.
    fn remove_user(&self, path: &str) -> Result<(), FsError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    #[test]
    fn validation() {
        for ok in ["a", "levels/jb_intro.bin", "user/save 1.dat", "a/b/c.d"] {
            assert_eq!(validate(ok), Ok(()), "{ok}");
        }
        let long = String::from("a").repeat(MAX_PATH + 1);
        for bad in ["", "/abs", "a/", "a//b", "../x", "a/../b", "./a", "a\\b", "c:x", "sdmc:/x", "a\0b", &long] {
            assert_eq!(validate(bad), Err(FsError::BadPath), "{bad:?}");
        }
    }

    struct Mem(&'static [u8], usize);

    impl DataFile for Mem {
        fn len(&self) -> u64 {
            self.0.len() as u64
        }
        fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize, FsError> {
            // Short reads of at most `self.1` bytes, to exercise the loop.
            let start = (offset as usize).min(self.0.len());
            let n = buf.len().min(self.0.len() - start).min(self.1);
            buf[..n].copy_from_slice(&self.0[start..start + n]);
            Ok(n)
        }
    }

    #[test]
    fn read_exact_loops_and_detects_eof() {
        let mut f = Mem(b"hello world", 3);
        let mut buf = [0u8; 5];
        f.read_exact_at(6, &mut buf).unwrap();
        assert_eq!(&buf, b"world");
        assert_eq!(f.read_exact_at(8, &mut buf), Err(FsError::UnexpectedEof));
        assert!(!f.is_empty());
    }
}
