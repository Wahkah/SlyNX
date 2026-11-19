//! The Switch entry point (`dist/slynx.nro`).
//!
//! The disc image is found as [`crate::settings`] says. Without one, the reason goes to the debug log and the game
//! waits for − to quit. The game starts at the title (or in the settings file's `world`); saves go to
//! `sdmc:/switch/SlyNX/user/saves/sly1/` through the platform's file system, and movies stream from the disc image. Panics go to the debug log (the platform's panic handler). A statistics line goes to the log
//! every five seconds.

use alloc::format;

use platform_switch::platform_api::{Event, Platform};
use platform_switch::{SwitchConfig, SwitchPlatform, debug_log};
use render::RenderError;
use render::deko3d::Deko3dBackend;

use crate::frame_input;
use crate::session::{Session, SessionOptions};
use crate::settings::{Settings, open_disc_fs, open_disc_reader};

/// Instance slots and draw commands per frame.
const MAX_INSTANCES: u32 = 1 << 15;
const MAX_DRAWS: u32 = 1 << 14;
/// Nanoseconds between statistics lines.
const LOG_EVERY_NS: u64 = 5_000_000_000;

fn now() -> u64 {
    unsafe extern "C" {
        fn slynx_ticks_ns() -> u64;
    }
    // SAFETY: reads the system tick counter; no preconditions.
    unsafe { slynx_ticks_ns() }
}

/// Called by libnx's startup code once the runtime is up.
#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    debug_log("SlyNX: start\n");
    let mut platform = match SwitchPlatform::new(SwitchConfig::default()) {
        Ok(p) => p,
        Err(e) => panic!("platform: {e}"),
    };
    let backend = match Deko3dBackend::new(&platform, MAX_INSTANCES, MAX_DRAWS) {
        Ok(b) => b,
        Err(e) => panic!("renderer: {e}"),
    };
    let fs = platform.fs();
    let settings = Settings::load(&fs);
    let disc = open_disc_fs(&fs, &settings);
    if let Err(e) = &disc {
        debug_log(&format!("SlyNX: {e}\n"));
    }
    let options =
        SessionOptions { camera_triggers: settings.triggers.unwrap_or(true), overlay: true, ..Default::default() };
    let mut session = Session::new(backend, disc, options);
    session.renderer.set_clock(now);
    session.set_saves(alloc::boxed::Box::new(save::SaveStore::new(fs)));
    let movie_settings = settings.clone();
    session.set_movie_source(alloc::boxed::Box::new(move || {
        let r = open_disc_reader(&fs, &movie_settings)?;
        Some(alloc::boxed::Box::new(r) as alloc::boxed::Box<dyn formats::iso::ReadAt>)
    }));
    if !session.world_names().is_empty() {
        platform.set_cpu_boost(true);
        let started = match settings.world.as_deref() {
            Some(first) => session.load_world(session.find_world(first).unwrap_or(1), None, 0),
            None => session.start_title(),
        };
        if let Err(e) = started {
            debug_log(&format!("SlyNX: {e}\n"));
        }
        platform.set_cpu_boost(false);
    }

    let mut last = now();
    let mut log_at = last + LOG_EVERY_NS;
    let mut paused = false;
    while platform.pump() {
        while let Some(event) = platform.poll_event() {
            match event {
                Event::Suspended => paused = true,
                Event::Resumed => paused = false,
                _ => {}
            }
        }
        let t = now();
        let frame_ns = t - last;
        last = t;
        let input = frame_input(platform.input());
        if input.pressed & platform_switch::platform_api::Buttons::MINUS.0 != 0 {
            break;
        }
        if !paused {
            match session.update(&input, frame_ns as f32 * 1e-9) {
                Ok(true) => {}
                Ok(false) => break,
                Err(e) => panic!("update: {e}"),
            }
        }
        match session.render(frame_ns as f64 * 1e-6) {
            Ok(_) => {}
            Err(RenderError::LimitReached(what)) => session.message = Some(format!("renderer limit reached: {what}")),
            Err(e) => panic!("render: {e}"),
        }
        if t >= log_at {
            for line in session.status_lines(frame_ns as f64 * 1e-6) {
                debug_log(&format!("SlyNX: {line}\n"));
            }
            log_at = t + LOG_EVERY_NS;
        }
    }
    drop(session);
    drop(platform);
    debug_log("SlyNX: exit\n");
    0
}
