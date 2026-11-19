//! Game-loop and behaviour tests on synthetic levels.

use alloc::vec::Vec;

use super::*;
use crate::behavior::Behavior;
use crate::world::WorldInfo;
use crate::world::tests::fixture::{Fixture, at};

const WORLD: WorldInfo = WorldInfo { index: 3, id: 0x100, task_mask: 0xE, identity: 11 };

fn game(f: Fixture, progress: Progress) -> Game {
    let world = f.world(WORLD);
    Game::new(world, progress, Systems::null(), GameConfig { seed: 1, ..Default::default() })
}

fn run(g: &mut Game, frames: u32) {
    for _ in 0..frames {
        g.step_fixed(&FrameInput::default());
    }
}

#[test]
fn coins_are_collected_and_remembered() {
    let mut f = Fixture::new();
    let c = f.top(88, 300, at(0.0, 0.0, 0.0));
    f.drawable(c);
    let far = f.top(88, 301, at(5000.0, 0.0, 0.0));
    f.drawable(far);
    let mut g = game(f, Progress::new_game());
    g.world.probe = Some([10.0, 0.0, 0.0]);
    g.world.probe_collects = true;
    run(&mut g, 1);
    assert_eq!(g.progress.coins, 1);
    let hc = g.world.find_oid(300, None).unwrap();
    assert!(!g.world.in_world(hc));
    assert!(g.render.changed.contains(&0), "the collected coin's drawable changed");
    assert_eq!(g.progress.level_ref(WORLD.id).unwrap().collected.len(), 1);

    // Reloading the level: the collected coin is not placed again, the other one is.
    let progress = g.progress.clone();
    let mut f = Fixture::new();
    let c = f.top(88, 300, at(0.0, 0.0, 0.0));
    f.drawable(c);
    let far = f.top(88, 301, at(5000.0, 0.0, 0.0));
    f.drawable(far);
    let mut g2 = game(f, progress);
    let hc = g2.world.find_oid(300, None).unwrap();
    let hf = g2.world.find_oid(301, None).unwrap();
    assert!(!g2.world.in_world(hc));
    assert!(g2.world.in_world(hf));
}

#[test]
fn keys_hourglass_and_key_exits() {
    let mut f = Fixture::new();
    let _k = f.top(89, 300, at(0.0, 0.0, 0.0));
    let hg = f.top(90, 301, at(2000.0, 0.0, 0.0));
    let e = f.top(125, 302, at(9000.0, 0.0, 0.0));
    f.set_options(e, &[(169, &[1.0]), (171, &[1.0])]);
    f.exit_body(e, 7, 50.0);
    let _ = hg;
    let mut g = game(f, Progress::new_game());
    let he = g.world.find_oid(302, None).unwrap();
    let hh = g.world.find_oid(301, None).unwrap();
    assert!(!g.world.in_world(he), "key exit absent before the key");
    assert!(!g.world.in_world(hh), "no hourglass before key and vault");
    g.world.probe = Some([0.0, 0.0, 0.0]);
    g.world.probe_collects = true;
    run(&mut g, 1);
    assert!(g.progress.has_flag(WORLD.id, progress::level_flag::KEY));
    assert!(g.world.in_world(he), "key exit appears");
    let reqs = g.take_requests();
    assert!(reqs.contains(&Request::Celebrate) && reqs.contains(&Request::Autosave));
    // Walking into the key exit leaves through the default exit.
    g.world.probe = Some([9000.0, 0.0, 0.0]);
    run(&mut g, 2);
    assert!(g.take_requests().contains(&Request::DefaultExit));
}

#[test]
fn charms_swirl_then_count_and_extra_ones_go() {
    let mut f = Fixture::new();
    let _a = f.top(86, 300, at(0.0, 0.0, 0.0));
    let _b = f.top(86, 301, at(3000.0, 0.0, 0.0));
    let mut p = Progress::new_game();
    p.charms = 1;
    let mut g = game(f, p);
    g.world.probe = Some([0.0, 0.0, 0.0]);
    g.world.probe_collects = true;
    run(&mut g, 1);
    assert_eq!(g.progress.charms, 1, "not yet: it swirls first");
    run(&mut g, 80);
    assert_eq!(g.progress.charms, 2);
    let hb = g.world.find_oid(301, None).unwrap();
    assert!(!g.world.in_world(hb), "the other placed charm is removed at two charms");
}

