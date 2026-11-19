//! The Sly 3 Switch entry point (`dist/slynx-sly3.nro`, feature `sly3-switch`).
//!
//! The disc image is the first of: the `disc` line of `sdmc:/switch/SlyNX/user/sly3.cfg`,
//! `sdmc:/switch/SlyNX/sly3.iso`, and `romfs:/sly3.iso` (where `slynx-pack` puts it). Without one, the reason goes to
//! the debug log and the program waits for − to quit. The session starts in the `world` of the settings file (a world
//! name, or a world slot number) or in [`FIRST_WORLD`]. − quits. A statistics line goes to the log every five
//! seconds. The session's sound (`Sly3Session::with_sound`) is not started here yet.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use formats_sly3::Disc;
use platform_api::{FileSystem, Root};
use platform_switch::platform_api::{Event, Platform};
use platform_switch::{SwitchConfig, SwitchPlatform, debug_log};
use render::RenderError;
use render::deko3d::Deko3dBackend;

use crate::frame_input;
use crate::settings::{FsReader, Settings};
use crate::sly3::{FIRST_WORLD, Sly3Session};

/// Instance slots and draw commands per frame.
const MAX_INSTANCES: u32 = 1 << 15;
const MAX_DRAWS: u32 = 1 << 14;
/// Nanoseconds between statistics lines.
const LOG_EVERY_NS: u64 = 5_000_000_000;
/// The Sly 2 settings file, below the SD card's user folder.
const SETTINGS_PATH: &str = "switch/SlyNX/user/sly3.cfg";

fn now() -> u64 {
    unsafe extern "C" {
        fn slynx_ticks_ns() -> u64;
    }
    // SAFETY: reads the system tick counter; no preconditions.
    unsafe { slynx_ticks_ns() }
}

/// The disc locations tried, in order.
fn candidates(settings: &Settings) -> Vec<(Root, String)> {
    let mut v = Vec::new();
    if let Some(d) = &settings.disc {
        v.push(d.clone());
    }
    v.push((Root::Sd, String::from("switch/SlyNX/sly3.iso")));
    v.push((Root::RomFs, String::from("sly3.iso")));
    v
}

/// Opens the first Sly 3 disc image found, or says which paths were tried.
fn open_disc<F: FileSystem>(fs: &F, settings: &Settings) -> Result<Disc<FsReader<F::File>>, String> {
    let mut tried = Vec::new();
    for (root, path) in candidates(settings) {
        if !fs.exists(root, &path) {
            tried.push(format!("{root:?}:/{path}"));
            continue;
        }
        let f = fs.open(root, &path).map_err(|e| format!("{path}: {e}"))?;
        return Disc::open(FsReader(f)).map_err(|e| format!("{path}: {e}"));
    }
    Err(format!("no Sly 3 disc image; tried {}", tried.join(", ")))
}

/// The world slot a `world` setting names: a slot number or a world name.
fn start_slot<R: formats::iso::ReadAt>(session: &Sly3Session<Deko3dBackend, R>, world: Option<&str>) -> Option<usize> {
    let worlds = session.worlds();
    match world.map(str::trim) {
        Some(s) => {
            s.parse::<usize>().ok().filter(|n| worlds.iter().any(|w| w.0 == *n)).or_else(|| session.find_world(s))
        }
        None => None,
    }
    .or_else(|| session.find_world(FIRST_WORLD))
    .or_else(|| worlds.first().map(|w| w.0))
}

/// Called by libnx's startup code once the runtime is up.
#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    debug_log("SlyNX 3: start\n");
    let mut platform = match SwitchPlatform::new(SwitchConfig::default()) {
        Ok(p) => p,
        Err(e) => panic!("platform: {e}"),
    };
    let backend = match Deko3dBackend::new(&platform, MAX_INSTANCES, MAX_DRAWS) {
        Ok(b) => b,
        Err(e) => panic!("renderer: {e}"),
    };
    let fs = platform.fs();
    let settings = match fs.read(Root::Sd, SETTINGS_PATH) {
        Ok(b) => Settings::parse(&String::from_utf8_lossy(&b)),
        Err(_) => Settings::default(),
    };
    let disc = open_disc(&fs, &settings);
    if let Err(e) = &disc {
        debug_log(&format!("SlyNX 3: {e}\n"));
    }
    let mut session = Sly3Session::new(backend, disc, 7);
    session.renderer.set_clock(now);
    if let Some(slot) = start_slot(&session, settings.world.as_deref()) {
        platform.set_cpu_boost(true);
        if let Err(e) = session.load_slot(slot) {
            debug_log(&format!("SlyNX 3: {e}\n"));
            session.message = Some(e);
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
        if !paused && !session.update(&input, frame_ns as f32 * 1e-9) {
            break;
        }
        match session.render() {
            Ok(_) => {}
            Err(RenderError::LimitReached(what)) => session.message = Some(format!("renderer limit reached: {what}")),
            Err(e) => panic!("render: {e}"),
        }
        if t >= log_at {
            for line in session.status_lines() {
                debug_log(&format!("SlyNX 3: {line}\n"));
            }
            log_at = t + LOG_EVERY_NS;
        }
    }
    drop(session);
    drop(platform);
    debug_log("SlyNX 3: exit\n");
    0
}
