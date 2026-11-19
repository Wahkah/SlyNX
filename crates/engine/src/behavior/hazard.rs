//! Hazards, water, barriers, conveyors, hiding baskets, bombs and mines (interactables.md 5 to 9).

use alloc::vec;
use alloc::vec::Vec;

use script::Value;

use super::{ClassBehavior, Cx, dist_sq};
use crate::math::{add, length, normalize, scale, sub};
use crate::object::{Arg, Handle, OptionValue};
use crate::world::shared::damage;
use crate::world::{Notice, PhysicsCommand, World};

/// Hazard options.
pub mod opt {
    /// Damage kind.
    pub const DAMAGE_KIND: u16 = 609;
    /// Recovery throw height (default 150).
    pub const THROW_HEIGHT: u16 = 208;
    /// Place the player on the recovery target at once.
    pub const PLACE: u16 = 209;
    /// Water current.
    pub const CURRENT: u16 = 203;
    /// Barrier: listed objects (OIDs).
    pub const BARRIER_OBJECTS: u16 = 344;
    /// Barrier: listed classes.
    pub const BARRIER_CLASSES: u16 = 345;
    /// Barrier: only the listed ones pass.
    pub const BARRIER_ONLY: u16 = 346;
    /// Barrier: the warp to return objects to.
    pub const BARRIER_WARP: u16 = 347;
    /// Conveyor belt speed.
    pub const BELT_SPEED: u16 = 625;
    /// Bomb: detonation radius against objects.
    pub const BOMB_RADIUS: u16 = 712;
    /// Bomb: detonation radius against the player.
    pub const BOMB_PLAYER_RADIUS: u16 = 713;
    /// Bomb: damage kind.
    pub const BOMB_DAMAGE: u16 = 715;
}

/// Default throw height after hazard damage.
pub const THROW_HEIGHT: f32 = 150.0;
/// The player's stand-in (the probe, which has no collision mesh) as a sphere: radius, and height of its centre
/// above the origin (pickups-and-progress.md 2: it covers the player's standing shape). Hazards have no reach and
/// no repeat timer of their own (interactables.md 5): only the player's immunity stops repeated damage.
pub const STAND_IN: (f32, f32) = (60.0, 15.0);
/// Splash sound and the downward speed that makes one.
pub const SPLASH_SOUND: i32 = 34;
/// See [`SPLASH_SOUND`].
pub const SPLASH_SPEED: f32 = 100.0;
/// Default conveyor speed.
pub const BELT_SPEED: f32 = 100.0;
/// Bomb defaults (interactables.md 6): radius, player radius, push speed, mass limit, delay, primed delay.
pub const BOMB_RADIUS: f32 = 200.0;
/// See [`BOMB_RADIUS`].
pub const BOMB_PUSH: f32 = 800.0;
/// See [`BOMB_RADIUS`].
pub const BOMB_MASS_LIMIT: f32 = 5000.0;
/// See [`BOMB_RADIUS`].
pub const BOMB_DELAY: f32 = 1.0;
/// See [`BOMB_RADIUS`].
pub const BOMB_PRIMED: f32 = 0.1;
/// The blast's push direction is raised by this height.
pub const BOMB_LIFT: f32 = 200.0;

/// The player's test position and object (`None`: the stand-in).
fn player(cx: &Cx<'_>) -> Option<(Option<Handle>, [f32; 3])> {
    match cx.world.player_target() {
        Some(p) => Some((Some(p), cx.world.position(p)?)),
        None => cx.world.probe.map(|p| (None, p)),
    }
}

/// The best recovery point of a hazard for a position: the nearest of its child points and curves' points.
fn recovery_point(world: &World, me: Handle, near: [f32; 3]) -> Option<[f32; 3]> {
    let mut pts: Vec<[f32; 3]> = Vec::new();
    for c in world.children(me) {
        match world.class(c) {
            Some(126..=129) => pts.extend(world.position(c)),
            Some(110..=113) => pts.extend(super::sensor::curve_points(world, c)),
            _ => {}
        }
    }
    pts.into_iter().min_by(|a, b| dist_sq(*a, near).total_cmp(&dist_sq(*b, near)))
}

