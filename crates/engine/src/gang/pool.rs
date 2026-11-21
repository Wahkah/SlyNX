//! Gadget copies on the world's objects (specs/sly2/gadgets.md 6): the copy a character takes from its pool (the
//! class-6 group under the character that holds the gadget templates) is moved out of the pool's group: to the
//! carrier while held (state 0), to the world root once thrown or placed (states 1 and 2), and back into the pool's
//! group, out of the world, when it returns (state 4 done). Each copy's carry state is kept in
//! `World::shared.gadget_copies`, so the renderer and the blast volumes can follow it.

use formats::geometry::Affine;

use crate::math::{Vec3, add, cos, sin};
use crate::object::{Handle, OptionValue};
use crate::world::World;

/// The gadget objects' classes (Sly 2): carryable objects (100), Bentley's bombs (101, `bentley_mine`), the trap
/// (102).
pub const GADGET_CLASSES: core::ops::RangeInclusive<i16> = 100..=102;
/// Carryable objects (the pocket items, TOM, the Alarm Clock).
pub const CARRYABLE_CLASS: i16 = 100;
/// Bentley's bombs.
pub const BOMB_CLASS: i16 = 101;
/// The class of the group the pool's templates hang in.
pub const GROUP_CLASS: i16 = 6;
/// The bomb's kind option (stream 1104): the item it is used as.
pub const KIND_OPTION: u16 = 1104;
/// Where a held copy sits in its carrier's frame (forward, left, up): at the hands [I].
pub const HAND: Vec3 = [30.0, 0.0, 100.0];

/// One copy out of its pool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GadgetCopy {
    /// The copy.
    pub object: Handle,
    /// The pool's group it came from.
    pub pool: Handle,
    /// The item it is used as.
    pub item: u8,
    /// Its carry state (0 held, 1 flying, 2 landed, 3 used, 4 fading, 5 the Health Extractor at work).
    pub state: u8,
    /// Time left in a triggered bomb's countdown (state 3), when one runs.
    pub countdown: Option<f32>,
}

/// A triggered bomb's countdown (the blink) before it goes off.
pub const BOMB_COUNTDOWN: f32 = 1.87;

/// The gadget class an item's copy comes from (gadgets.md 6): Bentley's Trigger Bomb (7) is a bomb; the pocket
/// items (26, 37, 38, 41) are carryable objects.
pub fn class_of_item(item: u8) -> Option<i16> {
    match item {
        7 => Some(BOMB_CLASS),
        26 | 37 | 38 | 41 => Some(CARRYABLE_CLASS),
        _ => None,
    }
}

/// The pool group under `character` holding copies of `class` (a class-6 child of the character).
pub fn pool_group(world: &World, character: Handle, class: i16) -> Option<Handle> {
    world
        .children(character)
        .find(|&g| world.class(g) == Some(GROUP_CLASS) && world.children(g).any(|c| world.class(c) == Some(class)))
}

/// Takes a copy of `item` from `character`'s pool: a copy of the item's class still in the pool's group moves to
/// the carrier's hands, enters the world and is given the item as its kind; its state is 0 (held). `None` when the
/// pool has none.
pub fn take(world: &mut World, character: Handle, item: u8) -> Option<Handle> {
    let class = class_of_item(item)?;
    let pool = pool_group(world, character, class)?;
    let copy = world.children(pool).find(|&c| world.class(c) == Some(class))?;
    world.reparent(copy, Some(character), false);
    let (s, c) = (sin(0.0), cos(0.0));
    world.set_local(copy, Affine { cols: [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]], t: HAND });
    world.set_in_world(copy, true);
    if let Some(o) = world.options_mut(copy) {
        o.set(KIND_OPTION, OptionValue::Int(i32::from(item)));
    }
    world.shared.gadget_copies.retain(|g| g.object != copy);
    world.shared.gadget_copies.push(GadgetCopy { object: copy, pool, item, state: 0, countdown: None });
    world.update_transforms();
    Some(copy)
}

/// The copy of `item` held or out, if any (the most recent).
pub fn copy_of(world: &World, item: u8) -> Option<GadgetCopy> {
    world.shared.gadget_copies.iter().rev().find(|g| g.item == item).copied()
}

/// Sets a copy's carry state.
pub fn set_state(world: &mut World, copy: Handle, state: u8) {
    if let Some(g) = world.shared.gadget_copies.iter_mut().find(|g| g.object == copy) {
        g.state = state;
    }
}

/// Throws or places a copy: it leaves its carrier for the world root at `at` (heading `yaw`), landed when
/// `landed` (state 2), else flying (state 1).
pub fn release(world: &mut World, copy: Handle, at: Vec3, yaw: f32, landed: bool) {
    world.reparent(copy, None, false);
    let (s, c) = (sin(yaw), cos(yaw));
    world.set_local(copy, Affine { cols: [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]], t: at });
    world.set_in_world(copy, true);
    set_state(world, copy, if landed { 2 } else { 1 });
    world.update_transforms();
}

/// Where a throw from `from` with `velocity` lands back at its start height (or after `max_time`), under `gravity`
/// (negative): the stand-in for the flight until the copy is simulated.
pub fn landing(from: Vec3, velocity: Vec3, gravity: f32, max_time: f32) -> Vec3 {
    let t = if gravity < 0.0 && velocity[2] > 0.0 { (-2.0 * velocity[2] / gravity).min(max_time) } else { 0.0 };
    add(from, [velocity[0] * t, velocity[1] * t, 0.0])
}

/// Triggers a copy: state 3, with the bomb's countdown before it goes off ([`step`]).
pub fn trigger(world: &mut World, copy: Handle) {
    if let Some(g) = world.shared.gadget_copies.iter_mut().find(|g| g.object == copy) {
        g.state = 3;
        g.countdown = Some(BOMB_COUNTDOWN);
    }
}

/// One step of the copies' countdowns: a bomb whose countdown ends goes off (a blast notice on the copy, which the
/// bomb behaviour turns into its blast) and returns to its pool. Returns the copies that went off.
pub fn step(world: &mut World, dt: f32) -> alloc::vec::Vec<Handle> {
    let mut done = alloc::vec::Vec::new();
    for g in &mut world.shared.gadget_copies {
        if let Some(t) = &mut g.countdown {
            *t -= dt;
            if *t <= 0.0 {
                done.push(g.object);
            }
        }
    }
    for &h in &done {
        world.notify(h, crate::world::Notice::Blast(h, [0.0; 3]));
        return_copy(world, h);
    }
    done
}

/// Returns a copy to its pool: back into the pool's group, out of the world (state 4 done).
pub fn return_copy(world: &mut World, copy: Handle) {
    let Some(g) = world.shared.gadget_copies.iter().find(|g| g.object == copy).copied() else { return };
    world.reparent(copy, Some(g.pool), false);
    world.set_local(copy, Affine::IDENTITY);
    world.set_in_world(copy, false);
    world.shared.gadget_copies.retain(|x| x.object != copy);
    world.update_transforms();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_constant() {
        assert_eq!(BOMB_COUNTDOWN, 1.87);
    }

    #[test]
    fn items_and_landing() {
        assert_eq!(class_of_item(7), Some(BOMB_CLASS));
        assert_eq!(class_of_item(41), Some(CARRYABLE_CLASS));
        assert_eq!(class_of_item(9), None);
        // 400 up at g = −2275 lands 0.3516 s later at the same height.
        let p = landing([0.0; 3], [100.0, 0.0, 400.0], -2275.0, 5.0);
        assert!((p[0] - 35.16).abs() < 0.01 && p[2] == 0.0, "{p:?}");
    }
}
