//! Gang tests on a flat synthetic floor: the spec's vectors (speeds, jump, flop, combo, crossbow, throw, drains)
//! and the hand-offs between the characters.

use alloc::vec;
use alloc::vec::Vec;

use formats::collision::{CollisionMesh, CollisionTree, CollisionTriangle};
use formats::geometry::Affine;

use super::consts::spec;
use super::gadgets::Refusal;
use super::*;
use crate::collision::TriMesh;
use crate::collision::world::{CollisionWorld, QueryFilter, Shape};
use crate::math::cos;

/// A floor at z = 0 for |x|, |y| ≤ 5000 and a wall at x = 3000 facing −x up to z = 2000.
fn floor_world() -> CollisionWorld {
    let e = 5000.0;
    let positions = vec![
        [-e, -e, 0.0],
        [e, -e, 0.0],
        [e, e, 0.0],
        [-e, e, 0.0],
        [3000.0, -e, 0.0],
        [3000.0, e, 0.0],
        [3000.0, -e, 2000.0],
        [3000.0, e, 2000.0],
    ];
    let tri =
        |c: [u16; 3]| CollisionTriangle { corners: c, radius: 8000.0, center: [0.0; 3], first_edge: 0, edge_count: 0 };
    let triangles = vec![tri([0, 1, 2]), tri([0, 2, 3]), tri([4, 6, 7]), tri([4, 7, 5])];
    let mesh = CollisionMesh { radius: 8000.0, positions, triangles, edges: Vec::new() };
    let mut cw = CollisionWorld::new();
    let s = cw.add_shape(Shape { main: TriMesh::new(&mesh, &CollisionTree::default()), camera: None });
    cw.add_body(s, Affine::IDENTITY, 0, 0, true);
    cw.rebuild();
    cw
}

