//! Pickups: coins, charms, life tokens, keys and sprint hourglasses (pickups-and-progress.md 2 and 3), and clue
//! bottles (4.1).
//!
//! A placed pickup rests until the player collects it: within 300 units, when it is inside the class's collect
//! radius or its sphere touches the player's collision mesh, or when an attack hits it. Collecting raises event 2,
//! updates the progress, and takes the pickup out of the world; a placed pickup stays collected across restarts and
//! reloads (except charms in world `0x308`). A pickup thrown out (by a breakable, or a charm the player cannot hold)
//! falls under its own gravity, bounces off solid surfaces, becomes collectable after 1 s and is removed after 5 s
//! of falling.

use alloc::vec::Vec;

use super::{ClassBehavior, Cx, dist_sq};
use crate::game::progress::{CoinResult, DEFAULT_SPRINT_TIME, level_flag};
use crate::math::{add, dot, length, madd, scale, sub};
use crate::object::Handle;
use crate::object::class::id;
use crate::world::{Notice, Request, World};

/// Within this distance of the player a pickup checks for collection.
pub const COLLECT_RANGE: f32 = 300.0;
/// Gravity of falling pickups, units per second squared.
pub const DROP_GRAVITY: f32 = -2250.0;
/// A falling pickup becomes collectable after this long.
pub const COLLECTABLE_AFTER: f32 = 1.0;
/// A pickup that has fallen this long is removed.
pub const FALL_LIMIT: f32 = 5.0;
/// Below this vertical speed after the last bounce, a pickup rests.
pub const REST_SPEED: f32 = 25.0;
/// A surface counts as a floor when its normal's z exceeds this.
pub const FLOOR_Z: f32 = 0.7;
/// The world where collected charms come back ("A Deadly Dance").
pub const CHARMS_RETURN_WORLD: u32 = 0x308;
/// Charm swirl, then stick, before it counts.
pub const CHARM_SWIRL: f32 = 1.0;
/// See [`CHARM_SWIRL`].
pub const CHARM_STICK: f32 = 0.25;
/// Velocity of a charm the player cannot hold (in the player's frame).
pub const CHARM_REJECT: [f32; 3] = [-250.0, 0.0, 600.0];
/// World option: the sprint's time.
pub const WORLD_OPT_SPRINT_TIME: u16 = 538;

/// Which pickup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupKind {
    /// Coin (class 88).
    Coin,
    /// Charm (class 86).
    Charm,
    /// Life token (class 87).
    Life,
    /// Key (class 89).
    Key,
    /// Sprint hourglass (class 90).
    Hourglass,
}

/// Per-class constants (pickups-and-progress.md 2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickupParams {
    /// Automatic-collect radius.
    pub collect_radius: f32,
    /// Collision sphere radius (bounces and touching the player).
    pub bounce_radius: f32,
    /// Normal factor of a bounce.
    pub normal: f32,
    /// Tangential factor of a bounce.
    pub tangential: f32,
    /// Upward speed of the last bounce on a floor, and its maximum.
    pub last_bounce: (f32, f32),
    /// Chance of a glint when the facing crosses the camera's side.
    pub glint: f32,
    /// Lifetime when dropped (`None`: stays).
    pub lifetime: Option<(f32, f32)>,
}

