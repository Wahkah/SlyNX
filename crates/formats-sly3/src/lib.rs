//! Readers for Sly 3: Honor Among Thieves (USA, SCUS-97464), written from `specs/sly3/` only. Where Sly 3 matches
//! Sly 2 the code of `formats-sly2` is reused (file records, the option table, option lists, transform lists,
//! collision meshes, strip decoding).
//!
//! - [`disc`]: the disc and its executable tables ([`exe`]): world slots, movies, options, classes;
//! - [`world`]: a decompressed world's header sections (key tables, groups, UTF-16 strings, option sets, partition
//!   trees per condition) and group bodies;
//! - [`filter`]: world filters and condition lists;
//! - [`textures`]: the two material-table copies, palettes at half intensity, colour grading, decoding;
//! - [`stream`], [`options`], [`geometry`], [`collision`]: the object stream with every class layout, mesh sets
//!   with filtered meshes, bone tables, colour tables and per-vertex weights;
//! - [`anim`]: clip data with shared key-time lists and packed tracks (curves as `formats::anim` curves);
//! - [`scene`]: a `formats::scene::Scene` for a world.
//!
//! `no_std` + `alloc`; the `std` feature (default) adds opening a disc image by path. See `docs/formats-sly3.md`.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;
#[cfg(any(test, feature = "std"))]
extern crate std;

pub mod anim;
pub mod collision;
pub mod disc;
pub mod exe;
pub mod filter;
pub mod geometry;
pub mod options;
pub mod scene;
pub mod stream;
pub mod textures;
pub mod world;

pub use disc::Disc;
pub use world::World;
