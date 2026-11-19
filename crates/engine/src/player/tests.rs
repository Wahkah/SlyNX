//! Player tests: the spec's vectors (jumps, speeds, rush) and the state transitions, on small synthetic worlds.

use alloc::vec;
use alloc::vec::Vec;

use formats::collision::{CollisionEdge, CollisionMesh, CollisionTree, CollisionTriangle};
use formats::geometry::Affine;

use super::*;
use crate::collision::world::{CollisionWorld, QueryFilter, Shape};
use crate::collision::{Ledge, TriMesh};

/// A floor at z = 0 for x < 100, a wall at x = 100 (facing −x) up to z = 200, and a block top at z = 200 for
/// 100 ≤ x ≤ 300, all for −1000 ≤ y ≤ 1000; plus a far floor around it. The top's edge over the wall is stored
/// (a ledge).
fn block_world() -> CollisionWorld {
    let (y0, y1) = (-1000.0, 1000.0);
    let positions = vec![
        [-2000.0, y0, 0.0],
        [100.0, y0, 0.0],
        [100.0, y1, 0.0],
        [-2000.0, y1, 0.0],
        [100.0, y0, 200.0],
        [100.0, y1, 200.0],
        [300.0, y0, 200.0],
        [300.0, y1, 200.0],
    ];
    let tri = |c: [u16; 3], first_edge: u32, edge_count: u8| CollisionTriangle {
        corners: c,
        radius: 3000.0,
        center: [0.0; 3],
        first_edge,
        edge_count,
    };
    let triangles = vec![
        tri([0, 1, 2], 0, 0),
        tri([0, 2, 3], 0, 0),
        tri([1, 4, 5], 0, 0),
        tri([1, 5, 2], 0, 0),
        tri([4, 6, 7], 0, 1),
        tri([4, 7, 5], 1, 0),
    ];
    let mesh =
        CollisionMesh { radius: 3000.0, positions, triangles, edges: vec![CollisionEdge { ends: [4, 5], other: 2 }] };
    let mut cw = CollisionWorld::new();
    let s = cw.add_shape(Shape { main: TriMesh::new(&mesh, &CollisionTree::default()), camera: None });
    cw.add_body(s, Affine::IDENTITY, 0, 0, true);
    cw.rebuild();
    cw
}

fn empty_world() -> CollisionWorld {
    let mut cw = CollisionWorld::new();
    cw.rebuild();
    cw
}

struct Sim {
    cw: CollisionWorld,
    features: Vec<Feature>,
    ledges: Vec<Ledge>,
    events: Vec<PlayerEvent>,
}

impl Sim {
    fn new(cw: CollisionWorld) -> Sim {
        let ledges = world_ledges(&cw);
        Sim { cw, features: Vec::new(), ledges, events: Vec::new() }
    }

    fn step(&mut self, p: &mut Player, input: &PlayerInput, dt: f32) -> PlayerOutput {
        let world = PlayerWorld {
            cw: &self.cw,
            filter: QueryFilter::default(),
            features: &self.features,
            ledges: &self.ledges,
            in_shadow: false,
            in_cover: false,
        };
        let out = p.update(dt, input, &world);
        self.events.extend_from_slice(&out.events);
        out
    }

    fn run(&mut self, p: &mut Player, input: &PlayerInput, dt: f32, n: usize) {
        for _ in 0..n {
            self.step(p, input, dt);
        }
    }
}

fn idle() -> PlayerInput {
    PlayerInput::default()
}

fn press_jump() -> PlayerInput {
    PlayerInput { jump: Button::PRESSED, ..Default::default() }
}

fn stick(x: f32, y: f32) -> PlayerInput {
    PlayerInput { stick: [x, y], ..Default::default() }
}

/// Jumps from rest on flat ground; returns (take-off speed, apex height, apex time, landing time).
fn ground_jump(consts: PlayerConstants) -> (f32, f32, f32, f32) {
    let mut sim = Sim::new(block_world());
    let mut p = Player::with(consts, Tunables::default(), [0.0, 0.0, 0.0], 0.0);
    let dt = 1.0 / 600.0;
    sim.step(&mut p, &idle(), dt);
    assert_eq!(p.state, State::Ground);
    sim.step(&mut p, &press_jump(), dt);
    assert!(matches!(p.state, State::Air { .. }));
    assert!(sim.events.contains(&PlayerEvent::Jumped(JumpKind::Ground)));
    let v0 = p.velocity[2];
    let (mut top, mut t_top, mut t) = (p.position[2], 0.0, 0.0);
    for _ in 0..2000 {
        sim.step(&mut p, &idle(), dt);
        t += dt;
        if p.position[2] > top {
            top = p.position[2];
            t_top = t;
        }
        if p.state == State::Ground {
            return (v0, top, t_top, t);
        }
    }
    panic!("never landed");
}

