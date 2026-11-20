//! The Sly 2 job route on the player's own Sly 2 disc (`SLYNX_DISC_SLY2`; skipped without it): episode 1 from a new
//! game, scripted play, stalled tasks journalled and then finished by request, save and continue. The `smoke`
//! example runs it longer after the Sly 2 route.

mod smoke_run;

use harness::golden::{Game, require_disc};
use smoke_run::{Config, run_sly2_job};

#[test]
fn episode_one_jobs_as_far_as_they_go() {
    let Some(disc) = require_disc(Game::Sly2, "episode_one_jobs_as_far_as_they_go") else { return };
    let cfg = Config { disc, seconds: 120.0, worlds: Vec::new(), vehicles: Vec::new() };
    let a = run_sly2_job(&cfg);
    for line in &a.journal {
        eprintln!("{line}");
    }
    eprintln!("stall: {}", a.stall);
    assert!(a.stage.failure.is_none(), "{:?}", a.stage.failure);
    assert!(!a.current.is_empty(), "a task became current");
    // Deterministic: the same journal and hashes twice.
    let b = run_sly2_job(&cfg);
    assert_eq!(a.journal, b.journal);
    assert_eq!(a.stage.hashes, b.stage.hashes);
}
