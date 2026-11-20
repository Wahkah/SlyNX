//! Sly 2's user interface (`specs/sly2/user-interface.md`): the fonts of screen object 884 in a glyph sheet
//! ([`font`]), the HUD ([`hud`]), the pause menu ([`pause`]) and the job screens ([`jobs`]), drawn into the
//! crate's [`crate::DrawList`] for the 2D layer. The screens' own vector pictures (classes 126 to 131,
//! `formats_sly2::screen`) are read but not played yet: SlyNX lays the screens out itself with the game's strings,
//! fonts and timings. See `docs/ui.md`.

pub mod font;
pub mod front;
pub mod grid;
pub mod hud;
pub mod jobs;
pub mod options;
pub mod pause;
pub mod shop;

pub use jobs::*;
