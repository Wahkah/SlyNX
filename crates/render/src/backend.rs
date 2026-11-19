//! The backend interface, and [`NullBackend`], which records what it is given.

use crate::frame::{Batch, DrawCmd, FramePacket, FrameUniforms, GpuInstance};
pub use crate::math::ClipSpace;
use crate::resources::LevelUpload;
use crate::types::{CullMode, RenderError};
use alloc::vec::Vec;

/// Fixed capacities of a backend. The renderer refuses to exceed them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Most live instances (size of the instance slot buffer).
    pub max_instances: u32,
    /// Most draw commands per frame.
    pub max_draws: u32,
    /// Most batches per frame.
    pub max_batches: u32,
    /// Most bone matrices (all skinned instances together).
    pub max_bones: u32,
    /// Most morphed vertices (all morphed instances together).
    pub max_morph_vertices: u32,
}

/// What a backend reports back from a submit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubmitInfo {
    /// Draw calls made to the graphics API.
    pub api_draw_calls: u32,
    /// GPU time of an earlier frame, when available.
    pub gpu_ns: Option<u64>,
    /// Time spent blocked on frame-slot fences (waiting for the GPU, not CPU work), if the backend has a clock.
    pub wait_ns: u64,
}

/// A graphics API behind the renderer. Implementations: [`NullBackend`], `gl::GlBackend`, `deko3d::Deko3dBackend`.
///
/// Contract:
/// - `upload_level` replaces all level data (arena and texture pools). It may wait for the GPU to go idle.
/// - `submit` draws one frame from the packet. It must copy what it needs before returning (the packet's memory is
///   reused for the next frame) and must not write memory the GPU may still be reading from an earlier frame
///   (per-frame-slot buffers behind fences).
/// - Batches are drawn in order; within a batch, draws in order.
pub trait Backend {
    fn name(&self) -> &'static str;
    /// Projection conventions; the camera matches them.
    fn clip_space(&self) -> ClipSpace;
    /// Size of the render target in pixels.
    fn target_size(&self) -> (u32, u32);
    fn limits(&self) -> Limits;
    fn upload_level(&mut self, level: &LevelUpload<'_>) -> Result<(), RenderError>;
    fn submit(&mut self, frame: &FramePacket<'_>) -> Result<SubmitInfo, RenderError>;
    /// A monotonic nanosecond clock, for [`SubmitInfo::wait_ns`]. Optional.
    fn set_clock(&mut self, _clock: fn() -> u64) {}
}

/// A backend that draws nothing and keeps a copy of the last packet. Used by tests and to time the
/// backend-neutral part of the renderer on its own. Counts API draw calls like the OpenGL backend (one multi-draw
/// per batch).
#[derive(Default)]
pub struct NullBackend {
    pub size: (u32, u32),
    pub uploads: u32,
    pub frames: u64,
    pub vertex_count: usize,
    pub extras: Vec<crate::VertexExtra>,
    pub index_count: usize,
    pub pool_layers: Vec<u32>,
    pub last_uniforms: Option<FrameUniforms>,
    pub last_instances: Vec<GpuInstance>,
    pub last_bones: Vec<crate::BoneMatrix>,
    pub last_morph: Vec<crate::MorphVertex>,
    pub last_draws: Vec<DrawCmd>,
    pub last_batches: Vec<Batch>,
    pub last_cull: CullMode,
    pub last_views: Vec<crate::frame::View>,
    /// Size of the movie frame uploaded by the last packet, if it carried one.
    pub last_video: Option<(u32, u32)>,
    /// Keep copies of each packet (costs a few microseconds per frame; turn off for timing).
    pub record: bool,
}

impl NullBackend {
    pub fn new(width: u32, height: u32) -> NullBackend {
        NullBackend { size: (width, height), record: true, ..Default::default() }
    }
}

impl Backend for NullBackend {
    fn name(&self) -> &'static str {
        "null"
    }

    fn clip_space(&self) -> ClipSpace {
        ClipSpace::GL
    }

    fn target_size(&self) -> (u32, u32) {
        self.size
    }

    fn limits(&self) -> Limits {
        Limits {
            max_instances: 1 << 16,
            max_draws: 1 << 16,
            max_batches: 1 << 16,
            max_bones: 1 << 16,
            max_morph_vertices: 1 << 16,
        }
    }

    fn upload_level(&mut self, level: &LevelUpload<'_>) -> Result<(), RenderError> {
        self.uploads += 1;
        self.vertex_count = level.vertices.len();
        self.extras = level.extras.to_vec();
        self.index_count = level.indices.len();
        self.pool_layers = level.pools.iter().map(|p| p.layers.len() as u32).collect();
        Ok(())
    }

    fn submit(&mut self, frame: &FramePacket<'_>) -> Result<SubmitInfo, RenderError> {
        self.frames += 1;
        if self.record {
            self.last_uniforms = Some(*frame.uniforms);
            self.last_instances.clear();
            self.last_instances.extend_from_slice(frame.instances);
            self.last_bones.clear();
            self.last_bones.extend_from_slice(frame.bones);
            self.last_morph.clear();
            self.last_morph.extend_from_slice(frame.morph);
            self.last_draws.clear();
            self.last_draws.extend_from_slice(frame.draws);
            self.last_batches.clear();
            self.last_batches.extend_from_slice(frame.batches);
            self.last_cull = frame.cull;
            self.last_views.clear();
            self.last_views.extend_from_slice(frame.views);
            self.last_video = frame.video.map(|v| (v.width, v.height));
        }
        Ok(SubmitInfo { api_draw_calls: frame.batches.len() as u32, gpu_ns: None, wait_ns: 0 })
    }
}
