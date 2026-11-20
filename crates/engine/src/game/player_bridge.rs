//! Sly in the world: `engine::player` behind the game loop's [`PlayerSystem`].
//!
//! At load the player object (class 7, OID 5) becomes `World::player`; its context features are gathered from the
//! world (pipes 112, rails 113, perch points 129, handle markers 118 as hooks, hiding baskets 58, water 31), the
//! ledges from the collision world, and its animations (the segments below its proxy) go to a `PlayerAnimator` on a
//! rig of its subtree. Each game frame the controller reads the input (camera-relative), moves through the
//! collision world, and the bridge places the object, writes the rig's local transforms into the player's bones,
//! hands damage from the world to the controller, turns attacks into notices on what they reach, and reports the
//! controller's events (`World::shared.player_events`) for the game to apply to the progress.

use alloc::vec::Vec;

use formats::anim::Segment;
use formats::geometry::Affine;
use platform_api::Buttons;

use super::systems::{CollisionQueries, FrameInput, PlayerSystem, StepInfo};
use crate::anim::rig::Rig;
use crate::collision::{Ledge, QueryFilter};
use crate::math::{add, length, scale, sub};
use crate::object::Handle;
use crate::object::class::id;
use crate::player::controller::{AttackKind, Player, PlayerEvent};
use crate::player::status::{DamageKind, Hiding as PHiding};
use crate::player::{
    Button, Feature, FeatureKind, PlayerAnimator, PlayerConstants, PlayerInput, PlayerWorld, world_ledges,
};
use crate::world::shared::Hiding;
use crate::world::{Notice, World};

/// The player's OID in every world (guards.md, object-stream.md 7.1).
pub const PLAYER_OID: i16 = 5;
/// An attack's effect object (player-moves.md 6, "Reach"): where it is placed in the player's frame and its shape's
/// box in its own space. The sweep's object follows the cane bone through the swing; it is taken at its placed
/// position here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectShape {
    /// Placed at, in the player's frame.
    pub at: [f32; 3],
    /// Box minimum and maximum around `at`.
    pub min: [f32; 3],
    /// See [`EffectShape::min`].
    pub max: [f32; 3],
}

/// The four effect shapes: sweep (OID 126), smash (127), rush (128), dive (441).
pub const SWEEP_SHAPE: EffectShape =
    EffectShape { at: [0.0, 0.0, -55.4], min: [-83.9, -201.2, -60.0], max: [28.8, -27.6, 40.0] };
/// See [`SWEEP_SHAPE`].
pub const SMASH_SHAPE: EffectShape =
    EffectShape { at: [4.8, 0.0, 29.8], min: [-50.0, -50.0, -1.4], max: [50.0, 50.0, 148.6] };
/// See [`SWEEP_SHAPE`].
pub const RUSH_SHAPE: EffectShape =
    EffectShape { at: [75.8, 0.0, 18.6], min: [-25.0, -87.5, -87.5], max: [25.0, 87.5, 87.5] };
/// See [`SWEEP_SHAPE`].
pub const DIVE_SHAPE: EffectShape =
    EffectShape { at: [-6.9, 0.0, -40.0], min: [-54.9, -53.6, -42.1], max: [54.9, 53.6, 42.1] };
/// Radius of a target marker (player-moves.md 6, "Aiming").
pub const TARGET_RADIUS: f32 = 25.0;

/// The effect shape and natural hit point (player's frame) and how far ahead the aim predicts, for an attack.
pub fn attack_shape(k: AttackKind) -> (EffectShape, [f32; 3], f32) {
    match k {
        AttackKind::SweepMoving | AttackKind::SweepStanding => (SWEEP_SHAPE, [175.0, 0.0, 0.0], 0.15),
        AttackKind::Rush => (RUSH_SHAPE, [250.0, 0.0, 125.0], 0.25),
        AttackKind::Smash => (SMASH_SHAPE, [125.0, 0.0, 0.0], 0.25),
        AttackKind::Dive => (DIVE_SHAPE, [0.0, 0.0, 0.0], 0.25),
    }
}

/// Whether a point (with a radius) is inside an effect shape placed by the player's frame (`pos`, `yaw`).
pub fn in_shape(s: &EffectShape, pos: [f32; 3], yaw: f32, p: [f32; 3], radius: f32) -> bool {
    let (sn, cs) = (crate::math::sin(yaw), crate::math::cos(yaw));
    let d = sub(p, pos);
    let local = [d[0] * cs + d[1] * sn - s.at[0], -d[0] * sn + d[1] * cs - s.at[1], d[2] - s.at[2]];
    (0..3).all(|k| local[k] >= s.min[k] - radius && local[k] <= s.max[k] + radius)
}

