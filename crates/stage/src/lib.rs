//! Sly 2 and Sly 3 worlds turned into renderer resources, shared by the viewer and the game:
//!
//! - [`prep`]: a scene build's materials, meshes, draws and merged static geometry ([`prep::prepare`]);
//! - [`light`]: the world's lights, shadow colour and fog, and the two colours of every lit vertex;
//! - [`texanim`]: texture animation (frames and UV motion) and its [`texanim::Animator`];
//! - [`geometry`]: the prepared data and its upload ([`geometry::upload`] before the commit, [`geometry::place`]
//!   after).
//!
//! A program loads a world in three calls: `prep::prepare` (any thread), then on the render thread
//! `geometry::upload`, `Renderer::commit` and `geometry::place`. See `docs/renderer.md`, "Sly 2 and Sly 3 levels".
#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod geometry;
pub mod light;
pub mod prep;
pub mod texanim;
