//! The end-to-end smoke runs as a tool (see `tests/smoke_run/mod.rs` for the routes and the checks).
//!
//! ```text
//! cargo run --release -p game --example smoke -- <disc.iso> [--game sly1|sly2|sly3] [--seconds N] [--worlds a,b] [--once] [--job]
//! ```
//!
//! `--worlds` names Sly 1 world indices, Sly 2 level rows or Sly 3 world slots; the defaults are `smoke_run`'s
//! routes. Sly 2 and 3 runs end with the machine stages (`SLY2_VEHICLES`, `SLY3_VEHICLES`: the player drives a machine
//! in its world) unless `--worlds` is given or `--no-vehicles` is set. Sly 1 is the default game. Sly 2 with `--job`
//! plays episode 1's jobs from a new game instead (`run_sly2_job`), for `--seconds`, and prints its journal.
//!
//! Runs the route twice (unless `--once`) and compares every frame's state hash, prints one line per stage, and
//! exits with 1 when anything failed: a world not reached, host errors growing within a stage, a 95th-percentile
//! frame over 16.7 ms (release builds), or the two runs differing.

#[path = "../tests/smoke_run/mod.rs"]
mod smoke_run;

use smoke_run::{
    Config, DEFAULT_WORLDS, SLY2_ROUTE, SLY2_VEHICLES, SLY3_ROUTE, SLY3_VEHICLES, Stage, Title, failures, report, run,
    run_gang, run_sly2_job,
};

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
    let given = value("--worlds").map(|s| s.split(',').filter_map(|w| w.parse().ok()).collect::<Vec<usize>>());
    let vehicles = match title {
        _ if given.is_some() || args.iter().any(|a| a == "--no-vehicles") => Vec::new(),
        Some(Title::Sly2) => SLY2_VEHICLES.to_vec(),
        Some(Title::Sly3) => SLY3_VEHICLES.to_vec(),
        None => Vec::new(),
    };
    let worlds = given.unwrap_or(default);
    let cfg = Config { disc: disc.into(), seconds, worlds, vehicles };
    // Sly 2 `--job`: episode 1's jobs from a new game, journalled (`smoke_run::run_sly2_job`).
    if title == Some(Title::Sly2) && args.iter().any(|a| a == "--job") {
        let r = run_sly2_job(&cfg);
        for line in &r.journal {
            println!("{line}");
        }
        println!("stall: {}", r.stall);
        println!("{}", report(std::slice::from_ref(&r.stage)));
        if let Some(f) = &r.stage.failure {
            println!("FAIL {f}");
            std::process::exit(1);
        }
        println!("smoke: passed");
        return;
    }
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
