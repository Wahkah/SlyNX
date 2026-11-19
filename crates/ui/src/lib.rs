//! Sly 1's user interface: the fonts and their sheet, the text markup and layout, the text tables read from the
//! executable, the HUD, the binocucom overlay and the menus. Everything draws into a backend-neutral
//! [`DrawList`] of quads that the renderer's overlay pass draws.
//!
//! Written from `specs/sly1/ui-and-menus.md`, `hud.md` and `textures.md`. `no_std` with `alloc`, no third-party
//! crates.
//!
//! | Module | What it does |
//! |---|---|
//! | [`font`] | font records, glyphs, decoding and packing the glyph sheet |
//! | [`text`] | markup tokens, layout (wrap, align, scale, colour), the typewriter |
//! | [`tables`] | prompt, response and name tables, the HUD placement table, `sprintf` |
//! | [`hud`] | element placement and slides, counters, the timer, the auto-save icon |
//! | [`binoc`] | the binocucom overlay model |
//! | [`nav`] | menu input and held-direction repeat |
//! | [`menu`] | title, pause, options, slots, prompts, hideout select, map screen |
//! | [`draw`] | the draw list |
//! | [`raster`] | a CPU rasterizer for draw lists (tests, captures) |
//! | [`sly2`] | Sly 2's job screens: job complete, saving, job help |

#![no_std]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod binoc;
pub mod draw;
pub mod font;
pub mod hud;
pub mod menu;
pub mod nav;
pub mod raster;
pub mod sly2; // Sly 2 hook: the job screens (job complete, saving, job help).
pub mod tables;
pub mod text;

pub use draw::{Canvas, DrawList, Quad, Rect, Texture};
pub use font::FontSet;
pub use hud::{Hud, HudValues};
pub use menu::{Action, Menus};
pub use nav::MenuInput;
pub use tables::TextTables;

use core::fmt;

/// Errors from reading fonts and tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The executable is not the supported one (SHA-256 mismatch): no table is read by address.
    UnknownExecutable,
    /// Data that breaks a rule.
    Invalid(&'static str),
    /// From the disc and world readers.
    Formats(formats::error::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnknownExecutable => write!(f, "unknown executable: tables are only read from the supported one"),
            Error::Invalid(w) => write!(f, "invalid data: {w}"),
            Error::Formats(e) => write!(f, "{e}"),
        }
    }
}

impl From<formats::error::Error> for Error {
    fn from(e: formats::error::Error) -> Self {
        Error::Formats(e)
    }
}

/// Largest integer not above `x` (no `std` float functions on the Switch target).
pub(crate) fn floor(x: f32) -> f32 {
    floor64(x as f64) as f32
}

/// Largest integer not above `x`.
pub(crate) fn floor64(x: f64) -> f64 {
    if x.is_nan() || x.abs() >= 4.0e18 {
        return x;
    }
    let t = x as i64 as f64;
    if t > x { t - 1.0 } else { t }
}

/// Square root (Newton's method).
pub(crate) fn sqrt(x: f32) -> f32 {
    if x <= 0.0 || x.is_nan() {
        return 0.0;
    }
    let x = x as f64;
    let mut y = f64::from_bits((x.to_bits() >> 1) + (0x3FF0_0000_0000_0000 >> 1));
    for _ in 0..5 {
        y = 0.5 * (y + x / y);
    }
    y as f32
}

#[cfg(test)]
mod tests {
    #[test]
    fn math() {
        assert_eq!(super::floor(-1.5), -2.0);
        assert_eq!(super::floor(2.0), 2.0);
        assert!((super::sqrt(2.0) - core::f32::consts::SQRT_2).abs() < 1e-6);
        assert_eq!(super::sqrt(-1.0), 0.0);
    }
}