/// A hazard: solid (class 54) or volume (class 55).
#[derive(Clone, Debug, PartialEq)]
pub struct Hazard {
    /// Volume hazard (55) rather than solid (54).
    pub volume: bool,
}

impl Hazard {
    /// A hazard of a class.
    pub fn new(class: i16) -> Hazard {
        Hazard { volume: class == 55 }
    }
}

impl ClassBehavior for Hazard {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let notices = cx.world.take_inbox(cx.me.index());
        let Some((who, p)) = player(cx) else { return };
        let kind = cx.world.options(cx.me).and_then(|o| o.i32(opt::DAMAGE_KIND)).unwrap_or(0) as u8;
        let touching = match (self.volume, who) {
            (false, Some(h)) => notices.iter().any(|n| matches!(n, Notice::Touched(t) if *t == h)),
            (false, None) => {
                let c = [p[0], p[1], p[2] + STAND_IN.1];
                cx.collision.sphere_touches(cx.world, cx.me, c, STAND_IN.0).unwrap_or(false)
            }
            (true, _) => {
                let inside = cx.collision.point_in_solid(cx.world, cx.me, p).unwrap_or(false);
                // A pit fires only while the player falls.
                inside && (kind != damage::PIT || cx.world.shared.player.velocity[2] <= 0.0)
            }
        };
        if !touching || cx.world.shared.player.invulnerable {
            return;
        }
        let o = cx.world.options(cx.me).cloned().unwrap_or_default();
        let recover = recovery_point(cx.world, cx.me, p).map(|r| {
            if o.bool_or(opt::PLACE, false) { r } else { add(r, [0.0, 0.0, o.f32_or(opt::THROW_HEIGHT, THROW_HEIGHT)]) }
        });
        cx.world.hurt_player(cx.me, kind, recover);
        cx.raise(script::event::TRIGGERED, Vec::new());
    }
}

/// Water (class 31): the player falling in loses a life; entering fast makes a splash.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Water {
    /// The player is in the water.
    pub player_inside: bool,
}

impl ClassBehavior for Water {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let Some((_, p)) = player(cx) else { return };
        let inside = cx.collision.point_in_solid(cx.world, cx.me, p).unwrap_or(false);
        if inside && !self.player_inside {
            if -cx.world.shared.player.velocity[2] > SPLASH_SPEED {
                cx.world.play_sound(SPLASH_SOUND, Some(cx.me));
            }
            if !cx.world.shared.player.invulnerable {
                cx.world.hurt_player(cx.me, damage::WATER, None);
            }
        }
        self.player_inside = inside;
    }
}

/// A barrier (class 52): blocks listed objects (or all but them) and returns what hits it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Barrier;

impl Barrier {
    /// Whether the barrier blocks an object (interactables.md 8).
    pub fn blocks(world: &World, me: Handle, h: Handle) -> bool {
        let Some(o) = world.options(me) else { return true };
        let oid = world.oid(h).unwrap_or(-1);
        let class = world.class(h).unwrap_or(-1);
        let listed = o.shorts(opt::BARRIER_OBJECTS).take(4).any(|x| x == oid)
            || o.shorts(opt::BARRIER_CLASSES).take(4).any(|x| x == class);
        if o.bool_or(opt::BARRIER_ONLY, false) { !listed } else { listed }
    }
}

impl ClassBehavior for Barrier {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let notices = cx.world.take_inbox(cx.me.index());
        let mut done: Vec<Handle> = Vec::new();
        for n in notices {
            let Notice::Touched(h) = n else { continue };
            if done.contains(&h) || !Barrier::blocks(cx.world, cx.me, h) {
                continue;
            }
            done.push(h);
            // Return it to the child point or the warp, velocity cleared.
            let target = cx
                .world
                .children(cx.me)
                .find(|&c| matches!(cx.world.class(c), Some(126..=129)))
                .and_then(|c| cx.world.position(c))
                .or_else(|| {
                    let w = cx.world.options(cx.me)?.i16(opt::BARRIER_WARP)?;
                    cx.world.position(cx.world.find_nearest(w, Some(cx.me))?)
                });
            if let Some(t) = target {
                cx.world.set_world_transform_position(h, t);
                cx.world.physics.push(PhysicsCommand::Dynamic {
                    object: h,
                    velocity: [0.0; 3],
                    angular: [0.0; 3],
                    radius: None,
                });
            }
            cx.raise(script::event::TRIGGERED, Vec::new());
            cx.raise(script::event::BARRIER_HIT, vec![Value::Object(Some(h.to_script()))]);
        }
    }
}

