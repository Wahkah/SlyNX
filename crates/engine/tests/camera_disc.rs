//! The camera against the player's own Sly 1 disc: constants from the executable, the camera zones and fade groups
//! of every world, and the follow camera behind Sly on a scripted run in Paris. Skips when `SLYNX_DISC_SLY1` is
//! not set.

use std::sync::OnceLock;

use engine::camera::fade::fade_sources;
use engine::camera::zone::zones_from_stream;
use engine::camera::{CameraConstants, CameraInput, CameraScene, FollowCamera, Lens, Subject};
use engine::collision::world::{CollisionWorld, QueryFilter};
use engine::math::{length, sub};
use engine::player::controller::Player;
use engine::player::{Button, PlayerInput, PlayerWorld, State, world_ledges};
use formats::disc::{Disc, Executable};
use formats::options::OptionTable;
use formats::world::World;
use harness::golden::{Game, require_disc};

struct Data {
    exe: Executable,
    worlds: Vec<Vec<u8>>,
}

fn disc(test: &str) -> Option<&'static Data> {
    let path = require_disc(Game::Sly1, test)?;
    static CELL: OnceLock<Data> = OnceLock::new();
    Some(CELL.get_or_init(|| {
        let mut d = Disc::open_path(&path).expect("open disc");
        let compressed: Vec<Vec<u8>> = (0..d.worlds.len()).map(|i| d.read_world_compressed(i).unwrap()).collect();
        let mut worlds = vec![Vec::new(); compressed.len()];
        std::thread::scope(|s| {
            for (o, c) in worlds.iter_mut().zip(&compressed) {
                s.spawn(move || *o = formats::compress::decompress(c).unwrap());
            }
        });
        Data { exe: d.executable.clone(), worlds }
    }))
}

#[test]
fn camera_constants_from_the_executable() {
    let Some(d) = disc("camera_constants_from_the_executable") else { return };
    let c = CameraConstants::from_executable(&d.exe).unwrap();
    assert_eq!(c, CameraConstants::SPEC);
    let mut other = d.exe.clone();
    let last = other.data.len() - 1;
    other.data[last] ^= 1;
    assert!(CameraConstants::from_executable(&other).is_err());
}

#[test]
fn zones_and_fade_groups_of_every_world() {
    let Some(d) = disc("zones_and_fade_groups_of_every_world") else { return };
    let table = OptionTable::sly1();
    let (mut zones, mut all, mut paths, mut fades) = (0, 0, 0, 0);
    for bytes in &d.worlds {
        let w = World::load(bytes).unwrap();
        all += w.objects.records.iter().filter(|r| r.class == 99).count();
        let z = zones_from_stream(w.data, &w.objects, &table).unwrap();
        paths += z.iter().filter(|z| matches!(z.frame, engine::camera::ZoneFrame::Path(_))).count();
        zones += z.len();
        fades += fade_sources(w.data, &w.objects).unwrap().len();
    }
    eprintln!("camera zones {all} on the disc, {zones} in worlds ({paths} with a path); {fades} fade sources");
    // camera.md sources: 442 camera zones on the disc.
    assert_eq!(all, 442);
    // Every zone is in a world hierarchy; collision.md 8: 83 solids carry camera-fade groups.
    assert_eq!(zones, 442);
    assert!(paths > 0);
    assert_eq!(fades, 83);
}

