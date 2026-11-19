//! Movies: `Request::Movie` played through the `video` crate (docs/video.md; the decoder is our own, see
//! docs/legal/MOVIE_PLAYBACK.md).
//!
//! The movie files are read from the player's own disc image (packaged with the game, or on the SD card or the PC):
//! the session gets an [`Opener`] that opens the image again, so a movie streams from its own reader while the disc
//! stays with the world loader. While a movie plays the world is paused and gets no input; Start (+) skips it; the
//! game resumes where it was when the movie ends.
//!
//! Without movie data (no opener, an executable without the movie table, a movie the table does not name, or a read
//! or decode error) the request is dropped and the game goes on: movies are never required to play.
//!
//! **Sound** ([`Movies::attach_audio`]): the movie's PCM goes to the sound engine's PCM stream (`audio::pcm`, resampled
//! to 48 kHz by the player), and the frames follow its clock (frames mixed so far); without a sound engine, or once
//! the soundtrack has run out, the real clock times the frames. The renderer has no streamed texture yet, so the
//! screen shows a card with the movie's title.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

use audio::pcm::{PCM_RATE, PcmWriter};

use formats::disc::Executable;
use formats::iso::ReadAt;
use video::movies::{MovieEntry, Region, movie_table};
use video::{MoviePlayer, PlayerStatus};

/// Opens the disc image again for a movie, or `None` when it cannot.
pub type Opener = Box<dyn FnMut() -> Option<Box<dyn ReadAt>>>;

/// A boxed image reader.
pub struct MovieReader(pub Box<dyn ReadAt>);

impl ReadAt for MovieReader {
    fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> formats::Result<()> {
        self.0.read_at(offset, buf)
    }
    fn image_len(&mut self) -> Option<u64> {
        self.0.image_len()
    }
}

struct Playing {
    player: MoviePlayer<MovieReader>,
    clock_ns: u64,
    title: Option<String>,
    audio: Option<AudioOut>,
}

/// The movie's soundtrack on its way to the sound engine.
struct AudioOut {
    writer: PcmWriter,
    /// Samples read from the movie and not yet taken by the stream.
    pending: Vec<i16>,
    /// The movie has no more sound.
    done: bool,
}

/// Samples read from the movie at a time.
const AUDIO_CHUNK: usize = 4096;

/// The game's movies.
pub struct Movies {
    table: Vec<MovieEntry>,
    opener: Opener,
    playing: Option<Playing>,
    /// Movies played to the end or skipped.
    pub played: u32,
    /// Movie requests dropped (not in the table, unreadable, undecodable).
    pub dropped: u32,
}

impl Movies {
    /// The movies of a disc whose executable is `exe`, read through `opener`; `None` without a movie table.
    pub fn new(exe: &Executable, opener: Opener) -> Option<Movies> {
        let table = movie_table(exe).ok()?;
        Some(Movies { table, opener, playing: None, played: 0, dropped: 0 })
    }

    /// Starts the movie with descriptor identity `identity` (the request's value). Returns whether it plays.
    pub fn start(&mut self, identity: u32) -> bool {
        let Some(entry) = self.table.iter().find(|m| m.identity() == identity) else {
            self.dropped += 1;
            return false;
        };
        let Some(src) = (self.opener)() else {
            self.dropped += 1;
            return false;
        };
        let mut player = MoviePlayer::new(Region::of_descriptor(MovieReader(src), &entry.descriptor));
        // No audio path yet: the movie's sound is parsed and dropped, and the real clock times the frames.
        player.set_audio_enabled(false);
        self.playing = Some(Playing { player, clock_ns: 0, title: entry.title.clone(), audio: None });
        true
    }

    /// Plays the movie's sound through `writer` (a stereo `audio::pcm` stream at 48 kHz) and times the frames by it.
    pub fn attach_audio(&mut self, writer: PcmWriter) {
        let Some(p) = self.playing.as_mut() else { return };
        p.player.set_audio_enabled(true);
        p.player.set_output_rate(Some(PCM_RATE));
        p.audio = Some(AudioOut { writer, pending: Vec::new(), done: false });
    }

    /// Whether the playing movie's sound goes to a stream.
    pub fn has_audio(&self) -> bool {
        self.playing.as_ref().is_some_and(|p| p.audio.is_some())
    }

    /// Whether a movie is playing.
    pub fn playing(&self) -> bool {
        self.playing.is_some()
    }

    /// The playing movie's title (none for the start-up logo).
    pub fn title(&self) -> Option<&str> {
        self.playing.as_ref().and_then(|p| p.title.as_deref())
    }

    /// Frames of the playing movie shown so far.
    pub fn frames_shown(&self) -> u64 {
        self.playing.as_ref().map_or(0, |p| p.player.frames_shown())
    }

    /// The frame to show now.
    pub fn frame(&self) -> Option<&video::Frame> {
        self.playing.as_ref().and_then(|p| p.player.frame())
    }

    /// Advances the playing movie by `seconds` of real time (or by its sound, see [`Movies::attach_audio`]); `skip`
    /// ends it. Returns whether it is still playing.
    pub fn update(&mut self, seconds: f32, skip: bool) -> bool {
        let Some(p) = self.playing.as_mut() else { return false };
        if skip {
            p.player.skip();
        }
        let real = (seconds.max(0.0) as f64 * 1e9) as u64;
        p.clock_ns = match p.audio.as_mut() {
            Some(a) => {
                feed(&mut p.player, a);
                let heard = a.writer.played_frames() * 1_000_000_000 / u64::from(PCM_RATE);
                // The sound clock while it runs; the real clock once it has run out (or never started).
                if heard > p.clock_ns {
                    heard
                } else if a.done && a.writer.buffered_frames() == 0 {
                    p.clock_ns + real
                } else {
                    p.clock_ns
                }
            }
            None => p.clock_ns + real,
        };
        match p.player.update(p.clock_ns) {
            Ok(PlayerStatus::Playing) => true,
            Ok(PlayerStatus::Ended) => {
                self.playing = None;
                self.played += 1;
                false
            }
            Err(_) => {
                self.playing = None;
                self.dropped += 1;
                false
            }
        }
    }
}

/// Moves the movie's decoded sound into the stream, as much as fits.
fn feed(player: &mut MoviePlayer<MovieReader>, a: &mut AudioOut) {
    loop {
        if a.pending.is_empty() {
            if a.done {
                return;
            }
            a.pending.resize(AUDIO_CHUNK, 0);
            let n = player.read_audio(&mut a.pending).unwrap_or(0);
            a.pending.truncate(n);
            if n == 0 {
                // The player reads the file further when its sound runs low, so nothing left is the end.
                a.done = true;
                a.writer.finish();
                return;
            }
        }
        let frames = a.writer.write(&a.pending);
        if frames == 0 {
            return;
        }
        a.pending.drain(..frames * 2);
    }
}