/// A conveyor (class 57).
#[derive(Clone, Debug, PartialEq)]
pub struct Conveyor {
    /// Belt speed.
    pub speed: f32,
    /// Texture scroll per unit of belt speed (the two floats after the solid layout).
    pub scroll: [f32; 2],
}

impl Conveyor {
    /// A conveyor; its scroll factors are read from the record.
    pub fn new(world: &World, h: Handle) -> Conveyor {
        let scroll = world
            .record(h)
            .map(|r| world.level.record(r).end)
            .and_then(|end| {
                let b = world.level.bytes.get(end.checked_sub(8)?..end)?;
                Some([f32::from_le_bytes([b[0], b[1], b[2], b[3]]), f32::from_le_bytes([b[4], b[5], b[6], b[7]])])
            })
            .unwrap_or([0.0; 2]);
        let speed = world.options(h).map_or(BELT_SPEED, |o| o.f32_or(opt::BELT_SPEED, BELT_SPEED));
        Conveyor { speed, scroll }
    }

    /// The velocity the belt gives a contact with normal `n` (interactables.md 9): the belt speed along the
    /// direction perpendicular to both the conveyor's local Y axis and the normal.
    pub fn carry(world: &World, me: Handle, speed: f32, n: [f32; 3]) -> [f32; 3] {
        let Some(w) = world.world_transform(me) else { return [0.0; 3] };
        scale(normalize(crate::math::cross(w.cols[1], n)), speed)
    }

    /// The texture scroll velocity (u, v): −speed × each factor.
    pub fn uv_scroll(&self) -> [f32; 2] {
        [-self.speed * self.scroll[0], -self.speed * self.scroll[1]]
    }
}

impl ClassBehavior for Conveyor {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let notices = cx.world.take_inbox(cx.me.index());
        for n in notices {
            if let Notice::Touched(h) = n
                && let Some(p) = cx.world.position(h)
            {
                let v = Conveyor::carry(cx.world, cx.me, self.speed, [0.0, 0.0, 1.0]);
                cx.world.set_world_transform_position(h, crate::math::madd(p, v, cx.dt));
            }
        }
    }

    fn option_changed(&mut self, _world: &mut World, _me: Handle, id: u16, value: &OptionValue) {
        if id == opt::BELT_SPEED {
            self.speed = value.as_f32().unwrap_or(BELT_SPEED);
        }
    }
}

/// A hiding basket (class 58): the player system puts the player in it (`World::shared.basket`); it follows the
/// player and springs back upright when free.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Basket {
    /// The player is inside.
    pub occupied: bool,
}

impl ClassBehavior for Basket {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let inside = cx.world.shared.basket == Some(cx.me);
        if inside != self.occupied {
            self.occupied = inside;
            cx.raise(if inside { script::event::TRIGGERED } else { script::event::UNTRIGGERED }, Vec::new());
        }
        if inside {
            if let Some(p) = cx.world.player_target().and_then(|p| cx.world.world_transform(p)) {
                cx.world.set_world_transform(cx.me, p);
            }
            if cx.world.notices(cx.me).iter().any(|n| matches!(n, Notice::Hit(_))) {
                cx.raise(script::event::HIDING_DAMAGE, vec![Value::Int(0)]);
            }
        } else if let Some(mut w) = cx.world.world_transform(cx.me) {
            // Spring back upright (rotation spring 3.0).
            let up = w.cols[2];
            let tilt = crate::math::cross(up, [0.0, 0.0, 1.0]);
            if length(tilt) > 1e-4 {
                let r = crate::world::rotation_from_vector(scale(tilt, (3.0 * cx.dt).min(1.0)));
                w.cols = crate::math::mat_mul(&r, &w.cols);
                cx.world.set_world_transform(cx.me, w);
            }
        }
        cx.world.take_inbox(cx.me.index());
    }
}

