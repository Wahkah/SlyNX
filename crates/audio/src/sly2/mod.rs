//! Sly 2 sound (specs/sly2/audio.md, movies.md): the disc and world readers sound needs, the world's sound
//! section, `VPK ` stream files, voice lines and movie soundtracks. Effect banks of version 3 are read by
//! [`crate::bank::EffectBank::parse`], and the engine plays them like Sly 1's. [`WorldAudio`] loads and installs a
//! world's sound; [`talk`] decodes the dialogue commands and plays lines with their subtitles.

pub mod disc;
pub mod movie;
pub mod music;
pub mod music_queue;
pub mod sound;
pub mod talk;
pub mod vpk;
pub mod world;

pub use disc::{FileRecord, LevelRow, Sly2Disc, WorldHeader};
pub use movie::MovieAudio;
pub use music::{CharacterMusicState, Falloff, MusicMood};
pub use music_queue::MusicQueue;
pub use sound::{CueValue, LineCues, TimedRecord, VoiceLine, VoiceTrack, WorldSound};
pub use talk::{Command as TalkCommand, Subtitle, Talk};
pub use vpk::{VpkHeader, VpkStream};
pub use world::WorldAudio;