#[test]
fn camera_follows_sly_in_paris() {
    let Some(d) = disc("camera_follows_sly_in_paris") else { return };
    let w = World::load(&d.worlds[1]).unwrap();
    let s = &w.objects;
    let table = OptionTable::sly1();
    let player = s.records.iter().position(|r| r.class == 7 && r.oid == 5).unwrap();
    let proxy = s.records[player].parent.unwrap();
    let range = proxy..s.records[proxy as usize].subtree_end;
    let mut cw = CollisionWorld::from_world(&w, &table).unwrap();
    let own: Vec<_> = (0..cw.bodies().len() as u32)
        .filter(|&i| range.contains(&cw.bodies()[i as usize].user))
        .map(engine::collision::BodyId)
        .collect();
    for b in own {
        cw.set_enabled(b, false);
    }
    let ledges = world_ledges(&cw);
    let zones = zones_from_stream(w.data, s, &table).unwrap();
    let transforms = s.world_transforms(true);
    let fades: Vec<_> = fade_sources(w.data, s)
        .unwrap()
        .into_iter()
        .map(|f| {
            let t = transforms[f.record as usize];
            (f, t)
        })
        .collect();
    let spawn = transforms[player];
    let yaw = engine::math::atan2(spawn.cols[0][1], spawn.cols[0][0]);
    let mut sly = Player::new(spawn.t, yaw);
    let mut cam = FollowCamera::new();
    cam.set_lens(Lens { fov: 1.1, near: 70.0, far: 60000.0, fog: [1000.0, 30000.0, 0.4], fog_color: None });
    let behind = [spawn.t[0] - 450.0 * spawn.cols[0][0], spawn.t[1] - 450.0 * spawn.cols[0][1], spawn.t[2] + 205.0];
    cam.cut_to(behind, spawn.t);
    let pw = PlayerWorld {
        cw: &cw,
        filter: QueryFilter::default(),
        features: &[],
        ledges: &ledges,
        in_shadow: false,
        in_cover: false,
    };
    let scene = CameraScene { zones: &zones, collision: Some(&cw), fades: &fades, ..Default::default() };
    let dt = 1.0 / 60.0;
    let (mut on_screen, mut frames, mut zone_frames) = (0, 0, 0);
    let mut run = |input: &PlayerInput, cam_input: &CameraInput, n: usize| {
        for _ in 0..n {
            sly.update(dt, input, &pw);
            let subject = Subject {
                position: sly.position,
                velocity: sly.velocity,
                yaw: sly.yaw,
                airborne: !matches!(sly.state, State::Ground),
                ..Subject::default()
            };
            let f = cam.update(dt, cam_input, &subject, &scene);
            assert!(f.eye.iter().chain(&f.target).all(|x| x.is_finite()));
            let dist = length(sub(f.eye, f.focus));
            assert!(dist < 2000.0, "eye {dist} from the focus");
            let p = f.project(f.focus, 16.0 / 9.0);
            on_screen += (p[0].abs() < 1.0 && p[1].abs() < 1.0) as usize;
            zone_frames += f.zone.is_some() as usize;
            frames += 1;
        }
    };
    let idle = PlayerInput::default();
    run(&idle, &CameraInput::default(), 60);
    // Run forward with the camera-relative stick (the camera's heading), orbit a little, recentre, jump.
    let forward = PlayerInput { stick: [0.0, 1.0], camera_yaw: yaw, ..Default::default() };
    run(&forward, &CameraInput::default(), 90);
    run(&forward, &CameraInput { right: [0.6, 0.0], ..Default::default() }, 30);
    run(&forward, &CameraInput { recentre: true, ..Default::default() }, 1);
    run(&PlayerInput { jump: Button::PRESSED, ..forward }, &CameraInput::default(), 1);
    run(&forward, &CameraInput::default(), 120);
    eprintln!("Paris: {on_screen} of {frames} frames with Sly on screen, {zone_frames} in a camera zone");
    // The focus stays on screen (the screen offset and springs keep it near the middle).
    assert_eq!(on_screen, frames);
}

#[test]
fn camera_system_behind_the_game_seam() {
    use engine::game::systems::{CameraSystem as _, FrameInput};
    let Some(d) = disc("camera_system_behind_the_game_seam") else { return };
    let world = engine::world::World::load(d.worlds[1].clone(), engine::world::WorldInfo::default()).unwrap();
    let mut cs = engine::camera::CameraSystem::default();
    cs.configure(&world);
    assert!(!cs.zones().is_empty());
    let input = FrameInput::default();
    let mut last = None;
    for _ in 0..120 {
        let v = cs.update(&world, &input, 1.0 / 60.0, &engine::game::systems::NoCollision);
        assert!(v.eye.iter().chain(&v.target).all(|x| x.is_finite()));
        assert!((v.fov_y - 1.1).abs() < 1e-3 && v.near == 70.0 && v.far == 60000.0);
        last = Some(v);
    }
    let f = cs.frame().unwrap();
    eprintln!("seam: eye {:?} focus {:?} zone {:?}", last.unwrap().eye, f.focus, f.zone);
    assert!(length(sub(f.eye, f.focus)) < 2000.0);
    // Without a player the focus holds still.
    let before = f.focus;
    cs.update(&world, &input, 1.0 / 60.0, &engine::game::systems::NoCollision);
    assert!(length(sub(cs.frame().unwrap().focus, before)) < 1e-3);
}
