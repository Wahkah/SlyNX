//! The Sly 2 and Sly 3 cameras on the player's own discs (`SLYNX_DISC_SLY2`, `SLYNX_DISC_SLY3`; each part skipped
//! when its disc is not set): camera zones load in every world (specs/sly2/camera.md 3 and 8; specs/sly3
//! options-by-class.md 2), the spec's zone test vector, and deterministic camera runs with a character followed.

use std::sync::OnceLock;

use engine::camera::system::CameraSystem;
use engine::camera::{CameraConstants, sly2 as camzones};
use engine::game::progress::Progress;
use engine::game::systems::{FrameInput, Systems};
use engine::game::{Game, GameConfig};
use engine::world::{World, WorldInfo};

fn path(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|p| !p.is_empty())
}

/// (row or slot, world id, bytes) per world, and the option table.
struct Data<T> {
    options: T,
    multipliers: [f32; 3],
    worlds: Vec<(usize, u32, Vec<u8>)>,
}

fn sly2() -> Option<&'static Data<formats_sly2::exe::OptionTable>> {
    static D: OnceLock<Data<formats_sly2::exe::OptionTable>> = OnceLock::new();
    let p = path("SLYNX_DISC_SLY2")?;
    Some(D.get_or_init(|| {
        let mut d = formats_sly2::Disc::open_path(&p).unwrap();
        let rows: Vec<usize> = d.world_rows().collect();
        let worlds = rows.into_iter().map(|r| (r, d.rows[r].world_id(), d.read_world(r).unwrap())).collect();
        Data { options: d.options.clone(), multipliers: [1.0; 3], worlds }
    }))
}

fn sly3() -> Option<&'static Data<formats_sly2::exe::OptionTable>> {
    static D: OnceLock<Data<formats_sly2::exe::OptionTable>> = OnceLock::new();
    let p = path("SLYNX_DISC_SLY3")?;
    Some(D.get_or_init(|| {
        let mut d = formats_sly3::Disc::open_path(&p).unwrap();
        let slots: Vec<usize> = d.world_slots().collect();
        let worlds = slots.into_iter().map(|s| (s, d.worlds[s].world_id(), d.read_world(s).unwrap())).collect();
        Data { options: d.options.clone(), multipliers: d.tangent_multipliers, worlds }
    }))
}

fn info(index: usize, id: u32) -> WorldInfo {
    WorldInfo { index: index as u32, id, task_mask: 0, identity: id }
}

fn load2(d: &Data<formats_sly2::exe::OptionTable>, i: usize) -> World {
    let (row, id, bytes) = &d.worlds[i];
    let opts = engine::world::sly2::Sly2Options {
        row: Some(*row as i16),
        info: info(*row, *id),
        ..engine::world::sly2::Sly2Options::default()
    };
    World::load_sly2(bytes.clone(), &d.options, opts).unwrap()
}

fn load3(d: &Data<formats_sly2::exe::OptionTable>, i: usize) -> World {
    let (slot, id, bytes) = &d.worlds[i];
    let opts = engine::world::sly3::Sly3Options {
        slot: Some(*slot as i16),
        info: info(*slot, *id),
        multipliers: d.multipliers,
        ..engine::world::sly3::Sly3Options::default()
    };
    World::load_sly3(bytes.clone(), &d.options, opts).unwrap()
}

/// 120 frames with the camera following the first object of `class` (a playable character), turning with the right
/// stick: the eyes and targets.
fn run(world: World, class: i16) -> Vec<[f32; 6]> {
    let mut systems = Systems::null();
    systems.camera = Box::new(CameraSystem::new(CameraConstants::SPEC, Default::default()));
    let mut g = Game::new(world, Progress::new_game(), systems, GameConfig { seed: 7, ..Default::default() });
    let player = g.world.handles().find(|&h| g.world.class(h) == Some(class));
    g.world.player = player;
    let input = FrameInput { right: [0.6, 0.0], ..Default::default() };
    (0..120)
        .map(|_| {
            g.step_fixed(&input);
            let c = g.render.camera;
            [c.eye[0], c.eye[1], c.eye[2], c.target[0], c.target[1], c.target[2]]
        })
        .collect()
}

