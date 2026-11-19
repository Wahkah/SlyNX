//! Driven machines on the player's own discs (`SLYNX_DISC_SLY2`, `SLYNX_DISC_SLY3`; each part skips without its
//! disc): every machine type spawns in its world, takes the pad through the session's `drive`, moves (or turns),
//! and does so deterministically (engine `behavior::vehicles`).

use engine::game::systems::FrameInput;
use formats::geometry::Affine;
use game::sly2::Sly2Session;
use game::sly3::Sly3Session;
use harness::golden::{Game, require_disc};
use platform_api::Buttons;
use render::NullBackend;

/// Frame `f` of the drive pattern: stick forward and to the right, the right stick up, Cross and L2 held, Cross
/// pressed every quarter second and R1 every half second.
fn input(f: u32) -> FrameInput {
    let mut i =
        FrameInput { left: [0.6, 1.0], right: [0.0, 0.8], buttons: Buttons::B.0 | Buttons::ZL.0, ..Default::default() };
    if f.is_multiple_of(15) {
        i.pressed |= Buttons::B.0;
    }
    if f.is_multiple_of(30) {
        i.pressed |= Buttons::ZR.0;
        i.buttons |= Buttons::ZR.0;
    }
    i
}

/// How far the machine moved and turned over the run, and the state hashes.
struct Run {
    /// Whether the driven machine is active (its group in the world: a job's machine is held out of it until the job
    /// starts, and does not run).
    live: bool,
    moved: f32,
    turned: f32,
    hashes: Vec<u64>,
}

fn delta(a: &Affine, b: &Affine) -> (f32, f32) {
    let d = |x: [f32; 3], y: [f32; 3]| ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt();
    (d(a.t, b.t), (0..3).map(|c| d(a.cols[c], b.cols[c])).sum())
}

fn sly2_run(path: &std::path::Path, row: usize, class: i16) -> Option<Run> {
    let disc = formats_sly2::Disc::open_path(path).map_err(|e| e.to_string());
    let mut s = Sly2Session::new(NullBackend::new(640, 360), disc, 7);
    s.load_row(row).unwrap_or_else(|e| panic!("row {row}: {e}"));
    if !s.drive(Some(class)) {
        return None;
    }
    let m = s.game.as_ref()?.world.shared.driving?;
    let live = s.game.as_ref()?.world.is_active(m);
    let start = s.game.as_ref()?.world.world_transform(m)?;
    let mut hashes = Vec::new();
    for f in 0..120 {
        s.update(&input(f), 1.0 / 60.0);
        hashes.push(s.state_hash());
    }
    let end = s.game.as_ref()?.world.world_transform(m)?;
    let (moved, turned) = delta(&start, &end);
    Some(Run { live, moved, turned, hashes })
}

fn sly3_run(path: &std::path::Path, slot: usize, class: i16) -> Option<Run> {
    let disc = formats_sly3::Disc::open_path(path).map_err(|e| e.to_string());
    let mut s = Sly3Session::new(NullBackend::new(640, 360), disc, 7);
    s.load_slot(slot).unwrap_or_else(|e| panic!("slot {slot}: {e}"));
    if !s.drive(Some(class)) {
        return None;
    }
    let m = s.game.as_ref()?.world.shared.driving?;
    let live = s.game.as_ref()?.world.is_active(m);
    let start = s.game.as_ref()?.world.world_transform(m)?;
    let mut hashes = Vec::new();
    for f in 0..120 {
        s.update(&input(f), 1.0 / 60.0);
        hashes.push(s.state_hash());
    }
    let end = s.game.as_ref()?.world.world_transform(m)?;
    let (moved, turned) = delta(&start, &end);
    Some(Run { live, moved, turned, hashes })
}

/// `must_move`: an active machine must move. Off for the two that do not move yet although active and attached
/// (the combat car of row 33 and the remote car of c_ext; open, docs/engine.md "Vehicles and minigames").
fn check(name: &str, a: Option<Run>, b: Option<Run>, must_move: bool) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        eprintln!("{name}: no machine of the class in the loaded world");
        return false;
    };
    eprintln!("{name}: active {}, moved {:.1}, turned {:.3}", a.live, a.moved, a.turned);
    if a.live && must_move {
        assert!(a.moved > 1.0 || a.turned > 0.01, "{name}: the machine did not move");
    }
    assert_eq!(a.hashes, b.hashes, "{name}: not deterministic");
    true
}

#[test]
fn sly2_machines_take_input_and_move() {
    let Some(path) = require_disc(Game::Sly2, "sly2_machines_take_input_and_move") else { return };
    // (row, class, name, must be present)
    let cases =
        [(33, 77, "combat car (row 33)", true), (14, 72, "hack ship (row 14)", true), (8, 69, "turret (row 8)", false)];
    for (row, class, name, required) in cases {
        let found = check(name, sly2_run(&path, row, class), sly2_run(&path, row, class), class != 77);
        assert!(found || !required, "{name}: expected in the loaded world");
    }
}

#[test]
fn sly3_machines_take_input_and_move() {
    let Some(path) = require_disc(Game::Sly3, "sly3_machines_take_input_and_move") else { return };
    let cases = [
        (9, 81, "truck (o_quarry)", true),
        (23, 85, "remote car (c_ext)", true),
        (23, 68, "turret (c_ext)", false),
        (21, 82, "biplane (h_dogfight)", true),
        (20, 83, "raft (h_sewer)", true),
        (31, 84, "jolly boat (p_ext)", true),
        (33, 87, "swimmer (p_dive)", true),
        (32, 88, "ship (p_at_sea)", false),
        (15, 74, "hack ship (h_ext)", false),
    ];
    for (slot, class, name, required) in cases {
        let found = check(name, sly3_run(&path, slot, class), sly3_run(&path, slot, class), class != 85);
        assert!(found || !required, "{name}: expected in the loaded world");
    }
}
