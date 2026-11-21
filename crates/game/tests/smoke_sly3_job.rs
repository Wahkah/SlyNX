//! The Sly 3 job route on the player's own Sly 3 disc (`SLYNX_DISC_SLY3`; skipped without it): episode 3 from a new
//! game, the safe house, then scripted play, journalling how far the first Venice job gets. `SLYNX_ROUTE_SECONDS`
//! runs it longer (60 s by default).

mod smoke_run;

use harness::golden::{Game, require_disc};
use smoke_run::{Config, run_sly3_job};

#[test]
fn the_first_venice_job_as_far_as_it_goes() {
    let Some(disc) = require_disc(Game::Sly3, "the_first_venice_job_as_far_as_it_goes") else { return };
    let seconds = std::env::var("SLYNX_ROUTE_SECONDS").ok().and_then(|v| v.parse().ok()).unwrap_or(60.0);
    let cfg = Config { disc, seconds, worlds: Vec::new(), vehicles: Vec::new() };
    let a = run_sly3_job(&cfg);
    for line in &a.journal {
        eprintln!("{line}");
    }
    eprintln!("stall: {}", a.stall);
    assert!(a.stage.failure.is_none(), "{:?}", a.stage.failure);
    assert_eq!(a.stage.world, Some(3), "Venice starts in v_ext");
    // missions.md 4.2: walking into the recon waypoint begins m1_recon at t1_recon_intro (2086).
    assert!(a.current.iter().any(|t| t.contains("(2086)")), "the job began: {:?}", a.current);
    // Deterministic: the same journal and hashes twice.
    let b = run_sly3_job(&cfg);
    assert_eq!(a.journal, b.journal);
    assert_eq!(a.stage.hashes, b.stage.hashes);
}
