//! A Sly 3 world's sound, ready for the [`SoundEngine`] (specs/sly3/audio.md): the effect bank (version 3) with its
//! sound-ID map and flag bytes, the world stream (44.1 kHz, its channels the music layers) and the voice and music
//! files the sound section names. Loading reads the disc; [`WorldAudio::install`] hands everything to the engine.
//!
//! The flag byte 2 marks a continuous sound (as Sly 2). The new value 1 (448 sounds) is passed through unchanged;
//! the engine treats it as an ordinary sound until the spec gives it a meaning.

use alloc::sync::Arc;
use alloc::vec::Vec;

use formats::iso::ReadAt;

use crate::Error;
use crate::bank::EffectBank;
use crate::engine::{SoundEngine, SoundHandle};
use crate::sly2::vpk::VpkStream;
use crate::sly2::{FileRecord, WorldSound};
use crate::sly3::{Sly3Disc, WorldHeader};
use crate::vag::VagSound;

/// The new per-sound flag value of Sly 3 (meaning open, audio.md 4).
pub const FLAG_SLY3_NEW: u8 = 1;

/// One world's sound, read from the disc.
#[derive(Clone)]
pub struct WorldAudio {
    /// The world slot.
    pub slot: usize,
    /// The world's sound section.
    pub sound: WorldSound,
    /// The world's strings (first language): (id, text).
    pub strings: Vec<(i16, alloc::string::String)>,
    /// The effect bank, when the world has one (all 39 do).
    pub bank: Option<Arc<EffectBank>>,
    /// The world stream, when the world has one (32 of 39).
    pub stream: Option<Arc<VpkStream>>,
}

impl WorldAudio {
    /// Installs the world's sound in `engine`: the bank in slot 0, the sound-ID map and flag bytes, and the world
    /// stream as the resident stream (its channels at their stored volumes until fall-offs or layer gains change
    /// them). Returns the stream's handle (`SoundHandle::NONE` without a stream).
    pub fn install(&self, engine: &mut SoundEngine) -> SoundHandle {
        engine.load_bank(0, self.bank.clone());
        engine.set_sound_map(Some(self.sound.sound_map()));
        engine.set_sound_flags(Some(self.sound.flags.clone()));
        match &self.stream {
            Some(s) => engine.play_resident(s.clone(), self.sound.resident_channels.clone()),
            None => SoundHandle::NONE,
        }
    }

    /// The game sound IDs this world's bank plays, in bank order (−1 where a bank sound has none).
    pub fn sound_ids(&self) -> &[i16] {
        &self.sound.sound_ids
    }

    /// Voice line `number` of this world.
    pub fn line(&self, number: i16) -> Option<&crate::sly2::VoiceLine> {
        self.sound.voice_line(number)
    }

    /// The subtitle of line `number`, by Sly 2's rule (the string whose id is the line number; see
    /// [`crate::sly3::talk`]).
    pub fn subtitle(&self, number: i16) -> Option<&str> {
        self.strings.iter().find(|s| s.0 == number).map(|s| s.1.as_str())
    }

    /// Whether bank sound `j` is continuous by its flag byte.
    pub fn continuous(&self, j: usize) -> bool {
        self.sound.continuous(j)
    }
}

impl<R: ReadAt> Sly3Disc<R> {
    /// Reads world `slot`'s sound: its header's sound section, the effect bank and the world stream.
    pub fn load_world_audio(&mut self, slot: usize) -> Result<WorldAudio, Error> {
        let world = self.read_world(slot)?;
        let header = WorldHeader::parse(&world)?;
        let (sound, strings) = (header.sound, header.strings);
        let bank = match sound.bank {
            Some(r) => Some(Arc::new(self.read_bank(&r)?)),
            None => None,
        };
        let stream = match sound.resident {
            Some(r) => Some(Arc::new(self.read_vpk(&r)?)),
            None => None,
        };
        Ok(WorldAudio { slot, sound, strings, bank, stream })
    }

    /// Reads music stream `n` of a world's sound section (`None` when it names no file).
    pub fn read_music_stream(&mut self, sound: &WorldSound, n: i16) -> Result<Option<VpkStream>, Error> {
        match sound.music_stream(n) {
            Some(r) => self.read_vpk(&r).map(Some),
            None => Ok(None),
        }
    }

    /// Reads voice line `number`'s first track (language 0) of a world's sound section.
    pub fn read_voice_line(&mut self, sound: &WorldSound, number: i16) -> Result<Option<VagSound>, Error> {
        let file: Option<FileRecord> = sound.voice_line(number).and_then(|l| l.tracks.first()).and_then(|t| t.file);
        match file {
            Some(r) => self.read_vag(&r).map(Some),
            None => Ok(None),
        }
    }

    /// The music numbers a world's sound section lists with a file.
    pub fn music_numbers(sound: &WorldSound) -> Vec<i16> {
        sound.music.iter().filter(|m| m.1.is_some()).map(|m| m.0).collect()
    }
}
