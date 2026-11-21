use super::*;
use crate::world::WorldInfo;
use crate::world::tests::fixture::{Fixture, at};

/// A world holding one object of `class`.
fn one(class: i16) -> (World, Handle) {
    let mut f = Fixture::new();
    f.top(class, 50, at(0.0, 0.0, 0.0));
    let w = f.world(WorldInfo::default());
    let h = w.handles().next().expect("object");
    (w, h)
}

const DT: f32 = 1.0 / 60.0;

fn pad(stick: [f32; 2], held: u32, pressed: u32) -> Pad {
    Pad { stick, right: [0.0; 2], held, pressed }
}

#[test]
fn buttons_map_like_the_gang() {
    let i = FrameInput { buttons: (1 << 1) | (1 << 9), pressed: 1 << 4, ..Default::default() };
    let p = Pad::from_frame(&i);
    assert_eq!(p.held, button::CROSS | button::R1);
    assert_eq!(p.pressed, button::L3);
    let mut presses = Presses::default();
    presses.update(&p, DT);
    assert!(presses.take(button::L3, 0.15) && !presses.take(button::L3, 0.15), "consumed");
    presses.update(&pad([0.0; 2], 0, button::CROSS), DT);
    for _ in 0..12 {
        presses.update(&Pad::default(), DT);
    }
    assert!(!presses.take(button::CROSS, 0.15), "older than 0.15 s");
}

#[test]
fn orientation_and_heading_round_trip() {
    let m = orientation(0.7, 0.3, 0.2);
    let (y, p) = heading_of(&m);
    assert!((y - 0.7).abs() < 1e-5 && (p - 0.3).abs() < 1e-5);
    assert!((crate::math::determinant(&m) - 1.0).abs() < 1e-5);
    assert!((wrap(3.5) - (3.5 - core::f32::consts::TAU)).abs() < 1e-5);
}

#[test]
fn turret_aims_within_limits_zooms_and_fires() {
    let mut t = turret_default();
    t.tilt_limits = [-0.75, 1.4];
    t.pan_limits = [-1.5, 3.0];
    t.fov_range = [1.0, 1.0];
    // Equal near and far: zooming in reaches half the field of view.
    assert_eq!(t.fov(), 1.0);
    for _ in 0..600 {
        t.step(&Pad { stick: [-1.0, 1.0], right: [0.0, 1.0], ..Default::default() }, DT);
    }
    assert_eq!(t.pan, 3.0, "pan clamped to the upper limit (stick left pans positive)");
    assert_eq!(t.tilt, 1.4);
    assert!((t.fov() - 0.5).abs() < 1e-6);
    // The speed cap: 1.3·f·0.9 at f = 0.5.
    let mut u = turret_default();
    u.fov_range = [0.5, 0.5];
    u.zoom = 0.0;
    for _ in 0..30 {
        u.step(&pad([1.0, 0.0], 0, 0), DT);
    }
    assert!(u.pan < 0.0);
    assert!(u.step(&pad([0.0; 2], 0, button::TRIANGLE), DT));
    assert_eq!(u.shots, 1);
}

fn turret_default() -> turret::Turret {
    let (w, h) = one(69);
    turret::Turret::new(&w, h, Title::Sly2)
}

#[test]
fn hack_knock_back_and_constants() {
    assert_eq!(hack::knock_speed([300.0, 50.0, 0.0], [0.0, 800.0], 2.0), 400.0);
    assert_eq!(hack::knock_speed([300.0, 50.0, 0.0], [0.0, 800.0], 20.0), 800.0);
    assert_eq!(hack::knock_speed([0.0, 20.0, 0.0], [0.0, 1000.0], 1.0), 20.0);
    assert_eq!((hack::SPEED, hack::ACCEL_GAIN, hack::DOCK_RANGE), (1000.0, 5.0, 50.0));
}

