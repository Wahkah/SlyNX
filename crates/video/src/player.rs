//! A movie player: reads a PSS movie in chunks, demultiplexes it, decodes video ahead of time and shows each frame
//! when the caller's clock (normally the audio clock) reaches it.
//!
//! Typical use, once per game frame:
//!
//! ```ignore
//! let mut player = MoviePlayer::new(Region::of_descriptor(file, &descriptor));
//! // audio thread side: pull PCM with player.read_audio(&mut buf) and count the samples played
//! let clock_ns = samples_played * 1_000_000_000 / 48_000;   // minus the output latency
//! if player.update(clock_ns)? == PlayerStatus::Ended { /* back to the game */ }
//! if let Some(frame) = player.frame() { /* upload frame.planes() and draw */ }
//! if skip_pressed { player.skip(); }
//! ```

use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;

use formats::iso::ReadAt;

use crate::Error;
use crate::audio::{AudioHeader, AudioStream, Resampler};
use crate::decoder::{Decoded, Decoder, DecoderStats};
use crate::demux::{Demuxer, PACK_SIZE, Packet};
use crate::frame::Frame;
use crate::movies::Region;

/// Where playback stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerStatus {
    Playing,
    /// The last frame has been shown for its full duration, or the movie was skipped.
    Ended,
}

/// The platform's audio output rate (docs/platform.md, Audio); movie audio at another rate is resampled to it.
pub const OUTPUT_RATE: u32 = 48_000;

/// Plays one movie.
pub struct MoviePlayer<R: ReadAt> {
    src: Region<R>,
    pos: u64,
    chunk: Vec<u8>,
    demux: Demuxer,
    decoder: Decoder,
    audio: AudioStream,
    audio_buf: VecDeque<i16>,
    audio_tmp: Vec<i16>,
    audio_enabled: bool,
    /// Rate of the samples [`MoviePlayer::read_audio`] gives (`None`: the movie's own).
    output_rate: Option<u32>,
    resampler: Option<Resampler>,
    resampled: Vec<i16>,
    input_done: bool,
    decoder_done: bool,
    current: Frame,
    has_current: bool,
    pending: Frame,
    has_pending: bool,
    last_index: Option<u64>,
    skipped: bool,
    frames_shown: u64,
    frames_dropped: u64,
}

impl<R: ReadAt> MoviePlayer<R> {
    /// A player for the movie in `src`. Nothing is read until the first call that needs data.
    pub fn new(src: Region<R>) -> Self {
        MoviePlayer {
            src,
            pos: 0,
            chunk: vec![0; 4 * PACK_SIZE],
            demux: Demuxer::new(),
            decoder: Decoder::new(),
            audio: AudioStream::new(),
            audio_buf: VecDeque::new(),
            audio_tmp: Vec::new(),
            audio_enabled: true,
            output_rate: Some(OUTPUT_RATE),
            resampler: None,
            resampled: Vec::new(),
            input_done: false,
            decoder_done: false,
            current: Frame::default(),
            has_current: false,
            pending: Frame::default(),
            has_pending: false,
            last_index: None,
            skipped: false,
            frames_shown: 0,
            frames_dropped: 0,
        }
    }

    /// With audio off, the audio stream is parsed but its samples are dropped (no buffer builds up).
    pub fn set_audio_enabled(&mut self, on: bool) {
        self.audio_enabled = on;
        if !on {
            self.audio_buf.clear();
        }
    }

    /// The rate of the samples [`Self::read_audio`] gives: [`OUTPUT_RATE`] by default (Sly 2 and 3 movies at
    /// 44.1 kHz are resampled), or `None` for the movie's own rate. Set it before reading any audio.
    pub fn set_output_rate(&mut self, rate: Option<u32>) {
        self.output_rate = rate;
    }

    /// The rate of the samples [`Self::read_audio`] gives (reads the audio header if needed).
    pub fn audio_rate(&mut self) -> Result<Option<u32>, Error> {
        let h = self.audio_header()?;
        Ok(self.output_rate.or(h.map(|h| h.sample_rate)))
    }

    /// Slice-decoding threads (feature `threads`).
    pub fn set_threads(&mut self, n: usize) {
        self.decoder.set_threads(n);
    }

    /// Reads one chunk of the file into the demultiplexer and hands its payloads on. Returns `false` once the
    /// input is exhausted.
    fn feed(&mut self) -> Result<bool, Error> {
        if self.input_done {
            return Ok(false);
        }
        let n = self.src.read(self.pos, &mut self.chunk)?;
        self.pos += n as u64;
        self.demux.push(&self.chunk[..n]);
        while let Some(p) = self.demux.next_packet()? {
            match p {
                Packet::Video { data, .. } => self.decoder.push(data),
                Packet::Audio(a) => {
                    self.audio_tmp.clear();
                    self.audio.push(a, &mut self.audio_tmp)?;
                    if self.resampler.is_none()
                        && let (Some(h), Some(out)) = (self.audio.header(), self.output_rate)
                        && h.sample_rate != out
                    {
                        self.resampler = Some(Resampler::new(h.sample_rate, out));
                    }
                    self.take_audio(false);
                }
            }
        }
        if n == 0 || self.pos >= self.src.len() || self.demux.ended() {
            self.input_done = true;
            self.decoder.finish();
            self.audio_tmp.clear();
            self.audio.finish(&mut self.audio_tmp);
            self.take_audio(true);
        }
        Ok(true)
    }

