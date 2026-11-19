//! SlyNX, the game application: the player's disc, the engine's world and loop, the renderer, on the Switch and the
//! PC.
//!
//! - [`level`]: a world's drawables on the renderer (one instance per drawn submesh, updated as objects move);
//! - [`session`]: the game itself, platform-independent: loading worlds, running frames, changing worlds;
//! - [`settings`]: the disc location and first world (Switch settings file, harness options);
//! - [`saving`]: the progress in the save slots (`save` crate), auto-saves, title continue;
//! - [`movie`]: movies through the `video` crate;
//! - [`sly2`]: the Sly 2 session (first stage: worlds, scripts, drawing, the free camera);
//! - [`sly3`]: the Sly 3 session (first stage: worlds, static drawing, the free camera, world sound);
//! - `pc` (PC only): the harness apps and the windowed game's set-up;
//! - the Switch entry point (`main`, Switch only).
//!
//! In worlds without Sly the free camera stands in for the player at exits, trigger volumes, checkpoints and
//! pickups. See `docs/engine.md`.
#![cfg_attr(target_os = "horizon", no_std)]

extern crate alloc;

pub mod frontend;
pub mod iris;
pub mod level;
pub mod movie;
pub mod saving;
pub mod session;
pub mod settings;
pub mod sly2;
pub mod sly3; // Sly 3 hook: the Sly 3 session (first stage).
pub mod smoke;
pub mod sound;
pub mod ui_layer;

#[cfg(not(target_os = "horizon"))]
pub mod pc;

#[cfg(all(
    target_os = "horizon",
    not(any(feature = "sly2-switch", feature = "sly3-switch", feature = "smoke-switch"))
))] // one entry per build
mod switch;
#[cfg(all(target_os = "horizon", feature = "sly2-switch"))] // Sly 2 hook: the Sly 2 Switch entry point
mod switch_sly2;
#[cfg(all(target_os = "horizon", feature = "sly3-switch", not(feature = "sly2-switch")))] // Sly 3 hook: its entry
mod switch_sly3;
#[cfg(all(
    target_os = "horizon",
    feature = "smoke-switch",
    not(any(feature = "sly2-switch", feature = "sly3-switch"))
))] // the smoke build
mod switch_smoke;

use engine::game::systems::FrameInput;

/// The controller state of player 1 as the engine reads it.
pub fn frame_input(input: &platform_api::InputState) -> FrameInput {
    let p = input.p1();
    FrameInput {
        buttons: p.held.0,
        pressed: p.pressed.0,
        released: p.released.0,
        left: [p.left.x, p.left.y],
        right: [p.right.x, p.right.y],
    }
}
