//! The player against the player's own Sly 1 disc: constants read from the executable, speeds measured from the
//! animation segments, and Sly spawned in Paris running a scripted input. Skips when `SLYNX_DISC_SLY1` is not set.

use std::sync::OnceLock;

use engine::anim::rig::Rig;
use engine::collision::world::{CollisionWorld, QueryFilter};
use engine::math::length;
use engine::player::controller::Player;
use engine::player::{
    Button, PlayerAnimator, PlayerConstants, PlayerInput, PlayerWorld, State, root_motion_speed, world_ledges,
};
use formats::anim::Segment;
use formats::disc::{Disc, Executable};
use formats::options::OptionTable;
use formats::world::World;
use harness::golden::{Game, require_disc};

const PARIS: usize = 1;

struct Data {
    exe: Executable,
    paris: Vec<u8>,
}

fn disc(test: &str) -> Option<&'static Data> {
    let path = require_disc(Game::Sly1, test)?;
    static CELL: OnceLock<Data> = OnceLock::new();
    Some(CELL.get_or_init(|| {
        let mut d = Disc::open_path(&path).expect("open disc");
        let paris = d.read_world(PARIS).unwrap();
        Data { exe: d.executable.clone(), paris }
    }))
}

/// The player's proxy record and the range of records below it.
fn player_range(w: &World<'_>) -> (u32, std::ops::Range<u32>) {
    let s = &w.objects;
    let player = s.records.iter().position(|r| r.class == 7 && r.oid == 5).unwrap();
    let proxy = s.records[player].parent.unwrap();
    (player as u32, proxy..s.records[proxy as usize].subtree_end)
}

fn player_segments(w: &World<'_>) -> Vec<(i16, Segment)> {
    let (_, range) = player_range(w);
    let table = OptionTable::sly1();
    range
        .filter(|&k| w.objects.records[k as usize].class == 131)
        .map(|k| {
            let r = &w.objects.records[k as usize];
            (r.oid, Segment::parse(w.data, r.offset + 6, &table).unwrap())
        })
        .collect()
}

#[test]
fn constants_from_the_executable() {
    let Some(d) = disc("constants_from_the_executable") else { return };
    let c = PlayerConstants::from_executable(&d.exe).unwrap();
    // Every value read from the executable matches the spec's.
    assert_eq!(c, PlayerConstants::SPEC);
    let mut other = d.exe.clone();
    let last = other.data.len() - 1;
    other.data[last] ^= 1;
    assert!(PlayerConstants::from_executable(&other).is_err());
}

#[test]
fn speeds_from_root_motion() {
    // player-moves.md 2, 5 and 6, measured from the segments' root motion in Paris.
    let Some(d) = disc("speeds_from_root_motion") else { return };
    let w = World::load(&d.paris).unwrap();
    let segs = player_segments(&w);
    let speed = |oid: i16| root_motion_speed(&segs.iter().find(|(o, _)| *o == oid).unwrap().1).unwrap();
    let close = |a: f32, b: f32| (a - b).abs() < 0.1;
    assert!(close(speed(130), 38.3), "{}", speed(130));
    assert!(close(speed(131), 638.4), "{}", speed(131));
    assert!(close(speed(261), 550.0), "{}", speed(261));
    assert!(close(speed(181), 72.6) && close(speed(182), 416.7));
    let flatten: Vec<f32> = (159..=164).map(speed).collect();
    assert_eq!(flatten.iter().filter(|&&v| close(v, 120.0)).count(), 2, "{flatten:?}");
    assert_eq!(flatten.iter().filter(|&&v| close(v, 135.0)).count(), 4, "{flatten:?}");
    let c = PlayerConstants::SPEC;
    assert!(close(c.run_slow, speed(130)) && close(c.run_fast, speed(131)));
}

#[test]
fn sly_runs_and_jumps_in_paris() {
    let Some(d) = disc("sly_runs_and_jumps_in_paris") else { return };
    let w = World::load(&d.paris).unwrap();
    let s = &w.objects;
    let (player_rec, range) = player_range(&w);
    let mut cw = CollisionWorld::from_world(&w, &OptionTable::sly1()).unwrap();
    // The player's own solids are not scenery.
    let own: Vec<_> = cw
        .bodies()
        .iter()
        .enumerate()
        .filter(|(_, b)| range.contains(&b.user))
        .map(|(i, _)| engine::collision::BodyId(i as u32))
        .collect();
    for b in &own {
        cw.set_enabled(*b, false);
    }
    let ledges = world_ledges(&cw);
    let world_t = s.world_transforms(true);
    let spawn = world_t[player_rec as usize];
    let yaw = engine::math::atan2(spawn.cols[0][1], spawn.cols[0][0]);
    let mut p = Player::new(spawn.t, yaw);
    // The animation driver on the player's rig.
    let (_, prange) = player_range(&w);
    let mut rig = Rig::from_stream(s, w.data, prange.start);
    let mut anim = PlayerAnimator::new(player_segments(&w), |oid| rig.find_oid(oid));
    let filter = QueryFilter::default();
    let pw = PlayerWorld { cw: &cw, filter, features: &[], ledges: &ledges, in_shadow: false, in_cover: false };
    let dt = 1.0 / 60.0;
    let mut frame = |p: &mut Player, input: &PlayerInput| {
        let out = p.update(dt, input, &pw);
        if let Some(req) = out.anim {
            anim.update(&req, out.restart_anim, dt, &mut rig);
        }
        rig.update_world(&formats::geometry::Affine {
            cols: engine::math::axis_angle([0.0, 0.0, p.yaw]),
            t: p.position,
        });
        assert!(rig.world.iter().all(|a| a.t.iter().all(|x| x.is_finite())));
        out
    };
    // Settle at the spawn point.
    for _ in 0..60 {
        frame(&mut p, &PlayerInput::default());
    }
    eprintln!("Paris spawn {:?} -> {:?} ({:?})", spawn.t, p.position, p.state);
    assert_eq!(p.state, State::Ground);
    assert!(length(engine::math::sub(p.position, spawn.t)) < 300.0);
    let start = p.position;
    // Run forward (camera behind the player) for 1.5 s.
    let run = PlayerInput { stick: [0.0, 1.0], camera_yaw: yaw, ..Default::default() };
    for _ in 0..90 {
        frame(&mut p, &run);
    }
    let moved = length([p.position[0] - start[0], p.position[1] - start[1], 0.0]);
    eprintln!("ran {moved} units to {:?} ({:?})", p.position, p.state);
    assert!(moved > 100.0, "{moved}");
    // Jump and land again.
    let mut jumped = false;
    let mut landed = false;
    let out = frame(&mut p, &PlayerInput { jump: Button::PRESSED, ..run });
    jumped |= !out.events.is_empty() && matches!(p.state, State::Air { .. });
    for _ in 0..180 {
        frame(&mut p, &run);
        if p.state == State::Ground {
            landed = true;
            break;
        }
    }
    eprintln!("jump: {jumped}, landed: {landed} at {:?}", p.position);
    assert!(jumped && landed);
}
