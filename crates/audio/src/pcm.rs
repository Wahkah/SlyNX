//! A PCM stream input: interleaved `i16` frames at the output rate (48 kHz), written on the game thread and mixed
//! by the audio thread, for movie soundtracks and other decoded audio.
//!
//! - [`pcm_stream`] allocates a bounded ring once (on the calling thread). [`PcmWriter`] stays on the game thread;
//!   [`PcmReader`] goes to the renderer with [`crate::SoundEngine::attach_pcm`]. The audio thread only copies
//!   samples and moves atomic indices: no locks, no allocation, no freeing (a detached reader is handed back to the
//!   game thread to be dropped).
//! - The writer accepts what fits ([`PcmWriter::write`] returns the frames taken), so the caller keeps the rest and
//!   retries next frame; [`PcmWriter::free_frames`] says how much fits now.
//! - **A/V sync.** [`PcmWriter::played_frames`] is the audio clock: frames the audio thread has mixed (it stops while
//!   the engine is paused or the ring runs dry). [`PcmWriter::buffered_frames`] and [`PcmWriter::latency`] give what
//!   is queued ahead of it. The platform's own output buffer (one render block) comes on top.
//! - An underrun (the ring empty while the stream is open) plays silence for the missing frames and is counted
//!   ([`PcmWriter::underruns`]); the clock does not advance over silence. [`PcmWriter::finish`] marks the end, after
//!   which running dry is not an underrun. [`PcmWriter::clear`] drops everything queued (a seek).
//! - The stream is mixed after the sound engine's groups, at [`PcmWriter::set_gain`] (default 1.0), before the
//!   output's soft clip. Mono streams play on both sides.

use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicBool, AtomicI16, AtomicU32, AtomicU64, AtomicUsize, Ordering};

/// The rate PCM streams play at (the output rate): no resampling.
pub const PCM_RATE: u32 = 48_000;

struct Shared {
    samples: Box<[AtomicI16]>,
    channels: usize,
    /// Frames consumed (reader side).
    head: AtomicUsize,
    /// Frames written (writer side).
    tail: AtomicUsize,
    /// Frames before this index are dropped by the reader (a clear).
    clear_to: AtomicUsize,
    played: AtomicU64,
    underruns: AtomicU32,
    gain: AtomicU32,
    finished: AtomicBool,
}

impl Shared {
    fn capacity(&self) -> usize {
        self.samples.len() / self.channels
    }
}

/// The game-thread half of a PCM stream.
pub struct PcmWriter {
    shared: Arc<Shared>,
}

/// The audio-thread half of a PCM stream (give it to [`crate::SoundEngine::attach_pcm`]).
pub struct PcmReader {
    shared: Arc<Shared>,
}

/// A PCM stream of `channels` (1 or 2) interleaved `i16` channels with room for `capacity_frames` frames (at least
/// one). 9,600 frames is 200 ms at 48 kHz.
pub fn pcm_stream(capacity_frames: usize, channels: u8) -> (PcmWriter, PcmReader) {
    let channels = channels.clamp(1, 2) as usize;
    let capacity = capacity_frames.max(1);
    let samples: Vec<AtomicI16> = (0..capacity * channels).map(|_| AtomicI16::new(0)).collect();
    let shared = Arc::new(Shared {
        samples: samples.into_boxed_slice(),
        channels,
        head: AtomicUsize::new(0),
        tail: AtomicUsize::new(0),
        clear_to: AtomicUsize::new(0),
        played: AtomicU64::new(0),
        underruns: AtomicU32::new(0),
        gain: AtomicU32::new(1.0f32.to_bits()),
        finished: AtomicBool::new(false),
    });
    (PcmWriter { shared: shared.clone() }, PcmReader { shared })
}

impl PcmWriter {
    /// Channels per frame (1 or 2).
    pub fn channels(&self) -> usize {
        self.shared.channels
    }

    /// Ring capacity in frames.
    pub fn capacity_frames(&self) -> usize {
        self.shared.capacity()
    }

    /// Frames queued and not yet mixed.
    pub fn buffered_frames(&self) -> usize {
        let s = &*self.shared;
        let head = s.head.load(Ordering::Acquire).max(s.clear_to.load(Ordering::Acquire));
        s.tail.load(Ordering::Relaxed).saturating_sub(head)
    }

    /// Frames that fit now.
    pub fn free_frames(&self) -> usize {
        let s = &*self.shared;
        s.capacity() - s.tail.load(Ordering::Relaxed).wrapping_sub(s.head.load(Ordering::Acquire)).min(s.capacity())
    }

    /// Appends whole frames from `samples` (interleaved); returns how many frames were taken. A trailing partial
    /// frame is never taken.
    pub fn write(&mut self, samples: &[i16]) -> usize {
        let s = &*self.shared;
        let ch = s.channels;
        let n = (samples.len() / ch).min(self.free_frames());
        let cap = s.capacity();
        let tail = s.tail.load(Ordering::Relaxed);
        for f in 0..n {
            let at = ((tail + f) % cap) * ch;
            for c in 0..ch {
                s.samples[at + c].store(samples[f * ch + c], Ordering::Relaxed);
            }
        }
        s.tail.store(tail.wrapping_add(n), Ordering::Release);
        if n > 0 {
            s.finished.store(false, Ordering::Release);
        }
        n
    }