#[test]
fn car_steers_drives_and_reverses() {
    let (w, h) = one(85);
    let mut c = car::Car::new(&w, h, 85);
    assert_eq!((c.forward_force, c.reverse_force, c.steer_limit, c.wheelbase), (1100.0, 700.0, 0.8, 300.0));
    let mut force = 0.0;
    for _ in 0..60 {
        force = c.control(&pad([1.0, 0.0], button::CROSS, 0), true, DT);
        c.drive(force, DT);
    }
    assert_eq!(force, 1100.0);
    assert!((c.steer + 0.8).abs() < 1e-3, "stick right steers to −limit: {}", c.steer);
    assert!(c.speed > 0.0 && c.heading < 0.0, "turning right while driving forward");
    assert_eq!(c.control(&pad([0.0; 2], button::SQUARE, 0), true, DT), -700.0);
    assert_eq!(c.control(&pad([0.0; 2], button::CROSS, 0), false, DT), 0.0, "not driven: no force");
    c.control(&pad([0.0; 2], 0, button::L3), true, DT);
    assert_eq!(c.honks, 1);
}

#[test]
fn combat_car_health_and_ammo() {
    let (w, h) = one(77);
    let mut c = car::CombatCar::new(&w, h);
    assert_eq!((c.max_health, c.ammo), (100.0, [30; 3]));
    for _ in 0..9 {
        assert!(!c.damage(10.0));
    }
    assert!(c.damage(10.0) && c.mode == 3);
    assert!(!c.damage(10.0), "defeated once");
}

#[test]
fn biplane_throttle_bank_and_pitch_limit() {
    let (w, h) = one(82);
    let mut b = air::Biplane::new(&w, h);
    for _ in 0..300 {
        b.fly(&pad([0.1, 1.0], button::CROSS, 0), DT);
    }
    assert!((b.speed - 2880.0).abs() < 1.0, "1,800 × 1.6: {}", b.speed);
    assert_eq!(b.bank, 0.0, "stick x under the dead zone");
    assert!((b.pitch - air::PITCH_LIMIT).abs() < 1e-3);
    let h0 = b.heading;
    for _ in 0..30 {
        b.fly(&pad([1.0, 0.0], 0, 0), DT);
    }
    assert!(b.heading < h0 && (b.bank - air::BANK).abs() < 1e-6, "stick right turns right");
}

#[test]
fn raft_strokes_turn_and_cannon() {
    let (w, h) = one(84);
    let mut r = craft::Raft::new(&w, h, true);
    r.paddle(&pad([0.0; 2], 0, button::CROSS), None, DT);
    assert!((r.velocity[0] - craft::STROKE * DT * (1.0 - r.drag * DT)).abs() < 1e-3);
    assert_eq!(r.strokes, 1);
    for _ in 0..120 {
        r.paddle(&pad([1.0, 0.0], 0, 0), Some(1.0), DT);
    }
    assert!((r.heading - 1.0).abs() < 1e-3, "turned to the wanted heading");
    // Cannon: hold R1 for 0.5 s, release: one shot of range 600 + 0.5 × 2000.
    for _ in 0..31 {
        r.paddle(&pad([0.0; 2], button::R1, 0), None, DT);
    }
    for _ in 0..3 {
        r.paddle(&Pad::default(), None, DT);
    }
    assert_eq!(r.shots.len(), 1);
    assert!((r.shots[0] - 1600.0).abs() < 40.0, "{:?}", r.shots);
}

#[test]
fn swimmer_turns_with_limited_acceleration() {
    let (w, h) = one(87);
    let mut s = craft::Swimmer::new(&w, h);
    s.steer(&pad([1.0, 0.0], 0, 0), DT);
    assert!((s.rates[0] + craft::SWIM_ACCEL * DT).abs() < 1e-5, "rate limited by 12 rad/s²");
    for _ in 0..600 {
        s.steer(&pad([0.0, -1.0], 0, 0), DT);
    }
    assert!(s.tilt.abs() <= craft::SWIM_TILT + 1e-6);
}

#[test]
fn ship_turns_by_rudder_and_length() {
    let (w, h) = one(88);
    let mut s = craft::Ship::new(&w, h);
    for _ in 0..120 {
        s.steer(&pad([0.0; 2], button::L2, 0), DT);
    }
    assert!((s.rudder - craft::RUDDER).abs() < 1e-5);
    assert!(s.heading > 0.0, "L2 turns left");
    let r = s.rudder;
    s.steer(&pad([0.0; 2], button::L2 | button::R2, 0), DT);
    assert!(s.rudder < r, "both held: back toward 0");
}

