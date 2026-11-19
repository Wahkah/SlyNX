//! Breakables (classes 33 to 38) and their pieces (class 39) (interactables.md 4), and the rewards they drop
//! (pickups-and-progress.md 3.3 and 3.4).
//!
//! A breakable counts the attacks that reach it (and, for fragile ones, hard impacts); each counted hit raises event
//! 20, and at the hit count of option 227 it breaks: its pieces are handed to the physics with a random push,
//! pickups and clue bottles inside are freed, its explosion is set off, sound 42 plays, coins drop (option 233, or
//! the adaptive difficulty when negative), event 2 is raised and the breakable leaves the world unless option 216
//! keeps it. A broken breakable stays broken after a restart.

use alloc::vec::Vec;

use script::Value;

use super::{ClassBehavior, Cx};
use crate::math::{add, madd, sub};
use crate::object::Handle;
use crate::object::class::id;
use crate::world::{Notice, PhysicsCommand, World};

/// Breakable options (options-by-class.md 2.6, interactables.md 4).
pub mod opt {
    /// Keep the broken object in the world.
    pub const KEEP: u16 = 216;
    /// Whack factor (default 1).
    pub const WHACK: u16 = 217;
    /// Piece offset factor.
    pub const OFFSET_FACTOR: u16 = 219;
    /// Piece velocity range (± per axis).
    pub const VELOCITY: u16 = 220;
    /// Piece spin range (±).
    pub const SPIN: u16 = 221;
    /// Hits to break (default 1).
    pub const HITS: u16 = 227;
    /// Coins dropped: 0 none, positive that many, negative the difficulty decides.
    pub const COINS: u16 = 233;
    /// Coin offset.
    pub const COIN_OFFSET: u16 = 234;
    /// The break explosion (OID).
    pub const EXPLOSION: u16 = 240;
    /// Fragile: impact limit.
    pub const IMPACT_LIMIT: u16 = 242;
    /// Piece fade time.
    pub const PIECE_FADE: u16 = 247;
    /// Piece lifetime.
    pub const PIECE_LIFE: u16 = 248;
}

/// The break sound.
pub const BREAK_SOUND: i32 = 42;
/// Default impact limit of fragile breakables (options-by-class.md 2.6: "break speed thresholds (default 300)").
pub const FRAGILE_LIMIT: f32 = 300.0;
/// Default piece lifetime.
pub const PIECE_LIFE: f32 = 10.0;
/// Drop ring: horizontal and vertical speed ranges, and the stagger between pickups.
pub const DROP_HORIZONTAL: (f32, f32) = (75.0, 100.0);
/// See [`DROP_HORIZONTAL`].
pub const DROP_VERTICAL: (f32, f32) = (1000.0, 1300.0);

/// A breakable.
#[derive(Clone, Debug, PartialEq)]
pub struct Breakable {
    /// Hits counted.
    pub hits: i32,
    /// Broken.
    pub broken: bool,
    /// Fragile (class 37): breaks on hard impacts.
    pub fragile: bool,
}

/// A piece of a breakable (class 39): fades and disappears after it is released.
#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    /// Released by its breakable.
    pub released: bool,
    /// Time since release.
    pub age: f32,
}

/// Whether a breakable has broken.
pub fn is_broken(world: &World, h: Handle) -> bool {
    matches!(world.behavior(h), Some(super::Behavior::Breakable(b)) if b.broken)
}

/// The stock template of a class (object-stream.md 7.1), if the world has one.
pub fn template_of_class(world: &World, class: i16) -> Option<i16> {
    (12..=42).find(|&oid| world.template_record(oid).is_some_and(|r| world.level.record(r).class == class))
}