/// The view stack on a Sly 2 world: the binocucom (near clip 49, 150 above the character), two-character framing,
/// a timed move, and back to following.
fn views_on(world: World) {
    use engine::camera::views::{CamState, TimedMove};
    let mut systems = Systems::null();
    systems.camera = Box::new(CameraSystem::new(CameraConstants::SPEC, Default::default()));
    let mut g = Game::new(world, Progress::new_game(), systems, GameConfig { seed: 7, ..Default::default() });
    let chars: Vec<_> =
        g.world.handles().filter(|&h| g.world.class(h).is_some_and(|c| (53..=55).contains(&c))).collect();
    g.world.player = chars.first().copied();
    let idle = FrameInput::default();
    for _ in 0..5 {
        g.step_fixed(&idle);
    }
    let follow_near = g.render.camera.near;
    let p = g.world.position(chars[0]).unwrap();
    g.world.shared.camera_binocucom = Some((chars[0], false));
    g.step_fixed(&idle);
    assert_eq!(g.render.camera.near, 49.0);
    assert!((g.render.camera.eye[2] - (p[2] + 150.0)).abs() < 1.0, "{:?} {p:?}", g.render.camera.eye);
    g.world.shared.camera_binocucom = None;
    g.step_fixed(&idle);
    assert_eq!(g.render.camera.near, follow_near);
    if chars.len() >= 2 {
        g.world.shared.camera_pair = Some((chars[0], chars[1]));
        g.step_fixed(&idle);
        let (a, b) = (g.world.position(chars[0]).unwrap(), g.world.position(chars[1]).unwrap());
        let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0];
        assert!((0..3).all(|k| (g.render.camera.target[k] - mid[k]).abs() < 1.0));
        g.world.shared.camera_pair = None;
    }
    let from = CamState { position: [0.0, 0.0, 1000.0], angles: [0.0; 3], fov: 1.0 };
    let to = CamState { position: [600.0, 0.0, 1000.0], angles: [0.0; 3], fov: 0.5 };
    g.world.shared.camera_move = Some(TimedMove { from, to, duration: 1.0, curve: 0, time: 0.0 });
    for _ in 0..30 {
        g.step_fixed(&idle);
    }
    assert!((g.render.camera.eye[0] - 300.0).abs() < 15.0, "{:?}", g.render.camera.eye);
    // After the move the follow view drives again.
    for _ in 0..40 {
        g.step_fixed(&idle);
    }
    assert!(g.world.shared.camera_move.is_some() && g.render.camera.fov_y != 0.5);
}

