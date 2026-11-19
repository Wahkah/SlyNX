//! The crate's error type.

use alloc::string::String;
use core::fmt;

/// Result alias used throughout the crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Everything that can go wrong while reading a disc or a world file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A read ran past the end of the data: `needed` bytes at `offset`, only `len` bytes in total.
    Eof { offset: usize, needed: usize, len: usize },
    /// An I/O error from the host (PC file access).
    Io(String),
    /// The image is not an ISO 9660 image this crate understands.
    BadIso(&'static str),
    /// A file is missing from the disc's root directory.
    FileNotFound(String),
    /// The executable is not a valid ELF, or its layout is not the expected one.
    BadExecutable(&'static str),
    /// The executable's SHA-256 is not the one of the supported release.
    UnknownExecutable,
    /// Data that breaks a rule of the format, at `offset`.
    Corrupt { offset: usize, what: &'static str },
    /// An option id whose value encoding is not in the [`crate::options::OptionTable`] in use.
    UnknownOption { offset: usize, id: i16 },
    /// A class id with no known body layout.
    UnknownClass { offset: usize, class: i16 },
    /// An index into a table was out of range.
    OutOfRange { what: &'static str, index: usize, len: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Eof { offset, needed, len } => {
                write!(f, "unexpected end of data: {needed} bytes at offset {offset:#x}, data is {len:#x} bytes")
            }
            Error::Io(e) => write!(f, "i/o error: {e}"),
            Error::BadIso(why) => write!(f, "not a supported ISO 9660 image: {why}"),
            Error::FileNotFound(name) => write!(f, "file {name} not found on the disc"),
            Error::BadExecutable(why) => write!(f, "bad executable: {why}"),
            Error::UnknownExecutable => write!(f, "unknown executable (not Sly Cooper USA, SCUS-97198)"),
            Error::Corrupt { offset, what } => write!(f, "corrupt data at offset {offset:#x}: {what}"),
            Error::UnknownOption { offset, id } => {
                write!(f, "option id {id} at offset {offset:#x} has no known encoding")
            }
            Error::UnknownClass { offset, class } => {
                write!(f, "class id {class} at offset {offset:#x} has no known layout")
            }
            Error::OutOfRange { what, index, len } => write!(f, "{what} index {index} out of range (len {len})"),
        }
    }
}

impl core::error::Error for Error {}

#[cfg(feature = "std")]
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        use alloc::string::ToString;
        Error::Io(e.to_string())
    }
}