/// Throws `count` pickups of `class` out at `at` in a ring (pickups-and-progress.md 3.3).
pub fn throw_pickups(cx: &mut Cx<'_>, class: i16, count: i32, at: [f32; 3]) {
    let Some(t) = template_of_class(cx.world, class) else { return };
    let base = cx.rng.next_f32() * core::f32::consts::TAU;
    for k in 0..count.max(0) {
        let place = formats::geometry::Affine { cols: crate::math::MAT3_IDENTITY, t: at };
        let Some(h) = cx.world.spawn_template(t, place) else { return };
        cx.world.set_in_world(h, true);
        let a = base + k as f32 * core::f32::consts::TAU / count.max(1) as f32;
        let (s, c) = crate::math::sin_cos(a);
        let hs = cx.rng.range(DROP_HORIZONTAL.0, DROP_HORIZONTAL.1);
        let v = [c * hs, s * hs, cx.rng.range(DROP_VERTICAL.0, DROP_VERTICAL.1)];
        if let Some(super::Behavior::Pickup(p)) = cx.world.behavior_mut(h) {
            let life = p.kind.params().lifetime.map(|(lo, hi)| lo + (hi - lo) * 0.5);
            p.throw(v, life);
        }
    }
}

/// Drops the reward of a break or a knocked-out guard: `count` coins, or the adaptive difficulty's choice when
/// negative (pickups-and-progress.md 3.4).
pub fn drop_reward(cx: &mut Cx<'_>, count: i32, at: [f32; 3]) {
    if count > 0 {
        throw_pickups(cx, id::COIN, count, at);
        return;
    }
    if count == 0 {
        return;
    }
    let world_id = cx.world.info.id;
    let mask = cx.world.info.task_mask;
    let key = cx.progress.has_flag(world_id, crate::game::progress::level_flag::KEY);
    let Some(tables) = cx.progress.difficulty_tables.clone() else {
        throw_pickups(cx, id::COIN, 1, at);
        return;
    };
    let s = *tables.record(mask, key);
    let drop = cx.progress.level(world_id).difficulty.on_break(&s, &tables.rewards, cx.rng);
    if drop.life {
        throw_pickups(cx, id::LIFE, 1, at);
    } else {
        throw_pickups(cx, id::COIN, drop.coins, at);
    }
}

impl Breakable {
    /// A breakable of a class.
    pub fn new(class: i16) -> Breakable {
        Breakable { hits: 0, broken: false, fragile: class == 37 }
    }

    /// Breaks it (interactables.md 4.1). `quiet`: restored on load, with no effects.
    pub fn break_now(&mut self, cx: &mut Cx<'_>, quiet: bool) {
        if self.broken {
            return;
        }
        self.broken = true;
        let o = cx.world.options(cx.me).cloned().unwrap_or_default();
        let centre = cx.my_position();
        let children: Vec<Handle> = cx.world.children(cx.me).collect();
        for c in children {
            let class = cx.world.class(c).unwrap_or(0);
            match class {
                id::PIECE => {
                    if quiet {
                        cx.world.remove_quietly(c);
                        continue;
                    }
                    cx.world.reparent(c, None, true);
                    let v = o.f32_or(opt::VELOCITY, 0.0);
                    let spin = o.f32_or(opt::SPIN, 0.0);
                    let off = sub(cx.world.position(c).unwrap_or(centre), centre);
                    let k = cx.rng.range(0.8, 1.2) * o.f32_or(opt::OFFSET_FACTOR, 1.0);
                    let vel = madd([cx.rng.range(-v, v), cx.rng.range(-v, v), cx.rng.range(-v, v)], off, k);
                    let ang = [cx.rng.range(-spin, spin), cx.rng.range(-spin, spin), cx.rng.range(-spin, spin)];
                    cx.world.physics.push(PhysicsCommand::Dynamic {
                        object: c,
                        velocity: vel,
                        angular: ang,
                        radius: None,
                    });
                    if let Some(super::Behavior::Piece(p)) = cx.world.behavior_mut(c) {
                        p.released = true;
                    }
                }
                id::CLUE | id::COIN | id::CHARM | id::LIFE | id::KEY | id::HOURGLASS => {
                    // Freed and set upright.
                    cx.world.reparent(c, None, true);
                    if let Some(mut w) = cx.world.world_transform(c) {
                        w.cols = crate::math::MAT3_IDENTITY;
                        cx.world.set_world_transform(c, w);
                    }
                }
                _ => {}
            }
        }
        if !quiet {
            if let Some(e) = o.i16(opt::EXPLOSION).and_then(|e| cx.world.find_nearest(e, Some(cx.me))) {
                super::effects::set_off(cx.world, e, centre);
            }
            cx.world.play_sound(BREAK_SOUND, Some(cx.me));
            let at = add(centre, o.vec3(opt::COIN_OFFSET).unwrap_or([0.0; 3]));
            let default = if cx.world.class(cx.me) == Some(33) { -1 } else { 0 };
            drop_reward(cx, o.i32(opt::COINS).unwrap_or(default), at);
            cx.raise(script::event::TRIGGERED, Vec::new());
            let world_id = cx.world.info.id;
            if let Some(k) = cx.world.persist_key(cx.me) {
                cx.progress.level(world_id).broken.insert(k);
            }
        }
        if !o.bool_or(opt::KEEP, false) {
            if quiet {
                cx.world.remove_quietly(cx.me);
            } else {
                cx.world.set_in_world(cx.me, false);
            }
        }
    }