#[test]
fn ground_jump_vector() {
    // player-moves.md 9: standing, the origin 90 above the contact: 839.8 units/s, apex 155 after 0.3691 s, landing
    // after 0.7383 s (the fall never reaches the drag speed).
    let (v0, top, t_top, landed) = ground_jump(PlayerConstants::SPEC);
    assert!((v0 - 839.8).abs() < 0.05, "{v0}");
    assert!((top - 155.0).abs() < 0.1, "apex {top}");
    assert!((t_top - 0.3691).abs() < 0.003, "apex time {t_top}");
    assert!((landed - 0.7383).abs() < 0.004, "landed {landed}");
    // The same rule with the origin at the contact (h = 0): 1055.8, apex 245 after 0.4641 s, landing after 0.9293 s
    // (0.9282 s without the drag).
    let h0 = PlayerConstants { origin_height: 0.0, ..PlayerConstants::SPEC };
    let (v0, top, t_top, landed) = ground_jump(h0);
    assert!((v0 - 1055.8).abs() < 0.05);
    assert!((top - 245.0).abs() < 0.1 && (t_top - 0.4641).abs() < 0.003);
    assert!((landed - 0.9293).abs() < 0.004, "landed {landed}");
    let no_drag = PlayerConstants { terminal: crate::physics::TerminalFall { speed: -1e9, rate: 5.0 }, ..h0 };
    let (_, _, _, landed) = ground_jump(no_drag);
    assert!((landed - 0.9282).abs() < 0.004, "landed {landed}");
    // At 60 frames/s with h = 0: 0.9291 s.
    let mut sim = Sim::new(block_world());
    let mut p = Player::with(h0, Tunables::default(), [0.0, 0.0, 0.0], 0.0);
    let dt = 1.0 / 60.0;
    sim.step(&mut p, &idle(), dt);
    sim.step(&mut p, &press_jump(), dt);
    // The press frame only starts the jump; the flight starts with the next frame.
    let mut t = 0.0;
    let mut vz = 0.0;
    while p.state != State::Ground && t < 2.0 {
        vz = p.velocity[2];
        sim.step(&mut p, &idle(), dt);
        t += dt;
    }
    assert!((t - 0.9291).abs() < dt + 1e-3, "landed {t}");
    assert!((vz + 1019.1).abs() < 40.0, "landing speed {vz}");
}

#[test]
fn turn_smoothing_vector() {
    // player-moves.md 2: standing, a 90° turn from rest: 3.33 rad/s after the first 1/60 s; about 0.18 s in all.
    let mut sim = Sim::new(block_world());
    let mut p = Player::new([-1500.0, 0.0, 0.0], 0.0);
    sim.step(&mut p, &idle(), 1.0 / 60.0);
    // A tiny deflection: the turn, not the run, is under test (the stick factor applies only when running).
    let left = PlayerInput { stick: [-1.0, 0.0], ..Default::default() };
    p.velocity = [0.0; 3];
    sim.step(&mut p, &left, 1.0 / 60.0);
    assert!((p.turn_speed - 200.0 / 60.0).abs() < 0.01, "{}", p.turn_speed);
    let mut t: f32 = 1.0 / 60.0;
    while (p.yaw - core::f32::consts::FRAC_PI_2).abs() > 0.01 && t < 1.0 {
        sim.step(&mut p, &left, 1.0 / 60.0);
        t += 1.0 / 60.0;
    }
    assert!((t - 0.18).abs() < 0.05, "{t}");
}

#[test]
fn swing_jump_vector() {
    // player-moves.md 9: from a swing with vz = 0, apex 140.0 units after 0.3508 s.
    let mut sim = Sim::new(empty_world());
    let mut p = Player::new([0.0; 3], 0.0);
    let mut out = PlayerOutput::default();
    p.start_jump(JumpKind::Swing, &mut out);
    assert!((p.velocity[2] - 798.1).abs() < 0.05);
    let dt = 1.0 / 1200.0;
    let (mut top, mut t_top, mut t) = (0.0f32, 0.0f32, 0.0f32);
    for _ in 0..1200 {
        sim.step(&mut p, &idle(), dt);
        t += dt;
        if p.position[2] > top {
            top = p.position[2];
            t_top = t;
        }
    }
    assert!((top - 140.0).abs() < 0.1 && (t_top - 0.3508).abs() < 0.002, "{top} at {t_top}");
}

