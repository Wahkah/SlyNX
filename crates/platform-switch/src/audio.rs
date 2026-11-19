//! Audio output: audout at 48 kHz stereo PCM16, fed by a mixer thread on core 2.
//!
//! The device holds a ring of [`BUFFERS`] buffers of [`FRAMES`] sample frames. The audio thread waits for the device
//! to release one, asks the renderer for the next [`FRAMES`] frames (as `f32`), converts them to `i16` and queues the
//! buffer again. Worst-case latency is the whole ring: 4 x 8 ms = 32 ms.

use crate::sys;
use crate::thread::{SwitchThread, spawn};
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use core::sync::atomic::{AtomicBool, Ordering};
use platform_api::audio::{f32_to_i16, frames_to_ns};
use platform_api::{AudioError, AudioRender, CHANNELS, Core, Priority, ThreadSpec};

pub const FRAMES: u32 = 384;
pub const BUFFERS: u32 = 4;
pub const LATENCY_NS: u64 = frames_to_ns(FRAMES as u64 * BUFFERS as u64);

pub struct AudioThread {
    stop: Arc<AtomicBool>,
    thread: Option<SwitchThread>,
}

impl AudioThread {
    pub fn start(mut render: Box<dyn AudioRender>) -> Result<AudioThread, AudioError> {
        // SAFETY: the platform keeps at most one AudioThread, so the device is not already open.
        let rc = unsafe { sys::slynx_audio_open(FRAMES, BUFFERS) };
        if rc != 0 {
            return Err(AudioError::Device(rc as i32));
        }
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = stop.clone();
        let job = Box::new(move || {
            let samples = FRAMES as usize * CHANNELS;
            let mut mix = vec![0.0f32; samples];
            while !stop_thread.load(Ordering::Acquire) {
                // SAFETY: the device is open until `stop` joins this thread.
                let buf = unsafe { sys::slynx_audio_acquire(20_000_000) };
                if buf.is_null() {
                    continue;
                }
                mix.fill(0.0);
                render.render(&mut mix);
                // SAFETY: `buf` is a device buffer of FRAMES * 2 i16 samples that the device released to us and that
                // nothing else touches until we submit it.
                let out = unsafe { core::slice::from_raw_parts_mut(buf, samples) };
                f32_to_i16(&mix, out);
                // SAFETY: `buf` came from `slynx_audio_acquire`.
                unsafe { sys::slynx_audio_submit(buf) };
            }
        });
        let spec = ThreadSpec::new("audio", Core::Audio).priority(Priority::High).stack_size(64 * 1024);
        match spawn(spec, job) {
            Ok(thread) => Ok(AudioThread { stop, thread: Some(thread) }),
            Err(_) => {
                // SAFETY: no thread uses the device.
                unsafe { sys::slynx_audio_close() };
                Err(AudioError::Thread)
            }
        }
    }
}

impl Drop for AudioThread {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        drop(self.thread.take()); // joins
        // SAFETY: the audio thread has exited, so nothing else uses the device.
        unsafe { sys::slynx_audio_close() };
    }
}