/// Controller buttons (Switch layout; the bottom face button jumps like the original's Cross).
pub fn player_input(i: &FrameInput, camera_yaw: f32) -> PlayerInput {
    let b = |bits: Buttons| Button { held: i.buttons & bits.0 != 0, pressed: i.pressed & bits.0 != 0 };
    PlayerInput {
        stick: i.left,
        camera_yaw,
        jump: b(Buttons::B),
        attack: b(Buttons::Y),
        action: b(Buttons::A),
        duck: b(Buttons::ZL),
        rush: b(Buttons::ZR),
        power: b(Buttons::X),
    }
}

/// The player system.
#[derive(Default)]
pub struct SlyPlayer {
    /// The controller, once a player object exists.
    pub player: Option<Player>,
    animator: Option<PlayerAnimator>,
    rig: Rig,
    nodes: Vec<(u32, u32)>,
    root_node: Option<u32>,
    features: Vec<Feature>,
    ledges: Vec<Ledge>,
    consts: Option<PlayerConstants>,
}

impl SlyPlayer {
    /// A player system; `consts` from `PlayerConstants::from_executable` (else the spec's values).
    pub fn new(consts: Option<PlayerConstants>) -> SlyPlayer {
        SlyPlayer { consts, ..Default::default() }
    }

    fn gather_features(world: &World) -> Vec<Feature> {
        let mut out = Vec::new();
        for h in world.handles() {
            if !world.is_active(h) {
                continue;
            }
            let kind = match world.class(h) {
                Some(112) => {
                    let pts = crate::behavior::sensor::curve_points(world, h);
                    match (pts.first(), pts.last()) {
                        (Some(&a), Some(&b)) if pts.len() >= 2 => {
                            let (bottom, top) = if a[2] <= b[2] { (a, b) } else { (b, a) };
                            FeatureKind::Pipe { bottom, top }
                        }
                        _ => continue,
                    }
                }
                Some(113) => {
                    let points = crate::behavior::sensor::curve_points(world, h);
                    if points.len() < 2 {
                        continue;
                    }
                    FeatureKind::Rail { points }
                }
                Some(129) => FeatureKind::Perch { position: world.position(h).unwrap_or([0.0; 3]) },
                Some(118) => FeatureKind::Hook { position: world.position(h).unwrap_or([0.0; 3]) },
                Some(58) => FeatureKind::HidingPlace { position: world.position(h).unwrap_or([0.0; 3]) },
                _ => continue,
            };
            out.push(Feature { id: h.index(), kind });
        }
        out
    }

    fn add_water(&mut self, world: &World, collision: &dyn CollisionQueries) {
        let Some(cw) = collision.collision_world() else { return };
        for b in cw.bodies() {
            let Some(h) = world.handle(b.user) else { continue };
            if world.class(h) != Some(31) || !b.enabled {
                continue;
            }
            let bb = b.bounds();
            self.features.push(Feature {
                id: h.index(),
                kind: FeatureKind::Water {
                    surface: bb.max[2],
                    min: [bb.min[0], bb.min[1]],
                    max: [bb.max[0], bb.max[1]],
                },
            });
        }
    }

    /// Writes the rig's local transforms into the player's bones.
    fn write_rig(&self, world: &mut World) {
        for &(node, slot) in &self.nodes {
            if Some(node) == self.root_node {
                continue;
            }
            if let (Some(h), Some(l)) = (world.handle(slot), self.rig.local.get(node as usize)) {
                world.set_local(h, l.to_affine());
            }
        }
    }

    /// Hits what an attack's effect shape holds (targets' positions, with the target-marker radius).
    fn attack_shape_hits(
        world: &mut World,
        player: Handle,
        k: AttackKind,
        pos: [f32; 3],
        yaw: f32,
        collision: &dyn CollisionQueries,
    ) {
        let (shape, _, _) = attack_shape(k);
        let reach =
            [shape.min, shape.max].iter().map(|c| length(add(*c, shape.at))).fold(0.0, f32::max) + TARGET_RADIUS;
        let mut near = Vec::new();
        if !collision.overlap_sphere(world, pos, reach, &mut near) {
            near = world
                .handles()
                .filter(|&h| world.position(h).is_some_and(|p| crate::behavior::dist_sq(p, pos) <= reach * reach))
                .collect();
        }
        for t in near {
            if t == player || world.parent(t) == Some(player) {
                continue;
            }
            if world.position(t).is_some_and(|p| in_shape(&shape, pos, yaw, p, TARGET_RADIUS)) {
                world.notify(t, Notice::Attack { by: player, kind: k as u32 });
            }
        }
    }