#[test]
fn dance_scores_presses_misses_and_help() {
    let mut s = dance::Score::default();
    s.beats.push(dance::Beat { dancer: None, button: button::CROSS, time: 1.0, done: false, passed: false });
    s.beats.push(dance::Beat { dancer: None, button: button::SQUARE, time: 2.0, done: false, passed: false });
    s.beats.push(dance::Beat { dancer: None, button: button::CIRCLE, time: 3.0, done: false, passed: false });
    assert_eq!(s.step(button::CROSS, 0.2, 0.0), [dance::EARLY]);
    let got = s.step(button::CROSS, 1.2, 0.0);
    assert!((got[0] - 0.5).abs() < 1e-6, "0.2 s late of 0.4: {got:?}");
    assert_eq!(s.step(button::CROSS, 1.3, 0.0), [dance::TWICE]);
    assert_eq!(s.step(button::CIRCLE, 2.0, 0.0), [dance::WRONG]);
    // Missed at full help: −1.5 × clamp(1.2 − 0.6) = −0.9.
    let got = s.step(0, 3.5, 1.0);
    assert!((got[0] + 0.9).abs() < 1e-6, "{got:?}");
    assert_eq!(dance::negative_scale(0.0), 1.0);
    assert_eq!(s.reaction(), 0);
}

#[test]
fn boss_block_then_hit_points() {
    let (w, h) = one(61);
    let mut b = boss::Boss::new(&w, h);
    b.hit_points = 100.0;
    b.block = 50.0;
    assert!(!b.hit(30.0));
    assert_eq!((b.block, b.hit_points), (20.0, 100.0));
    assert!(!b.hit(30.0));
    assert_eq!((b.block, b.hit_points), (0.0, 90.0));
    assert!(b.hit(90.0) && b.knocked_out);
}

#[test]
fn biplane_pilot_scores_targets_and_fires_at_its_rate() {
    use core::f32::consts::{FRAC_PI_2, PI};
    // Ahead and near beats behind; the current target and a player's plane are preferred.
    assert_eq!(air::chase_score(1000.0, 0.1, air::TURN_ANGLE, false, false), 1000.0);
    let behind = air::chase_score(1000.0, PI, air::TURN_ANGLE, false, false);
    assert!((behind - 4000.0).abs() < 1e-3, "full penalty behind: {behind}");
    let side = air::chase_score(0.0, FRAC_PI_2, air::TURN_ANGLE, false, false);
    assert!(side > 500.0 && side < 3000.0);
    assert_eq!(air::chase_score(1000.0, 0.0, air::TURN_ANGLE, true, true), 1000.0 - 1000.0 - 750.0);
    // The eight manoeuvres: three headings by three pitches, speeds between the slow and fast factors.
    assert_eq!(air::evasion(0, 0.0), (-0.6, air::PITCH_LIMIT, air::BRAKE));
    assert_eq!(air::evasion(4, 1.0).0, 0.0);
    assert!((air::evasion(7, 1.0).2 - air::BOOST).abs() < 1e-6);

    let (w, h) = one(82);
    let mut b = air::Biplane::new(&w, h);
    let enemy = air::Contact {
        plane: h,
        position: [5000.0, 0.0, 0.0],
        forward: [1.0, 0.0, 0.0],
        side: 1,
        player: true,
        targets_me: false,
    };
    let mut r = 0.5f32;
    let mut rand = || {
        r = (r * 7.31).fract();
        r
    };
    for _ in 0..30 {
        b.pilot([0.0; 3], &[enemy], &mut rand, DT);
    }
    assert_eq!(b.target, Some(h));
    assert!(b.threat.is_some(), "a player's plane of another side is a threat");
    // Guns: at most the rate (10 per second) holding the trigger, the entries in turn.
    for _ in 0..60 {
        b.guns([0.0; 3], true, DT);
    }
    assert!((9..=11).contains(&b.shots.len()), "{}", b.shots.len());
    let v = b.shots[0].velocity;
    assert!(crate::math::length(v) > air::SHOT_SPEED * 0.9);
    // Launching sends them as projectiles and empties the list, so it never grows while flying.
    let mut w = w;
    let n = b.shots.len();
    b.launch(&mut w, h);
    assert!(b.shots.is_empty());
    assert_eq!(w.shared.vehicles.shots.len(), n);
}

