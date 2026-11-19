//! The game in a window on the PC.
//!
//! ```text
//! cargo run --release -p game --example game_pc -- [disc.iso] [world | --title]
//! ```
//!
//! The disc path may be left out when `SLYNX_DISC_SLY1` is set; `world` is a name, part of a name, an id or an index
//! (default Paris), `--title` starts from Splash with the title menu. Sly is played with player 1's controls
//! (keyboard or gamepad); + opens the pause menu (and skips a movie), − or Esc quits. Saves go to the user folder
//! (`SLYNX_USER_DIR`, or the platform's default). In worlds without Sly the free camera flies (WASD /
//! IJKL) and stands in for him: flying into an exit changes worlds, into a coin collects it.

use std::process::ExitCode;

use game::pc::{clock_ns, disc_path, gl_backend, open_disc};
use game::session::{Session, SessionOptions};
use platform_api::{Buttons, Platform};
use platform_pc::{PcConfig, PcPlatform};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("game: {e}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let explicit =
        if args.first().is_some_and(|a| std::path::Path::new(a).is_file()) { Some(args.remove(0)) } else { None };
    let which = args.first().cloned().unwrap_or_else(|| "Paris".into());
    let disc = disc_path(explicit.as_deref()).and_then(|p| open_disc(&p));
    let mut platform = PcPlatform::new(PcConfig { title: "SlyNX".into(), ..PcConfig::default() }.with_env())?;
    let backend = gl_backend(&platform)?;
    let disc_file = disc_path(explicit.as_deref()).ok();
    let mut session = Session::new(backend, disc, SessionOptions::default());
    session.renderer.set_clock(clock_ns);
    // Saves in the user folder (SLYNX_USER_DIR or the platform default); movies stream from the disc image.
    session.set_saves(Box::new(save::SaveStore::new(platform.fs())));
    session.set_movie_source(Box::new(move || {
        let f = std::fs::File::open(disc_file.as_ref()?).ok()?;
        Some(Box::new(f) as Box<dyn formats::iso::ReadAt>)
    }));
    let (sound, renderer) = audio::create(audio::Config::default());
    if platform.start_audio(Box::new(renderer)).is_ok() {
        session.sound = Some(game::sound::Sound::new(sound));
    }
    if which == "--title" {
        session.start_title()?;
    } else if let Some(i) = session.find_world(&which) {
        session.load_world(i, None, 0)?;
    }
    let mut last = clock_ns();
    while platform.pump() {
        while platform.poll_event().is_some() {}
        let t = clock_ns();
        let frame_ns = t - last;
        last = t;
        let input = game::frame_input(platform.input());
        if input.pressed & Buttons::MINUS.0 != 0 {
            break;
        }
        if !session.update(&input, frame_ns as f32 * 1e-9).map_err(|e| e.to_string())? {
            break;
        }
        session.render(frame_ns as f64 * 1e-6).map_err(|e| e.to_string())?;
        platform.present();
    }
    drop(session);
    Ok(())
}
