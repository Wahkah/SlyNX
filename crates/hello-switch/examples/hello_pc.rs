//! The platform demo on the PC.
//!
//! ```sh
//! cargo run -p hello-switch --example hello_pc                 # window; Esc or Enter (+) exits, F11 toggles 1080p
//! cargo run -p hello-switch --example hello_pc -- --headless 120 out.ppm   # render 120 frames offscreen, save the last
//! ```
//!
//! `SLYNX_DEMO_FRAMES=n` closes the window after `n` frames and prints the status lines (for scripted checks).
//!
//! RomFS is `crates/hello-switch/romfs/` unless `SLYNX_ROMFS_DIR` says otherwise; see `PcConfig::with_env` for the
//! other variables.

use hello_switch::demo::Demo;
use hello_switch::scene::Scene;
use platform_api::{DebugDraw, Platform};
use platform_pc::{PcConfig, PcPlatform};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let headless = args.first().is_some_and(|a| a == "--headless");
    let frames: u32 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(120);
    let out = args.get(2).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("hello_pc.ppm"));

    let base = PcConfig {
        title: "SlyNX platform demo".into(),
        romfs_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("romfs"),
        ..PcConfig::default()
    };
    let config = if headless {
        PcConfig { title: base.title, romfs_dir: base.romfs_dir, ..PcConfig::headless(1280, 720) }
    } else {
        base
    };
    let mut platform = match PcPlatform::new(config.with_env()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("hello_pc: {e}");
            std::process::exit(1);
        }
    };
    let (version, renderer) = platform.gl().describe();
    eprintln!("hello_pc: {} | GL {version} | {renderer}", platform.name());

    let mut draw = platform.debug_draw(Scene::MAX_VERTICES).expect("debug draw");
    let mut demo = Demo::new(&mut platform);
    let mut vertices = Vec::with_capacity(Scene::MAX_VERTICES);
    let mut frame = 0u32;
    let frame_limit: Option<u32> = std::env::var("SLYNX_DEMO_FRAMES").ok().and_then(|v| v.parse().ok());

    while platform.pump() {
        vertices.clear();
        let Some(clear) = demo.frame(&mut platform, &mut vertices) else { break };
        frame += 1;
        if headless {
            draw.draw(clear, &vertices);
            if frame >= frames {
                let (w, h) = platform.gl().size();
                let rgba = platform.read_pixels();
                let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
                ppm.extend(rgba.as_chunks::<4>().0.iter().flat_map(|p| [p[0], p[1], p[2]]));
                std::fs::write(&out, ppm).expect("write the image");
                eprintln!("hello_pc: wrote {} after {frame} frames", out.display());
                break;
            }
        } else if let Err(e) = draw.frame(clear, &vertices) {
            eprintln!("hello_pc: {e}");
            break;
        }
        if frame_limit.is_some_and(|n| frame >= n) {
            break;
        }
    }
    for line in demo.status() {
        eprintln!("hello_pc: {line}");
    }
}