#[test]
fn exits_wait_for_their_delay_and_are_blocked_after_arrival() {
    let mut f = Fixture::new();
    let e = f.top(125, 302, at(0.0, 0.0, 0.0));
    f.set_options(e, &[(163, &[446.0]), (166, &[2.0]), (173, &[0.5])]);
    f.exit_body(e, 7, 100.0);
    let w = f.top(116, 446, at(0.0, 0.0, 0.0));
    f.set_options(w, &[(157, &[1.0])]);
    let mut g = game(f, Progress::new_game());
    // Arriving at the warp, inside the exit: the exit is blocked until the player leaves.
    g.world.probe = Some([0.0, 0.0, 0.0]);
    let a = g.arrive(None, false).expect("default warp");
    assert_eq!(g.world.oid(a.warp), Some(446));
    run(&mut g, 60);
    assert!(g.take_requests().is_empty());
    g.world.probe = Some([1000.0, 0.0, 0.0]);
    run(&mut g, 1);
    g.world.probe = Some([0.0, 0.0, 0.0]);
    run(&mut g, 20);
    assert!(g.take_requests().is_empty(), "the 0.5 s delay");
    run(&mut g, 15);
    assert_eq!(g.take_requests(), [Request::LoadWorld { identity: 7, warp: Some(446), wipe: 2 }]);
}

#[test]
fn buttons_and_volumes_press_and_reset() {
    let mut f = Fixture::new();
    let b = f.top(40, 400, at(0.0, 0.0, 0.0));
    f.set_options(b, &[(258, &[3.0]), (257, &[0.5])]);
    let v = f.top(41, 401, at(500.0, 0.0, 0.0));
    f.set_options(v, &[(133, &[100.0]), (269, &[0.25])]);
    let mut g = game(f, Progress::new_game());
    let hb = g.world.find_oid(400, None).unwrap();
    let hv = g.world.find_oid(401, None).unwrap();
    g.world.notify(hb, crate::world::Notice::Hit(hb));
    run(&mut g, 1);
    assert!(
        matches!(g.world.behavior(hb), Some(Behavior::Button(x)) if matches!(x.state, crate::behavior::button::ButtonState::Pressed(_)))
    );
    assert!(g.audio.one_shots.iter().any(|s| s.sound == 2 && s.object == Some(hb)));
    run(&mut g, 31);
    assert!(
        matches!(g.world.behavior(hb), Some(Behavior::Button(x)) if x.state == crate::behavior::button::ButtonState::Untouched)
    );

    g.world.probe = Some([520.0, 0.0, 0.0]);
    run(&mut g, 1);
    let pressed = |g: &Game| matches!(g.world.behavior(hv), Some(Behavior::Volume(x)) if matches!(x.state, crate::behavior::volume::VolumeState::Pressed(_)));
    assert!(pressed(&g));
    // Still inside after the repush delay: it fires again (no "must exit").
    run(&mut g, 20);
    assert!(pressed(&g));
    g.world.probe = Some([0.0, 0.0, 0.0]);
    run(&mut g, 20);
    assert!(!pressed(&g));
}

#[test]
fn checkpoints_record_and_rearm() {
    let mut f = Fixture::new();
    let c = f.top(56, 600, at(0.0, 0.0, 0.0));
    let v = f.child(c, 123, 601, at(0.0, 0.0, 0.0));
    f.volume(v, 50.0);
    let _w = f.child(c, 116, 602, at(0.0, 0.0, 0.0));
    let mut g = game(f, Progress::new_game());
    g.world.probe = Some([0.0, 0.0, 0.0]);
    run(&mut g, 1);
    let rec = g.progress.level_ref(WORLD.id).unwrap();
    assert_eq!(rec.checkpoints, 1);
    assert_eq!(g.progress.respawn, Some(progress::Respawn { world: WORLD.id, warp: Some(602) }));
    assert!(g.take_requests().iter().any(|r| matches!(r, Request::Rumble { .. })));
    run(&mut g, 5);
    assert!(g.take_requests().is_empty(), "not re-armed while near");
    g.world.probe = Some([5000.0, 0.0, 0.0]);
    run(&mut g, 1);
    g.world.probe = Some([0.0, 0.0, 0.0]);
    run(&mut g, 1);
    assert!(!g.take_requests().is_empty(), "re-armed after moving away");
}