#[test]
fn swimmer_push_decays_from_3375_to_1125() {
    assert!((craft::push_strength(1.0) - 3375.0).abs() < 1e-2);
    assert!((craft::push_strength(0.0) - 1125.0).abs() < 1e-2);
    let (w, h) = one(87);
    let mut s = craft::Swimmer::new(&w, h);
    s.tilt = 0.3;
    s.step(&pad([0.0; 2], 0, button::CROSS), DT);
    assert_eq!((s.state, s.strokes), (3, 1));
    assert!(s.push > 0.9 && s.push_dir == -1.0, "Cross above level pushes back along −X");
    for _ in 0..30 {
        s.step(&Pad::default(), DT);
    }
    assert_eq!(s.state, 4);
    assert_eq!(s.push, 0.0, "u falls by 2 per second");
    for _ in 0..31 {
        s.step(&Pad::default(), DT);
    }
    assert_eq!(s.state, 2);
    // State 5 takes no input.
    s.state = 5;
    let t = s.tilt;
    s.step(&pad([0.0, 1.0], 0, button::TRIANGLE), DT);
    assert_eq!((s.tilt, s.strokes), (t, 1));
}

#[test]
fn ship_sailing_records_and_throttle() {
    use crate::object::Arg;
    use core::f32::consts::{FRAC_PI_2, PI};
    assert!((craft::throttle(0.0) - 0.3).abs() < 1e-6, "into the wind");
    assert!((craft::throttle(FRAC_PI_2) - 1.55).abs() < 1e-6, "across it");
    assert!((craft::throttle(PI) - 1.3).abs() < 1e-6, "before it");
    assert!((craft::throttle(PI / 24.0) - 0.4).abs() < 1e-5, "interpolated half way to 0.5");
    let rec = |v: f32| alloc::vec![Arg::Float(v)];
    let (s4, sp4, t4, end) = (alloc::vec![Arg::Int(4)], rec(750.0), rec(0.9), alloc::vec![]);
    let (s2, sp2, t2) = (alloc::vec![Arg::Int(2)], rec(3000.0), rec(1.58));
    let actions: alloc::vec::Vec<(u16, &[Arg])> = alloc::vec![
        (1954, &s4[..]),
        (1955, &sp4[..]),
        (1959, &t4[..]),
        (1961, &end[..]),
        (1954, &s2[..]),
        (1955, &sp2[..]),
        (1959, &t2[..]),
        (1961, &end[..]),
    ];
    let r = craft::sail_records(&actions);
    assert_eq!(r.len(), 2);
    assert_eq!((r[0].state, r[0].speed, r[0].turn), (4, 750.0, 0.9));
    assert_eq!((r[1].state, r[1].speed, r[1].turn), (2, 3000.0, 1.58));
}

#[test]
fn projectiles_hit_guards_and_expire() {
    let mut f = Fixture::new();
    f.top(58, 50, at(1000.0, 0.0, 0.0));
    f.top(69, 51, at(0.0, 0.0, 0.0));
    let mut w = f.world(WorldInfo::default());
    w.update_transforms();
    let hs: alloc::vec::Vec<Handle> = w.handles().collect();
    let (guard, gun) = (hs[0], hs[1]);
    let shot = |velocity, life| shots::Projectile {
        position: [0.0; 3],
        velocity,
        life,
        owner: gun,
        damage: 10.0,
        gravity: 0.0,
    };
    shots::fire(&mut w, shot([3000.0, 0.0, 0.0], 2.0));
    shots::fire(&mut w, shot([0.0, 3000.0, 0.0], 0.1));
    w.begin_notice_frame();
    for _ in 0..30 {
        shots::step(&mut w, DT);
    }
    assert!(w.notices(guard).contains(&crate::world::Notice::Hit(gun)), "the guard was hit by the gun");
    assert!(w.notices(gun).is_empty(), "a shot never hits its own machine");
    assert_eq!(w.shared.vehicles.hits[0], 1);
    assert!(w.shared.vehicles.shots.is_empty(), "the hit one ended, the other ran out of life");
}

