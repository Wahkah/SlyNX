//! Readers for Sly 2: Band of Thieves (USA, SCUS-97316), written from `specs/sly2/` only.
//!
//! - [`disc`]: the disc, its executable ([`exe`]), file records ([`record`]) and the level table;
//! - [`world`]: a decompressed world's outer layout and package bodies (decompression is Sly 1's,
//!   `formats::compress`);
//! - [`textures`]: material tables and texture decoding;
//! - [`stream`], [`options`], [`transforms`], [`geometry`], [`collision`]: the object stream with every class
//!   layout, transforms and meshes;
//! - [`anim`]: clips, state machines, the characters' lookup tables (curves as `formats::anim` curves);
//! - [`rig`]: posing an object: channel values by transform kind, mirror pairs, two-bone chains, skin matrices;
//! - [`scene`]: a `formats::scene::Scene` for a world, so the Sly 1 viewer and renderer draw Sly 2 levels.
//!
//! `no_std` + `alloc`; the `std` feature (default) adds opening a disc image by path. See `docs/formats-sly2.md`.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(any(test, feature = "std"))]
extern crate std;

pub mod anim;
pub mod collision;
pub mod disc;
pub mod exe;
pub mod geometry;
pub mod options;
pub mod record;
pub mod rig;
pub mod scene;
pub mod screen; // Screen movies (classes 126 to 131) read in full, for the Sly 2 user interface.
pub mod stream;
pub mod textures;
pub mod transforms;
pub mod world;

pub use disc::Disc;
pub use world::World;