#[test]
fn run_speeds() {
    let p = Player::new([0.0; 3], 0.0);
    assert!((p.run_speed(0.0) - 38.3).abs() < 1e-3);
    assert!((p.run_speed(1.0) - 638.4).abs() < 1e-3);
    // player-moves.md 2: the left foot's plant-to-lift intervals weight the blend.
    for (u, v) in [(0.25, 48.8), (0.5, 68.7), (0.75, 121.2), (0.9, 233.2)] {
        assert!((p.run_speed(u) - v).abs() < 0.1, "u {u}: {}", p.run_speed(u));
    }
    // Full deflection on flat ground: 638.4 at steady state.
    let mut sim = Sim::new(block_world());
    let up = PlayerInput { stick: [0.0, 1.0], camera_yaw: core::f32::consts::FRAC_PI_2, ..Default::default() };
    let mut p = Player::new([-1500.0, -900.0, 0.0], core::f32::consts::FRAC_PI_2);
    sim.run(&mut p, &up, 1.0 / 60.0, 120);
    let v = crate::math::length([p.velocity[0], p.velocity[1], 0.0]);
    assert!((v - 638.4).abs() < 0.5, "{v}");
    assert_eq!(p.state, State::Ground);
    // Camera-relative: stick up with the camera looking along +y moves along +y.
    assert!(p.velocity[1] > 600.0 && p.velocity[0].abs() < 1.0);
    // Letting go decelerates to a stop.
    sim.run(&mut p, &idle(), 1.0 / 60.0, 60);
    assert!(crate::math::length(p.velocity) < 1e-3);
    // Dead zone.
    let (_, u) = p.stick(&stick(0.1, 0.0), 1.0);
    assert_eq!(u, 0.0);
}

#[test]
fn rush_vector() {
    // player-moves.md 9: rush from standstill: 550 units forward after 1.0 s.
    let mut sim = Sim::new(block_world());
    let mut p = Player::new([-1500.0, 0.0, 0.0], 0.0);
    sim.step(&mut p, &idle(), 1.0 / 60.0);
    let x0 = p.position[0];
    sim.step(&mut p, &PlayerInput { rush: Button::PRESSED, ..Default::default() }, 1.0 / 60.0);
    // The press frame decides the move; the rush then moves for 60 frames and ends.
    sim.run(&mut p, &idle(), 1.0 / 60.0, 60);
    assert!((p.position[0] - x0 - 550.0).abs() < 1.0, "{}", p.position[0] - x0);
    assert_eq!(p.state, State::Ground);
}

#[test]
fn boosted_second_press_and_smash() {
    let mut sim = Sim::new(block_world());
    let mut p = Player::new([-1500.0, 0.0, 0.0], 0.0);
    let dt = 1.0 / 60.0;
    sim.step(&mut p, &idle(), dt);
    sim.step(&mut p, &press_jump(), dt);
    // No minimum delay: the boost works at once, and never lowers an upward speed.
    let vz = p.velocity[2];
    sim.step(&mut p, &press_jump(), dt);
    assert!(p.second_used);
    assert!(sim.events.contains(&PlayerEvent::Jumped(JumpKind::Boosted)));
    assert!(p.velocity[2] > vz - 2275.0 * dt * 1.01);
    // Once per jump.
    let n = sim.events.iter().filter(|e| **e == PlayerEvent::Jumped(JumpKind::Boosted)).count();
    sim.run(&mut p, &idle(), dt, 5);
    sim.step(&mut p, &press_jump(), dt);
    assert_eq!(sim.events.iter().filter(|e| **e == PlayerEvent::Jumped(JumpKind::Boosted)).count(), n);
    // player-moves.md 9: falling at −500 the boost gives 182; rising at 300 it gives 682.
    let c = PlayerConstants::SPEC;
    assert_eq!((-500.0f32).max(c.boosted_jump.eval(-500.0)), 182.0);
    assert_eq!(300.0f32.max(c.boosted_jump.eval(300.0)), 682.0);
    // Smash after 0.35 s in the air: no speed of its own; it reports its landing.
    sim.run(&mut p, &idle(), dt, 15);
    let before = p.velocity[2];
    sim.step(&mut p, &PlayerInput { attack: Button::PRESSED, ..Default::default() }, dt);
    assert_eq!(p.state, State::Smash);
    assert!(p.velocity[2] > before - 2275.0 * dt * 1.01 - 1e-3);
    sim.run(&mut p, &idle(), dt, 120);
    assert_eq!(p.state, State::Ground);
    assert!(sim.events.iter().any(|e| matches!(e, PlayerEvent::SmashLanded { window, .. } if *window == 0.25)));
    // Just before landing the second press does nothing.
    let mut p = Player::new([-1500.0, 0.0, 30.0], 0.0);
    p.state = State::Air { target: None };
    p.velocity = [0.0, 0.0, -400.0];
    p.jump_time = Some(0.5);
    sim.step(&mut p, &press_jump(), dt);
    assert!(!p.second_used);
}

