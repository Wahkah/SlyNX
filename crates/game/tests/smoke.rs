//! Sly 1 end-to-end smoke test on the player's own disc (`SLYNX_DISC_SLY1`; skipped without it): the title, a new
//! game, Paris, the Hideout and each episode's first level, with scripted play. Short here; the `smoke` example runs
//! the same route for longer and checks the frame budget in release builds.

mod smoke_run;

use harness::golden::{Game, require_disc};
use smoke_run::{Config, DEFAULT_WORLDS, failures, report, run};

#[test]
fn title_new_game_paris_hideout_and_episodes() {
    let Some(disc) = require_disc(Game::Sly1, "title_new_game_paris_hideout_and_episodes") else { return };
    let cfg = Config { disc, seconds: 4.0, worlds: DEFAULT_WORLDS.to_vec(), vehicles: Vec::new() };
    let a = run(&cfg, |_| {});
    let b = run(&cfg, |_| {});
    eprintln!("{}", report(&a));
    assert_eq!(a.len(), 2 + DEFAULT_WORLDS.len());
    // Frame times mean nothing in a debug build; the example checks them in release.
    let f = failures(&a, Some(&b), !cfg!(debug_assertions));
    assert!(f.is_empty(), "{f:#?}");
}
