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
    let mut b = craft::Biplane::new(&w, h);
    for _ in 0..300 {
        b.fly(&pad([0.1, 1.0], button::CROSS, 0), DT);
    }
    assert!((b.speed - 2880.0).abs() < 1.0, "1,800 × 1.6: {}", b.speed);
    assert_eq!(b.bank, 0.0, "stick x under the dead zone");
    assert!((b.pitch - craft::PITCH_LIMIT).abs() < 1e-3);
    let h0 = b.heading;
    for _ in 0..30 {
        b.fly(&pad([1.0, 0.0], 0, 0), DT);
    }
    assert!(b.heading < h0 && (b.bank - craft::BANK).abs() < 1e-6, "stick right turns right");
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
    s.beats.push(dance::Beat { dancer: None, button: button::CROSS, time: 1.0, done: false });
    s.beats.push(dance::Beat { dancer: None, button: button::SQUARE, time: 2.0, done: false });
    s.beats.push(dance::Beat { dancer: None, button: button::CIRCLE, time: 3.0, done: false });
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