impl PickupKind {
    /// The class's constants.
    pub fn params(self) -> PickupParams {
        match self {
            PickupKind::Coin => PickupParams {
                collect_radius: 50.0,
                bounce_radius: 45.0,
                normal: 0.75,
                tangential: 0.6,
                last_bounce: (50.0, 250.0),
                glint: 0.25,
                lifetime: Some((8.0, 10.0)),
            },
            PickupKind::Charm => PickupParams {
                collect_radius: 60.0,
                bounce_radius: 60.0,
                normal: 0.5,
                tangential: 0.75,
                last_bounce: (0.0, 300.0),
                glint: 0.35,
                lifetime: None,
            },
            PickupKind::Life => PickupParams {
                collect_radius: 50.0,
                bounce_radius: 45.0,
                normal: 0.5,
                tangential: 0.05,
                last_bounce: (50.0, 250.0),
                glint: 0.0,
                lifetime: Some((4.0, 6.0)),
            },
            PickupKind::Key => PickupParams {
                collect_radius: 35.0,
                bounce_radius: 35.0,
                normal: 0.6,
                tangential: 0.6,
                last_bounce: (250.0, 500.0),
                glint: 0.75,
                lifetime: None,
            },
            PickupKind::Hourglass => PickupParams {
                collect_radius: 60.0,
                bounce_radius: 60.0,
                normal: 0.6,
                tangential: 0.6,
                last_bounce: (250.0, 500.0),
                glint: 0.75,
                lifetime: None,
            },
        }
    }
}

/// A pickup's state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PickupState {
    /// Placed, waiting.
    Resting,
    /// Thrown out, under gravity.
    Falling {
        /// Time since it started falling.
        time: f32,
        /// The last bounce on a floor has happened.
        settled: bool,
    },
    /// A charm circling the player before it counts.
    Swirling(f32),
    /// Collected or expired.
    Removed,
}

/// A pickup.
#[derive(Clone, Debug, PartialEq)]
pub struct Pickup {
    /// Which one.
    pub kind: PickupKind,
    /// State.
    pub state: PickupState,
    /// Velocity while falling.
    pub velocity: [f32; 3],
    /// Time left before a dropped pickup expires.
    pub lifetime: Option<f32>,
}

impl Pickup {
    /// A resting pickup.
    pub fn new(kind: PickupKind) -> Pickup {
        Pickup { kind, state: PickupState::Resting, velocity: [0.0; 3], lifetime: None }
    }

    /// Throws the pickup out with a velocity (a drop): it falls, becomes collectable after 1 s, and expires after
    /// its class's lifetime.
    pub fn throw(&mut self, velocity: [f32; 3], lifetime: Option<f32>) {
        self.velocity = velocity;
        self.state = PickupState::Falling { time: 0.0, settled: false };
        self.lifetime = lifetime;
    }

    /// The state as option 981 reads it: 0 normal, 1 being collected, 2 removed, 3 falling, 4 swirling, 6 lost.
    pub fn state_code(&self) -> i32 {
        match self.state {
            PickupState::Resting => 0,
            PickupState::Removed => 2,
            PickupState::Falling { .. } => 3,
            PickupState::Swirling(_) => 4,
        }
    }

    fn collectable(&self) -> bool {
        match self.state {
            PickupState::Resting => true,
            PickupState::Falling { time, .. } => time >= COLLECTABLE_AFTER,
            _ => false,
        }
    }

    fn wants_collect(&self, cx: &mut Cx<'_>, hit: bool) -> bool {
        if !self.collectable() {
            return false;
        }
        if hit {
            return true;
        }
        let Some(c) = cx.collector_position() else { return false };
        let p = cx.my_position();
        let d2 = dist_sq(p, c);
        if d2 > COLLECT_RANGE * COLLECT_RANGE {
            return false;
        }
        let params = self.kind.params();
        if d2 <= params.collect_radius * params.collect_radius {
            return true;
        }
        // A sphere of the collect radius around the pickup against the player's collision shape (pickups-and-progress.md
        // 2); the stand-in (no shape) is a sphere of its own (`hazard::STAND_IN`).
        let (r, up) = super::hazard::STAND_IN;
        let stand_in = dist_sq(p, [c[0], c[1], c[2] + up]) <= (r + params.collect_radius) * (r + params.collect_radius);
        match cx.world.player {
            Some(player) => cx.collision.sphere_touches(cx.world, player, p, params.collect_radius).unwrap_or(stand_in),
            None => stand_in,
        }
    }

