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
use video::movies::{Region, movie_table};
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

/// A movie file on the disc.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clip {
    /// The movie's identity (the name the game uses for it).
    pub identity: u32,
    /// Byte offset and length in the image.
    pub offset: u64,
    pub len: u64,
    /// The game's title for it, if it has one.
    pub title: Option<String>,
}

/// The game's movies.
pub struct Movies {
    table: Vec<Clip>,
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
        let table = movie_table(exe)
            .ok()?
            .into_iter()
            .map(|m| Clip {
                identity: m.identity(),
                offset: m.descriptor.offset(),
                len: u64::from(m.descriptor.size),
                title: m.title,
            })
            .collect();
        Some(Movies::from_clips(table, opener))
    }

    /// Movies from a list of clips (Sly 2 and Sly 3: the file records of `video::movies::sly2_movies` and
    /// `sly3_movies`).
    pub fn from_clips(table: Vec<Clip>, opener: Opener) -> Movies {
        Movies { table, opener, playing: None, played: 0, dropped: 0 }
    }

    /// Clips of Sly 2 / Sly 3 file records (no titles).
    pub fn clips_of_records(records: &[(usize, formats_sly2::record::FileRecord)]) -> Vec<Clip> {
        records
            .iter()
            .map(|(_, r)| Clip { identity: r.identity, offset: r.offset(), len: u64::from(r.size), title: None })
            .collect()
    }

    /// Whether the table has a movie of `identity`.
    pub fn has(&self, identity: u32) -> bool {
        self.table.iter().any(|m| m.identity == identity)
    }

    /// Starts the movie with descriptor identity `identity` (the request's value). Returns whether it plays.
    pub fn start(&mut self, identity: u32) -> bool {
        let Some(entry) = self.table.iter().find(|m| m.identity == identity) else {
            self.dropped += 1;
            return false;
        };
        let Some(src) = (self.opener)() else {
            self.dropped += 1;
            return false;
        };
        let mut player = MoviePlayer::new(Region::new(MovieReader(src), entry.offset, entry.len));
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

/// Copies a decoded picture's planes into tightly packed Y, U and V buffers (the video layer's upload format).
pub fn pack_planes(p: &video::Planes<'_>, out: &mut [Vec<u8>; 3]) {
    let (w, h) = (p.width, p.height);
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let copy = |src: &[u8], stride: usize, w: usize, h: usize, dst: &mut Vec<u8>| {
        dst.clear();
        for row in 0..h {
            dst.extend_from_slice(&src[row * stride..][..w]);
        }
    };
    let [y, u, v] = out;
    copy(p.y, p.y_stride, w, h, y);
    copy(p.cb, p.chroma_stride, cw, ch, u);
    copy(p.cr, p.chroma_stride, cw, ch, v);
}

/// Movies on screen for the Sly 2 and Sly 3 sessions: the [`Movies`], the renderer's video layer (made with each
/// level, [`MovieScreen::set_layer`]) and the frame upload. While a movie plays the session holds its world; Start
/// (+) skips the movie.
#[derive(Default)]
pub struct MovieScreen {
    pub movies: Option<Movies>,
    layer: Option<render::video::VideoLayer>,
    shown: u64,
    size: (u32, u32),
    on: bool,
    buf: [Vec<u8>; 3],
    rect: Option<[f32; 4]>,
}

impl MovieScreen {
    /// The video layer of the level just uploaded.
    pub fn set_layer(&mut self, layer: Option<render::video::VideoLayer>) {
        self.layer = layer;
        self.on = false;
        self.shown = 0;
    }

    /// Starts movie `identity`, its sound on `sound`'s PCM stream when given. Returns whether it plays.
    pub fn start(&mut self, identity: u32, sound: Option<&mut audio::SoundEngine>) -> bool {
        let Some(m) = self.movies.as_mut() else { return false };
        if !m.start(identity) {
            return false;
        }
        if let Some(s) = sound {
            let (writer, reader) = audio::pcm::pcm_stream(PCM_RATE as usize / 5, 2);
            s.attach_pcm(Some(reader));
            m.attach_audio(writer);
        }
        true
    }

    /// Whether a movie plays.
    pub fn playing(&self) -> bool {
        self.movies.as_ref().is_some_and(Movies::playing)
    }

    /// Where the last [`MovieScreen::present`] put the picture (target pixels), if it showed one.
    pub fn rect(&self) -> Option<[f32; 4]> {
        self.rect
    }

    /// Advances the movie by `seconds` of real time; `skip` ends it. Returns whether it still plays (when it ends,
    /// `sound`'s PCM stream is detached).
    pub fn update(&mut self, seconds: f32, skip: bool, sound: Option<&mut audio::SoundEngine>) -> bool {
        let Some(m) = self.movies.as_mut() else { return false };
        if !m.playing() {
            return false;
        }
        let still = m.update(seconds, skip);
        if !still && let Some(s) = sound {
            s.attach_pcm(None);
        }
        still
    }

    /// Puts the current frame on the video layer (letterboxed in the target) or hides the layer when no movie plays.
    /// Returns the picture's rectangle in target pixels while one shows.
    pub fn present<B: render::Backend>(
        &mut self,
        r: &mut render::Renderer<B>,
    ) -> Result<Option<[f32; 4]>, render::RenderError> {
        self.rect = self.show(r)?;
        Ok(self.rect)
    }

    fn show<B: render::Backend>(
        &mut self,
        r: &mut render::Renderer<B>,
    ) -> Result<Option<[f32; 4]>, render::RenderError> {
        let Some(layer) = self.layer.as_ref() else { return Ok(None) };
        let movie = self.movies.as_ref().filter(|m| m.playing());
        match movie {
            Some(m) => {
                if m.frames_shown() != self.shown
                    && let Some(f) = m.frame()
                {
                    let p = f.planes();
                    pack_planes(&p, &mut self.buf);
                    let [y, u, v] = &self.buf;
                    let (fw, fh) = (p.width as u32, p.height as u32);
                    r.set_video_frame(Some(render::video::VideoFrame { width: fw, height: fh, y, u, v }))?;
                    self.shown = m.frames_shown();
                    self.size = (fw, fh);
                }
                if self.shown > 0 {
                    let rect = render::video::VideoLayer::fit(r.backend().target_size(), self.size);
                    layer.show(r, rect, [1.0; 4])?;
                    self.on = true;
                    return Ok(Some(rect));
                }
                Ok(None)
            }
            None => {
                if self.on {
                    layer.hide(r)?;
                    r.set_video_frame(None)?;
                    self.on = false;
                    self.shown = 0;
                }
                Ok(None)
            }
        }
    }
}

/// Black around a movie's picture at `rect` (target pixels of a `width` × `height` target), or the whole canvas black
/// before its first frame.
pub fn movie_bars(list: &mut ui::draw::DrawList, width: u32, height: u32, rect: Option<[f32; 4]>) {
    use ui::draw::Rect;
    let (cw, ch) = (list.canvas.width, list.canvas.height);
    let black = [0.0, 0.0, 0.0, 1.0];
    let Some(rect) = rect else {
        list.rect(Rect::new(0.0, 0.0, cw, ch), black);
        return;
    };
    let _ = height;
    let k = cw / width.max(1) as f32;
    let [x, y, w, h] = rect.map(|v| v * k);
    for r in [
        Rect::new(0.0, 0.0, cw, y),
        Rect::new(0.0, y + h, cw, ch - (y + h)),
        Rect::new(0.0, y, x, h),
        Rect::new(x + w, y, cw - (x + w), h),
    ] {
        if r.w > 0.0 && r.h > 0.0 {
            list.rect(r, black);
        }
    }
}