    /// The heading to the best attack target (player-moves.md 6, "Aiming"): target markers (class 117) and characters
    /// (guards; 550 reach with a 250 bonus) within 300 of the attack's natural hit point predicted ahead, within the
    /// attack's heading range (±1.6 rad for a moving sweep, ±1 for the rush, any for a standing sweep or a smash),
    /// the closest to that point.
    fn aim(world: &World, player: Handle, k: AttackKind, pos: [f32; 3], yaw: f32, vel: [f32; 3]) -> Option<f32> {
        let (_, natural, ahead) = attack_shape(k);
        let (sn, cs) = (crate::math::sin(yaw), crate::math::cos(yaw));
        let base = add(pos, scale(vel, ahead));
        let hit = [
            base[0] + natural[0] * cs - natural[1] * sn,
            base[1] + natural[0] * sn + natural[1] * cs,
            base[2] + natural[2],
        ];
        let limit = match k {
            AttackKind::SweepMoving => Some(1.6),
            AttackKind::Rush => Some(1.0),
            _ => None,
        };
        let mut best: Option<(f32, f32)> = None;
        for h in world.handles() {
            if h == player || !world.in_world(h) {
                continue;
            }
            let character = match world.class(h) {
                Some(117) => false,
                Some(c) if crate::behavior::guard::is_guard_class(c) => true,
                _ => continue,
            };
            let Some(p) = world.position(h) else { continue };
            let d = length(sub(p, hit));
            let (reach, bonus) = if character { (550.0, 250.0) } else { (300.0, 0.0) };
            if d > reach {
                continue;
            }
            let heading = crate::math::atan2(p[1] - pos[1], p[0] - pos[0]);
            let mut off = heading - yaw;
            while off > core::f32::consts::PI {
                off -= 2.0 * core::f32::consts::PI;
            }
            while off < -core::f32::consts::PI {
                off += 2.0 * core::f32::consts::PI;
            }
            if limit.is_some_and(|l| off.abs() > l) {
                continue;
            }
            let score = d - bonus;
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, heading));
            }
        }
        best.map(|b| b.1)
    }

    fn attack(
        world: &mut World,
        player: Handle,
        at: [f32; 3],
        radius: f32,
        kind: u32,
        collision: &dyn CollisionQueries,
    ) {
        let mut hit = Vec::new();
        if !collision.overlap_sphere(world, at, radius, &mut hit) {
            let r2 = radius * radius;
            hit = world
                .handles()
                .filter(|&h| world.position(h).is_some_and(|p| crate::behavior::dist_sq(p, at) <= r2))
                .collect();
        }
        for h in hit {
            if h != player && world.parent(h) != Some(player) {
                world.notify(h, Notice::Attack { by: player, kind });
            }
        }
    }
}

impl PlayerSystem for SlyPlayer {
    fn load(&mut self, world: &mut World, collision: &dyn CollisionQueries) {
        let Some(&slot) = world.slots_of_class(id::PLAYER).iter().find(|&&s| world.oid_of_slot(s) == PLAYER_OID) else {
            return;
        };
        let Some(h) = world.handle(slot) else { return };
        world.player = Some(h);
        let w = world.world_transform(h).unwrap_or(Affine::IDENTITY);
        let yaw = crate::math::atan2(w.cols[0][1], w.cols[0][0]);
        let mut p = Player::new(w.t, yaw);
        if let Some(c) = self.consts {
            p.consts = c;
        }
        self.player = Some(p);

        // The rig: the player's proxy and everything below it.
        let rec = world.record(h).unwrap_or(0);
        let stream = &world.level.stream;
        let root = stream.records[rec as usize].parent.unwrap_or(rec);
        let range = root..stream.records[root as usize].subtree_end;
        self.rig = Rig::from_stream(stream, &world.level.bytes, root);
        self.root_node = self.rig.node_of_record(rec);
        self.nodes = world
            .handles()
            .filter(|&o| world.persist_key(o).is_some_and(|k| k >> 32 == 0))
            .filter_map(|o| {
                let r = world.record(o)?;
                if !range.contains(&r) {
                    return None;
                }
                Some((self.rig.node_of_record(r)?, o.index()))
            })
            .collect();
        let segments: Vec<(i16, Segment)> = range
            .clone()
            .filter(|&k| stream.records[k as usize].class == id::SEGMENT)
            .filter_map(|k| {
                let r = &stream.records[k as usize];
                Some((r.oid, Segment::parse(&world.level.bytes, r.offset + 6, &world.level.table).ok()?))
            })
            .collect();
        let rig = &self.rig;
        self.animator = Some(PlayerAnimator::new(segments, |oid| rig.find_oid(oid)));
        self.features = Self::gather_features(world);
        self.add_water(world, collision);
        if let Some(cw) = collision.collision_world() {
            self.ledges = world_ledges(cw);
        }
    }