    /// Frames the audio thread has mixed since the stream was made: the audio clock (divide by [`PCM_RATE`]).
    pub fn played_frames(&self) -> u64 {
        self.shared.played.load(Ordering::Acquire)
    }

    /// Seconds of sound queued ahead of the audio clock.
    pub fn latency(&self) -> f32 {
        self.buffered_frames() as f32 / PCM_RATE as f32
    }

    /// Times the ring ran dry while the stream was open.
    pub fn underruns(&self) -> u32 {
        self.shared.underruns.load(Ordering::Relaxed)
    }

    /// Output gain (1.0 = as stored).
    pub fn set_gain(&mut self, gain: f32) {
        self.shared.gain.store(gain.max(0.0).to_bits(), Ordering::Relaxed);
    }

    /// No more frames will come: running dry after this is not an underrun. Writing again reopens the stream.
    pub fn finish(&mut self) {
        self.shared.finished.store(true, Ordering::Release);
    }

    /// Whether the stream is finished and everything queued has been mixed.
    pub fn drained(&self) -> bool {
        self.shared.finished.load(Ordering::Acquire) && self.buffered_frames() == 0
    }

    /// Drops every queued frame (a seek). The audio clock keeps its value.
    pub fn clear(&mut self) {
        let s = &*self.shared;
        s.clear_to.store(s.tail.load(Ordering::Relaxed), Ordering::Release);
    }
}

impl PcmReader {
    /// Adds the next frames to `out` (interleaved stereo `f32`, as the renderer's output), one frame per stereo
    /// pair. Audio thread; no allocation.
    pub fn mix_into(&mut self, out: &mut [f32]) {
        let s = &*self.shared;
        let ch = s.channels;
        let cap = s.capacity();
        let mut head = s.head.load(Ordering::Relaxed);
        let clear_to = s.clear_to.load(Ordering::Acquire);
        if clear_to.wrapping_sub(head) as isize > 0 {
            head = clear_to;
        }
        let tail = s.tail.load(Ordering::Acquire);
        let want = out.len() / 2;
        let n = tail.wrapping_sub(head).min(want);
        let gain = f32::from_bits(s.gain.load(Ordering::Relaxed)) / 32768.0;
        for f in 0..n {
            let at = ((head + f) % cap) * ch;
            let l = s.samples[at].load(Ordering::Relaxed) as f32 * gain;
            let r = if ch == 2 { s.samples[at + 1].load(Ordering::Relaxed) as f32 * gain } else { l };
            out[f * 2] += l;
            out[f * 2 + 1] += r;
        }
        s.head.store(head.wrapping_add(n), Ordering::Release);
        s.played.fetch_add(n as u64, Ordering::Release);
        if n < want && !s.finished.load(Ordering::Acquire) && s.played.load(Ordering::Relaxed) > 0 {
            s.underruns.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_ring_clock_and_underruns() {
        let (mut w, mut r) = pcm_stream(4, 2);
        assert_eq!(w.free_frames(), 4);
        // Five frames offered, four taken.
        let five = [100i16, -100, 200, -200, 300, -300, 400, -400, 500, -500];
        assert_eq!(w.write(&five), 4);
        assert_eq!((w.buffered_frames(), w.free_frames()), (4, 0));
        let mut out = [0.0f32; 6];
        r.mix_into(&mut out);
        assert_eq!(out[0], 100.0 / 32768.0);
        assert_eq!(out[5], -300.0 / 32768.0);
        assert_eq!((w.played_frames(), w.buffered_frames()), (3, 1));
        assert!((w.latency() - 1.0 / 48_000.0).abs() < 1e-9);
        // Wrap around.
        assert_eq!(w.write(&five[8..]), 1);
        let mut out = [0.0f32; 8];
        r.mix_into(&mut out);
        assert_eq!(out[2], 500.0 / 32768.0);
        assert_eq!(w.played_frames(), 5);
        assert_eq!(w.underruns(), 1, "two frames were missing");
        w.finish();
        r.mix_into(&mut out);
        assert_eq!(w.underruns(), 1);
        assert!(w.drained());
    }

    #[test]
    fn mono_gain_and_clear() {
        let (mut w, mut r) = pcm_stream(8, 1);
        w.set_gain(0.5);
        w.write(&[1000, 2000, 3000]);
        let mut out = [0.0f32; 2];
        r.mix_into(&mut out);
        assert_eq!(out, [500.0 / 32768.0, 500.0 / 32768.0]);
        w.clear();
        assert_eq!(w.buffered_frames(), 0);
        let mut out = [0.0f32; 4];
        r.mix_into(&mut out);
        assert_eq!(out, [0.0; 4]);
        assert_eq!(w.played_frames(), 1);
        w.write(&[7]);
        r.mix_into(&mut out[..2]);
        assert_eq!(out[0], 3.5 / 32768.0);
    }
}