    /// Collects the pickup (by the player, or a hit).
    fn collect(&mut self, cx: &mut Cx<'_>) {
        let world_id = cx.world.info.id;
        let remember = !(self.kind == PickupKind::Charm && world_id == CHARMS_RETURN_WORLD);
        let placed = cx.world.flags(cx.me) & crate::object::flags::SPAWNED == 0;
        match self.kind {
            PickupKind::Coin => {
                let r = cx.progress.add_coin();
                if r == CoinResult::GaveCharm {
                    remove_placed_charms(cx.world, cx.progress.charms, world_id);
                }
            }
            PickupKind::Charm => {
                if cx.progress.charms >= crate::game::progress::MAX_CHARMS {
                    // Thrown away instead (it stays in the world as a falling, collectable-later charm).
                    let frame = cx.world.player.and_then(|p| cx.world.world_transform(p));
                    let v = frame.map_or(CHARM_REJECT, |f| f.transform_vector(CHARM_REJECT));
                    self.throw(v, None);
                    return;
                }
                self.state = PickupState::Swirling(0.0);
            }
            PickupKind::Life => cx.progress.add_life(),
            PickupKind::Key => {
                if cx.progress.collect_key(world_id) {
                    cx.world.request(Request::Celebrate);
                    cx.world.request(Request::Autosave);
                }
                show_key_exits(cx.world);
            }
            PickupKind::Hourglass => {
                let t = cx.world.world_options.f32_or(WORLD_OPT_SPRINT_TIME, DEFAULT_SPRINT_TIME);
                cx.progress.start_sprint(world_id, t);
                remove_placed_charms(cx.world, crate::game::progress::MAX_CHARMS, world_id);
            }
        }
        if remember
            && placed
            && let Some(k) = cx.world.persist_key(cx.me)
        {
            cx.progress.level(world_id).collected.insert(k);
        }
        cx.raise(script::event::TRIGGERED, Vec::new());
        if self.state != PickupState::Swirling(0.0) {
            self.state = PickupState::Removed;
            cx.world.set_in_world(cx.me, false);
        }
    }

    fn fall(&mut self, cx: &mut Cx<'_>) {
        let PickupState::Falling { time, settled } = self.state else { return };
        let dt = cx.dt;
        let time = time + dt;
        if time >= FALL_LIMIT && !settled {
            self.state = PickupState::Removed;
            cx.world.set_in_world(cx.me, false);
            return;
        }
        let params = self.kind.params();
        self.velocity[2] += DROP_GRAVITY * dt;
        let from = cx.my_position();
        let to = madd(from, self.velocity, dt);
        let mut settled = settled;
        let mut pos = to;
        if let Some(hit) = cx.collision.sweep_sphere(cx.world, from, to, params.bounce_radius, cx.me) {
            // Reflect: the normal part reversed and scaled, the tangential part scaled.
            let n = hit.normal;
            let vn = scale(n, dot(self.velocity, n));
            let vt = sub(self.velocity, vn);
            self.velocity = add(scale(vn, -params.normal), scale(vt, params.tangential));
            pos = madd(from, sub(to, from), hit.t);
            if n[2] > FLOOR_Z {
                if !settled {
                    // The last bounce: a fixed upward speed, at most the maximum.
                    self.velocity[2] = params.last_bounce.0.min(params.last_bounce.1);
                    settled = true;
                } else if self.velocity[2].abs() < REST_SPEED {
                    self.velocity = [0.0; 3];
                    self.state = PickupState::Resting;
                    cx.world.set_world_transform_position(cx.me, pos);
                    return;
                }
            }
        }
        cx.world.set_world_transform_position(cx.me, pos);
        self.state = PickupState::Falling { time, settled };
    }
}

impl World {
    /// Moves an object so its world position is `p` (its rotation unchanged).
    pub fn set_world_transform_position(&mut self, h: Handle, p: [f32; 3]) {
        if let Some(mut w) = self.world_transform(h) {
            w.t = p;
            self.set_world_transform(h, w);
        }
    }
}

