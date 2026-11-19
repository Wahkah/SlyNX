//! CPU cost of the engine's per-frame world update (no renderer), on worlds of the player's disc.
//!
//! ```text
//! cargo run --release -p game --example world_bench -- [disc.iso] [world | --all] [frames]
//! ```
//!
//! Loads the world, starts it (scripts, post-load), then times `frames` fixed 1/60 s frames with the free camera
//! flying forward (so exits, volumes and pickups are tested against a moving probe). Prints the mean, median, 95th
//! and 99th percentile and maximum per frame, and the load time.

use std::time::Instant;

use engine::game::progress::Progress;
use engine::game::systems::{FrameInput, Systems};
use engine::game::{Game, GameConfig};
use engine::world::{World, WorldInfo};
use formats::disc::Disc;
use script::OptionMap;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let explicit =
        if args.first().is_some_and(|a| std::path::Path::new(a).is_file()) { Some(args.remove(0)) } else { None };
    let path = game::pc::disc_path(explicit.as_deref()).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let which = args.first().cloned().unwrap_or_else(|| "Paris".into());
    let frames: usize = args.get(1).and_then(|f| f.parse().ok()).unwrap_or(1200);
    let mut disc = Disc::open_path(&path).expect("open disc");
    let map = OptionMap::from_executable(&disc.executable).ok();
    let indices: Vec<usize> = if which == "--all" {
        (0..disc.worlds.len()).collect()
    } else {
        vec![game::settings::world_index(&disc.worlds, &which).expect("no such world")]
    };
    println!(
        "{:<30} {:>7} {:>6} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "world", "objects", "tick", "load ms", "mean", "median", "p95", "p99", "max"
    );
    for i in indices {
        let e = disc.worlds[i].clone();
        let bytes = disc.read_world(i).expect("read world");
        let info = WorldInfo { index: i as u32, id: e.id, task_mask: e.task_mask, identity: e.file.identity };
        let t0 = Instant::now();
        let world = World::load(bytes, info).expect("load");
        let cfg = GameConfig { seed: 1, option_map: map.clone(), no_scripts: false };
        let mut g = Game::new(world, Progress::new_game(), Systems::default(), cfg);
        let load_ms = t0.elapsed().as_secs_f64() * 1e3;
        g.world.probe_collects = true;
        let input = FrameInput { left: [0.0, 0.5], right: [0.2, 0.0], ..Default::default() };
        let mut times = Vec::with_capacity(frames);
        for _ in 0..frames {
            g.world.probe = Some(g.render.camera.eye);
            let t = Instant::now();
            g.step_fixed(&input);
            times.push(t.elapsed().as_secs_f64() * 1e3);
        }
        let ticking = g.world.ticking().len();
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        times.sort_by(f64::total_cmp);
        let q = |p: f64| times[((times.len() - 1) as f64 * p).round() as usize];
        println!(
            "{:<30} {:>7} {:>6} {:>8.1} {:>8.4} {:>8.4} {:>8.4} {:>8.4} {:>8.4}",
            e.name,
            g.world.len(),
            ticking,
            load_ms,
            mean,
            q(0.5),
            q(0.95),
            q(0.99),
            times[times.len() - 1]
        );
    }
}
