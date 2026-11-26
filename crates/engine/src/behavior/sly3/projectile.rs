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

fn targets(cx: &Cx<'_>) -> Vec<(Handle, [f32; 3])> {
    cx.world
        .slots_of_class(TARGET_CLASS)
        .iter()
        .filter_map(|&s| cx.world.handle(s))
        .filter(|&h| cx.world.in_world(h) && cx.world.is_active(h) && !cx.world.shared.sly3_unhittable.contains(&h))
        .filter_map(|h| Some((h, cx.world.position(h)?)))
        .collect()
}

impl ClassBehavior for Projectile {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if !cx.world.in_world(cx.me) {
            self.age = 0.0;
            return;
        }
        let Some(v) = cx.world.options(cx.me).and_then(|o| o.vec3(VELOCITY)) else { return };
        if v == [0.0; 3] {
            return;
        }
        let Some(p) = cx.world.position(cx.me) else { return };
        let to = [p[0] + v[0] * cx.dt, p[1] + v[1] * cx.dt, p[2] + v[2] * cx.dt];
        cx.world.set_position(cx.me, to);
        self.age += cx.dt;
        let r2 = HIT_RADIUS * HIT_RADIUS;
        let hit = targets(cx).into_iter().find(|&(_, q)| {
            let d = [q[0] - to[0], q[1] - to[1], q[2] - to[2]];
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= r2
        });
        if let Some((h, _)) = hit {
            if let Some(slot) = cx.world.slot(h) {
                cx.world.shared.sly3_last_hit.insert(slot, cx.me);
            }
            cx.world.raise(h, HIT_EVENT, Vec::new());
        }
        if hit.is_some() || self.age >= LIFETIME {
            self.age = 0.0;
            cx.world.set_in_world(cx.me, false);
        }
    }
}
