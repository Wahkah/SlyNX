//! The Sly 3 session on the player's own Sly 3 disc (`SLYNX_DISC_SLY3`), on the renderer's null backend; skipped
//! when the disc is not set.

use engine::game::systems::FrameInput;
use game::sly3::{FIRST_WORLD, Sly3Session};
use harness::golden::{Game, require_disc};
use render::NullBackend;

fn session(
    path: &std::path::Path,
    sound: bool,
) -> (Sly3Session<NullBackend, std::fs::File>, Option<audio::AudioRenderer>) {
    let disc = formats_sly3::Disc::open_path(path).map_err(|e| e.to_string());
    let s = Sly3Session::new(NullBackend::new(1280, 720), disc, 7);
    if !sound {
        return (s, None);
    }
    let (engine, renderer) = audio::create(audio::Config::default());
    let ad = audio::sly3::Sly3Disc::open_path(path).unwrap();
    (s.with_sound(engine, ad), Some(renderer))
}

/// Loads `slot`, runs `frames` frames drawing each and rendering the sound; returns the per-frame state hashes and
/// a hash of the sound.
fn run(path: &std::path::Path, slot: usize, frames: usize, sound: bool) -> (Vec<u64>, u64) {
    let (mut s, mut renderer) = session(path, sound);
    s.load_slot(slot).unwrap_or_else(|e| panic!("slot {slot}: {e}"));
    assert!(s.current().unwrap().objects > 0);
    let input = FrameInput { left: [0.0, 1.0], right: [0.2, 0.0], ..Default::default() };
    let mut hashes = Vec::with_capacity(frames);
    let mut out = vec![0.0f32; 800 * 2];
    let mut audio_hash = engine::game::Fnv::new();
    for _ in 0..frames {
        s.update(&input, 1.0 / 60.0);
        s.render().unwrap();
        if let Some(r) = renderer.as_mut() {
            r.render(&mut out);
            for v in &out {
                audio_hash.u64(v.to_bits() as u64);
            }
        }
        hashes.push(s.state_hash());
    }
    (hashes, audio_hash.finish())
}

#[test]
fn every_world_loads_and_renders_deterministically() {
    let Some(path) = require_disc(Game::Sly3, "every_world_loads_and_renders_deterministically") else { return };
    let (mut s, _) = session(&path, false);
    // The static view above the world (the follow camera's view depends on where the crew arrives).
    s.follow = false;
    let worlds = s.worlds();
    assert_eq!(worlds.len(), 39);
    let mut unseen = Vec::new();
    for (slot, name) in &worlds {
        s.load_slot(*slot).unwrap_or_else(|e| panic!("{name}: {e}"));
        let input = FrameInput::default();
        s.update(&input, 1.0 / 60.0);
        let stats = s.render().unwrap();
        // The menu world's solids are hidden or outline-only: nothing static to draw. Loading it opens the episode
        // menu (the front end), whose 2D layer is all that is in view there.
        if name == "dvd_menu" {
            assert_eq!(s.front_page(), Some(ui::sly2::front::Page::Episodes), "dvd_menu opens the episode menu");
        } else {
            assert!(s.drawn() > 0, "{name}: nothing drawn");
            // The level's instances, plus the 2D layer's (the HUD draws through the same renderer).
            assert!(stats.instances >= s.drawn(), "{name}: {} < {}", stats.instances, s.drawn());
        }
        if stats.visible == 0 {
            unseen.push(name.clone());
        }
        eprintln!("{:?} visible {}", s.status_lines(), stats.visible);
    }
    // The starting view (above the middle of the drawn instances) sees part of every world; the menu world shows its
    // episode menu.
    assert!(unseen.is_empty(), "worlds with nothing in view from the start: {unseen:?}");
    // Every world runs 60 frames the same way twice.
    for (slot, name) in &worlds {
        let a = run(&path, *slot, 60, false);
        let b = run(&path, *slot, 60, false);
        assert_eq!(a, b, "{name}: not deterministic");
    }
}

#[test]
fn the_apartment_runs_with_sound_deterministically() {
    let Some(path) = require_disc(Game::Sly3, "the_apartment_runs_with_sound_deterministically") else { return };
    let (s, _) = session(&path, false);
    let slot = s.find_world(FIRST_WORLD).unwrap();
    let a = run(&path, slot, 120, true);
    let b = run(&path, slot, 120, true);
    assert_eq!(a, b);
    assert_ne!(a.1, run(&path, slot, 120, false).1, "the world's sound plays");
}