#[test]
fn clock_pause_and_real_clock_objects() {
    let mut f = Fixture::new();
    let a = f.top(2, 1, at(0.0, 0.0, 0.0));
    f.set_options(a, &[(21, &[60.0, 0.0, 0.0])]);
    let b = f.top(2, 2, at(0.0, 0.0, 0.0));
    f.set_options(b, &[(21, &[60.0, 0.0, 0.0][..]), (119, &[1.0])]);
    let mut g = game(f, Progress::new_game());
    let ha = g.world.find_oid(1, None).unwrap();
    let hb = g.world.find_oid(2, None).unwrap();
    run(&mut g, 60);
    assert!((g.world.position(ha).unwrap()[0] - 60.0).abs() < 1e-2);
    assert!((g.clock.game_time - 1.0).abs() < 1e-4);
    g.clock.rate = 0.0;
    run(&mut g, 30);
    assert!((g.clock.game_time - 1.0).abs() < 1e-4, "paused");
    assert!((g.world.position(ha).unwrap()[0] - 60.0).abs() < 1e-2);
    // Movers only move on game frames, real-clock ones included (they ride the game pass while it runs).
    let _ = hb;
    // A long frame advances at most 1/30 s.
    g.clock.rate = 1.0;
    g.frame(&FrameInput::default(), 0.5);
    assert!((g.clock.game_time - (1.0 + 1.0 / 30.0)).abs() < 1e-4);
}

#[test]
fn deterministic_runs() {
    let make = || {
        let mut f = Fixture::new();
        let a = f.top(2, 1, at(0.0, 0.0, 0.0));
        f.set_options(a, &[(22, &[0.0, 0.0, 1.0])]);
        let _ = f.top(46, 2, at(100.0, 0.0, 0.0));
        game(f, Progress::new_game())
    };
    let (mut a, mut b) = (make(), make());
    let input = FrameInput { left: [0.3, 1.0], right: [0.1, 0.0], ..Default::default() };
    let mut ha = Vec::new();
    let mut hb = Vec::new();
    for _ in 0..120 {
        a.step_fixed(&input);
        b.step_fixed(&input);
        ha.push(a.state_hash());
        hb.push(b.state_hash());
    }
    assert_eq!(ha, hb);
    assert_ne!(ha[0], ha[119]);
}

#[test]
fn audio_frame_lists_ambient_sounds() {
    let mut f = Fixture::new();
    let a = f.top(2, 1, at(0.0, 0.0, 0.0));
    f.set_options(a, &[(101, &[77.0][..]), (102, &[1000.0]), (103, &[100.0]), (21, &[60.0, 0.0, 0.0])]);
    let mut g = game(f, Progress::new_game());
    run(&mut g, 2);
    assert_eq!(g.audio.emitters.len(), 1);
    let e = g.audio.emitters[0];
    assert_eq!((e.sound, e.start_distance, e.full_distance), (77, 1000.0, 100.0));
    assert!((e.velocity[0] - 60.0).abs() < 1.0);
    assert!((g.audio.music_scale - 1.0).abs() < 1e-6);
}

#[test]
fn world_options_set_lens_and_fog() {
    let mut f = Fixture::new();
    f.world_options(&[
        (529, &[1.1][..]),
        (530, &[70.0]),
        (531, &[60000.0]),
        (535, &[255.0, 0.0, 0.0, 128.0]),
        (532, &[1000.0]),
    ]);
    let mut g = game(f, Progress::new_game());
    run(&mut g, 1);
    let c = g.render.camera;
    assert_eq!((c.fov_y, c.near, c.far), (1.1, 70.0, 60000.0));
    let fog = g.render.fog.expect("fog colour set");
    assert_eq!(fog.color, [1.0, 0.0, 0.0]);
    assert_eq!((fog.start, fog.end), (1000.0, render_scene::DEFAULT_FOG[1]));
}

