//! The viewer in a window on the PC.
//!
//! ```text
//! cargo run --release -p viewer --example viewer_pc -- [disc.iso] [world]
//! ```
//!
//! The disc (Sly 1 or Sly 2) may be left out when `SLYNX_DISC_SLY1` (or else `SLYNX_DISC_SLY2`) is set; `world` is
//! a name, part of a name, an id or an index (default Paris; Sly 2: the first row, or `0x3B`, `row3`). Keys (player 1): WASD fly, IJKL look, 3 (ZR) up, 1 (ZL) down, Z (B) faster, Q / E (L / R)
//! previous / next world, V (X) overlay, C (Y) back to the start, arrows up / down change the speed, Enter (+) or
//! Esc quits. Worlds load on a background thread.

use std::process::ExitCode;

use platform_api::Platform;
use platform_pc::{PcConfig, PcPlatform};
use viewer::camera::Controls;
use viewer::pc::{clock_ns, disc_path_for, gl_backend, open_disc};
use viewer::session::Session;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("viewer: {e}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let explicit =
        if args.first().is_some_and(|a| std::path::Path::new(a).is_file()) { Some(args.remove(0)) } else { None };
    // A missing disc is shown in the window, as on the Switch.
    let disc = disc_path_for(explicit.as_deref(), 1)
        .or_else(|e| disc_path_for(None, 2).map_err(|_| e))
        .and_then(|p| open_disc(&p));
    let index = match &disc {
        Ok(d) => {
            let which = args.first().cloned().unwrap_or_else(|| if d.game() >= 2 { "0" } else { "Paris" }.into());
            Some(d.world_index(&which).ok_or_else(|| format!("no world `{which}`"))?)
        }
        Err(_) => None,
    };

    let mut platform = PcPlatform::new(PcConfig { title: "SlyNX viewer".into(), ..PcConfig::default() }.with_env())?;
    let backend = gl_backend(&platform)?;
    let mut session = Session::new(backend, disc).map_err(|e| e.to_string())?;
    session.renderer.set_clock(clock_ns);
    if let Some(i) = index {
        session.request_world(&mut platform, i);
    }
    let mut last = clock_ns();
    while platform.pump() {
        while platform.poll_event().is_some() {}
        let t = clock_ns();
        let frame_ns = t - last;
        last = t;
        let controls = Controls::from_platform(platform.input());
        if !session.update(&mut platform, &controls, (frame_ns as f32 * 1e-9).min(0.1)).map_err(|e| e.to_string())? {
            break;
        }
        session.render(frame_ns).map_err(|e| e.to_string())?;
        platform.present();
    }
    drop(session);
    Ok(())
}
