//! Readers for the Sly 1 (USA, SCUS-97198) disc and world files.
//!
//! Everything here is written from the format specs in `specs/sly1/`. The crate is `no_std` with `alloc` so it runs
//! on the Switch; the `std` feature (on by default) adds opening a disc image by path.
//!
//! Layers, from the bottom up:
//!
//! | Module | What it reads |
//! |---|---|
//! | [`hash`] | SHA-256 and CRC-32, used to identify the executable and check decompressed worlds |
//! | [`iso`] | ISO 9660 primary volume descriptor and root directory |
//! | [`disc`] | `SYSTEM.CNF`, the executable's address mapping, file descriptors, the 46-entry world table |
//! | [`compress`] | the world-file LZ decompressor |
//! | [`world`] | the outer layout of a decompressed world: header, prolog, scripts, world options, descriptors, material tables, the object-stream start and the texture base |
//! | [`options`] | option lists and the option-id encoding table |
//! | [`textures`] | palettes, bitmaps, shaders, fonts; decoding bitmaps to RGBA8 |
//! | [`anim`] | animation segments: channels, curves, events, settings; character lookup lists |
//! | [`collision`] | solid bodies: collision meshes and trees, convex volumes, the material tables |
//! | [`geometry`] | mesh sets, meshes, submeshes, strips to triangle lists, GPU vertex output, transforms |
//! | [`stream`] | the object-stream walker (per-class body layouts, hierarchy, proxies) |
//! | [`scene`] | a renderer-ready scene: vertex and index arenas, meshes, per-draw transforms, materials and decoded textures |

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod anim;
pub mod collision;
pub mod compress;
pub mod disc;
pub mod error;
pub mod geometry;
pub mod hash;
pub mod iso;
pub mod options;
pub mod reader;
pub mod scene;
pub mod stream;
pub mod textures;
pub mod world;

pub use error::{Error, Result};
