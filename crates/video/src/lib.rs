//! MPEG-2 video decoding and movie playback for the Sly 1 movies (specs/sly1/movies.md).
//!
//! - [`demux`]: program-stream demultiplexing of the game's PSS files (ISO/IEC 13818-1).
//! - [`audio`]: the `SShd`/`SSbd` PCM audio stream, de-blocked to interleaved stereo.
//! - [`decoder`]: an MPEG-2 video decoder (ITU-T H.262 | ISO/IEC 13818-2, Main Profile, 4:2:0) producing frames in
//!   display order.
//! - [`frame`]: decoded Y/Cb/Cr planes, conversion to RGBA8 and the matrix for converting on the GPU.
//! - [`player`]: timing against an audio clock, frame dropping, skip and end of movie.
//! - [`movies`]: the movie and title tables read from the player's own executable.
//!
//! Written from the public standards and the spec; `no_std` with `alloc`, no third-party crates.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod audio;
mod bits;
pub mod decoder;
pub mod demux;
pub mod frame;
pub mod headers;
pub mod idct;
mod mc;
pub mod movies;
pub mod player;
mod slice;
pub mod tables;
pub mod vlc;

pub use decoder::{Decoded, Decoder, DecoderStats, Parallel};
pub use frame::{ColorMatrix, Frame, FrameInfo, Planes};
pub use headers::{PictureType, SequenceHeader};
pub use player::{MoviePlayer, PlayerStatus};
pub use slice::MbStats;

use core::fmt;

/// Errors from the demultiplexer, the audio parser and the decoder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The video breaks a rule of ISO/IEC 13818-2; the text says which.
    Bitstream(&'static str),
    /// A feature these movies do not use and the decoder does not implement.
    Unsupported(&'static str),
    /// The program stream is malformed at `offset` (from the start of the stream).
    Container { offset: u64, what: &'static str },
    /// The audio stream's header is malformed.
    Audio(&'static str),
    /// Reading the disc failed.
    Disc(formats::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Bitstream(w) => write!(f, "invalid MPEG-2 video: {w}"),
            Error::Unsupported(w) => write!(f, "unsupported: {w}"),
            Error::Container { offset, what } => write!(f, "invalid program stream at {offset:#x}: {what}"),
            Error::Audio(w) => write!(f, "invalid movie audio: {w}"),
            Error::Disc(e) => write!(f, "disc: {e}"),
        }
    }
}

impl core::error::Error for Error {}

impl From<formats::Error> for Error {
    fn from(e: formats::Error) -> Self {
        Error::Disc(e)
    }
}
