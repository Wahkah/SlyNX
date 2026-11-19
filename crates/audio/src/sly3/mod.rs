//! Sly 3 sound (specs/sly3/audio.md, movies.md): what differs from Sly 2 is where things are. The disc keeps its
//! data in `SLY3.WAD` (read by sector, like Sly 2's unnamed sectors), the world list and the movie list are tables
//! in the executable, and a world's header has new sections before its sound section. Everything sound itself is
//! Sly 2's format: the sound section ([`crate::sly2::WorldSound`]), version-3 effect banks, `VPK ` streams (44.1 kHz
//! in Sly 3), voice lines and movie soundtracks ([`crate::sly2::MovieAudio`]). [`WorldAudio`] reads a world's
//! bank and stream and installs them in the [`crate::SoundEngine`].

pub mod disc;
pub mod talk;
pub mod world;

pub use disc::{Group, Sly3Disc, WorldHeader, WorldSlot};
pub use world::WorldAudio;