    /// Moves the samples in `audio_tmp` to the output buffer, through the resampler when there is one.
    fn take_audio(&mut self, last: bool) {
        let samples = match self.resampler.as_mut() {
            Some(r) => {
                self.resampled.clear();
                r.push(&self.audio_tmp, &mut self.resampled);
                if last {
                    r.finish(&mut self.resampled);
                }
                &self.resampled
            }
            None => &self.audio_tmp,
        };
        if self.audio_enabled {
            self.audio_buf.extend(samples.iter().copied());
        }
    }

    /// The audio header (read from the start of the file if needed).
    pub fn audio_header(&mut self) -> Result<Option<AudioHeader>, Error> {
        while self.audio.header().is_none() && self.feed()? {}
        Ok(self.audio.header().copied())
    }

    /// Movie length from the audio data size, in nanoseconds.
    pub fn duration_ns(&mut self) -> Result<Option<u64>, Error> {
        Ok(self.audio_header()?.map(|h| h.duration_ns()))
    }

    /// Fills `out` with interleaved stereo samples (L, R, ...); returns how many were written (fewer only at the
    /// end of the movie).
    pub fn read_audio(&mut self, out: &mut [i16]) -> Result<usize, Error> {
        while self.audio_buf.len() < out.len() && !self.skipped && self.feed()? {}
        let n = out.len().min(self.audio_buf.len());
        for (o, s) in out.iter_mut().zip(self.audio_buf.drain(..n)) {
            *o = s;
        }
        Ok(n)
    }

    /// Samples buffered for [`Self::read_audio`].
    pub fn audio_buffered(&self) -> usize {
        self.audio_buf.len()
    }

    /// When frame `index` (display order) is due, in nanoseconds from the start.
    pub fn frame_time_ns(&self, index: u64) -> u64 {
        let (num, den) = self.decoder.sequence().map_or((1001, 30000), |s| s.frame_period());
        (u128::from(index) * u128::from(num) * 1_000_000_000 / u128::from(den)) as u64
    }

    /// Advances to the frame due at `clock_ns` (time since the movie started, normally from the audio clock),
    /// decoding as needed. When several frames are due at once (the player fell behind), each becomes current in
    /// turn and only the last is shown; the others count as dropped. While behind, B pictures are skipped without
    /// being decoded until the player catches up.
    pub fn update(&mut self, clock_ns: u64) -> Result<PlayerStatus, Error> {
        if self.skipped {
            return Ok(PlayerStatus::Ended);
        }
        let mut presented = 0u64;
        loop {
            if !self.has_pending && !self.decoder_done {
                match self.decoder.decode()? {
                    Decoded::Frame(f) => {
                        self.pending.copy_from(f);
                        self.has_pending = true;
                    }
                    Decoded::NeedData => {
                        if !self.feed()? {
                            self.decoder.finish();
                        }
                        continue;
                    }
                    Decoded::End => self.decoder_done = true,
                }
            }
            if !self.has_pending {
                break;
            }
            let idx = self.pending.info.display_index;
            if self.frame_time_ns(idx) > clock_ns {
                break;
            }
            self.has_pending = false;
            self.last_index = Some(idx);
            core::mem::swap(&mut self.current, &mut self.pending);
            self.has_current = true;
            presented += 1;
            // Behind when the next frame is already due: B pictures are then not worth decoding.
            self.decoder.set_skip_b(self.frame_time_ns(idx + 1) <= clock_ns);
        }
        self.frames_shown += presented.min(1);
        self.frames_dropped += presented.saturating_sub(1);
        if self.decoder_done && !self.has_pending {
            let end = self.last_index.map_or(0, |i| self.frame_time_ns(i + 1));
            if clock_ns >= end {
                return Ok(PlayerStatus::Ended);
            }
        }
        Ok(PlayerStatus::Playing)
    }

    /// The frame to show now (none before the first one is due).
    pub fn frame(&self) -> Option<&Frame> {
        self.has_current.then_some(&self.current)
    }

    /// Stops playback: the next [`Self::update`] reports [`PlayerStatus::Ended`] and no more audio is produced.
    pub fn skip(&mut self) {
        self.skipped = true;
        self.audio_buf.clear();
    }

    pub fn is_finished(&self) -> bool {
        self.skipped || (self.decoder_done && !self.has_pending)
    }

    /// Updates that changed the frame to show.
    pub fn frames_shown(&self) -> u64 {
        self.frames_shown
    }

    /// Frames decoded and due but replaced by a later frame within the same update (B pictures skipped while
    /// behind are in [`DecoderStats::skipped_b`] instead).
    pub fn frames_dropped(&self) -> u64 {
        self.frames_dropped
    }

    pub fn decoder_stats(&self) -> &DecoderStats {
        self.decoder.stats()
    }
}
