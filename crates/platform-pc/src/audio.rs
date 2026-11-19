//! Audio output through an SDL audio device. SDL runs the callback on its own audio thread, which plays the part of
//! the Switch's mixer thread: it refills one device buffer at a time from the renderer.

use crate::context::Sdl;
use crate::ffi::{self, sdl_error};
use platform_api::audio::frames_to_ns;
use platform_api::{AudioError, AudioRender, CHANNELS, SAMPLE_RATE};
use std::ffi::c_void;
use std::rc::Rc;

/// Sample frames per device buffer: 512 at 48 kHz is 10.7 ms.
const FRAMES: u16 = 512;

type Renderer = Box<dyn AudioRender>;

pub struct PcAudio {
    device: u32,
    renderer: *mut Renderer,
    latency_ns: u64,
    _sdl: Rc<Sdl>,
}

unsafe extern "C" fn callback(userdata: *mut c_void, stream: *mut u8, len: i32) {
    // SAFETY: SDL passes back the `Box<Renderer>` pointer we registered, which lives until the device is closed,
    // and calls this from one thread at a time. `stream` holds `len` bytes of f32 samples (we asked for
    // AUDIO_F32SYS without allowing format changes, so SDL converts if the hardware differs) and is 4-aligned.
    unsafe {
        let renderer = &mut *(userdata as *mut Renderer);
        let out = std::slice::from_raw_parts_mut(stream as *mut f32, len.max(0) as usize / 4);
        out.fill(0.0);
        renderer.render(out);
    }
}

impl PcAudio {
    pub fn open(sdl: Rc<Sdl>, renderer: Renderer) -> Result<PcAudio, AudioError> {
        sdl.subsystem(ffi::SDL_INIT_AUDIO).map_err(|_| AudioError::Device(-1))?;
        let renderer = Box::into_raw(Box::new(renderer));
        let desired = ffi::SDL_AudioSpec {
            freq: SAMPLE_RATE as i32,
            format: ffi::AUDIO_F32SYS,
            channels: CHANNELS as u8,
            silence: 0,
            samples: FRAMES,
            padding: 0,
            size: 0,
            callback: Some(callback),
            userdata: renderer.cast(),
        };
        // SAFETY: a zeroed spec (null callback and userdata) is a valid out-parameter.
        let mut obtained: ffi::SDL_AudioSpec = unsafe { std::mem::zeroed() };
        // SAFETY: `desired` is fully initialised; the callback and userdata stay valid until SDL_CloseAudioDevice.
        let device = unsafe { ffi::SDL_OpenAudioDevice(std::ptr::null(), 0, &desired, &mut obtained, 0) };
        if device == 0 {
            eprintln!("audio: {}", sdl_error());
            // SAFETY: SDL never saw a device, so the renderer is ours again.
            drop(unsafe { Box::from_raw(renderer) });
            return Err(AudioError::Device(-1));
        }
        // SAFETY: `device` is open.
        unsafe { ffi::SDL_PauseAudioDevice(device, 0) };
        // SDL keeps one buffer playing while the callback fills the next.
        let latency_ns = frames_to_ns(2 * obtained.samples.max(FRAMES) as u64);
        Ok(PcAudio { device, renderer, latency_ns, _sdl: sdl })
    }

    pub fn latency_ns(&self) -> u64 {
        self.latency_ns
    }
}

impl Drop for PcAudio {
    fn drop(&mut self) {
        // SAFETY: closing waits for the callback to return and stops further calls, so the renderer is free after.
        unsafe {
            ffi::SDL_CloseAudioDevice(self.device);
            drop(Box::from_raw(self.renderer));
        }
    }
}