#[test]
fn proximity_guards_alert_their_group_and_search_when_the_player_leaves() {
    let mut f = Fixture::new();
    let g1 = f.top(10, 700, at(0.0, 0.0, 0.0));
    f.set_options(g1, &[(845, &[1000.0, 1500.0][..]), (841, &[3000.0])]);
    let _g2 = f.top(10, 701, at(200.0, 0.0, 0.0));
    let grp = f.top(137, 702, at(0.0, 0.0, 0.0));
    f.set_options(grp, &[(694, &[700.0][..]), (694, &[701.0])]);
    let mut g = game(f, Progress::new_game());
    assert_eq!(g.world.shared.groups.len(), 1, "both guards in the listed group");
    g.world.probe = Some([500.0, 0.0, 0.0]);
    run(&mut g, 2);
    let grp = &g.world.shared.groups[0];
    assert_eq!(grp.level, crate::world::AlertLevel::Attack);
    assert_eq!(g.world.shared.intensity(), crate::world::shared::INTENSITY_ALARM);
    // Out of the leash: the group goes searching.
    g.world.probe = Some([9000.0, 0.0, 0.0]);
    run(&mut g, 2);
    assert_eq!(g.world.shared.groups[0].level, crate::world::AlertLevel::Search);
    // A knock-out without stuns left: the guard is down for good.
    let h = g.world.find_oid(700, None).unwrap();
    let other = g.world.find_oid(701, None).unwrap();
    g.world.notify(h, crate::world::Notice::Attack { by: other, kind: 0 });
    run(&mut g, 1);
    assert!(matches!(g.world.behavior(h), Some(Behavior::Guard(x)) if x.is_down()));
    assert!(!g.progress.level_ref(WORLD.id).unwrap().defeated.is_empty());
}

#[test]
fn alarms_trigger_and_arm_their_sensors() {
    let mut f = Fixture::new();
    let a = f.top(47, 800, at(0.0, 0.0, 0.0));
    f.set_options(a, &[(290, &[801.0][..])]);
    let s = f.top(50, 801, at(0.0, 0.0, 0.0));
    f.set_options(s, &[(305, &[0.4][..])]);
    let mut g = game(f, Progress::new_game());
    run(&mut g, 1);
    let ha = g.world.find_oid(800, None).unwrap();
    let hs = g.world.find_oid(801, None).unwrap();
    assert!(matches!(g.world.behavior(hs), Some(Behavior::Sensor(x)) if x.alarm == Some(ha)));
    crate::behavior::sensor::set_alarm(&mut g.world, ha, crate::behavior::sensor::AlarmState::Triggered);
    run(&mut g, 1);
    assert_eq!(g.world.shared.alarms_triggered, 1);
    assert_eq!(g.world.shared.intensity(), crate::world::shared::INTENSITY_ALARM);
    assert!(g.audio.one_shots.iter().any(|o| o.sound == crate::behavior::sensor::SIREN));
    assert!(
        matches!(g.world.behavior(hs), Some(Behavior::Sensor(x)) if x.mode == crate::behavior::sensor::SensorMode::Damage)
    );
    crate::behavior::sensor::set_alarm(&mut g.world, ha, crate::behavior::sensor::AlarmState::Disabled);
    run(&mut g, 2);
    assert_eq!(g.world.shared.intensity(), 0);
    assert!(g.progress.level_ref(WORLD.id).unwrap().pressed.len() == 1, "disabled alarms are remembered");
}

#[test]
fn breakables_release_pieces_and_bombs_break_them() {
    let mut f = Fixture::new();
    let b = f.top(33, 900, at(0.0, 0.0, 0.0));
    f.set_options(b, &[(233, &[0.0][..])]);
    let _p = f.child(b, 39, 901, at(0.0, 0.0, 10.0));
    let bomb = f.top(59, 902, at(100.0, 0.0, 0.0));
    let _ = bomb;
    let mut g = game(f, Progress::new_game());
    let hb = g.world.find_oid(902, None).unwrap();
    g.world.notify(hb, crate::world::Notice::Hit(hb));
    run(&mut g, 70);
    let br = g.world.find_oid(900, None).unwrap();
    assert!(crate::behavior::breakable::is_broken(&g.world, br), "the blast broke it");
    assert!(!g.world.in_world(br));
    assert!(!g.world.in_world(hb), "the bomb is gone");
    let piece = g.world.find_oid(901, None).unwrap();
    assert!(matches!(g.world.behavior(piece), Some(Behavior::Piece(p)) if p.released));
    assert_eq!(g.progress.level_ref(WORLD.id).unwrap().broken.len(), 1);
}

