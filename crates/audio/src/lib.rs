//! Sly 1 sound: SPU ADPCM and VAG decoding, effect banks played through their steps, the adaptive music sequencer,
//! dialogue and ambient streams, positioned sounds, and a real-time mixer that implements the platform's
//! [`platform_api::AudioRender`].
//!
//! Written from `specs/sly1/audio.md`. `no_std` with `alloc`, no third-party crates. The `std` feature (default)
//! only adds opening a disc image by path through `formats`.
//!
//! | Module | What it does |
//! |---|---|
//! | [`adpcm`] | SPU ADPCM blocks, one-pass decoding, the block cursor voices play with |
//! | [`vag`] | VAG headers and whole VAG sounds |
//! | [`envelope`] | the SPU volume envelope |
//! | [`bank`] | effect banks: sound records, steps, voice parameters |
//! | [`music`] | music files: instruments, key regions, segments, events, adaptive commands |
//! | [`effects`] | sound instances stepping through their steps |
//! | [`sequencer`] | the music sequencer |
//! | [`mixer`] | voice pool, resampling, envelopes, reverb |
//! | [`spatial`] | distance, pan from position, Doppler |
//! | [`engine`] | [`SoundEngine`] (game thread) and [`AudioRenderer`] (audio thread) |
//! | [`disc`] | finding the sound data on the player's disc |
//!
//! ```no_run
//! # use std::sync::Arc;
//! # fn demo(platform: &mut impl platform_api::Platform, bank0: audio::bank::EffectBank) {
//! use audio::{Config, Emitter, create};
//! let (mut sound, renderer) = create(Config::default());
//! platform.start_audio(Box::new(renderer)).ok();
//! sound.load_bank(0, Some(Arc::new(bank0)));
//! let h = sound.play(12, Emitter::at([0.0, 0.0, 0.0], 2000.0, 300.0));
//! // every frame:
//! sound.update(1.0 / 60.0);
//! sound.stop(h);
//! # }
//! ```

#![no_std]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod adpcm;
pub mod bank;
pub mod disc;
pub mod effects;
pub mod engine;
pub mod envelope;
pub mod fmath;
pub mod mixer;
pub mod music;
pub mod pcm;
pub mod queue;
pub mod rng;
pub mod sequencer;
pub mod sly2;
pub mod sly3;
pub mod spatial;
pub mod vag;

pub use engine::{
    AudioRenderer, Config, INTENSITY_ALARM, INTENSITY_HIDING, IntensityRequest, Intermittent, SoundEngine, SoundHandle,
    create, group,
};
pub use spatial::{Emitter, Listener, Mix};

use core::fmt;

/// Errors from parsing sound data or reading the disc.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The data ends before `needed` bytes (`len` available).
    Truncated {
        /// What was being read.
        what: &'static str,
        /// Bytes needed.
        needed: usize,
        /// Bytes available.
        len: usize,
    },
    /// A magic tag is missing.
    BadMagic(&'static str),
    /// A value breaks a rule of the format.
    Invalid(&'static str),
    /// From the disc reader.
    Disc(formats::error::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Truncated { what, needed, len } => write!(f, "{what}: needs {needed} bytes, data has {len}"),
            Error::BadMagic(m) => write!(f, "missing magic {m:?}"),
            Error::Invalid(why) => write!(f, "invalid sound data: {why}"),
            Error::Disc(e) => write!(f, "{e}"),
        }
    }
}

impl From<formats::error::Error> for Error {
    fn from(e: formats::error::Error) -> Self {
        Error::Disc(e)
    }
}

/// Bounds-checked little-endian reads.
pub(crate) mod bytes {
    use super::Error;

    fn get<const N: usize>(b: &[u8], off: usize) -> Result<[u8; N], Error> {
        off.checked_add(N).and_then(|end| b.get(off..end)).and_then(|s| s.try_into().ok()).ok_or(Error::Truncated {
            what: "field",
            needed: off.saturating_add(N),
            len: b.len(),
        })
    }

    pub fn u16_at(b: &[u8], off: usize) -> Result<u16, Error> {
        get::<2>(b, off).map(u16::from_le_bytes)
    }

    pub fn i16_at(b: &[u8], off: usize) -> Result<i16, Error> {
        get::<2>(b, off).map(i16::from_le_bytes)
    }

    pub fn u32_at(b: &[u8], off: usize) -> Result<u32, Error> {
        get::<4>(b, off).map(u32::from_le_bytes)
    }

    pub fn i32_at(b: &[u8], off: usize) -> Result<i32, Error> {
        get::<4>(b, off).map(i32::from_le_bytes)
    }

    pub fn i8_at(b: &[u8], off: usize) -> Result<i8, Error> {
        get::<1>(b, off).map(|a| a[0] as i8)
    }

    pub fn magic(b: &[u8], off: usize, tag: &'static [u8; 4]) -> Result<(), Error> {
        if get::<4>(b, off)? == *tag {
            Ok(())
        } else {
            Err(Error::BadMagic(match tag {
                b"SBlk" => "SBlk",
                b"SBv2" => "SBv2",
                b"MMID" => "MMID",
                b"MID " => "MID ",
                _ => "tag",
            }))
        }
    }
}