/// Walks the crew in world `name` for `frames` frames: forward with a turn, a jump, then a switch to Bentley and
/// Murray and back to Sly. Returns the per-frame state hashes and the played object's positions.
fn walk(path: &std::path::Path, name: &str, frames: usize) -> (Vec<u64>, Vec<(i16, [f32; 3])>) {
    let (mut s, _) = session(path, false);
    let slot = s.find_world(name).unwrap();
    s.load_slot(slot).unwrap_or_else(|e| panic!("{name}: {e}"));
    // The arrival's scripts may hold the pad for a cutscene (command 0x1F; v_apt's 2360 does for 2.5 s): wait
    // them out.
    s.update(&FrameInput::default(), 1.0 / 60.0);
    let mut held = 0;
    while s.script_paused && held < 1200 {
        s.update(&FrameInput::default(), 1.0 / 60.0);
        held += 1;
    }
    eprintln!("{name}: the pad was held {held} frames on arrival");
    let mut hashes = Vec::with_capacity(frames);
    let mut trail = Vec::with_capacity(frames);
    for f in 0..frames {
        let mut input = FrameInput { left: [0.3, 1.0], ..Default::default() };
        if f % 90 == 45 {
            input.pressed |= platform_api::Buttons::B.0;
            input.buttons |= platform_api::Buttons::B.0;
        }
        match f {
            120 => input.pressed |= platform_api::Buttons::RIGHT.0,
            150 => input.pressed |= platform_api::Buttons::UP.0,
            180 => input.pressed |= platform_api::Buttons::LEFT.0,
            _ => {}
        }
        s.update(&input, 1.0 / 60.0);
        s.render().unwrap();
        hashes.push(s.state_hash());
        let w = &s.game.as_ref().unwrap().world;
        let p = w.player.expect("a player");
        trail.push((w.class(p).unwrap(), w.position(p).unwrap()));
    }
    (hashes, trail)
}

#[test]
fn sly_walks_v_apt_and_v_ext_deterministically() {
    let Some(path) = require_disc(Game::Sly3, "sly_walks_v_apt_and_v_ext_deterministically") else { return };
    for name in ["v_apt", "v_ext"] {
        let (a, trail) = walk(&path, name, 240);
        let (b, trail_b) = walk(&path, name, 240);
        assert_eq!(a, b, "{name}: not deterministic");
        assert_eq!(trail, trail_b);
        // Sly plays first, then Bentley (class 54), Murray (53), Sly (52) as the d-pad asks.
        assert_eq!(trail[0].0, 52, "{name}: Sly starts");
        assert_eq!(trail[130].0, 54, "{name}: switched to Bentley");
        assert_eq!(trail[160].0, 53, "{name}: switched to Murray");
        assert_eq!(trail[200].0, 52, "{name}: back to Sly");
        // Sly moved and stayed finite.
        let (p0, p1) = (trail[0].1, trail[119].1);
        // v_apt: after the arrival cutscene Sly still stands on the arrival floor (surface 401, z ≈ 46), held by the
        // collision tree's one-sided rule.
        if name == "v_apt" {
            assert!((p0[2] - 46.0).abs() < 6.0, "v_apt: Sly at {p0:?} after the arrival");
        }
        let moved = ((p1[0] - p0[0]).powi(2) + (p1[1] - p0[1]).powi(2)).sqrt();
        assert!(moved > 50.0, "{name}: Sly moved {moved} units");
        assert!(trail.iter().all(|t| t.1.iter().all(|v| v.is_finite())));
        eprintln!("{name}: start {:?}, after 2 s {:?}, moved {moved:.0}", p0, p1);
    }
}

#[test]
fn the_crew_arrives_at_its_checkpoint() {
    let Some(path) = require_disc(Game::Sly3, "the_crew_arrives_at_its_checkpoint") else { return };
    let (mut s, _) = session(&path, false);
    // v_ext: the checkpoint with OID 424 (characters.md 2 and 7).
    s.load_slot(s.find_world("v_ext").unwrap()).unwrap();
    let w = &s.game.as_ref().unwrap().world;
    let cp = w.slots_with_sly3_id(424).into_iter().find(|&k| w.class(w.handle(k).unwrap()) == Some(40)).unwrap();
    let cp = w.world_transform(w.handle(cp).unwrap()).unwrap().t;
    let p = w.position(w.player.unwrap()).unwrap();
    assert!((p[0] - cp[0]).abs() < 1.0 && (p[1] - cp[1]).abs() < 1.0, "Sly at {p:?}, checkpoint at {cp:?}");
    // v_apt has no checkpoint 424: the one of its six that answers to 424 by option 843 (level-flow.md 5.2).
    s.load_slot(s.find_world("v_apt").unwrap()).unwrap();
    let w = &s.game.as_ref().unwrap().world;
    let a = game::sly3::flow::find_arrival(w, 424, &[40]).expect("a point answering to 424");
    assert!(a.alias);
    assert!([139_910, 139_967, 140_036, 140_827, 140_831, 140_836].contains(&a.oid));
    let cp = w.world_transform(a.handle).unwrap().t;
    let p = w.position(w.player.unwrap()).unwrap();
    assert!((p[0] - cp[0]).abs() < 1.0 && (p[1] - cp[1]).abs() < 1.0, "Sly at {p:?}, checkpoint at {cp:?}");
}

#[test]
fn the_coffee_shop_floor_holds() {
    let Some(path) = require_disc(Game::Sly3, "the_coffee_shop_floor_holds") else { return };
    let (mut s, _) = session(&path, false);
    s.load_slot(s.find_world("v_apt").unwrap()).unwrap();
    let cw = game::sly3::crew::collision_from_world(&s.game.as_ref().unwrap().world);
    let f = engine::collision::world::QueryFilter { two_sided: true, camera: false, ignore: None };
    // Where Sly used to fall through (worlds.md, the collision test vector): the floor polygon (z −33) is there.
    for (x, y) in [(-3109.0, 5002.0), (-2897.0, 5415.0), (-2848.0, 5579.0)] {
        let g = cw.ground_probe([x, y, 20.0], 2000.0, &f).map(|h| h.point[2]);
        assert!(g.is_some_and(|z| (z + 33.0).abs() < 2.0), "({x}, {y}): ground {g:?}");
    }
}