#[test]
fn sly2_zones_load_and_runs_are_deterministic() {
    let Some(d) = sly2() else {
        eprintln!("skipped: SLYNX_DISC_SLY2 is not set");
        return;
    };
    // camera.md 8: zones per level-table row.
    let expect =
        [(0, 22), (2, 26), (3, 9), (4, 10), (5, 15), (8, 35), (12, 19), (14, 53), (17, 41), (27, 35), (32, 35)];
    // camera.md 8: (row, visibility volumes, camera objects).
    let expect_views =
        [(0, 8, 8), (2, 2, 11), (3, 0, 2), (4, 0, 10), (5, 0, 4), (8, 0, 8), (12, 2, 12), (14, 2, 9), (17, 0, 8)];
    let (mut volumes, mut objects_total) = (0, 0);
    let (mut total, mut inside_own) = (0, 0);
    for i in 0..d.worlds.len() {
        let w = load2(d, i);
        let (zones, extras) = camzones::zones(&w);
        let objects = w.handles().filter(|&h| w.class(h) == Some(camzones::ZONE_CLASS_SLY2)).count();
        assert_eq!(zones.len(), objects, "row {}: every zone reads its volume", d.worlds[i].0);
        total += zones.len();
        let row = d.worlds[i].0;
        let stack = engine::camera::views::ViewStack::new(&w);
        let records = |hs: Vec<engine::object::Handle>| {
            hs.into_iter().filter_map(|h| w.record(h)).collect::<std::collections::BTreeSet<u32>>().len()
        };
        let vol = records(stack.volumes.iter().map(|v| v.object).collect());
        let obj = records(stack.objects.iter().map(|o| o.0).collect());
        volumes += vol;
        objects_total += obj;
        if let Some(&(_, v, o)) = expect_views.iter().find(|e| e.0 == row) {
            assert_eq!((vol, obj), (v, o), "row {row}: visibility volumes, camera objects");
        }
        assert!(stack.volumes.iter().all(|v| v.mask != 0));
        if let Some(&(_, n)) = expect.iter().find(|e| e.0 == row) {
            let records: std::collections::BTreeSet<u32> =
                extras.iter().filter_map(|e| e.object).filter_map(|h| w.record(h)).collect();
            // The spec counts records; a zone in a placed package has one object per copy.
            assert_eq!(records.len(), n, "row {row}");
        }
        // No zone contains a far point; with the stored winding (outward normals) the zone's own placement point,
        // where the camera stands, is inside most zones.
        for z in &zones {
            assert!(!z.contains([1.0e7, 1.0e7, 1.0e7]));
        }
        inside_own += extras
            .iter()
            .zip(&zones)
            .filter(|(e, z)| e.object.and_then(|h| w.position(h)).is_some_and(|p| z.contains(p)))
            .count();
        // camera.md 8: zone 1524 in row 3, a script-switched zone at priority 5, 400 behind, 150 above.
        if row == 3 {
            let k =
                extras.iter().position(|e| e.object.is_some_and(|h| w.sly2_id(h) == Some(1524))).expect("zone 1524");
            let z = &zones[k];
            assert_eq!((z.priority, z.framing.state_override), (5, Some(false)));
            assert_eq!((z.framing.distance, z.framing.offsets[2].c[0], z.framing.screen[1]), (-400.0, 150.0, 0.3));
        }
        if row == 2 {
            views_on(load2(d, i));
        }
        if [2usize, 3, 14].contains(&row) {
            let a = run(load2(d, i), 53);
            assert_eq!(a, run(load2(d, i), 53), "row {row} camera is deterministic");
            assert!(a.iter().all(|v| v.iter().all(|x| x.is_finite())));
        }
    }
    eprintln!(
        "Sly 2 camera zones: {total} ({inside_own} hold their origin); volumes {volumes}; camera objects {objects_total}"
    );
    assert!(inside_own * 4 > total * 3, "the stored winding puts most zones' origins inside");
    assert_eq!((volumes, objects_total), (30, 182));
}

#[test]
fn sly3_zones_load_and_runs_are_deterministic() {
    let Some(d) = sly3() else {
        eprintln!("skipped: SLYNX_DISC_SLY3 is not set");
        return;
    };
    let (mut total, mut worlds_with, mut paths, mut volumes) = (0, 0, 0, 0);
    for i in 0..d.worlds.len() {
        let w = load3(d, i);
        let (zones, _) = camzones::zones(&w);
        paths += zones.iter().filter(|z| matches!(z.frame, engine::camera::ZoneFrame::Path(_))).count();
        volumes += engine::camera::views::visibility_volumes(&w).len();
        let objects = w.handles().filter(|&h| w.class(h) == Some(camzones::ZONE_CLASS_SLY3)).count();
        assert_eq!(zones.len(), objects, "world {i}: every zone reads its volume");
        total += zones.len();
        worlds_with += usize::from(!zones.is_empty());
        if i % 8 == 0 {
            let a = run(load3(d, i), 52);
            assert_eq!(a, run(load3(d, i), 52), "world {i} camera is deterministic");
            assert!(a.iter().all(|v| v.iter().all(|x| x.is_finite())));
        }
    }
    eprintln!("Sly 3 camera zones: {total} in {worlds_with} worlds, {paths} with a path; visibility volumes {volumes}");
    assert!(paths > 0 && volumes > 0);
    assert!(total > 400 && worlds_with > 30);
}

