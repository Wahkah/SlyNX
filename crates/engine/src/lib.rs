//! Engine foundations shared by the games: animation playback and skinning ([`anim`]) and collision structures
//! and queries ([`collision`]).
//!
//! `#![no_std]` with `alloc`, no third-party crates and no `unsafe`. The modules are independent of the game's object
//! model: they take data parsed by `formats` and plain node indices, and give back transforms, matrices and hits.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod anim;
pub mod behavior;
pub mod camera;
pub mod collision;
pub mod game;
pub mod gang;
pub mod math;
pub mod object;
pub mod physics;
pub mod player;
pub mod world;
