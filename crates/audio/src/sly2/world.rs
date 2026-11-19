//! A Sly 2 world's sound, ready for the [`SoundEngine`] (specs/sly2/audio.md, dialogue-and-voice.md): the effect
//! bank (version 3) with the sound-ID map and flag bytes, the world stream, and the voice lines with their
//! subtitles. [`Sly2Disc::load_world_audio`] reads them; [`WorldAudio::install`] hands them to the engine;
//! [`Sly2Disc::read_line`] reads a line's voice file for [`crate::sly2::talk::Talk`].

use alloc::sync::Arc;
use alloc::vec::Vec;

use formats::iso::ReadAt;

use crate::Error;
use crate::bank::EffectBank;
use crate::engine::{SoundEngine, SoundHandle};
use crate::sly2::talk::TALK_TARGETS_VA;
use crate::sly2::vpk::VpkStream;
use crate::sly2::{Sly2Disc, VoiceLine, WorldHeader, WorldSound};
use crate::vag::VagSound;

/// One world's sound, read from the disc.
#[derive(Clone)]
pub struct WorldAudio {
    /// The level row.
    pub row: usize,
    /// The world header through the string table (its sound section, level table and strings).
    pub header: WorldHeader,
    /// The effect bank, when the world has one.
    pub bank: Option<Arc<EffectBank>>,
    /// The world stream (distance layers), when the world has one.
    pub stream: Option<Arc<VpkStream>>,
}

impl WorldAudio {
    /// The sound section.
    pub fn sound(&self) -> &WorldSound {
        &self.header.sound
    }

    /// Installs the world's sound in `engine`: the bank in slot 0, the sound-ID map and flag bytes, and the stream
    /// as the resident stream. Returns the stream's handle (`SoundHandle::NONE` without one).
    pub fn install(&self, engine: &mut SoundEngine) -> SoundHandle {
        engine.load_bank(0, self.bank.clone());
        engine.set_sound_map(Some(self.header.sound.sound_map()));
        engine.set_sound_flags(Some(self.header.sound.flags.clone()));
        match &self.stream {
            Some(s) => engine.play_resident(s.clone(), self.header.sound.resident_channels.clone()),
            None => SoundHandle::NONE,
        }
    }

    /// Voice line `number` of this world.
    pub fn line(&self, number: i16) -> Option<&VoiceLine> {
        self.header.sound.voice_line(number)
    }

    /// The subtitle of line `number`: the world string with that id (dialogue-and-voice.md 4).
    pub fn subtitle(&self, number: i16) -> Option<&[u8]> {
        self.header.string(number)
    }
}

impl<R: ReadAt> Sly2Disc<R> {
    /// Reads level row `row`'s sound: the header (sound section, strings), the effect bank and the world stream.
    pub fn load_world_audio(&mut self, row: usize) -> Result<WorldAudio, Error> {
        let world = self.read_world(row)?;
        let header = WorldHeader::parse(&world)?;
        let bank = match header.sound.bank {
            Some(r) => Some(Arc::new(self.read_bank(&r)?)),
            None => None,
        };
        let stream = match header.sound.resident {
            Some(r) => Some(Arc::new(self.read_vpk(&r)?)),
            None => None,
        };
        Ok(WorldAudio { row, header, bank, stream })
    }

    /// Reads the voice file of line `number` (first track); `None` when the world has no such line or the track
    /// names no file.
    pub fn read_line(&mut self, audio: &WorldAudio, number: i16) -> Result<Option<VagSound>, Error> {
        match audio.line(number).and_then(|l| l.tracks.first()).and_then(|t| t.file) {
            Some(r) => self.read_vag(&r).map(Some),
            None => Ok(None),
        }
    }

    /// The talk-target table (dialogue-and-voice.md 4): the state-machine object IDs timed-record targets 0 to 5 set
    /// goals on (−1 for target 3, the eyes).
    pub fn talk_targets(&self) -> Result<[i32; 6], Error> {
        let b = self.bytes_at(TALK_TARGETS_VA, 24)?;
        let mut out = [0i32; 6];
        for (i, o) in out.iter_mut().enumerate() {
            *o = i32::from_le_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
        }
        Ok(out)
    }
}

/// The voice lines of a world that have a file (first track).
pub fn voiced_lines(sound: &WorldSound) -> Vec<i16> {
    sound.voice_lines.iter().filter(|l| l.tracks.first().is_some_and(|t| t.file.is_some())).map(|l| l.number).collect()
}
