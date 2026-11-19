//! The SlyNX packager: turns the player's own disc image and the SlyNX program into a Switch homebrew package, on
//! the player's machine. Nothing derived from the game is part of this crate or its output unless the player supplies
//! the disc.
//!
//! Everything is written in Rust, with no devkitPro tools needed at run time:
//!
//! - [`game`] identifies the disc by its boot executable's SHA-256;
//! - [`romfs`] builds the RomFS (the disc image goes in whole: the game reads it through the same `formats` disc
//!   API as on the PC);
//! - [`nacp`] writes the title, author and version block;
//! - [`nro`] turns the linked program (ELF) into an NRO image and appends icon, NACP and RomFS;
//! - [`icon`] and [`jpeg`] draw and encode the default icon;
//! - [`pack`] runs the whole pipeline and can read a finished `.nro` back ([`pack::inspect_nro`]).
//!
//! See `docs/packager.md`.

pub mod game;
pub mod icon;
pub mod jpeg;
pub mod nacp;
pub mod nro;
pub mod pack;
pub mod romfs;