#[test]
fn air_steering_coast_and_short() {
    // A running jump with the stick held at full deflection keeps its momentum after the take-off (coast); letting
    // go of the stick within 0.07 s makes the jump short-steered and it slows toward the empty wish.
    let dt = 1.0 / 60.0;
    let run = |release: bool| {
        let mut sim = Sim::new(empty_world());
        let mut p = Player::new([0.0, 0.0, 1000.0], 0.0);
        let mut out = PlayerOutput::default();
        p.velocity = [638.4, 0.0, 0.0];
        p.start_jump(JumpKind::Ground, &mut out);
        let full = PlayerInput { stick: [0.0, 1.0], jump: Button { held: true, pressed: false }, ..Default::default() };
        for i in 0..36 {
            let i_in = if release && i > 1 {
                idle()
            } else if i < 20 {
                full
            } else {
                idle()
            };
            sim.step(&mut p, &i_in, dt);
        }
        (p.steer_mode, p.velocity[0])
    };
    let (mode, vx) = run(false);
    assert_eq!(mode, SteerMode::Coast);
    assert!(vx > 500.0, "momentum kept: {vx}");
    let (mode, vx) = run(true);
    assert_eq!(mode, SteerMode::Short);
    assert!(vx < 300.0, "short-steered: {vx}");
}

#[test]
fn ledge_grab_hang_and_ledge_jump() {
    let mut sim = Sim::new(block_world());
    assert_eq!(sim.ledges.len(), 1);
    // Falling with the hands just above the edge (hands at z + 160; the edge at x = 100, z = 200).
    let mut p = Player::new([60.0, 0.0, 60.0], 0.0);
    p.velocity = [0.0, 0.0, -100.0];
    let dt = 1.0 / 60.0;
    for _ in 0..30 {
        sim.step(&mut p, &idle(), dt);
        if matches!(p.state, State::LedgeHang { .. }) {
            break;
        }
    }
    assert!(matches!(p.state, State::LedgeHang { .. }), "{:?} at {:?}", p.state, p.position);
    assert!(sim.events.contains(&PlayerEvent::LedgeGrabbed));
    // Eased onto the hang position: the origin 37 out from the edge and 25 below it.
    sim.run(&mut p, &idle(), dt, 60);
    // The origin 25 below the edge: the feet 115 below it.
    assert!((p.position[0] - 63.0).abs() < 1e-2 && (p.position[2] - 85.0).abs() < 1e-2, "{:?}", p.position);
    // No shimmy: the stick does not move the player along the edge.
    let y = p.position[1];
    sim.run(&mut p, &stick(1.0, 0.0), dt, 30);
    assert!((p.position[1] - y).abs() < 1e-3);
    // The only way off is the ledge jump: 912.2 up.
    sim.step(&mut p, &press_jump(), dt);
    assert!(sim.events.contains(&PlayerEvent::Jumped(JumpKind::Edge)));
    assert!(matches!(p.state, State::Air { .. }));
    assert_eq!(p.steer_mode, SteerMode::Short);
}

