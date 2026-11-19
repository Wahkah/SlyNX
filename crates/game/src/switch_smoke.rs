//! The Switch smoke build (`dist/slynx-smoke.nro`, feature `smoke-switch`): runs the Sly 1 smoke route of
//! [`crate::smoke`] once on the hardware with the real renderer and no player input, and writes
//! `sdmc:/switch/SlyNX/user/smoke.log` after every stage (each line also goes to the debug log). The disc image is
//! found as [`crate::settings`] says. Press + to stop early. The log ends with the failures, checked against the
//! 60 fps budget, and `smoke: passed` or `smoke: failed`.

use alloc::format;
use alloc::string::String;

use platform_switch::platform_api::{Buttons, FileSystem, Platform};
use platform_switch::{SwitchConfig, SwitchPlatform, debug_log};
use render::deko3d::Deko3dBackend;

use crate::session::{Session, SessionOptions};
use crate::settings::{Settings, open_disc_fs};
use crate::smoke::{DEFAULT_WORLDS, Hooks, Stage, failures, report, run_sly1};

/// The log, in the user folder.
pub const LOG_FILE: &str = "smoke.log";
/// Seconds of play per stage.
const SECONDS: f32 = 20.0;
/// Instance slots and draw commands per frame (as the game).
const MAX_INSTANCES: u32 = 1 << 15;
const MAX_DRAWS: u32 = 1 << 14;

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
    debug_log("SlyNX smoke: start\n");
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
    let mut text = String::new();
    let mut log = |line: &str| {
        debug_log(line);
        text.push_str(line);
        // Written after every stage, so a run cut short still leaves its results.
        let _ = fs.write_user(LOG_FILE, text.as_bytes());
    };
    let disc = open_disc_fs(&fs, &settings);
    if let Err(e) = &disc {
        log(&format!("smoke: no disc: {e}\n"));
        return 0;
    }
    let mut session = Session::new(backend, disc, SessionOptions::default());
    session.renderer.set_clock(now);
    log(&format!("smoke: Sly 1 route, {SECONDS} s per stage, worlds {DEFAULT_WORLDS:?}\n"));

    let mut stop = false;
    let mut frame_done = || {
        if !platform.pump() || platform.input().p1().pressed.0 & Buttons::PLUS.0 != 0 {
            stop = true;
        }
        !stop
    };
    let mut hooks = Hooks { now_ns: &now, frame_done: &mut frame_done };
    let stages = run_sly1(&mut session, SECONDS, &DEFAULT_WORLDS, &mut hooks, &mut |st: &Stage| {
        log(&report(core::slice::from_ref(st)))
    });
    let f = failures(&stages, None, true);
    for line in &f {
        log(&format!("FAIL {line}\n"));
    }
    log(if stop {
        "smoke: stopped\n"
    } else if f.is_empty() {
        "smoke: passed\n"
    } else {
        "smoke: failed\n"
    });
    drop(session);
    debug_log("SlyNX smoke: exit\n");
    0
}
