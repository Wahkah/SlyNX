//! SlyNX renderer.
//!
//! A backend-neutral core ([`Renderer`]) owns the level's resources and instances and turns them, once per frame,
//! into a [`FramePacket`]: a sorted, culled list of instance records, indexed draw commands and batches. Backends
//! upload a packet with a handful of API calls, independent of the number of objects:
//!
//! - all geometry of a level lives in one vertex buffer and one index buffer (the *arena*), uploaded once;
//! - every per-object value (transform, tint, texture layer, alpha cutoff) is in one instance buffer per frame
//!   slot, read by the vertex shader through an instance slot index, so nothing is rebound per draw;
//! - draws are grouped into batches by pass (opaque, alpha-tested, blended) and texture pool (one 2D array texture
//!   per size and format), so a batch is one `glMultiDrawElementsIndirect` on PC and a run of bare indexed draws on
//!   the Switch.
//!
//! See `docs/renderer.md` for the design and the frame budget.
//!
//! Backends: [`NullBackend`] (records packets; tests and CPU benchmarks), `gl::GlBackend` (OpenGL 4.3, PC) and
//! `deko3d::Deko3dBackend` (Switch).
#![cfg_attr(target_os = "horizon", no_std)]

extern crate alloc;

pub mod backend;
pub mod demo;
pub mod frame;
pub mod iris;
pub mod math;
pub mod overlay;
pub mod panel;
pub mod perf;
mod renderer;
mod resources;
pub mod types;
pub mod ui2d;
pub mod video;

pub use backend::{Backend, ClipSpace, Limits, NullBackend, SubmitInfo};
pub use frame::{Batch, DrawCmd, FramePacket, FrameStats, FrameUniforms, GpuInstance, View};
pub use renderer::{
    Camera, Fog, MAX_OUTLINE_EDGES, MAX_PANELS, MAX_PROJECTORS, Panel, PlacedPiece, Projector, Renderer,
};
pub use resources::{LevelUpload, PoolUpload};
pub use types::{
    BlendMode, BoneMatrix, CullMode, InstanceId, Lighting, Material, MaterialId, MeshData, MeshId, MorphVertex, Pass,
    RenderError, SkinVertex, SortMode, TextureData, TextureFormat, TextureId, Vertex, VertexExtra, VertexWeights,
};

#[cfg(not(target_os = "horizon"))]
pub mod gl;

#[cfg(target_os = "horizon")]
pub mod deko3d;

#[cfg(all(target_os = "horizon", feature = "switch-demo"))]
mod switch_demo;