fn table() -> ItemTable {
    let mut raw = vec![0u8; items::ITEM_COUNT * 32];
    let mut put = |n: usize, rec: [i32; 8]| {
        for (i, v) in rec.iter().enumerate() {
            raw[n * 32 + i * 4..n * 32 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
    };
    put(7, [2, 8, 200, 0, 1, 129, 130, 209]);
    put(12, [2, 8, 1200, 1, 6, 139, 140, 208]);
    put(22, [2, 9, 1000, 3, 4, 155, 156, 208]);
    put(29, [1, 7, 1600, 2, 7, 175, 176, 208]);
    put(36, [-1, 7, 0, 0, -1, 189, 190, 206]);
    put(42, [1, 7, 0, 1, -1, 201, 202, 208]);
    put(18, [-1, 9, 0, 0, -1, 0, 0, 206]);
    put(23, [2, 7, 0, 3, 1, 0, 0, 205]);
    put(26, [2, 7, 0, 1, 1, 0, 0, 216]);
    put(27, [-1, 7, 600, 0, 5, 171, 172, 211]);
    put(34, [2, 7, 0, 2, 1, 0, 0, 205]);
    ItemTable::from_bytes(&raw)
}

fn shared() -> Shared {
    Shared { consts: GangConstants::SPEC, tun: GangTunables::default(), items: table(), owned: Owned::default() }
}

const DT: f32 = 1.0 / 60.0;

struct Sim {
    cw: CollisionWorld,
    targets: Vec<Target>,
    s: Shared,
    events: Vec<GangEvent>,
    pools: Vec<(u8, u32)>,
}

impl Sim {
    fn new() -> Sim {
        Sim { cw: floor_world(), targets: Vec::new(), s: shared(), events: Vec::new(), pools: Vec::new() }
    }

    fn world(&self) -> GangWorld<'_> {
        GangWorld {
            cw: &self.cw,
            filter: QueryFilter::default(),
            targets: &self.targets,
            help: 0.0,
            safehouse: false,
            pools: &[],
        }
    }

    fn step(&mut self, c: &mut Character, input: &GangInput) {
        let mut out = Vec::new();
        let world = GangWorld {
            cw: &self.cw,
            filter: QueryFilter::default(),
            targets: &self.targets,
            help: 0.0,
            safehouse: false,
            pools: &self.pools,
        };
        c.update(DT, input, &world, &self.s, &mut out);
        self.events.extend(out);
    }

    fn run(&mut self, c: &mut Character, input: &GangInput, frames: usize) {
        for _ in 0..frames {
            self.step(c, input);
        }
    }

    fn press(&mut self, c: &mut Character, b: u32) {
        self.step(c, &GangInput { pressed: b, held: b, ..GangInput::default() });
        self.step(c, &GangInput::default());
    }

    fn took(&mut self) -> Vec<GangEvent> {
        core::mem::take(&mut self.events)
    }
}

fn idle() -> GangInput {
    GangInput::default()
}

fn forward(held: u32) -> GangInput {
    GangInput { stick: [0.0, 1.0], held, ..GangInput::default() }
}

#[test]
fn sly_speeds_and_jump_height() {
    let mut sim = Sim::new();
    let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
    sim.run(&mut c, &forward(button::R1), 60);
    assert_eq!(c.state, sly::RUN);
    // characters.md 1.1: the run gait 640 × 1.3 = 832; the move gait at full stick 510.
    assert!((c.velocity[0] - 832.0).abs() < 1.0, "{:?}", c.velocity);
    sim.run(&mut c, &forward(0), 30);
    assert_eq!(c.state, sly::SNEAK_RUN);
    assert!((c.velocity[0] - 510.0).abs() < 1.0);
    // A hand-made input's stick length is the deflection (the pad's goes through `Gang::stick`).
    let half = GangInput { stick: [0.0, 0.5], ..idle() };
    sim.run(&mut c, &half, 10);
    assert!((c.velocity[0] - 320.0).abs() < 1.0, "half stick: {:?}", c.velocity);
    sim.run(&mut c, &idle(), 5);
    assert_eq!(c.state, sly::STAND);
    // Jump kind 1: the base point to 215 above the floor (989.1 units/s).
    sim.took();
    sim.press(&mut c, button::CROSS);
    assert_eq!(c.state, sly::AIR);
    let mut peak: f32 = 0.0;
    for _ in 0..120 {
        sim.step(&mut c, &idle());
        peak = peak.max(c.position[2]);
    }
    assert!((peak - 215.0).abs() < 6.0, "peak {peak}");
    assert_eq!(c.state, sly::STAND);
    let ev = sim.took();
    assert!(ev.contains(&GangEvent::Jumped) && ev.contains(&GangEvent::Landed));
}

#[test]
fn sly_air_attacks_pickpocket_and_fight() {
    let mut sim = Sim::new();
    let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
    sim.press(&mut c, button::CROSS);
    sim.press(&mut c, button::SQUARE);
    assert_eq!(c.state, sly::AIR_SQUARE);
    assert!(sim.took().contains(&GangEvent::AirAttack(sly::AIR_SQUARE)));
    sim.run(&mut c, &idle(), 120);
    assert_eq!(c.state, sly::STAND);
    // A robbable guard facing away, just ahead: running past (R1) does not stop Sly; Circle pickpockets.
    sim.targets.push(Target {
        id: 5,
        position: [100.0, 0.0, 0.0],
        yaw: 0.0,
        kind: TargetKind::Guard { robbable: true, launched: false },
    });
    sim.step(&mut c, &GangInput { held: button::R1, ..idle() });
    sim.step(&mut c, &GangInput { pressed: button::CIRCLE, held: button::CIRCLE | button::R1, ..idle() });
    assert_eq!(c.state, sly::PICKPOCKET);
    assert!(sim.took().contains(&GangEvent::Pickpocket(5)));
    // The pickpocket clip (280) lasts 1 s.
    sim.run(&mut c, &GangInput { held: button::R1, ..idle() }, 55);
    assert_eq!(c.state, sly::PICKPOCKET);
    sim.run(&mut c, &GangInput { held: button::R1, ..idle() }, 10);
    assert_eq!(c.state, sly::STAND);
    // Without R1 a guard nearby puts Sly in the fight stance.
    sim.run(&mut c, &idle(), 2);
    assert_eq!(c.state, sly::FIGHT);
    // Facing the guard's front, Circle does not pickpocket.
    sim.targets[0].yaw = core::f32::consts::PI;
    sim.took();
    sim.press(&mut c, button::CIRCLE);
    assert!(!sim.took().iter().any(|e| matches!(e, GangEvent::Pickpocket(_))));
}

#[test]
fn murray_combo_is_five_punches() {
    let mut sim = Sim::new();
    sim.targets.push(Target {
        id: 9,
        position: [120.0, 40.0, 0.0],
        yaw: 0.0,
        kind: TargetKind::Object { liftable: false, attackable: true },
    });
    let mut c = Character::new(Who::Murray, [0.0; 3], 0.0);
    sim.step(&mut c, &idle());
    assert_eq!(c.state, murray::STANCE);
    let mut punches = Vec::new();
    for _ in 0..80 {
        let i = GangInput { pressed: button::SQUARE, held: button::SQUARE, ..idle() };
        sim.step(&mut c, &i);
        for _ in 0..5 {
            sim.step(&mut c, &idle());
        }
        for e in sim.took() {
            if let GangEvent::Punch { index, target } = e {
                punches.push((index, target));
            }
        }
        if c.state != murray::PUNCH && punches.len() >= 5 {
            break;
        }
    }
    // The target is on Murray's left: the first punch is clip 0, then they alternate; a combo has five.
    assert_eq!(punches[..5].iter().map(|p| p.0).collect::<Vec<_>>(), vec![0, 1, 0, 1, 0]);
    assert!(punches.iter().all(|p| p.1 == Some(9)));
    // Triangle starts the second pair on the other side.
    sim.targets[0].position[1] = -40.0;
    sim.run(&mut c, &idle(), 90);
    sim.took();
    sim.press(&mut c, button::TRIANGLE);
    assert!(sim.took().contains(&GangEvent::Punch { index: 3, target: Some(9) }));
}

#[test]
fn murray_thunder_flop() {
    let mut sim = Sim::new();
    sim.s.owned.add(items::id::INFERNO_FLOP);
    let mut c = Character::new(Who::Murray, [0.0, 0.0, 1000.0], 0.0);
    c.grounded = false;
    c.state = murray::AIR;
    c.jump_kind = 1;
    sim.step(&mut c, &idle());
    let z0 = c.position[2];
    sim.press(&mut c, button::SQUARE);
    assert_eq!(c.state, murray::FLOP);
    // Wind-up: no motion.
    sim.run(&mut c, &idle(), 10);
    assert!((c.position[2] - z0).abs() < 1e-3, "{} {}", c.position[2], z0);
    sim.run(&mut c, &idle(), 4);
    // Then −500 straight down, steering toward 0.75 × 515 × stick.
    sim.run(&mut c, &forward(0), 60);
    assert_eq!(c.velocity[2], -500.0);
    let target = 0.75 * 515.0;
    assert!(c.velocity[0] > 0.9 * target && c.velocity[0] <= target + 1e-3, "{:?}", c.velocity);
    sim.run(&mut c, &idle(), 200);
    assert_eq!(c.state, murray::STAND);
    assert!(sim.took().iter().any(|e| matches!(e, GangEvent::FlopLanded { inferno: true, .. })));
    // Too close to the ground: no flop, a normal landing.
    let mut c = Character::new(Who::Murray, [0.0, 0.0, 10.0], 0.0);
    c.grounded = false;
    c.state = murray::AIR;
    sim.press(&mut c, button::SQUARE);
    assert_ne!(c.state, murray::FLOP);
}

#[test]
fn murray_lift_carry_throw_and_long_toss() {
    let mut sim = Sim::new();
    sim.targets.push(Target {
        id: 3,
        position: [100.0, 0.0, 0.0],
        yaw: 0.0,
        kind: TargetKind::Object { liftable: true, attackable: true },
    });
    let mut c = Character::new(Who::Murray, [0.0; 3], 0.0);
    sim.press(&mut c, button::CIRCLE);
    assert_eq!(c.state, murray::LIFT);
    assert!(sim.took().contains(&GangEvent::Lift(3)));
    // Picking up: clip 452, 1 s.
    sim.run(&mut c, &idle(), 61);
    assert_eq!(c.state, murray::CARRY);
    sim.press(&mut c, button::CIRCLE);
    assert_eq!(c.state, murray::THROW_AIM);
    let a = c.aim.unwrap();
    assert_eq!(a.range, [200.0, 1500.0]);
    sim.press(&mut c, button::SQUARE);
    let thrown = sim.took().into_iter().find_map(|e| match e {
        GangEvent::Thrown { id, velocity } => Some((id, velocity)),
        _ => None,
    });
    let (id, v) = thrown.expect("thrown");
    assert_eq!(id, 3);
    assert!(v[0] > 0.0 && v[2] > 0.0);
    assert_eq!(c.carrying, None);
    // Long Toss: range × 1.5 and flight time × 1.35.
    sim.s.owned.add(items::id::LONG_TOSS);
    c.carrying = Some(3);
    c.set_state(murray::CARRY, &mut Vec::new());
    sim.press(&mut c, button::CIRCLE);
    let a = c.aim.unwrap();
    assert_eq!(a.range, [300.0, 2250.0]);
    assert!((a.flight_time - 1.5 * 1.35).abs() < 1e-5);
    // Circle cancels back to carrying.
    sim.press(&mut c, button::TRIANGLE);
    assert_eq!(c.state, murray::CARRY);
    assert!(sim.took().contains(&GangEvent::ThrowCancelled));
}

#[test]
fn murray_grabs_launched_guard_and_script_hold_drops() {
    let mut sim = Sim::new();
    sim.targets.push(Target {
        id: 4,
        position: [50.0, 0.0, 300.0],
        yaw: 0.0,
        kind: TargetKind::Guard { robbable: false, launched: true },
    });
    let mut c = Character::new(Who::Murray, [0.0; 3], 0.0);
    sim.press(&mut c, button::CIRCLE);
    assert_eq!(c.state, murray::CARRY);
    assert!(sim.took().contains(&GangEvent::GrabbedLaunched(4)));
    // Held by a script at level 2: one level drops per second without a press.
    let mut c = Character::new(Who::Murray, [0.0; 3], 0.0);
    c.hold(2, &mut Vec::new());
    sim.targets.clear();
    sim.run(&mut c, &idle(), 61);
    assert_eq!((c.state, c.hold_level), (murray::HELD, 1));
    sim.run(&mut c, &idle(), 30);
    sim.press(&mut c, button::CROSS);
    sim.run(&mut c, &idle(), 50);
    assert_eq!(c.state, murray::HELD, "a press resets the timer");
    sim.run(&mut c, &idle(), 20);
    assert_eq!(c.state, murray::STAND);
}

#[test]
fn bentley_crossbow_darts_and_timed_state() {
    let mut sim = Sim::new();
    sim.targets.push(Target {
        id: 1,
        position: [-150.0, 10.0, 60.0],
        yaw: 0.0,
        kind: TargetKind::Guard { robbable: false, launched: false },
    });
    let mut c = Character::new(Who::Bentley, [0.0; 3], 0.0);
    // Standing: 3.15 rad covers a target behind him; he turns to face it; 60 above is the high clip.
    sim.press(&mut c, button::SQUARE);
    let ev = sim.took();
    assert!(ev.contains(&GangEvent::Crossbow { target: Some(1), high: 1.0, mode: 0 }), "{ev:?}");
    assert!(cos(c.yaw) < -0.9);
    // Moving: only 1.5 rad either side of the heading.
    let mut c = Character::new(Who::Bentley, [0.0; 3], 0.0);
    sim.step(&mut c, &forward(0));
    sim.step(&mut c, &GangInput { pressed: button::SQUARE, ..forward(button::SQUARE) });
    assert!(sim.took().iter().any(|e| matches!(e, GangEvent::Crossbow { target: None, mode: 1, .. })));
    // A dart along the screen centre to the wall at x = 3000; reload 2 s; darts last 4 s.
    let mut c = Character::new(Who::Bentley, [0.0; 3], 0.0);
    let cam = GangInput { camera_eye: [0.0, 0.0, 100.0], camera_forward: [1.0, 0.0, 0.0], ..idle() };
    sim.step(&mut c, &GangInput { pressed: button::R1, held: button::R1, ..cam });
    let dart = sim.took().into_iter().find_map(|e| match e {
        GangEvent::Dart { from, to, hit } => Some((from, to, hit)),
        _ => None,
    });
    let (from, to, hit) = dart.expect("dart");
    assert_eq!(from, [10.0, 0.0, 100.0]);
    assert!(hit && (to[0] - 3000.0).abs() < 1.0);
    sim.step(&mut c, &GangInput { pressed: button::R1, held: button::R1, ..cam });
    assert!(!sim.took().iter().any(|e| matches!(e, GangEvent::Dart { .. })), "reloading");
    sim.run(&mut c, &cam, 125);
    sim.step(&mut c, &GangInput { pressed: button::R1, held: button::R1, ..cam });
    assert!(sim.took().iter().any(|e| matches!(e, GangEvent::Dart { .. })));
    assert_eq!(c.darts.len(), 2);
    sim.run(&mut c, &cam, 241);
    assert!(c.darts.is_empty());
    let mut out = Vec::new();
    c.enter_timed(&mut out);
    sim.run(&mut c, &idle(), 119);
    assert_eq!(c.state, bentley::TIMED);
    sim.run(&mut c, &idle(), 2);
    assert_eq!(c.state, bentley::STAND);
}

#[test]
fn gadgets_hold_drain_refuse_and_hover() {
    let mut sim = Sim::new();
    for n in [7, 12, 29] {
        sim.s.owned.add(n);
    }
    let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
    assert!(c.slots.assign(0, 29, Who::Sly, &sim.s.items, &sim.s.owned));
    c.power = Power::new(10.0, 100.0);
    // Thief Reflexes: pays 2, scales time 0.25 while held, drains 2 per second.
    sim.step(&mut c, &GangInput { pressed: button::L1, held: button::L1, ..idle() });
    assert!(sim.took().contains(&GangEvent::ItemStarted(29)));
    assert_eq!(c.power.value, 8.0);
    sim.run(&mut c, &GangInput { held: button::L1, ..idle() }, 60);
    assert_eq!(c.time_scale, 0.25);
    assert!((c.power.value - 6.0).abs() < 1.0 + 1e-3, "{}", c.power.value);
    sim.step(&mut c, &idle());
    assert!(sim.took().contains(&GangEvent::ItemEnded(29)));
    assert_eq!(c.time_scale, 1.0);
    c.power = Power::new(1.0, 100.0);
    sim.step(&mut c, &GangInput { pressed: button::L1, held: button::L1, ..idle() });
    assert!(sim.took().contains(&GangEvent::ItemRefused { item: 29, reason: Refusal::Power }), "not enough power");

    // Hover Pack: only in the air; 300 across and −100 down, at least 0.5 s.
    let mut b = Character::new(Who::Bentley, [0.0; 3], 0.0);
    assert!(b.slots.assign(1, 12, Who::Bentley, &sim.s.items, &sim.s.owned));
    sim.step(&mut b, &GangInput { pressed: button::L2, held: button::L2, ..idle() });
    assert!(sim.took().contains(&GangEvent::ItemRefused { item: 12, reason: Refusal::State }));
    sim.press(&mut b, button::CROSS);
    sim.run(&mut b, &idle(), 20);
    sim.step(&mut b, &GangInput { pressed: button::L2, held: button::L2, ..idle() });
    assert!(sim.took().contains(&GangEvent::ItemStarted(12)));
    sim.run(&mut b, &GangInput { held: button::L2, ..forward(button::L2) }, 10);
    assert_eq!(b.velocity[2], -100.0);
    assert!((b.velocity[0] - 300.0).abs() < 1.0);
    // Bombs: summon on press, throw on release, trigger with the next press.
    let mut b = Character::new(Who::Bentley, [0.0; 3], 0.0);
    assert!(b.slots.assign(0, 7, Who::Bentley, &sim.s.items, &sim.s.owned));
    sim.step(&mut b, &GangInput { pressed: button::L1, held: button::L1, ..idle() });
    assert_eq!(b.state, bentley::HOLDING);
    assert!(sim.took().contains(&GangEvent::Summoned(7)));
    sim.step(&mut b, &idle());
    assert_eq!(b.state, bentley::THROW_AIM);
    sim.press(&mut b, button::SQUARE);
    assert!(sim.took().iter().any(|e| matches!(e, GangEvent::Thrown { id: 7, .. })));
    sim.run(&mut b, &idle(), 30);
    sim.press(&mut b, button::L1);
    assert!(sim.took().contains(&GangEvent::Triggered(7)));
}

#[test]
fn damage_reaction_and_recovery() {
    let mut sim = Sim::new();
    let mut c = Character::new(Who::Murray, [0.0; 3], 0.0);
    let hit = Hit {
        amount: 10.0,
        kind: health::kind::THROWN,
        source: [-100.0, 0.0, 0.0],
        source_yaw: 0.0,
        vector: [0.0; 3],
        shrunk: false,
    };
    let mut out = Vec::new();
    let world = sim.world();
    let r = c.hit(&sim.s.consts, &hit, &world, &mut out);
    assert!(matches!(r, Reaction::Hurt { velocity: [600.0, 0.0, 800.0], fatal: false, cry: true }));
    assert_eq!(c.state, murray::DAMAGE);
    assert_eq!(c.health.value, 30.0);
    sim.run(&mut c, &idle(), 120);
    assert_eq!(c.state, murray::STAND);
    assert!(c.position[0] > 100.0);
}

#[test]
fn gang_switching_and_bentley_toss() {
    let cw = floor_world();
    let mut g = Gang::new(
        vec![Character::new(Who::Murray, [0.0; 3], 0.0), Character::new(Who::Bentley, [100.0, 0.0, 0.0], 0.0)],
        shared(),
    );
    assert_eq!(g.player, Who::Murray);
    let world =
        GangWorld { cw: &cw, filter: QueryFilter::default(), targets: &[], help: 0.0, safehouse: false, pools: &[] };
    let mut all = Vec::new();
    let mut step = |g: &mut Gang, i: GangInput, n: usize| {
        for _ in 0..n {
            all.extend(g.update(DT, &i, &world));
        }
    };
    step(&mut g, idle(), 1);
    // Murray lifts Bentley, carries him, throws him; Bentley lands and becomes the player.
    step(&mut g, GangInput { pressed: button::CIRCLE, held: button::CIRCLE, ..idle() }, 1);
    assert_eq!(g.get(Who::Bentley).unwrap().state, bentley::CARRIED);
    step(&mut g, idle(), 70);
    assert_eq!(g.get(Who::Murray).unwrap().state, murray::CARRY);
    assert!((g.get(Who::Bentley).unwrap().position[2] - 150.0).abs() < 1.0);
    assert!(!g.switch_to(Who::Bentley), "carried");
    step(&mut g, GangInput { pressed: button::CIRCLE, held: button::CIRCLE, ..idle() }, 1);
    step(&mut g, GangInput { pressed: button::SQUARE, held: button::SQUARE, ..idle() }, 1);
    assert!(g.get(Who::Bentley).unwrap().tossed);
    step(&mut g, idle(), 180);
    assert_eq!(g.player, Who::Bentley);
    let b = g.get(Who::Bentley).unwrap();
    assert!(b.position[0] > 300.0 && b.grounded && b.state == bentley::STAND, "{:?}", b.position);
    assert!(g.switch_to(Who::Murray));
    assert!(!g.switch_to(Who::Sly), "not present");
    let _ = spec::HEALTH;
}

fn press_slot(sim: &mut Sim, c: &mut Character, b: u32) {
    sim.step(c, &GangInput { pressed: b, held: b, ..idle() });
}

#[test]
fn gadgets_pay_times_overlays_pockets_and_pools() {
    let mut sim = Sim::new();
    for n in [23, 26, 34] {
        sim.s.owned.add(n);
    }
    let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
    c.power = Power::new(10.0, 100.0);
    assert!(c.slots.assign(0, 23, Who::Sly, &sim.s.items, &sim.s.owned));
    assert!(c.slots.assign(1, 34, Who::Sly, &sim.s.items, &sim.s.owned));
    assert!(c.slots.assign(2, 26, Who::Sly, &sim.s.items, &sim.s.owned));
    // The Smoke Bomb pays at its activation event, then clears the alarms with a 2 s resume delay.
    press_slot(&mut sim, &mut c, button::L1);
    assert!(sim.took().contains(&GangEvent::ItemStarted(23)));
    assert_eq!(c.power.value, 10.0);
    sim.run(&mut c, &idle(), 20);
    let ev = sim.took();
    assert!(ev.contains(&GangEvent::ItemActivated(23)));
    assert!(ev.contains(&GangEvent::World(super::gadgets::WorldEffect::Smoke { resume: 2.0 })));
    assert_eq!(c.power.value, 7.0);
    // An overlay: applied at its event; using it again removes it without paying.
    press_slot(&mut sim, &mut c, button::L2);
    c.activation_event(&sim.s, &mut Vec::new());
    assert_eq!((c.overlay, c.power.value), (Some(34), 5.0));
    press_slot(&mut sim, &mut c, button::L2);
    assert_eq!((c.overlay, c.power.value), (None, 5.0));
    assert!(sim.took().contains(&GangEvent::Overlay { item: 34, on: false }));
    // A pocket item: refused with fewer than 2 free copies; else aimed and paid on the throw.
    sim.pools = vec![(26, 1)];
    press_slot(&mut sim, &mut c, button::R2);
    assert!(sim.took().contains(&GangEvent::ItemRefused { item: 26, reason: Refusal::Pool }));
    sim.pools = vec![(26, 2)];
    press_slot(&mut sim, &mut c, button::R2);
    assert_eq!((c.state, c.carrying, c.power.value), (sly::THROW_AIM, Some(26), 5.0));
    sim.press(&mut c, button::SQUARE);
    let ev = sim.took();
    assert!(ev.iter().any(|e| matches!(e, GangEvent::Thrown { id: 26, .. })));
    assert!(ev.contains(&GangEvent::ItemActivated(26)));
    assert_eq!(c.power.value, 4.0);
    // Not enough power.
    c.power = Power::new(2.0, 100.0);
    sim.run(&mut c, &idle(), 30);
    press_slot(&mut sim, &mut c, button::L1);
    assert!(sim.took().contains(&GangEvent::ItemRefused { item: 23, reason: Refusal::Power }));
}

#[test]
fn passives_paraglide_and_atlas_strength() {
    let mut sim = Sim::new();
    sim.s.owned.add(27);
    sim.s.owned.add(18);
    let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
    sim.run(&mut c, &idle(), 30);
    sim.press(&mut c, button::CROSS);
    // Within 0.3 s of the last landing R1 does nothing; later it deploys the paraglider and the fall slows.
    sim.run(&mut c, &idle(), 25);
    sim.press(&mut c, button::R1);
    assert!(c.gliding && sim.took().contains(&GangEvent::SpecialJump(19)));
    // gadgets.md 5.1: with the stick centred (f = 0.5) the glide eases to 550 forward and −300 down (from high up,
    // so it has time to settle).
    c.position[2] = 1500.0;
    sim.run(&mut c, &idle(), 90);
    let fwd = c.velocity[0] * c.yaw.cos() + c.velocity[1] * c.yaw.sin();
    assert!((fwd - 550.0).abs() < 30.0 && (c.velocity[2] + 300.0).abs() < 30.0, "{:?} fwd {fwd}", c.velocity);
    let mut frames = 0;
    while c.gliding && frames < 300 {
        sim.step(&mut c, &idle());
        frames += 1;
    }
    assert!(!c.gliding && c.grounded, "{:?} {:?}", c.position, c.velocity);
    // Atlas Strength: Murray jumps while carrying.
    let mut m = Character::new(Who::Murray, [0.0; 3], 0.0);
    m.carrying = Some(3);
    m.set_state(murray::CARRY, &mut Vec::new());
    sim.press(&mut m, button::CROSS);
    assert!(sim.took().contains(&GangEvent::SpecialJump(15)));
    assert!(m.velocity[2] > 0.0 && m.carrying == Some(3));
}

#[test]
fn gaits_and_double_jump() {
    use super::consts::gait;
    // characters.md 1.1.
    for (g, u, v) in [
        (gait::SLY_RUN, 1.0, 640.0),
        (gait::SLY_MOVE, 0.0, 225.0),
        (gait::SLY_MOVE, 0.5, 320.0),
        (gait::SLY_MOVE, 1.0, 510.0),
        (gait::MURRAY_RUN, 1.0, 800.0),
        (gait::MURRAY_WALK, 0.0, 225.0),
        (gait::MURRAY_WALK, 1.0, 510.0),
        (gait::MURRAY_CARRY, 0.0, 125.0),
        (gait::MURRAY_CARRY, 1.0, 516.67),
        (gait::BENTLEY_RUN, 1.0, 750.0),
        (gait::BENTLEY_WALK, 0.0, 165.0),
        (gait::BENTLEY_WALK, 1.0, 510.0),
        (gait::BENTLEY_TIMED, 1.0, 1500.0),
        (gait::SLY_DODGE, 1.0, 1500.0),
    ] {
        assert!((g.speed(u) - v).abs() < 0.05, "{g:?} at {u}: {}", g.speed(u));
    }
    assert!((super::consts::jump_speed(215.0, 0.0) - 989.1).abs() < 0.1);
    // Sly's double jump: Cross in the air brings the peak to 315 above the first jump's floor; once per jump.
    let mut sim = Sim::new();
    let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
    sim.run(&mut c, &idle(), 5);
    sim.press(&mut c, button::CROSS);
    sim.run(&mut c, &idle(), 10);
    sim.press(&mut c, button::CROSS);
    assert!(sim.took().contains(&GangEvent::DoubleJump));
    let mut peak: f32 = 0.0;
    for _ in 0..90 {
        sim.step(&mut c, &idle());
        peak = peak.max(c.position[2]);
    }
    assert!((peak - 315.0).abs() < 8.0, "peak {peak}");
    // Murray has none.
    let mut m = Character::new(Who::Murray, [0.0; 3], 0.0);
    sim.run(&mut m, &idle(), 5);
    sim.press(&mut m, button::CROSS);
    sim.run(&mut m, &idle(), 10);
    sim.press(&mut m, button::CROSS);
    assert!(!sim.took().contains(&GangEvent::DoubleJump));
}

#[test]
fn move_items_follow_their_clips() {
    let mut sim = Sim::new();
    for n in [20, 24, 28, 33] {
        sim.s.owned.add(n);
    }
    // Berserker Charge: 700/s for 0.66 s, then the stop at 250/s for 1 s.
    let mut m = Character::new(Who::Murray, [-2000.0, 0.0, 0.0], 0.0);
    m.set_state(murray::CHARGE, &mut Vec::new());
    sim.run(&mut m, &idle(), 20);
    assert!((m.velocity[0] - 700.0).abs() < 1.0, "{:?}", m.velocity);
    sim.run(&mut m, &idle(), 30);
    assert!((m.velocity[0] - 250.0).abs() < 1.0);
    sim.run(&mut m, &idle(), 60);
    assert_eq!(m.state, murray::STAND);
    // The juggle strike with Silent Obliteration homes in toward 200 from the target.
    let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
    sim.targets.push(Target {
        id: 4,
        position: [800.0, 300.0, 0.0],
        yaw: 0.0,
        kind: TargetKind::Guard { robbable: false, launched: true },
    });
    c.juggle_strike(4, &mut Vec::new());
    sim.run(&mut c, &idle(), 50);
    let d = crate::math::length(crate::math::sub([800.0, 300.0, 0.0], c.position));
    // From 854 the distance eases toward 200 at up to 1,000/s at half the frame step (500/s): 437.9 after 50 frames.
    assert!((d - 437.9).abs() < 1.0, "{d}");
    // The combat dodge circles the guard at about 250 and ends with its velocity halved.
    let mut c = Character::new(Who::Sly, [-190.0, 0.0, 0.0], 0.0);
    sim.targets = vec![Target {
        id: 5,
        position: [0.0; 3],
        yaw: 0.0,
        kind: TargetKind::Guard { robbable: false, launched: false },
    }];
    sim.step(&mut c, &idle());
    assert_eq!(c.state, sly::FIGHT);
    c.slots.0[0] = Some(24);
    sim.s.items.items[24] =
        super::items::Item { button: 2, owner: 7, price: 0, cost: 0, episode: 0, name: 0, description: 0, usage: 0 };
    sim.step(&mut c, &GangInput { pressed: button::L1, held: button::L1, ..idle() });
    assert_eq!(c.state, sly::DODGE);
    assert_eq!(c.move_target, Some(5));
    let mut trace = Vec::new();
    // The dodge clip lasts 1/3 s (20 frames after the press).
    for _ in 0..21 {
        sim.step(&mut c, &idle());
        trace.push((c.position, c.state));
    }
    // Ended (back to the fight stance, then standing: the guard at 250 is beyond the 200 reach).
    assert_ne!(c.state, sly::DODGE);
    let r = crate::math::length([c.position[0], c.position[1], 0.0]);
    assert!((r - 250.0).abs() < 40.0, "{r} {trace:?}");
    assert!(crate::math::length(c.velocity) < 800.0);
}

#[test]
fn dodge_target_and_strike_times() {
    let mut sim = Sim::new();
    // gadgets.md 5.4: a guard 1,000 away with the stick pushed fully toward it (score 0) beats one 300 away behind
    // the player (score 300); with the stick released the nearer wins (0.75·d − 250). No distance limit.
    let guard = |id, x| Target {
        id,
        position: [x, 0.0, 0.0],
        yaw: 0.0,
        kind: TargetKind::Guard { robbable: false, launched: false },
    };
    sim.targets = vec![guard(1, 1000.0), guard(2, -300.0), guard(3, 50_000.0)];
    let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
    c.last_stick = [1.0, 0.0, 0.0];
    c.last_deflection = 1.0;
    assert_eq!(c.dodge_target(&sim.world()).map(|g| g.id), Some(1));
    c.last_deflection = 0.0;
    assert_eq!(c.dodge_target(&sim.world()).map(|g| g.id), Some(2));
    sim.targets.clear();
    assert_eq!(c.dodge_target(&sim.world()), None);
    // gadgets.md 5.3: the juggle strike lasts 1.1667 s (clip 196) without Silent Obliteration.
    sim.targets = vec![guard(4, 300.0)];
    c.juggle_strike(4, &mut Vec::new());
    sim.run(&mut c, &idle(), 69);
    assert_eq!(c.state, sly::JUGGLE_STRIKE);
    sim.run(&mut c, &idle(), 2);
    assert_ne!(c.state, sly::JUGGLE_STRIKE);
    assert!((spec::SLAM_TIME - 0.8333).abs() < 1e-4 && spec::LIGHTNING_SPIN_TIME == 1.0);
}

#[test]
fn sly3_combo_definitions_drive_state_23() {
    use super::combo::{ComboDef, ComboSet, button_of_pad};
    let mut sim = Sim::new();
    let raw = vec![0u8; super::sly3::ITEM_COUNT * 60];
    sim.s.items = super::sly3::Sly3ItemTable::from_bytes(&raw).to_item_table();
    let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
    c.combo_defs = ComboSet {
        defs: vec![ComboDef {
            oid: 7,
            button: button_of_pad(0x10),
            priority: 2,
            clips: [237, -1, -1],
            ..ComboDef::default()
        }],
    };
    sim.run(&mut c, &idle(), 3);
    sim.press(&mut c, button::TRIANGLE);
    assert_eq!(c.state, sly::COMBO);
    assert!(sim.took().contains(&GangEvent::ComboClip { def: 7, clip: 237 }));
    // The attack (0.5 s) and the 0.35 s window pass: back to the fight stance.
    sim.run(&mut c, &idle(), 60);
    assert!(sim.took().contains(&GangEvent::ComboEnded));
    assert_ne!(c.state, sly::COMBO);
}

#[test]
fn popup_hits_add_a_held_second_reaction() {
    // characters.md 2.2: after the knock-back reaction of a 0x8000 hit, sound 0x6A and a second, held reaction.
    let mut sim = Sim::new();
    let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
    sim.run(&mut c, &idle(), 5);
    let hit = Hit {
        amount: 3.0,
        kind: 0x8000 | health::kind::KNOCKBACK,
        source: [-100.0, 0.0, 0.0],
        source_yaw: 0.0,
        vector: [0.0; 3],
        shrunk: false,
    };
    let mut out = Vec::new();
    let world = sim.world();
    c.hit(&sim.s.consts, &hit, &world, &mut out);
    assert_eq!((c.state, c.held_reaction), (sly::DAMAGE, 1));
    let mut held_at = None;
    for f in 0..300 {
        sim.step(&mut c, &idle());
        if held_at.is_none() && sim.took().contains(&GangEvent::HeldReaction) {
            held_at = Some(f);
            assert_eq!(c.state, sly::DAMAGE, "still in the reaction");
        }
        if c.state != sly::DAMAGE {
            break;
        }
    }
    assert!(held_at.is_some());
    assert_eq!((c.state, c.held_reaction), (sly::STAND, 0));
    // A plain knock-back gets up at once.
    let mut d = Character::new(Who::Sly, [0.0; 3], 0.0);
    sim.run(&mut d, &idle(), 5);
    let world = sim.world();
    d.hit(&sim.s.consts, &Hit { kind: health::kind::KNOCKBACK, ..hit }, &world, &mut out);
    assert_eq!(d.held_reaction, 0);
}
