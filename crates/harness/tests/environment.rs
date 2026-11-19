//! Tests that depend on the environment: goldens kept outside the repository and the player's disc images.
//! Without the variables they skip, printing a `skipped:` note (visible with `cargo test -- --nocapture`).

use harness::apps::PatternApp;
use harness::golden::{Game, assert_golden, require_disc};
use harness::{AppConfig, InputScript, RunConfig, Tolerance, run};

#[test]
fn pattern_app_matches_its_golden() {
    let mut app = PatternApp::new(&AppConfig { width: 64, height: 36, seed: 3, ..Default::default() });
    let config = RunConfig {
        frames: 30,
        captures: vec![29],
        seed: 3,
        script: InputScript::parse("0..20 hold right\n10 tap a").unwrap(),
        ..Default::default()
    };
    let result = run(&mut app, &config).unwrap();
    assert_golden("harness/pattern-64x36-f29", result.capture(29).unwrap(), &Tolerance::exact());
}

#[test]
fn disc_images_are_readable_when_given() {
    for game in [Game::Sly1, Game::Sly2, Game::Sly3] {
        let Some(path) = require_disc(game, "disc_images_are_readable_when_given") else { continue };
        let meta = std::fs::metadata(&path).unwrap();
        assert!(meta.is_dir() || meta.len() > 0, "{} is empty", path.display());
    }
}