#[test]
fn the_vault_opens_on_its_combination() {
    let mut f = Fixture::new();
    let v = f.top(23, 950, at(0.0, 0.0, 0.0));
    f.set_options(v, &[(1046, &[12.0][..])]);
    let mut g = game(f, Progress::new_game());
    g.world.probe = Some([50.0, 0.0, 0.0]);
    run(&mut g, 2);
    let press = |b: platform_api::Buttons| FrameInput { pressed: b.0, buttons: b.0, ..Default::default() };
    // 0 1 2: next dial, up once, next dial, up twice.
    for b in [
        platform_api::Buttons::RIGHT,
        platform_api::Buttons::UP,
        platform_api::Buttons::RIGHT,
        platform_api::Buttons::UP,
        platform_api::Buttons::UP,
    ] {
        g.step_fixed(&press(b));
        g.step_fixed(&FrameInput::default());
    }
    assert!(g.progress.has_flag(WORLD.id, progress::level_flag::VAULT));
    assert_eq!(g.progress.vaults[&1], 1);
}

#[test]
fn effects_lightning_particles_and_darkening() {
    let mut o = crate::object::OptionStore::new();
    o.push(1133, crate::object::OptionValue::Color([255, 255, 255, 128]));
    o.push(1134, crate::object::OptionValue::Color([255, 0, 0, 64]));
    o.push(1136, crate::object::OptionValue::Float(0.1));
    o.push(1137, crate::object::OptionValue::Float(0.2));
    let c = crate::behavior::effects::Lightning::color_at(&o, 0.05).unwrap();
    assert!((c[1] - 0.5).abs() < 1e-5 && (c[3] - 0.75).abs() < 1e-5);
    assert!(crate::behavior::effects::Lightning::color_at(&o, 0.31).is_none());

    let mut f = Fixture::new();
    let e = f.top(80, 960, at(0.0, 0.0, 0.0));
    f.set_options(e, &[(351, &[60.0, 60.0][..]), (387, &[0.5]), (377, &[100.0, 100.0])]);
    let mut g = game(f, Progress::new_game());
    run(&mut g, 30);
    let n = g.world.shared.particles.len();
    assert!((25..=31).contains(&n), "{n} live particles at 60 per second for 0.5 s");
    run(&mut g, 60);
    assert!(g.world.shared.particles.iter().all(|p| p.age < 0.5));
    g.world.shared.screen.darkening_target = 0.3;
    run(&mut g, 60);
    assert!((g.world.shared.screen.darkening - 0.3).abs() < 1e-3);
}

#[test]
fn dialogues_play_once_and_are_remembered() {
    let mut f = Fixture::new();
    let d = f.top(101, 970, at(0.0, 0.0, 0.0));
    f.set_options(d, &[(972, &[1.0][..])]);
    let mut g = game(f, Progress::new_game());
    g.world.request(Request::Dialogue(970));
    run(&mut g, 2);
    let h = g.world.find_oid(970, None).unwrap();
    assert!(
        matches!(g.world.behavior(h), Some(Behavior::Dialogue(x)) if x.state == crate::behavior::dialogue::DialogueState::Done)
    );
    assert!(g.progress.level_ref(WORLD.id).unwrap().dialogues.contains(&970));
}

#[test]
fn machines_are_not_started_by_their_goal_option() {
    let mut f = Fixture::new();
    let _m = f.top(136, 980, at(0.0, 0.0, 0.0));
    let mut g = game(f, Progress::new_game());
    let h = g.world.find_oid(980, None).unwrap();
    if let Some(Behavior::Machine(m)) = g.world.behavior(h) {
        assert!(!m.run.running);
    }
    g.world.set_option(h, 516, crate::object::OptionValue::Short(3));
    run(&mut g, 1);
    if let Some(Behavior::Machine(m)) = g.world.behavior(h) {
        assert!(!m.run.running);
    }
}

