//! SlyNX level viewer: loads a Sly 1 world from the player's disc image and shows it with a free camera.
//!
//! - [`anim`]: the player character's animation, drawn with GPU skinning;
//! - [`look`]: a world's clear colour, fog, lights and the per-vertex lighting of shader kind 0;
//! - [`live`], [`texanim`]: backdrops, billboards and texture animation, each frame;
//! - [`level`]: a world's scene converted for the renderer (materials, passes, instances, spawn point);
//! - [`loader`]: reading the disc on the I/O thread, the disc location and settings;
//! - [`camera`]: the free camera and the controls;
//! - [`session`]: the viewer itself, platform-independent;
//! - `pc` (PC only): the harness app (`--app viewer`) and the windowed viewer's set-up;
//! - the Switch entry point (`main`, Switch only, feature `switch-app`).
//!
//! See `docs/viewer.md`.
#![cfg_attr(target_os = "horizon", no_std)]

extern crate alloc;

pub mod anim;
pub mod camera;
pub mod hud;
pub mod iris;
pub mod level;
pub mod live;
pub mod loader;
pub mod look;
pub mod session;
pub mod sly2;
pub mod sly2_light;
// Sly 3 hook: the Sly 3 path.
pub mod sly3;
pub mod texanim;

#[cfg(not(target_os = "horizon"))]
pub mod pc;

#[cfg(all(target_os = "horizon", feature = "switch-app"))]
mod switch;

use alloc::string::String;
use formats::disc::WorldEntry;

/// Finds a world by table index (`0`..`45`), id (`0x103`), full name, or a case-insensitive part of the name
/// (`paris`, `hideout`).
pub fn world_index(worlds: &[WorldEntry], which: &str) -> Option<usize> {
    let which = which.trim();
    if let Some(hex) = which.strip_prefix("0x")
        && let Ok(id) = u32::from_str_radix(hex, 16)
    {
        return worlds.iter().position(|w| w.id == id);
    }
    let names: alloc::vec::Vec<&str> = worlds.iter().map(|w| w.name.as_str()).collect();
    find_by_name(&names, which)
}

/// Finds a name by index, exact name (ignoring case), or a case-insensitive part of a name. A name that starts
/// with the text wins over one that only contains it.
pub fn find_by_name<S: AsRef<str>>(names: &[S], which: &str) -> Option<usize> {
    let which = which.trim();
    if let Ok(i) = which.parse::<usize>() {
        return (i < names.len()).then_some(i);
    }
    if which.is_empty() {
        return None;
    }
    let want = which.to_ascii_lowercase();
    let lower: alloc::vec::Vec<String> = names.iter().map(|n| n.as_ref().to_ascii_lowercase()).collect();
    lower
        .iter()
        .position(|n| *n == want)
        .or_else(|| lower.iter().position(|n| n.starts_with(&want)))
        .or_else(|| lower.iter().position(|n| n.contains(&want)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        let names = ["Splash", "Paris, France", "The Hideout", "A Rocky Start"];
        assert_eq!(find_by_name(&names, "paris"), Some(1));
        assert_eq!(find_by_name(&names, "hideout"), Some(2));
        assert_eq!(find_by_name(&names, "THE HIDEOUT"), Some(2));
        assert_eq!(find_by_name(&names, "a"), Some(3)); // starts with beats contains
        assert_eq!(find_by_name(&names, "3"), Some(3));
        assert_eq!(find_by_name(&names, "9"), None);
        assert_eq!(find_by_name(&names, "nowhere"), None);
    }
}
