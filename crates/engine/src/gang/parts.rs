//! The gang's gameplay-state parts (specs/sly2/characters.md 9.3.1): objects in the characters' packages that are
//! in or out of the world by the character's state, for the renderer, which draws what is in the world.
//!
//! - **The canes**: exactly one of Sly's canes is in the world. 316 (with 317, its child) normally; 318 on a pipe
//!   (modes 2 and 11 are the pipe climb: state 8 after the 0.4167 s grip). The new cane takes the old one's pose when
//!   they swap. While Sly hides, all three are out.
//! - **The throw marker** (586, under each character): in the world only while that character aims a throw; its
//!   trail follows the predicted flight ([`marker_path`]).

use alloc::vec::Vec;

use super::Who;
use super::character::{Character, bentley, murray, sly};
use crate::math::Vec3;
use crate::object::Handle;
use crate::world::World;

/// Sly's first cane (the shaft; the hook 317 hangs below it).
pub const CANE: i32 = 316;
/// The first cane's hook.
pub const CANE_HOOK: i32 = 317;
/// Sly's second cane, used in the air and on climbs.
pub const CANE_AIR: i32 = 318;
/// The throw marker under each character.
pub const THROW_MARKER: i32 = 586;
/// Sly 2's hooks (the class Sly 3's 38 stands for; interactables.md 1).
pub const HOOK_CLASS: i16 = 40;

/// The pipe grip's length (clip 150): the first cane stays in hand until it ends (mode 10), then mode 2.
pub const PIPE_GRIP: f32 = 0.4167;

/// Whether Sly holds the second cane: modes 2 and 11 are the pipe climb (characters.md 9.3.1), so on a pipe (state 8)
/// once the 0.4167 s grip is over; never in ordinary jumps. (Mode 11, the pipe jump-off clip, is not told apart.)
pub fn air_cane(state: u8, state_time: f32) -> bool {
    state == sly::CLIMB && state_time >= PIPE_GRIP
}

/// Whether `c` is aiming a throw.
pub fn aiming(c: &Character) -> bool {
    let aim = match c.who {
        Who::Sly => sly::THROW_AIM,
        Who::Murray => murray::THROW_AIM,
        Who::Bentley => bentley::THROW_AIM,
    };
    c.state == aim
}

fn part(world: &World, character: Handle, id: i32) -> Option<Handle> {
    let below = |h: Handle| {
        let mut a = world.parent(h);
        while let Some(x) = a {
            if x == character {
                return true;
            }
            a = world.parent(x);
        }
        false
    };
    world.find_sly2_id(id, Some(character)).filter(|&h| below(h))
}

/// The character's parts (316, 317, 318, 586), looked up once and kept in `World::shared.gang_parts`.
pub fn parts_of(world: &mut World, character: Handle) -> [Option<Handle>; 4] {
    let key = character.index();
    if let Some(p) = world.shared.gang_parts.get(&key) {
        return *p;
    }
    let p = [CANE, CANE_HOOK, CANE_AIR, THROW_MARKER].map(|id| part(world, character, id));
    world.shared.gang_parts.insert(key, p);
    p
}

/// Puts the character's state parts in or out of the world for this frame. `hiding`: the character is in a hiding
/// spot. Returns the throw marker while it is in the world (its trail is [`marker_path`]).
pub fn update(world: &mut World, character: Handle, c: &Character, hiding: bool) -> Option<Handle> {
    if c.who == Who::Sly {
        let air = air_cane(c.state, c.state_time);
        let [shaft, hook, second, _] = parts_of(world, character);
        // The swap: the incoming cane takes the outgoing one's pose.
        let (out_cane, in_cane) = if air { (shaft, second) } else { (second, shaft) };
        if let (Some(o), Some(i)) = (out_cane, in_cane)
            && world.in_world(o)
            && !world.in_world(i)
            && let Some(pose) = world.world_transform(o)
        {
            world.set_world_transform(i, pose);
        }
        for (h, on) in [(shaft, !air && !hiding), (hook, !air && !hiding), (second, air && !hiding)] {
            if let Some(h) = h
                && world.in_world(h) != on
            {
                world.set_in_world(h, on);
            }
        }
    }
    let marker = parts_of(world, character)[3]?;
    let on = aiming(c) && !c.aim_path.is_empty();
    if world.in_world(marker) != on {
        world.set_in_world(marker, on);
    }
    on.then_some(marker)
}

/// The throw marker's trail: the predicted flight from the landing point back toward the launch (characters.md
/// 9.3.1), each point with the direction of flight there.
pub fn marker_path(c: &Character) -> Vec<(Vec3, Vec3)> {
    let p = &c.aim_path;
    (0..p.len())
        .rev()
        .map(|i| {
            let (a, b) = if i + 1 < p.len() {
                (p[i], p[i + 1])
            } else if i > 0 {
                (p[i - 1], p[i])
            } else {
                (p[i], p[i])
            };
            (p[i], crate::math::normalize(crate::math::sub(b, a)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cane_modes_and_marker_path() {
        assert!(air_cane(sly::CLIMB, 0.5) && !air_cane(sly::CLIMB, 0.2) && !air_cane(sly::AIR, 1.0));
        let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
        assert!(!aiming(&c));
        c.state = sly::THROW_AIM;
        assert!(aiming(&c));
        c.aim_path = alloc::vec![[0.0; 3], [100.0, 0.0, 50.0], [200.0, 0.0, 0.0]];
        let path = marker_path(&c);
        assert_eq!(path.first().map(|p| p.0), Some([200.0, 0.0, 0.0]), "from the landing point back");
        assert_eq!(path.len(), 3);
    }
}