    fn decide(&mut self, world: &mut World, input: &FrameInput, step: &StepInfo, collision: &dyn CollisionQueries) {
        let (Some(p), Some(h)) = (self.player.as_mut(), world.player) else { return };
        if !world.contains(h) {
            return;
        }
        let Some(cw) = collision.collision_world() else { return };
        let (charms, lives) = world.shared.vitals;
        p.vitals.charms = charms.clamp(0, 2) as u8;
        p.vitals.lives = lives;
        p.reward_mask = world.shared.player.rewards;
        let mut events: Vec<PlayerEvent> = Vec::new();

        // Damage dealt to the player since last frame.
        let damage = world.take_damage_events();
        let mut keep = Vec::new();
        for d in damage {
            if d.target.is_none() || d.target == Some(h) {
                let mut out = Default::default();
                let r = p.damage(DamageKind::from_code(d.kind as i16), &mut out);
                // A guard's attack effect throws the player (guards.md 6.2).
                if let Some(v) = d.throw
                    && r == crate::player::status::DamageOutcome::CharmLost
                {
                    p.velocity = v;
                }
                let crate::player::controller::PlayerOutput { events: ev, .. } = out;
                events.extend(ev);
                if let Some(r) = d.recover_to {
                    p.position = r;
                }
            } else {
                keep.push(d);
            }
        }
        world.shared.damage = keep;

        let pin = player_input(input, world.shared.camera_yaw);
        let pw = PlayerWorld {
            cw,
            filter: QueryFilter::default(),
            features: &self.features,
            ledges: &self.ledges,
            in_shadow: false,
            in_cover: false,
        };
        let out = p.update(step.game_dt, &pin, &pw);
        events.extend(out.events.iter().copied());

        // Place the object and animate.
        let rot = crate::math::axis_angle([0.0, 0.0, p.yaw]);
        let base = Affine { cols: rot, t: p.position };
        world.set_world_transform(h, base);
        if let (Some(anim), Some(req)) = (self.animator.as_mut(), out.anim) {
            anim.update(&req, out.restart_anim, step.game_dt, &mut self.rig);
            self.rig.update_world(&base);
        }

        // What other objects need to know.
        let hiding = match p.hiding(false) {
            PHiding::None => Hiding::None,
            PHiding::Flattened { detectable, .. } => Hiding::Wall { detectable },
            PHiding::Ducking { detectable, .. } => Hiding::Cover { detectable },
            PHiding::InHidingPlace { still } => {
                if still {
                    Hiding::Basket
                } else {
                    Hiding::None
                }
            }
            PHiding::Fourth => Hiding::Casual,
        };
        let s = &mut world.shared.player;
        s.hiding = hiding;
        s.airborne = p.airborne();
        s.velocity = p.velocity;
        s.base = Some(p.position);
        s.invulnerable = p.flash > 0.0;
        world.shared.time_scale = out.time_scale;
        let yaw = p.yaw;
        let pos = p.position;
        let vel = p.velocity;
        for e in &events {
            match *e {
                PlayerEvent::Attack(k @ (AttackKind::SweepMoving | AttackKind::SweepStanding | AttackKind::Rush)) => {
                    // Aiming: turn toward the best target first, then hit what the effect shape holds.
                    let yaw = Self::aim(world, h, k, pos, yaw, vel).unwrap_or(yaw);
                    if let Some(p) = self.player.as_mut() {
                        p.yaw = yaw;
                    }
                    Self::attack_shape_hits(world, h, k, pos, yaw, collision);
                }
                PlayerEvent::SmashLanded { position, .. } => {
                    Self::attack_shape_hits(world, h, AttackKind::Smash, position, yaw, collision)
                }
                PlayerEvent::DiveLanded { position, affect_radius, .. } => {
                    Self::attack(world, h, position, affect_radius, AttackKind::Dive as u32, collision)
                }
                PlayerEvent::HidingEntered(id) => world.shared.basket = world.handle(id),
                PlayerEvent::HidingLeft(_) => world.shared.basket = None,
                _ => {}
            }
        }
        world.shared.player_events.extend(events);
    }

    fn place(&mut self, world: &mut World, transform: Affine) {
        if let (Some(p), Some(h)) = (self.player.as_mut(), world.player) {
            p.respawn(transform.t, crate::math::atan2(transform.cols[0][1], transform.cols[0][0]));
            world.set_world_transform(h, transform);
        }
    }

    fn after_physics(&mut self, world: &mut World, _step: &StepInfo, _collision: &dyn CollisionQueries) {
        self.write_rig(world);
    }
}