#[test]
fn pipes_rails_perches_and_hooks() {
    let dt = 1.0 / 60.0;
    let mut sim = Sim::new(block_world());
    sim.features = vec![
        Feature { id: 1, kind: FeatureKind::Pipe { bottom: [-500.0, 0.0, 0.0], top: [-500.0, 0.0, 800.0] } },
        Feature { id: 2, kind: FeatureKind::Rail { points: vec![[-1500.0, 300.0, 400.0], [-500.0, 300.0, 400.0]] } },
        Feature { id: 3, kind: FeatureKind::Perch { position: [-1400.0, -600.0, 0.0] } },
        Feature { id: 4, kind: FeatureKind::Hook { position: [-1000.0, -300.0, 600.0] } },
    ];
    let action = PlayerInput { action: Button::PRESSED, ..Default::default() };

    // Pipe: grab from the ground, climb, stop at the hands limit, drop.
    let mut p = Player::new([-470.0, 0.0, 0.0], 0.0);
    sim.step(&mut p, &idle(), dt);
    sim.step(&mut p, &action, dt);
    assert!(matches!(p.state, State::Pipe { id: 1, .. }), "{:?}", p.state);
    sim.run(&mut p, &stick(0.0, 1.0), dt, 600);
    // Hands stop 35 from the top: feet at 800 − 35 − 160.
    assert!((p.position[2] - 605.0).abs() < 1e-2, "{}", p.position[2]);
    // A weak opposite stick does not reverse; a strong one does.
    sim.run(&mut p, &stick(0.0, -0.3), dt, 10);
    assert!((p.position[2] - 605.0).abs() < 1e-2);
    sim.run(&mut p, &stick(0.0, -1.0), dt, 10);
    assert!(p.position[2] < 605.0);
    sim.step(&mut p, &PlayerInput { duck: Button::PRESSED, ..Default::default() }, dt);
    assert!(matches!(p.state, State::Air { .. }));

    // Rail: catch it from the air with the context button, walk, reverse only below −0.2.
    let mut p = Player::new([-1000.0, 300.0, 420.0], 0.0);
    sim.step(&mut p, &action, dt);
    assert!(matches!(p.state, State::Rail { id: 2, .. }), "{:?}", p.state);
    sim.run(&mut p, &stick(0.0, 1.0), dt, 30);
    assert!((p.position[0] + 850.0).abs() < 1.0, "{:?}", p.position);
    sim.run(&mut p, &stick(0.0, -1.0), dt, 30);
    assert!((p.position[0] + 1000.0).abs() < 1.0, "{:?}", p.position);

    // Perch: a jump with the stick toward a perch is aimed at it and lands on it.
    let mut p = Player::new([-1400.0, -200.0, 0.0], -core::f32::consts::FRAC_PI_2);
    sim.step(&mut p, &idle(), dt);
    let toward = PlayerInput { jump: Button::PRESSED, stick: [1.0, 0.0], camera_yaw: 0.0, ..Default::default() };
    sim.step(&mut p, &toward, dt);
    assert!(matches!(p.state, State::Air { target: Some(3) }), "{:?}", p.state);
    sim.run(&mut p, &idle(), dt, 90);
    assert_eq!(p.state, State::Perch { id: 3 });
    assert!(sim.events.contains(&PlayerEvent::Attached(3)));

    // Hook: catch it in the air, swing, jump off with the swing jump.
    let mut p = Player::new([-1000.0, -300.0, 450.0], 0.0);
    sim.step(&mut p, &action, dt);
    assert!(matches!(p.state, State::Swing { id: 4, .. }), "{:?}", p.state);
    sim.run(&mut p, &stick(0.0, 1.0), dt, 30);
    assert!((crate::math::length(crate::math::sub(p.position, [-1000.0, -300.0, 600.0])) - 150.0).abs() < 1e-2);
    sim.step(&mut p, &press_jump(), dt);
    assert!(sim.events.contains(&PlayerEvent::Jumped(JumpKind::Swing)));
}