#[test]
fn reference_options_read_as_objects_and_mass_takes_integers() {
    use script::ScriptHost;
    let mut f = Fixture::new();
    f.top(9, 700, at(0.0, 0.0, 0.0));
    f.top(41, 701, at(100.0, 0.0, 0.0));
    let mut g = game(f, Progress::new_game());
    let guard = g.world.find_oid(700, None).unwrap();
    let volume = g.world.find_oid(701, None).unwrap();
    let mut rng = Rng::new(1);
    let none = super::systems::NoCollision;
    let mut host = Game::host(&mut g.world, &mut rng, 0.0, &none);
    let me = guard.to_script();
    for o in [205u16, 844, 848, 880, 1084] {
        assert!(matches!(host.get_option(me, o), Value::Object(None)), "unset {o} is the null object");
        host.set_option(me, o, &Value::Object(Some(volume.to_script()))).unwrap();
        assert!(matches!(host.get_option(me, o), Value::Object(Some(v)) if v == volume.to_script()), "{o}");
        host.set_option(me, o, &Value::Object(None)).unwrap();
        assert!(matches!(host.get_option(me, o), Value::Object(None)), "cleared {o}");
    }
    host.set_option(me, 127, &Value::Int(1_000_000)).unwrap();
    assert!(matches!(host.get_option(me, 127), Value::Float(x) if x == 1_000_000.0));
}

#[test]
fn script_actions_pause_emitters_retract_lasers_and_bind_guard_states() {
    use crate::object::Arg;
    let mut f = Fixture::new();
    let e = f.top(80, 960, at(0.0, 0.0, 0.0));
    f.set_options(e, &[(351, &[60.0, 60.0][..]), (387, &[5.0]), (377, &[100.0, 100.0])]);
    f.top(49, 961, at(0.0, 0.0, 0.0));
    f.top(9, 962, at(0.0, 0.0, 0.0));
    let mut g = game(f, Progress::new_game());
    let he = g.world.find_oid(960, None).unwrap();
    // 363 pauses forever, 364 resumes, 362 pauses for a time.
    assert!(g.world.run_action(he, 363, &[]));
    run(&mut g, 30);
    assert!(g.world.shared.particles.is_empty());
    assert!(g.world.run_action(he, 364, &[]));
    run(&mut g, 30);
    let n = g.world.shared.particles.len();
    assert!(n > 0);
    assert!(g.world.run_action(he, 362, &[Arg::Float(0.5)]));
    run(&mut g, 15);
    assert_eq!(g.world.shared.particles.len(), n, "paused for 0.5 s");
    run(&mut g, 30);
    assert!(g.world.shared.particles.len() > n, "emitting again");
    // 316 retracts a laser's beams over the given time, 315 extends them.
    let hl = g.world.find_oid(961, None).unwrap();
    assert!(g.world.run_action(hl, 316, &[Arg::Float(0.5)]));
    run(&mut g, 40);
    assert!(matches!(g.world.behavior(hl), Some(Behavior::Sensor(x)) if x.extension == 0.0));
    assert!(g.world.run_action(hl, 315, &[]));
    run(&mut g, 1);
    assert!(matches!(g.world.behavior(hl), Some(Behavior::Sensor(x)) if x.extension == 1.0));
    let cut = crate::behavior::sensor::cut_polyline(&[[0.0; 3], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]], 0.75);
    assert_eq!(cut, alloc::vec![[0.0; 3], [10.0, 0.0, 0.0], [10.0, 5.0, 0.0]]);
    // 657 binds an animation to a guard state.
    let hg = g.world.find_oid(962, None).unwrap();
    assert!(g.world.run_action(hg, 657, &[Arg::Int(16), Arg::Short(6, 1234)]));
    assert!(matches!(g.world.behavior(hg), Some(Behavior::Guard(x)) if x.anims[16] == Some(1234)));
    // An action nothing implements is not run (the script host counts it).
    assert!(!g.world.run_action(hg, 663, &[Arg::Int(2)]));
}

#[test]
fn guards_turn_with_the_walkers_rule() {
    use crate::behavior::guard::turn_step;
    // r = 1: a 90-degree turn from rest is within 5 degrees after about 0.41 s (physics.md 5.1).
    let (mut angle, mut speed, mut t) = (core::f32::consts::FRAC_PI_2, 0.0f32, 0.0f32);
    while angle > 5f32.to_radians() && t < 2.0 {
        let (s, turned) = turn_step(angle, speed, 1.0, 1.0 / 60.0);
        speed = s;
        angle -= turned;
        t += 1.0 / 60.0;
    }
    assert!((t - 0.41).abs() < 0.06, "{t}");
    assert!(speed <= 25.0);
    // A larger spin ratio turns faster.
    let (s2, _) = turn_step(1.0, 0.0, 2.0, 1.0 / 60.0);
    let (s1, _) = turn_step(1.0, 0.0, 1.0, 1.0 / 60.0);
    assert!(s2 > s1);
}
