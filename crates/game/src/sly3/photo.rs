//! Sly 3's photo targets (class 43; specs/sly3/interactables.md 5a) on the shared picture-taking rules
//! ([`crate::photo`]): Sly 3's option numbers, its constants and which targets are eligible.
//!
//! - **Constants:** a 640 × 480 screen with the reticle at (320, 240), the eye 20 units above the character, and
//!   the camera's own field of view.
//! - **Eligible:** in the world, its linked object (1309) out of it, and its task (1308) available.
//! - **Opening:** R3 on the controlled character (Sly 2's rule), or command `0x46` with a scripted view looking at
//!   an object; `0x6E` closes it.

use alloc::vec::Vec;

use engine::world::World;

pub use crate::photo::{Accepted, Framing, Options, Photo, Target, View, state};

/// The photo target class.
pub const CLASS: i16 = 43;
/// The task event a picture accepted broadcasts.
pub const TASK_EVENT: u32 = 10;
/// Sly 3's option numbers (Sly 2's 1195 to 1206).
pub const OPTIONS: Options =
    Options { radius: 1302, size: 1303, tolerance: 1304, list_title: 1306, line: 1307, task: 1308, not_yet: 1312 };
/// The linked object (a class-120 waypoint), out of the world for the target to count.
pub const LINKED: u16 = 1309;
/// The screen and its reticle.
pub const SCREEN: [f32; 2] = [640.0, 480.0];
/// See [`SCREEN`].
pub const RETICLE: [f32; 2] = [320.0, 240.0];
/// The eye above the character.
pub const EYE_HEIGHT: f32 = 20.0;

/// Sly 3's constants with the camera's vertical field of view `fov_y`.
pub fn params(fov_y: f32) -> crate::photo::Params {
    crate::photo::Params { screen: SCREEN, reticle: RETICLE, eye_height: EYE_HEIGHT, fov_y }
}

/// The eligible targets of the world.
pub fn targets(world: &World) -> Vec<Target> {
    world
        .handles()
        .filter(|&h| world.class(h) == Some(CLASS))
        .filter(|&h| world.in_world(h))
        .filter(|&h| {
            let linked = world.options(h).and_then(|o| o.i32(LINKED)).filter(|&l| l >= 0);
            linked.and_then(|id| world.find_sly3_id(id, None)).is_none_or(|l| !world.in_world(l))
        })
        .map(|h| Target::new(world, h, &OPTIONS))
        .filter(|t| t.task.is_some_and(|k| world.shared.sly3_task_states.get(&k) == Some(&1)))
        .collect()
}
