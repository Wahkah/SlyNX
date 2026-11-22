//! Sly 2's photo targets (class 46; missions.md 4.3, specs/sly3/interactables.md 5a) on the shared picture-taking
//! rules ([`crate::photo`]): Sly 2's option numbers, its constants and which targets are eligible.
//!
//! - **Constants** (missions.md 4.3): no minimum distance (the nearest eligible target wins), the size divisor
//!   1/120, centring 1 − (d − 128)/128, the shutter mask 0x0A (0xFA when ready); a 640 × 492.8 screen with the
//!   reticle at its centre (the 246.4 vertical scale), the eye 20 units above the character, the camera's field of
//!   view.
//! - **Eligible:** in the world, and its task (1201) available (state 1).
//! - **Opening:** the binocucom (R3 on the controlled character); a good picture asks the target's task for state 2.

use alloc::vec::Vec;

use engine::world::World;

pub use crate::photo::{Accepted, Framing, Options, Photo, Target, View, state};

/// The photo target class.
pub const CLASS: i16 = 46;
/// Sly 2's option numbers (specs/sly3/interactables.md 5a: radius 1195, size 1196, tolerance 1197, list title 1199,
/// line 1200, task 1201; 1205, the "not yet" flag by its place in the list [I], is on three targets).
pub const OPTIONS: Options =
    Options { radius: 1195, size: 1196, tolerance: 1197, list_title: 1199, line: 1200, task: 1201, not_yet: 1205 };
/// The screen and its reticle.
pub const SCREEN: [f32; 2] = [640.0, 492.8];
/// See [`SCREEN`].
pub const RETICLE: [f32; 2] = [320.0, 246.4];
/// The eye above the character.
pub const EYE_HEIGHT: f32 = 20.0;
/// Task events of the binocucom, with no object (missions.md 4.3): opened (7; 12 in radio-talk mode), the photo
/// prompt entering state 0 (9), closed (8; 13 in talk mode), the prompt going to state 10 (11), a picture accepted
/// (10).
pub const BINOCUCOM_OPENED: u32 = 7;
/// See [`BINOCUCOM_OPENED`].
pub const PROMPT_STARTED: u32 = 9;
/// See [`BINOCUCOM_OPENED`].
pub const BINOCUCOM_CLOSED: u32 = 8;
/// See [`BINOCUCOM_OPENED`].
pub const PROMPT_ENDED: u32 = 11;
/// See [`BINOCUCOM_OPENED`].
pub const PICTURE_ACCEPTED: u32 = 10;

/// Sly 2's constants with the camera's vertical field of view `fov_y`.
pub fn params(fov_y: f32) -> crate::photo::Params {
    crate::photo::Params { screen: SCREEN, reticle: RETICLE, eye_height: EYE_HEIGHT, fov_y }
}

/// The eligible targets of the world: in it, with their task available.
pub fn targets(world: &World) -> Vec<Target> {
    let available = |id: i32| {
        world
            .sly2
            .as_deref()
            .and_then(|l| l.jobs.as_deref())
            .and_then(|j| j.task_state(id))
            .is_some_and(|s| s == engine::game::sly2_jobs::state::AVAILABLE)
    };
    world
        .handles()
        .filter(|&h| world.class(h) == Some(CLASS) && world.in_world(h))
        .map(|h| Target::new(world, h, &OPTIONS))
        .filter(|t| t.task.is_some_and(available))
        .collect()
}