#[test]
fn flatten_duck_hide_and_hidden_rules() {
    let dt = 1.0 / 60.0;
    let mut sim = Sim::new(block_world());
    sim.features = vec![Feature { id: 9, kind: FeatureKind::HidingPlace { position: [-800.0, 0.0, 0.0] } }];
    let action = PlayerInput { action: Button::PRESSED, ..Default::default() };
    // Flatten against the wall ahead.
    let mut p = Player::new([40.0, 0.0, 0.0], 0.0);
    sim.step(&mut p, &idle(), dt);
    sim.step(&mut p, &action, dt);
    assert!(matches!(p.state, State::Flatten { .. }), "{:?}", p.state);
    assert!((p.position[0] - 69.0).abs() < 1e-2);
    // Facing away from the wall: hidden from a guard on the wall side only.
    let h = p.hiding(false);
    assert!(h.hidden_from(p.position, [200.0, 0.0, 0.0], 0));
    assert!(!h.hidden_from(p.position, [-200.0, 0.0, 0.0], 0));
    // Sidestep along the wall, capped like sneaking: 135 units/s at full deflection.
    sim.run(&mut p, &stick(1.0, 0.0), dt, 60);
    assert!((crate::math::length(p.velocity) - 135.0).abs() < 0.5, "{:?}", p.velocity);
    sim.step(&mut p, &action, dt);
    assert_eq!(p.state, State::Ground);
    // Duck while held.
    sim.step(&mut p, &PlayerInput { duck: Button::PRESSED, ..Default::default() }, dt);
    assert_eq!(p.state, State::Duck);
    assert!(matches!(p.hiding(true), Hiding::Ducking { .. }));
    sim.step(&mut p, &idle(), dt);
    assert_eq!(p.state, State::Ground);
    // A hiding place.
    let mut p = Player::new([-750.0, 0.0, 0.0], 0.0);
    sim.step(&mut p, &idle(), dt);
    sim.step(&mut p, &action, dt);
    assert_eq!(p.state, State::Hiding { id: 9 });
    assert_eq!(p.hiding(false), Hiding::InHidingPlace { still: true });
    sim.run(&mut p, &stick(0.0, 1.0), dt, 30);
    assert!((crate::math::length(p.velocity) - 416.7).abs() < 0.1);
    sim.step(&mut p, &action, dt);
    assert!(sim.events.contains(&PlayerEvent::HidingLeft(9)));
}

#[test]
fn damage_death_respawn_and_water() {
    let dt = 1.0 / 60.0;
    let mut sim = Sim::new(block_world());
    let mut p = Player::new([-1500.0, 0.0, 0.0], 0.0);
    p.vitals = Vitals { charms: 1, lives: 2 };
    sim.step(&mut p, &idle(), dt);
    let mut out = PlayerOutput::default();
    assert_eq!(p.damage(DamageKind::Fire, &mut out), DamageOutcome::CharmLost);
    // Flashing: the next hit is ignored.
    assert_eq!(p.damage(DamageKind::Fire, &mut out), DamageOutcome::Ignored);
    sim.run(&mut p, &idle(), dt, 130);
    assert_eq!(p.state, State::Ground);
    assert_eq!(p.damage(DamageKind::Blunt, &mut out), DamageOutcome::LifeLost);
    assert!(matches!(p.state, State::Dead { .. }));
    sim.run(&mut p, &idle(), dt, 200);
    assert_eq!(sim.events.iter().filter(|e| **e == PlayerEvent::RespawnRequested).count(), 1);
    p.respawn([-1500.0, 0.0, 50.0], 0.0);
    sim.run(&mut p, &idle(), dt, 60);
    assert_eq!(p.state, State::Ground);
    // Falling into water uses a charm (reaction 229) and puts the player back.
    p.vitals.charms = 2;
    sim.features = vec![Feature {
        id: 1,
        kind: FeatureKind::Water { surface: 10.0, min: [-2000.0, -1000.0], max: [0.0, 1000.0] },
    }];
    sim.step(&mut p, &idle(), dt);
    assert!(matches!(p.state, State::Hurt { kind: DamageKind::Water, .. }));
    assert_eq!(p.anim_request().first, 229);
    assert_eq!((p.vitals.charms, p.vitals.lives), (1, 1));
    sim.run(&mut p, &idle(), dt, 60);
    assert_eq!(sim.events.iter().filter(|e| **e == PlayerEvent::RespawnRequested).count(), 2);
    // Without a charm: the sinking death (235).
    p.respawn([-1500.0, 0.0, 0.0], 0.0);
    p.vitals.charms = 0;
    p.flash = 0.0;
    sim.step(&mut p, &idle(), dt);
    assert!(matches!(p.state, State::Dead { kind: DamageKind::Water, .. }));
    assert_eq!(p.anim_request().first, 235);
    // Moving backward faster than 250 when dying: the backward death (234), turned away from the motion.
    sim.features.clear();
    let mut p = Player::new([-1500.0, 0.0, 0.0], 0.0);
    p.vitals = Vitals { charms: 0, lives: 3 };
    p.state = State::Ground;
    p.velocity = [-400.0, -300.0, 0.0];
    p.damage(DamageKind::Blunt, &mut out);
    assert_eq!(p.anim_request().first, 234);
    // At 500 units/s the turn is complete: facing straight away from the motion.
    assert!((p.yaw - crate::math::atan2(300.0, 400.0)).abs() < 1e-4, "{}", p.yaw);
}