/// When the player holds two charms (or a sprint starts), the placed charms left are removed, except in world
/// `0x308` (pickups-and-progress.md 3.2).
fn remove_placed_charms(world: &mut World, charms: i32, world_id: u32) {
    if charms < crate::game::progress::MAX_CHARMS || world_id == CHARMS_RETURN_WORLD {
        return;
    }
    let slots: Vec<u32> = world.slots_of_class(id::CHARM).to_vec();
    for s in slots {
        let Some(h) = world.handle(s) else { continue };
        let resting = matches!(world.behavior(h), Some(super::Behavior::Pickup(p)) if p.state == PickupState::Resting);
        if resting && world.in_world(h) {
            world.set_in_world(h, false);
        }
    }
}

/// Key exits (exit option 169) appear once the level's key is collected (level-flow.md 2).
fn show_key_exits(world: &mut World) {
    let slots: Vec<u32> = world.slots_of_class(id::EXIT).to_vec();
    for s in slots {
        let Some(h) = world.handle(s) else { continue };
        if world.options(h).is_some_and(|o| o.bool_or(super::exit::opt::KEY_EXIT, false)) {
            world.set_in_world(h, true);
        }
    }
}

impl ClassBehavior for Pickup {
    fn ticks(&self) -> bool {
        self.state != PickupState::Removed
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        let world_id = cx.world.info.id;
        let collected = cx
            .world
            .persist_key(cx.me)
            .is_some_and(|k| cx.progress.level_ref(world_id).is_some_and(|l| l.collected.contains(&k)));
        let keep = match self.kind {
            _ if collected && !(self.kind == PickupKind::Charm && world_id == CHARMS_RETURN_WORLD) => false,
            PickupKind::Charm => {
                cx.progress.charms < crate::game::progress::MAX_CHARMS || world_id == CHARMS_RETURN_WORLD
            }
            PickupKind::Key => !cx.progress.has_flag(world_id, level_flag::KEY),
            PickupKind::Hourglass => {
                // The hourglass appears only in a level with a sprint goal, once its key and vault are done.
                let mask = cx.world.info.task_mask;
                let need = level_flag::KEY | level_flag::VAULT;
                mask & level_flag::SPRINT != 0
                    && cx.progress.level_ref(world_id).is_some_and(|l| l.flags & need == need)
                    && cx.progress.sprint.is_none()
            }
            _ => true,
        };
        if !keep {
            self.state = PickupState::Removed;
            cx.world.remove_quietly(cx.me);
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let hit = cx.world.take_inbox(cx.me.index()).iter().any(|n| matches!(n, Notice::Hit(_)));
        match self.state {
            PickupState::Removed => {}
            PickupState::Swirling(t) => {
                let t = t + cx.dt;
                if let Some(p) = cx.world.player.and_then(|p| cx.world.position(p)) {
                    cx.world.set_world_transform_position(cx.me, add(p, [0.0, 0.0, 100.0]));
                }
                if t >= CHARM_SWIRL + CHARM_STICK {
                    cx.progress.add_charm();
                    remove_placed_charms(cx.world, cx.progress.charms, cx.world.info.id);
                    self.state = PickupState::Removed;
                    cx.world.set_in_world(cx.me, false);
                } else {
                    self.state = PickupState::Swirling(t);
                }
            }
            _ => {
                if let Some(l) = self.lifetime.as_mut() {
                    *l -= cx.dt;
                    if *l <= 0.0 {
                        self.state = PickupState::Removed;
                        cx.world.set_in_world(cx.me, false);
                        return;
                    }
                }
                if self.wants_collect(cx, hit) {
                    self.collect(cx);
                    return;
                }
                self.fall(cx);
            }
        }
    }
}

/// Spin rate range of a clue bottle, radians per second, with a random sign (pickups-and-progress.md 4.1).
pub const BOTTLE_SPIN: (f32, f32) = (0.8, 1.2);
/// Reach of the player's stand-in when the bottle has no sphere option (the stand-in has no collision shape).
/// *Placeholder* for the free-camera development mode only.
pub const BOTTLE_TOUCH: f32 = 50.0;
/// Reward bit that pulls bottles (and coins) toward the player, and the pull's reach.
pub const PULL_REWARD: u32 = 0x20000;
/// See [`PULL_REWARD`].
pub const PULL_REACH: f32 = 300.0;

/// A clue bottle (pickups-and-progress.md 4.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Bottle {
    /// Its number in load order among the world's bottles.
    pub index: u32,
    /// Bottles in the world.
    pub total: u32,
    /// Spin rate about the vertical axis, radians per second.
    pub spin: f32,
    /// Touch radius (option 133, or the placeholder).
    pub radius: f32,
    /// Collected.
    pub collected: bool,
}

