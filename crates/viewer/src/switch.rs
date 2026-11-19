//! The Switch entry point (`dist/slynx-viewer.nro`).
//!
//! The disc image (Sly 1 or Sly 2) is read from `sdmc:/switch/SlyNX/sly1.iso` or `sly2.iso` (or the path in
//! `sdmc:/switch/SlyNX/user/viewer.cfg`, or `romfs:/sly1.iso` / `sly2.iso` in a packaged build). Without a disc the viewer shows why on screen and keeps running until
//! + is pressed. Panics go to the debug log (the platform's panic handler).

use alloc::format;

use platform_switch::platform_api::{Event, Platform};
use platform_switch::{SwitchConfig, SwitchPlatform, debug_log};
use render::RenderError;
use render::deko3d::Deko3dBackend;

use crate::camera::Controls;
use crate::find_by_name;
use crate::loader::{Settings, open_disc_fs};
use crate::session::Session;

/// Instance slots and draw commands per frame (Paris uses about 6,500 instances and a few thousand draws).
const MAX_INSTANCES: u32 = 1 << 15;
const MAX_DRAWS: u32 = 1 << 14;
/// World shown first when the settings name none: Paris (Sly 2: the first row).
const FIRST_WORLD: &str = "Paris";
/// Seconds between statistics lines in the debug log.
const LOG_EVERY_NS: u64 = 5_000_000_000;

fn now() -> u64 {
    // The platform's clock is a method; the renderer wants a plain function. The tick counter is global, so ask
    // the platform layer's C clock directly.
    unsafe extern "C" {
        fn slynx_ticks_ns() -> u64;
    }
    // SAFETY: reads the system tick counter; no preconditions.
    unsafe { slynx_ticks_ns() }
}

/// Called by libnx's startup code once the runtime is up.
#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    debug_log("SlyNX viewer: start\n");
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
    platform.set_cpu_boost(true);
    let disc = open_disc_fs(&fs, &settings);
    platform.set_cpu_boost(false);
    if let Err(e) = &disc {
        debug_log(&format!("SlyNX viewer: {e}\n"));
    }
    let mut session = match Session::new(backend, disc) {
        Ok(s) => s,
        Err(e) => panic!("renderer: {e}"),
    };
    session.renderer.set_clock(now);
    let first = settings.world.as_deref().unwrap_or(FIRST_WORLD);
    let names = session.world_names();
    if !names.is_empty() {
        let index = find_by_name(names, first).unwrap_or(0);
        session.request_world(&mut platform, index);
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
        let controls = Controls::from_platform(platform.input());
        let dt = if paused { 0.0 } else { (frame_ns as f32 * 1e-9).min(0.1) };
        match session.update(&mut platform, &controls, dt) {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) => panic!("upload: {e}"),
        }
        let stats = match session.render(frame_ns) {
            Ok(s) => s,
            Err(RenderError::LimitReached(what)) => {
                // The frame was shown without what did not fit; say so on screen and in the log, and go on.
                let msg = format!("renderer limit reached: {what}");
                if session.message.as_deref() != Some(msg.as_str()) {
                    debug_log(&format!("SlyNX viewer: {msg}\n"));
                    session.message = Some(msg);
                }
                *session.last_stats()
            }
            Err(e) => panic!("render: {e}"),
        };
        if t >= log_at {
            let m = session.smoothed();
            let cmd = session.renderer.backend().cmd_usage();
            let name = session.current().map_or("-", |c| c.name.as_str());
            debug_log(&format!(
                "{name}: fps {:.1}, renderer CPU {:.3} ms (build {:.3}, submit {:.3}), fence wait {:.3} ms, \
                 {}/{} instances, {} batches, {} draws, {} triangles, command memory {} of {} KiB\n",
                1000.0 / m.frame_ms.max(1e-3),
                m.cpu_ms,
                m.build_ms,
                m.submit_ms,
                stats.wait_ns as f64 * 1e-6,
                stats.visible,
                stats.instances,
                stats.batches,
                stats.draw_commands,
                stats.triangles,
                cmd.used / 1024,
                cmd.budget / 1024,
            ));
            log_at = t + LOG_EVERY_NS;
        }
    }
    drop(session);
    drop(platform);
    debug_log("SlyNX viewer: exit\n");
    0
}