#[test]
fn power_ups_need_their_bits() {
    let dt = 1.0 / 60.0;
    let mut sim = Sim::new(block_world());
    let mut p = Player::new([-1500.0, 0.0, 0.0], 0.0);
    p.selected_power = Some(PowerUp::SlowMotion);
    let power = PlayerInput { power: Button::PRESSED, ..Default::default() };
    let out = sim.step(&mut p, &power, dt);
    assert_eq!(out.time_scale, 1.0);
    p.reward_mask = 0x1008;
    let out = sim.step(&mut p, &power, dt);
    assert_eq!(out.time_scale, 0.25);
    assert!(out.events.contains(&PlayerEvent::PowerUsed(PowerUp::SlowMotion)));
    p.reward_mask = 0xFFFF;
    p.selected_power = Some(PowerUp::Decoy);
    let out = sim.step(&mut p, &power, dt);
    assert!(out.events.iter().any(|e| matches!(e, PlayerEvent::SpawnPowerObject(PowerUp::Decoy, _))));
    // Dive: only in the air after 0.35 s; a 0.333 s hover, then 1556.3 units/s down; the landing is reported.
    let mut p2 = Player::new([-1500.0, 0.0, 4000.0], 0.0);
    p2.reward_mask = 0xFFFF;
    p2.selected_power = Some(PowerUp::Dive);
    p2.state = State::Air { target: None };
    p2.velocity = [300.0, 0.0, 200.0];
    sim.run(&mut p2, &idle(), dt, 25);
    sim.step(&mut p2, &power, dt);
    assert_eq!(p2.state, State::Dive);
    let (z0, vz0) = (p2.position[2], p2.velocity[2]);
    sim.run(&mut p2, &idle(), dt, 19);
    // Gravity off and the velocity pulled to zero at 10 per second: at most |v|/10 of drift.
    assert!((p2.position[2] - z0).abs() <= vz0.abs() / 10.0 + 1.0, "hovering: {}", p2.position[2] - z0);
    assert!(p2.velocity[2].abs() < vz0.abs() * 0.2);
    sim.run(&mut p2, &idle(), dt, 10);
    assert!((p2.velocity[2] + 1556.25).abs() < 0.1, "{:?}", p2.velocity);
    assert!(p2.velocity[0].abs() < 300.0 * 0.1);
    sim.run(&mut p2, &idle(), dt, 200);
    assert!(sim.events.iter().any(|e| matches!(e, PlayerEvent::DiveLanded { affect_radius, coin_radius, .. } if *affect_radius == 300.0 && *coin_radius == 500.0)));
    // Rolling ball from standing: 850 units/s along the facing; held with the button, ended after 0.25 s.
    let mut p = Player::new([-1500.0, 0.0, 0.0], 0.0);
    p.reward_mask = 0xFFFF;
    p.selected_power = Some(PowerUp::RollingBall);
    sim.step(&mut p, &idle(), dt);
    let hold = PlayerInput { power: Button { held: true, pressed: true }, ..Default::default() };
    sim.step(&mut p, &hold, dt);
    assert_eq!(p.state, State::Ball);
    assert!((p.velocity[0] - 850.0).abs() < 1e-3, "{:?}", p.velocity);
    let held = PlayerInput { power: Button { held: true, pressed: false }, ..Default::default() };
    sim.run(&mut p, &held, dt, 20);
    assert_eq!(p.state, State::Ball);
    sim.step(&mut p, &idle(), dt);
    assert_ne!(p.state, State::Ball);
}

#[test]
fn animations_follow_the_state() {
    let mut p = Player::new([0.0; 3], 0.0);
    p.state = State::Ground;
    p.velocity = [638.4, 0.0, 0.0];
    assert_eq!(p.anim_request(), AnimRequest::blend(130, 131, 1.0));
    p.state = State::Rush { t: 0.0 };
    assert_eq!(p.anim_request().first, 261);
    p.state = State::Dead { kind: DamageKind::Pit, t: 1.0, requested: false };
    assert_eq!(p.anim_request().first, 237);
}