impl Bottle {
    /// A bottle; its number is its position among the world's class-46 objects.
    pub fn new(world: &World, h: Handle) -> Bottle {
        let list = world.slots_of_class(id::CLUE);
        let index = list.iter().position(|&s| s == h.index()).unwrap_or(0) as u32;
        let radius = world.options(h).and_then(|o| o.f32(crate::world::opt::SPHERE_RADIUS)).unwrap_or(BOTTLE_TOUCH);
        Bottle { index, total: list.len() as u32, spin: 0.0, radius, collected: false }
    }
}

impl ClassBehavior for Bottle {
    fn ticks(&self) -> bool {
        !self.collected
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        let world_id = cx.world.info.id;
        if cx.progress.level_ref(world_id).is_some_and(|l| self.index < 64 && l.bottles & (1 << self.index) != 0) {
            self.collected = true;
            cx.world.remove_quietly(cx.me);
            return;
        }
        let sign = if cx.rng.next_u32() & 1 == 0 { 1.0 } else { -1.0 };
        self.spin = sign * cx.rng.range(BOTTLE_SPIN.0, BOTTLE_SPIN.1);
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if self.collected {
            return;
        }
        if let Some(l) = cx.world.local(cx.me) {
            let r = crate::world::rotation_from_vector([0.0, 0.0, self.spin * cx.dt]);
            cx.world.set_rotation(cx.me, crate::math::mat_mul(&r, &l.cols));
        }
        let hit = cx
            .world
            .take_inbox(cx.me.index())
            .iter()
            .any(|n| matches!(n, Notice::Hit(_) | Notice::Touched(_) | Notice::Attack { .. }));
        // Collected on contact with its collision shape (a sphere with option 133); the stand-in uses the radius.
        let near = match cx.world.player_target() {
            Some(p) => cx.collision.sphere_touches(cx.world, p, cx.my_position(), self.radius).unwrap_or_else(|| {
                cx.world.position(p).is_some_and(|q| length(sub(q, cx.my_position())) <= self.radius)
            }),
            None => cx.collector_position().is_some_and(|c| length(sub(c, cx.my_position())) <= self.radius),
        };
        // With the pull power-up, bottles within 300 units drift toward the player.
        if !near
            && cx.world.shared.player.rewards & PULL_REWARD != 0
            && let Some(p) = cx.world.player_target().and_then(|p| cx.world.position(p))
        {
            let d = sub(p, cx.my_position());
            if length(d) <= PULL_REACH {
                let step = crate::math::scale(crate::math::normalize(d), (PULL_REACH * cx.dt).min(length(d)));
                cx.world.set_world_transform_position(cx.me, add(cx.my_position(), step));
            }
        }
        if !(hit || near) {
            return;
        }
        self.collected = true;
        let world_id = cx.world.info.id;
        let (first, all) = cx.progress.collect_bottle(world_id, self.index, self.total);
        if first {
            cx.world.request(Request::Dialogue(840));
        }
        if all {
            cx.world.request(Request::Dialogue(842));
        }
        cx.raise(script::event::TRIGGERED, Vec::new());
        cx.world.set_in_world(cx.me, false);
    }
}

impl World {
    /// Takes an object out of the world without raising event 5 (post-load adjustments: things already done).
    pub fn remove_quietly(&mut self, h: Handle) {
        if let Some(s) = self.slot(h) {
            self.clear_in_world_flag(s);
        }
    }
}
