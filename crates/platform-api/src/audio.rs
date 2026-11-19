//! Audio output format and the mixer callback.
//!
//! The platform owns an audio thread (Switch: pinned to core 2) that keeps a small ring of output buffers queued on
//! the device. Whenever the device finishes a buffer, the thread calls [`AudioRender::render`] to refill it and
//! queues it again. Total latency is the ring size times the buffer length; each backend reports it with
//! `Platform::audio_latency_ns`, and both stay under 50 ms.

use core::fmt;

/// Output sample rate in Hz, on every platform.
pub const SAMPLE_RATE: u32 = 48_000;
/// Interleaved output channels (left, right).
pub const CHANNELS: usize = 2;

/// The mixer. Runs on the audio thread, so it must not block, wait on locks the game thread holds for long, or do
/// I/O. Talk to it through atomics or a lock-free queue.
pub trait AudioRender: Send + 'static {
    /// Fills `out` with interleaved stereo samples (`out.len()` is always even) in -1.0..=1.0. Whatever is left
    /// unwritten plays as it was given: the platform zeroes the buffer before each call.
    fn render(&mut self, out: &mut [f32]);
}

impl<F: FnMut(&mut [f32]) + Send + 'static> AudioRender for F {
    fn render(&mut self, out: &mut [f32]) {
        self(out)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioError {
    /// No output device, or the audio service refused to open it. The code is the backend's.
    Device(i32),
    /// The audio thread could not be started.
    Thread,
    /// Audio is disabled in this configuration (for example a headless test run).
    Disabled,
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioError::Device(c) => write!(f, "audio device error {c:#x}"),
            AudioError::Thread => f.write_str("could not start the audio thread"),
            AudioError::Disabled => f.write_str("audio is disabled"),
        }
    }
}

/// Converts float samples to signed 16-bit, clamping out-of-range values. `dst` and `src` must be the same length;
/// extra elements of the longer one are left alone.
pub fn f32_to_i16(src: &[f32], dst: &mut [i16]) {
    for (d, &s) in dst.iter_mut().zip(src) {
        // NaN clamps to 0 through `as`; the float-to-int cast saturates.
        *d = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
    }
}

/// Nanoseconds that `frames` sample frames take to play at [`SAMPLE_RATE`].
pub const fn frames_to_ns(frames: u64) -> u64 {
    frames * 1_000_000_000 / SAMPLE_RATE as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;

    #[test]
    fn conversion_clamps() {
        let src = [0.0, 1.0, -1.0, 2.0, -2.0, 0.5, f32::NAN];
        let mut dst = [7i16; 7];
        f32_to_i16(&src, &mut dst);
        assert_eq!(dst, [0, 32767, -32767, 32767, -32767, 16383, 0]);
    }

    #[test]
    fn closures_are_renderers() {
        let mut r: Box<dyn AudioRender> = Box::new(|out: &mut [f32]| out.fill(0.25));
        let mut buf = [0.0; 8];
        r.render(&mut buf);
        assert_eq!(buf, [0.25; 8]);
    }

    #[test]
    fn latency_math() {
        assert_eq!(frames_to_ns(48_000), 1_000_000_000);
        assert_eq!(frames_to_ns(480), 10_000_000);
    }
}
