//! The end-to-end smoke runs as a tool (see `tests/smoke_run/mod.rs` for the routes and the checks).
//!
//! ```text
//! cargo run --release -p game --example smoke -- <disc.iso> [--game sly1|sly2|sly3] [--seconds N] [--worlds a,b] [--once]
//! ```
//!
//! `--worlds` names Sly 1 world indices, Sly 2 level rows or Sly 3 world slots; the defaults are `smoke_run`'s
//! routes. Sly 1 is the default game.
//!
//! Runs the route twice (unless `--once`) and compares every frame's state hash, prints one line per stage, and
//! exits with 1 when anything failed: a world not reached, host errors growing within a stage, a 95th-percentile
//! frame over 16.7 ms (release builds), or the two runs differing.

#[path = "../tests/smoke_run/mod.rs"]
mod smoke_run;

use smoke_run::{Config, DEFAULT_WORLDS, SLY2_ROUTE, SLY3_ROUTE, Stage, Title, failures, report, run, run_gang};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(disc) = args.first() else {
        eprintln!("usage: smoke <disc.iso> [--game sly1|sly2|sly3] [--seconds N] [--worlds a,b,...] [--once]");
        std::process::exit(2);
    };
    let value = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
    let seconds = value("--seconds").and_then(|s| s.parse().ok()).unwrap_or(20.0);
    let title = match value("--game").map(String::as_str) {
        None | Some("sly1") => None,
        Some("sly2") => Some(Title::Sly2),
        Some("sly3") => Some(Title::Sly3),
        Some(g) => {
            eprintln!("unknown game {g}: sly1, sly2 or sly3");
            std::process::exit(2);
        }
    };
    let default = match title {
        None => DEFAULT_WORLDS.to_vec(),
        Some(Title::Sly2) => SLY2_ROUTE.to_vec(),
        Some(Title::Sly3) => SLY3_ROUTE.to_vec(),
    };
    let worlds = value("--worlds").map(|s| s.split(',').filter_map(|w| w.parse().ok()).collect()).unwrap_or(default);
    let cfg = Config { disc: disc.into(), seconds, worlds };
    let route = |progress: &mut dyn FnMut(&Stage)| match title {
        None => run(&cfg, progress),
        Some(t) => run_gang(t, &cfg, progress),
    };
    let a = route(&mut |st: &Stage| eprint!("{}", report(std::slice::from_ref(st))));
    let b = (!args.iter().any(|a| a == "--once")).then(|| {
        eprintln!("-- second run (determinism)");
        route(&mut |_| {})
    });
    println!("{}", report(&a));
    let f = failures(&a, b.as_deref(), !cfg!(debug_assertions));
    if f.is_empty() {
        println!("smoke: passed");
    } else {
        for line in &f {
            println!("FAIL {line}");
        }
        std::process::exit(1);
    }
}