#[test]
fn follow_constants_match_in_both_executables() {
    use camzones::FollowConstants;
    let (Some(p2), Some(p3)) = (path("SLYNX_DISC_SLY2"), path("SLYNX_DISC_SLY3")) else {
        eprintln!("skipped: SLYNX_DISC_SLY2 and SLYNX_DISC_SLY3 are needed");
        return;
    };
    let s2 = FollowConstants::from_sly2_executable(&formats_sly2::Disc::open_path(&p2).unwrap().executable).unwrap();
    let s3 = FollowConstants::from_sly3_executable(&formats_sly3::Disc::open_path(&p3).unwrap().executable).unwrap();
    // Sly 2 camera.md 4 and 6; Sly 3 camera.md 6: the same values at Sly 3's addresses.
    assert_eq!(FollowConstants { binocucom: s2.binocucom, ..FollowConstants::SPEC }, s2);
    assert_eq!(FollowConstants { binocucom: s2.binocucom, ..s3.clone() }, s2);
    // The binocucom block is not one-for-one: of Sly 2's seven 16-byte chunks, the character limits, the zoom, the
    // limit smoothing and the turn scale (chunks 0, 3, 5, 6) are Sly 3's chunks 0 to 3; Sly 3's last three differ.
    let chunk = |b: &[f32; 28], k: usize| b[k * 4..k * 4 + 4].to_vec();
    for (two, three) in [(0, 0), (3, 1), (5, 2), (6, 3)] {
        assert_eq!(chunk(&s2.binocucom, two), chunk(&s3.binocucom, three), "chunk {two}");
    }
}

#[test]
fn tree_floor_at_the_v_apt_arrival() {
    // v_apt surface 401 stores 3 corners and 1 edge, but is the floor at the arrival point (1586, −5000, z ≈ 46):
    // the tree is solid below 46 there. The tree probe finds it whether or not the stored triangles cover the point.
    let Some(p) = path("SLYNX_DISC_SLY3") else {
        eprintln!("skipped: SLYNX_DISC_SLY3 is not set");
        return;
    };
    let mut d = formats_sly3::Disc::open_path(&p).unwrap();
    let slot = d.world_slots().find(|&s| d.worlds[s].name == "v_apt").expect("v_apt");
    let (id, bytes) = (d.worlds[slot].world_id(), d.read_world(slot).unwrap());
    let opts = engine::world::sly3::Sly3Options {
        slot: Some(slot as i16),
        info: info(slot, id),
        multipliers: d.tangent_multipliers,
        ..engine::world::sly3::Sly3Options::default()
    };
    let w = World::load_sly3(bytes, &d.options, opts).unwrap();
    let cw = camzones::collision_world(&w).expect("collision");
    let f = engine::collision::world::QueryFilter::default();
    let g = cw.tree_ground_probe([1586.0, -5000.0, 200.0], 400.0, &f).expect("a floor");
    let ray = cw.ground_probe([1586.0, -5000.0, 200.0], 400.0, &f).map(|h| h.point[2]);
    eprintln!("tree floor {:?} normal {:?}; ray floor {ray:?}", g.point, g.normal);
    assert!((g.point[2] - 46.0).abs() < 1.5 && g.is_ground, "{g:?}");
    // A crew member dropped there lands on that floor, and walks on it.
    use engine::gang::sly3::Member;
    use engine::gang::{Character, GangInput, GangWorld, Who};
    let shared = engine::gang::sly3::shared();
    let gw = GangWorld { cw: &cw, filter: f, targets: &[], help: 0.0, safehouse: false, pools: &[] };
    let mut c = Character::new(Who::Sly, [1586.0, -5000.0, 146.0], 0.0);
    c.leave_ground();
    let mut out = Vec::new();
    for _ in 0..60 {
        c.update(1.0 / 60.0, &GangInput::default(), &gw, &shared, &mut out);
    }
    eprintln!("Sly after the drop: {:?} grounded {}", c.position, c.grounded);
    assert!(c.grounded && (c.position[2] - g.point[2]).abs() < 2.0, "{:?}", c.position);
    let _ = Member::Sly;
}
