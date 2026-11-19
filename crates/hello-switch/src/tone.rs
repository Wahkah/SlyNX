//! A one-voice test tone: a sine with a short decay, retriggered from the game thread through atomics.

use crate::scene::sin_cos;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU32, Ordering};
use platform_api::{AudioRender, SAMPLE_RATE};

const TAU: f32 = core::f32::consts::TAU;
/// Amplitude multiplier per sample: falls to 1/e in about 0.2 s.
const DECAY: f32 = 1.0 - 1.0 / (0.2 * SAMPLE_RATE as f32);
const VOLUME: f32 = 0.3;

/// Shared between the game thread (which triggers) and the audio thread (which plays).
#[derive(Default)]
pub struct ToneControl {
    triggers: AtomicU32,
    freq_bits: AtomicU32,
    /// Samples rendered so far (wraps), so the demo can show the audio thread is alive.
    rendered: AtomicU32,
}

impl ToneControl {
    /// Starts a new note at `freq` Hz.
    pub fn trigger(&self, freq: f32) {
        self.freq_bits.store(freq.to_bits(), Ordering::Relaxed);
        self.triggers.fetch_add(1, Ordering::Release);
    }

    pub fn rendered_frames(&self) -> u32 {
        self.rendered.load(Ordering::Relaxed)
    }
}

/// The audio-thread half.
pub struct ToneSynth {
    control: Arc<ToneControl>,
    seen: u32,
    phase: f32,
    step: f32,
    env: f32,
}

impl ToneSynth {
    pub fn new(control: Arc<ToneControl>) -> ToneSynth {
        ToneSynth { control, seen: 0, phase: 0.0, step: 0.0, env: 0.0 }
    }
}

impl AudioRender for ToneSynth {
    fn render(&mut self, out: &mut [f32]) {
        let triggers = self.control.triggers.load(Ordering::Acquire);
        if triggers != self.seen {
            self.seen = triggers;
            let freq = f32::from_bits(self.control.freq_bits.load(Ordering::Relaxed));
            self.step = TAU * freq / SAMPLE_RATE as f32;
            self.env = 1.0;
        }
        for frame in out.as_chunks_mut::<2>().0 {
            let s = if self.env > 1e-4 { sin_cos(self.phase).0 * self.env * VOLUME } else { 0.0 };
            frame[0] = s;
            frame[1] = s;
            self.env *= DECAY;
            self.phase += self.step;
            if self.phase >= TAU {
                self.phase -= TAU;
            }
        }
        self.control.rendered.fetch_add((out.len() / 2) as u32, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn silent_until_triggered_then_decays() {
        let control = Arc::new(ToneControl::default());
        let mut synth = ToneSynth::new(control.clone());
        let mut buf = vec![1.0f32; 960];
        synth.render(&mut buf);
        assert!(buf.iter().all(|&s| s == 0.0));

        control.trigger(440.0);
        synth.render(&mut buf);
        let peak = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.25 && peak <= VOLUME, "peak {peak}");
        assert!(buf.as_chunks::<2>().0.iter().all(|f| f[0] == f[1]), "mono in both channels");

        // After two seconds the note has died away.
        for _ in 0..200 {
            synth.render(&mut buf);
        }
        assert!(buf.iter().all(|s| s.abs() < 1e-3));
        assert_eq!(control.rendered_frames(), 480 * 202);
    }
}