#[test]
fn dance_commands_from_the_sequence_play_a_dance() {
    use crate::behavior::ClassBehavior;
    use crate::object::{Arg, OptionValue};
    let (mut w, h) = one(121);
    let mut d = dance::Director::new(&w, h);
    assert_eq!(d.score.window, dance::WINDOW, "no 1344 / 1345 here: the defaults");
    // The sequence's commands: two beats (Cross at 0.5 s, Square at 1.0 s; dancer −1), a default move, playing.
    assert!(d.action(&mut w, h, 700, &[Arg::Float(0.5), Arg::Int(64), Arg::Int(-1), Arg::Int(-1)]));
    assert!(d.action(&mut w, h, 700, &[Arg::Float(1.0), Arg::Int(128), Arg::Int(-1), Arg::Int(-1)]));
    assert!(d.action(&mut w, h, 701, &[Arg::Int(4), Arg::Int(-1)]));
    assert!(d.action(&mut w, h, 699, &[Arg::Int(2)]));
    assert!(!d.action(&mut w, h, 1344, &[Arg::Int(2)]), "1344 is the early window, not a command");
    assert_eq!((d.state, d.score.beats.len(), d.default_moves.len()), (2, 2, 1));
    assert_eq!(d.score.beats[0].button, button::CROSS);
    // On the beat: a press (task event 53), a score (54), the beat passing (52).
    let r = d.score.step_full(button::CROSS, 0.5, 0.0);
    assert!(r.pressed && r.passed == 1 && (r.scores[0] - 1.0).abs() < 1e-6, "{r:?}");
    let r = d.score.step_full(0, 1.5, 0.0);
    assert_eq!((r.passed, r.scores.clone()), (1, alloc::vec![dance::MISSED]), "the second beat passed unpressed");
    // The jobs read the mean as option 1355 and fail the dance below −0.75 themselves.
    assert_eq!(d.get_option(&w, h, 1355), Some(OptionValue::Float((1.0 + dance::MISSED) / 2.0)));
    assert!(d.action(&mut w, h, 1356, &[]));
    assert_eq!(d.score.mean(), 0.0, "1356 clears the scores");
}

#[test]
fn combat_car_shots_are_solved_for_their_target() {
    // A 45° arc: v² = d·g, clamped to the limits.
    assert!((car::shot_speed(1000.0, 1750.0) - (1000.0f32 * 1750.0).sqrt()).abs() < 1e-1);
    assert_eq!(car::shot_speed(1.0, 1750.0), car::SHOT_SPEEDS[0]);
    assert_eq!(car::shot_speed(1e7, 1750.0), car::SHOT_SPEEDS[1]);
    assert_eq!((shots::BLAST_RADIUS, shots::CONTACT_KIND, shots::DAMAGE), (200.0, 0x100, 10.0));
}

#[test]
fn hack_ship_is_destroyed_at_zero_and_ignores_hits_meanwhile() {
    let (mut w, h) = one(72);
    let mut ship = hack::HackShip::new(&w, h, Title::Sly2);
    assert_eq!((ship.hit_points, ship.state), (10.0, 0));
    assert!(ship.hit(&mut w, 4.0));
    assert_eq!(ship.state, 2, "hit");
    assert!(!ship.hit(&mut w, 4.0), "no second hit while hit");
    ship.state = 1;
    assert!(ship.hit(&mut w, 6.0));
    assert_eq!((ship.hit_points, ship.state), (0.0, 4), "destroyed: lost");
    assert!(!ship.hit(&mut w, 1.0));
}

#[test]
fn the_machine_index_is_cached_and_follows_the_world() {
    let (mut w, h) = one(58);
    assert_eq!(shots::candidates(&mut w), alloc::vec![h]);
    // Cached: the same list until the object count changes or the refresh comes round.
    let built = w.shared.vehicles.index.clone();
    assert_eq!(shots::candidates(&mut w), alloc::vec![h]);
    assert_eq!(w.shared.vehicles.index.handles_for_test(), built.handles_for_test());
    for _ in 0..shots::Index::REFRESH {
        shots::candidates(&mut w);
    }
    assert_eq!(shots::candidates(&mut w), alloc::vec![h], "rebuilt after the refresh");
}