/// A bomb, missile or mine (classes 59 to 63, interactables.md 6).
#[derive(Clone, Debug, PartialEq)]
pub struct Bomb {
    /// Seconds until it goes off, once lit.
    pub fuse: Option<f32>,
    /// Gone off.
    pub detonated: bool,
}

impl Bomb {
    /// An unlit bomb.
    pub fn new() -> Bomb {
        Bomb { fuse: None, detonated: false }
    }

    /// Lights the fuse (a thrown bomb, a script).
    pub fn light(&mut self, seconds: f32) {
        if self.fuse.is_none_or(|f| f > seconds) {
            self.fuse = Some(seconds);
        }
    }

    /// Goes off: pushes and hits every solid in range, hurts the player in range, primes other bombs.
    pub fn detonate(&mut self, cx: &mut Cx<'_>) {
        if self.detonated {
            return;
        }
        self.detonated = true;
        let o = cx.world.options(cx.me).cloned().unwrap_or_default();
        let r = o.f32_or(opt::BOMB_RADIUS, BOMB_RADIUS);
        let rp = o.f32_or(opt::BOMB_PLAYER_RADIUS, BOMB_RADIUS);
        let c = cx.my_position();
        let mut hit = Vec::new();
        if !cx.collision.overlap_sphere(cx.world, c, r, &mut hit) {
            hit = cx
                .world
                .handles()
                .filter(|&h| h != cx.me && cx.world.position(h).is_some_and(|p| dist_sq(p, c) <= r * r))
                .filter(|&h| cx.world.class(h).and_then(crate::object::class_info).is_some_and(|i| i.is_solid()))
                .collect();
        }
        for h in hit {
            if h == cx.me || Some(h) == cx.world.player_target() {
                continue;
            }
            let p = cx.world.position(h).unwrap_or(c);
            let d = sub(p, add(c, [0.0, 0.0, -BOMB_LIFT]));
            let closeness = (1.0 - length(sub(p, c)) / r).clamp(0.0, 1.0);
            let push = scale(normalize(d), BOMB_PUSH * closeness);
            cx.world.notify(h, Notice::Blast(cx.me, push));
            if matches!(cx.world.behavior(h), Some(super::Behavior::Bomb(_))) {
                cx.world.notify(h, Notice::Blast(cx.me, [0.0; 3]));
            }
            cx.world.physics.push(PhysicsCommand::Impulse { object: h, impulse: push, point: p });
        }
        if let Some((_, p)) = player(cx)
            && dist_sq(p, c) <= rp * rp
        {
            let kind = o.i32(opt::BOMB_DAMAGE).unwrap_or(0) as u8;
            cx.world.hurt_player(cx.me, kind, None);
        }
        let effects: Vec<Handle> =
            cx.world.children(cx.me).filter(|&h| matches!(cx.world.class(h), Some(120..=122))).collect();
        for e in effects {
            super::effects::set_off(cx.world, e, c);
        }
        cx.world.physics.push(PhysicsCommand::Release(cx.me));
        cx.world.set_in_world(cx.me, false);
    }
}

impl Default for Bomb {
    fn default() -> Self {
        Bomb::new()
    }
}

impl ClassBehavior for Bomb {
    fn ticks(&self) -> bool {
        !self.detonated
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        for n in cx.world.take_inbox(cx.me.index()) {
            match n {
                Notice::Blast(..) => self.light(BOMB_PRIMED),
                Notice::Impact(..) => self.light(0.0),
                Notice::Hit(_) | Notice::Attack { .. } => self.light(BOMB_DELAY),
                _ => {}
            }
        }
        if let Some(f) = self.fuse.as_mut() {
            *f -= cx.dt;
            if *f <= 0.0 {
                self.detonate(cx);
            }
        }
    }

    fn action(&mut self, _world: &mut World, _me: Handle, _id: u16, _args: &[Arg]) -> bool {
        // Script actions on bombs (detonate, light) are not described; the spec's triggers are hits and blasts.
        false
    }
}

/// Classes handled here.
pub fn is_bomb_class(class: i16) -> bool {
    (59..=63).contains(&class)
}