    fn hit(&mut self, cx: &mut Cx<'_>, by: Option<Handle>) {
        self.hits += 1;
        cx.raise(script::event::HIT, alloc::vec![Value::Int(self.hits)]);
        // A whack pushes it with the attacker's velocity times option 217.
        if let Some(b) = by
            && let Some(v) = cx.world.position(b).map(|p| sub(cx.my_position(), p))
        {
            let k = cx.world.options(cx.me).map_or(1.0, |o| o.f32_or(opt::WHACK, 1.0));
            let me = cx.my_position();
            cx.world.physics.push(PhysicsCommand::Impulse {
                object: cx.me,
                impulse: crate::math::scale(crate::math::normalize(v), k),
                point: me,
            });
        }
        let need = cx.world.options(cx.me).and_then(|o| o.i32(opt::HITS)).unwrap_or(1).max(1);
        if self.hits >= need {
            self.break_now(cx, false);
        }
    }
}

impl ClassBehavior for Breakable {
    fn ticks(&self) -> bool {
        !self.broken
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        let world_id = cx.world.info.id;
        if cx
            .world
            .persist_key(cx.me)
            .is_some_and(|k| cx.progress.level_ref(world_id).is_some_and(|l| l.broken.contains(&k)))
        {
            self.break_now(cx, true);
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let notices = cx.world.take_inbox(cx.me.index());
        for n in notices {
            if self.broken {
                break;
            }
            match n {
                Notice::Hit(by) | Notice::Attack { by, .. } | Notice::Blast(by, _) => self.hit(cx, Some(by)),
                Notice::Impact(_, speed) if self.fragile => {
                    let limit =
                        cx.world.options(cx.me).map_or(FRAGILE_LIMIT, |o| o.f32_or(opt::IMPACT_LIMIT, FRAGILE_LIMIT));
                    if speed > limit {
                        self.break_now(cx, false);
                    }
                }
                _ => {}
            }
        }
    }
}

impl ClassBehavior for Piece {
    fn ticks(&self) -> bool {
        self.released
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if !self.released {
            return;
        }
        self.age += cx.dt;
        let life = cx.world.options(cx.me).map_or(PIECE_LIFE, |o| o.f32_or(opt::PIECE_LIFE, PIECE_LIFE));
        if self.age >= life {
            self.released = false;
            cx.world.physics.push(PhysicsCommand::Release(cx.me));
            cx.world.set_in_world(cx.me, false);
        }
    }
}
