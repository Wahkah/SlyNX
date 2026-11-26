//! Sly 3 projectiles (class 89; missions.md 4.1a, the canal chase's missiles 150052).
//!
//! - A pooled instance taken by script ID 825 is launched by its script: option 64 (stream 30) is its velocity in
//!   the world, option 549 (stream 439) the damage it carries.
//! - Each frame it moves by that velocity. Stand-in, a working choice [I] (the flight's end and its hit test are not
//!   traced): it hits the first active reactive object (class 111, the enemy boats) within [`HIT_RADIUS`], or ends
//!   after [`LIFETIME`]; either way it leaves the world, back to its pool. Objects in
//!   `World::shared.sly3_unhittable` (a defeated enemy boat, stand-in) are passed through.
//! - A hit gives the target script event 19, the event the enemy boats' module answers, with the projectile as the
//!   target's last hitter (script ID 1105, stream 1126; `World::shared.sly3_last_hit`), whose damage the module
//!   reads as 549 [I].

use alloc::vec::Vec;

use crate::behavior::{ClassBehavior, Cx};
use crate::object::Handle;
use crate::world::World;

/// The projectile class.
pub const CLASS: i16 = 89;
/// The class it can hit (reactive objects: the enemy boats).
pub const TARGET_CLASS: i16 = 111;
/// Velocity (script ID 64).
const VELOCITY: u16 = 30;
/// How near a target a hit lands (working value [I]).
pub const HIT_RADIUS: f32 = 300.0;
/// How long a flight lasts at most, seconds (working value [I]).
pub const LIFETIME: f32 = 2.0;
/// The hit event.
pub const HIT_EVENT: u32 = 19;

/// A projectile.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Projectile {
    /// Seconds since it came into the world.
    pub age: f32,
}

fn targets(world: &World) -> Vec<(Handle, [f32; 3])> {
    world
        .slots_of_class(TARGET_CLASS)
        .iter()
        .filter_map(|&s| world.handle(s))
        .filter(|&h| world.in_world(h) && world.is_active(h) && !world.shared.sly3_unhittable.contains(&h))
        .filter_map(|h| Some((h, world.position(h)?)))
        .collect()
}

impl Projectile {
    /// One step of `dt` seconds for projectile `me`: the flight, the hit test and the end.
    pub fn step(&mut self, world: &mut World, me: Handle, dt: f32) {
        if !world.in_world(me) {
            self.age = 0.0;
            return;
        }
        let Some(v) = world.options(me).and_then(|o| o.vec3(VELOCITY)) else { return };
        if v == [0.0; 3] {
            return;
        }
        let Some(p) = world.position(me) else { return };
        let to = [p[0] + v[0] * dt, p[1] + v[1] * dt, p[2] + v[2] * dt];
        world.set_position(me, to);
        self.age += dt;
        let r2 = HIT_RADIUS * HIT_RADIUS;
        let hit = targets(world).into_iter().find(|&(_, q)| {
            let d = [q[0] - to[0], q[1] - to[1], q[2] - to[2]];
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= r2
        });
        if let Some((h, _)) = hit {
            if let Some(slot) = world.slot(h) {
                world.shared.sly3_last_hit.insert(slot, me);
            }
            world.raise(h, HIT_EVENT, Vec::new());
        }
        if hit.is_some() || self.age >= LIFETIME {
            self.age = 0.0;
            world.set_in_world(me, false);
        }
    }
}

impl ClassBehavior for Projectile {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        self.step(cx.world, cx.me, cx.dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::object::OptionValue;
    use crate::world::WorldInfo;
    use crate::world::tests::fixture::{Fixture, at};

    const DT: f32 = 1.0 / 60.0;

    /// A projectile at the origin flying along x at `speed`, and a target at `target` x.
    fn world(speed: f32, target: f32) -> (World, Handle, Handle) {
        let mut f = Fixture::new();
        f.top(CLASS, 50, at(0.0, 0.0, 0.0));
        f.top(TARGET_CLASS, 51, at(target, 0.0, 0.0));
        let mut w = f.world(WorldInfo::default());
        let hs: Vec<Handle> = w.handles().collect();
        let (p, t) = (hs[0], hs[1]);
        w.options_mut(p).unwrap().set(VELOCITY, OptionValue::Vector([speed, 0.0, 0.0]));
        w.update_transforms();
        (w, p, t)
    }

    #[test]
    fn flies_by_its_velocity() {
        let (mut w, p, _) = world(600.0, 100_000.0);
        let mut b = Projectile::default();
        for _ in 0..30 {
            b.step(&mut w, p, DT);
            w.update_transforms();
        }
        let x = w.position(p).unwrap()[0];
        assert!((x - 300.0).abs() < 0.5, "half a second at 600: {x}");
        assert!(w.in_world(p));
    }

    #[test]
    fn ends_after_its_lifetime() {
        let (mut w, p, _) = world(600.0, 100_000.0);
        let mut b = Projectile::default();
        for _ in 0..(LIFETIME / DT) as usize + 2 {
            b.step(&mut w, p, DT);
            w.update_transforms();
        }
        assert!(!w.in_world(p), "out of the world after {LIFETIME} s");
        assert_eq!(b.age, 0.0, "ready for the pool");
    }

    #[test]
    fn hits_the_first_target_in_reach_once() {
        let (mut w, p, t) = world(6000.0, 1000.0);
        let mut b = Projectile::default();
        for _ in 0..20 {
            b.step(&mut w, p, DT);
            w.update_transforms();
        }
        assert!(!w.in_world(p), "the hit ends the flight");
        assert_eq!(w.shared.sly3_last_hit.get(&w.slot(t).unwrap()), Some(&p), "the target's last hitter");
        // An unhittable target is passed through.
        let (mut w, p, t) = world(6000.0, 1000.0);
        w.shared.sly3_unhittable.push(t);
        let mut b = Projectile::default();
        for _ in 0..20 {
            b.step(&mut w, p, DT);
            w.update_transforms();
        }
        assert!(w.shared.sly3_last_hit.is_empty());
    }
}
