//! deko3d backend (Switch): a thin wrapper over the C side in `c/slynx_render.c`.
//!
//! The whole frame packet crosses the FFI boundary in one call per frame; the C side copies it into the frame
//! slot's GPU memory and records one indexed draw per draw command with no state changes in between (state changes
//! only at batch boundaries). The device, queue and swapchain are the platform layer's.

use crate::backend::{Backend, ClipSpace, Limits, SubmitInfo};
use crate::frame::{Batch, DrawCmd, FramePacket};
use crate::resources::LevelUpload;
use crate::types::{RenderError, TextureFormat};
use alloc::format;
use alloc::vec::Vec;
use core::ffi::c_void;
use core::marker::PhantomData;
use platform_switch::SwitchPlatform;

#[repr(C)]
struct RawPool {
    width: u32,
    height: u32,
    mip_levels: u32,
    layer_count: u32,
    format: u32,
    layer_bytes: u32,
    clamp: u32,
    layers: *const *const u8,
}

#[repr(C)]
struct RawFrame {
    uniforms: *const c_void,
    instances: *const c_void,
    instance_count: u32,
    draw_count: u32,
    bones: *const c_void,
    morph: *const c_void,
    bone_count: u32,
    morph_count: u32,
    draws: *const DrawCmd,
    batches: *const Batch,
    batch_count: u32,
    cull: u32,
    clear: [f32; 4],
    views: *const crate::frame::View,
    view_count: u32,
    reserved: u32,
    video_planes: [*const u8; 3],
    video_width: u32,
    video_height: u32,
}

#[repr(C)]
#[derive(Default)]
struct RawStats {
    api_draw_calls: u32,
    cmd_bytes_budget: u32,
    cmd_bytes_used: u32,
    reserved: u32,
    wait_ns: u64,
}

/// Command memory of the last frame: what it used (to 16 KiB chunks) and the per-slot budget. `used` above
/// `budget` means the slot ran out and more memory was allocated for that frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CmdUsage {
    pub used: u32,
    pub budget: u32,
}

unsafe extern "C" {
    fn slynx_render_init(max_instances: u32, max_draws: u32, max_bones: u32, max_morph: u32) -> i32;
    fn slynx_render_size(width: *mut u32, height: *mut u32);
    fn slynx_render_upload_level(
        vertices: *const c_void,
        vertex_bytes: u32,
        extras: *const c_void,
        weights: *const c_void,
        weight_count: u32,
        outline_points: *const c_void,
        outline_count: u32,
        indices: *const u16,
        index_count: u32,
        pools: *const RawPool,
        pool_count: u32,
    ) -> i32;
    fn slynx_render_frame(frame: *const RawFrame, stats: *mut RawStats) -> i32;
    fn slynx_render_exit();
}

fn err(what: &str, rc: i32) -> RenderError {
    let why = match rc {
        1 => "already initialised",
        2 => "not initialised",
        3 => "out of GPU memory",
        4 => "bad argument",
        5 => "frame larger than the limits",
        6 => "no level uploaded",
        7 => "platform display not initialised",
        _ => "unknown error",
    };
    RenderError::Backend(format!("deko3d {what}: {why} ({rc})"))
}

/// Most batches per frame (`SLYNX_RENDER_MAX_BATCHES` in `c/slynx_render.h`).
pub const MAX_BATCHES: u32 = 4096;
/// Bone matrices per frame for [`Deko3dBackend::new`] (192 KiB per frame slot).
pub const DEFAULT_MAX_BONES: u32 = 4096;
/// Morphed vertices per frame for [`Deko3dBackend::new`] (256 KiB per frame slot).
pub const DEFAULT_MAX_MORPH_VERTICES: u32 = 8192;

/// The deko3d backend. One may exist at a time, on the thread that owns the platform.
pub struct Deko3dBackend {
    limits: Limits,
    cmd: CmdUsage,
    _single_thread: PhantomData<*mut ()>,
}

impl Deko3dBackend {
    /// Creates the renderer on the platform's display. `platform` proves the display exists and outlives nothing
    /// here by itself: drop the backend before the platform.
    pub fn new(platform: &SwitchPlatform, max_instances: u32, max_draws: u32) -> Result<Deko3dBackend, RenderError> {
        Self::with_limits(platform, max_instances, max_draws, DEFAULT_MAX_BONES, DEFAULT_MAX_MORPH_VERTICES)
    }

    /// Like [`Deko3dBackend::new`], with the per-frame bone matrix and morphed vertex capacities.
    pub fn with_limits(
        _platform: &SwitchPlatform,
        max_instances: u32,
        max_draws: u32,
        max_bones: u32,
        max_morph_vertices: u32,
    ) -> Result<Deko3dBackend, RenderError> {
        // SAFETY: the display exists (the platform does); the C side refuses a second init itself.
        let rc = unsafe { slynx_render_init(max_instances, max_draws, max_bones, max_morph_vertices) };
        if rc != 0 {
            return Err(err("init", rc));
        }
        // Same rule as the C side (slynx_render.h, SLYNX_RENDER_MAX_BATCHES).
        let max_batches = max_draws.min(MAX_BATCHES);
        Ok(Deko3dBackend {
            limits: Limits { max_instances, max_draws, max_batches, max_bones, max_morph_vertices },
            cmd: CmdUsage::default(),
            _single_thread: PhantomData,
        })
    }

    /// Command memory of the last frame.
    pub fn cmd_usage(&self) -> CmdUsage {
        self.cmd
    }
}

impl Backend for Deko3dBackend {
    fn name(&self) -> &'static str {
        "deko3d"
    }

    fn clip_space(&self) -> ClipSpace {
        ClipSpace::DEKO3D
    }

    fn target_size(&self) -> (u32, u32) {
        let (mut w, mut h) = (0, 0);
        // SAFETY: two live out-parameters.
        unsafe { slynx_render_size(&mut w, &mut h) };
        (w, h)
    }

    fn limits(&self) -> Limits {
        self.limits
    }

    fn upload_level(&mut self, level: &LevelUpload<'_>) -> Result<(), RenderError> {
        // Per pool: one pointer per layer, kept alive until the call returns.
        let layer_ptrs: Vec<Vec<*const u8>> =
            level.pools.iter().map(|p| p.layers.iter().map(|t| t.data.as_ptr()).collect()).collect();
        let mut pools = Vec::with_capacity(level.pools.len());
        for (p, ptrs) in level.pools.iter().zip(&layer_ptrs) {
            match p.format {
                TextureFormat::Rgba8 => {}
            }
            let layer_bytes = p.layers.first().map_or(0, |t| t.data.len());
            if p.layers.iter().any(|t| t.data.len() != layer_bytes) || layer_bytes > u32::MAX as usize {
                return Err(RenderError::BadTexture("pool layers differ in size"));
            }
            pools.push(RawPool {
                width: p.width,
                height: p.height,
                mip_levels: p.mip_levels,
                layer_count: p.layers.len() as u32,
                format: 0,
                layer_bytes: layer_bytes as u32,
                clamp: p.clamp as u32,
                layers: ptrs.as_ptr(),
            });
        }
        let vbytes = core::mem::size_of_val(level.vertices);
        if level.extras.len() != level.vertices.len() {
            return Err(RenderError::BadMesh("second vertex stream does not match the vertices"));
        }
        if vbytes > u32::MAX as usize / 2 || level.indices.len() > u32::MAX as usize / 4 {
            return Err(RenderError::LimitReached("arena size"));
        }
        // SAFETY: every pointer describes a live slice of the stated length (vertices, indices, the pool table and
        // each layer's packed mip chain, validated when the texture was added); the C side copies everything
        // before returning.
        let rc = unsafe {
            slynx_render_upload_level(
                level.vertices.as_ptr().cast(),
                vbytes as u32,
                level.extras.as_ptr().cast(),
                level.weights.as_ptr().cast(),
                level.weights.len() as u32,
                level.outline_points.as_ptr().cast(),
                level.outline_points.len() as u32,
                level.indices.as_ptr(),
                level.indices.len() as u32,
                pools.as_ptr(),
                pools.len() as u32,
            )
        };
        if rc != 0 { Err(err("upload", rc)) } else { Ok(()) }
    }

    fn submit(&mut self, frame: &FramePacket<'_>) -> Result<SubmitInfo, RenderError> {
        let raw = RawFrame {
            uniforms: (frame.uniforms as *const crate::FrameUniforms).cast(),
            instances: frame.instances.as_ptr().cast(),
            instance_count: frame.instances.len() as u32,
            draw_count: frame.draws.len() as u32,
            bones: frame.bones.as_ptr().cast(),
            morph: frame.morph.as_ptr().cast(),
            bone_count: frame.bones.len() as u32,
            morph_count: frame.morph.len() as u32,
            draws: frame.draws.as_ptr(),
            batches: frame.batches.as_ptr(),
            batch_count: frame.batches.len() as u32,
            cull: frame.cull as u32,
            clear: frame.clear,
            views: frame.views.as_ptr(),
            view_count: frame.views.len() as u32,
            reserved: 0,
            video_planes: frame.video.map_or([core::ptr::null(); 3], |v| [v.y.as_ptr(), v.u.as_ptr(), v.v.as_ptr()]),
            video_width: frame.video.map_or(0, |v| v.width),
            video_height: frame.video.map_or(0, |v| v.height),
        };
        let mut stats = RawStats::default();
        // SAFETY: the packet's slices are live for the call and match the counts; the C side validates batch ranges
        // and pool indices, copies the data and keeps no pointers.
        let rc = unsafe { slynx_render_frame(&raw, &mut stats) };
        self.cmd = CmdUsage { used: stats.cmd_bytes_used, budget: stats.cmd_bytes_budget };
        if rc == 3 {
            // Presented, but without the draws that did not fit in command memory.
            return Err(RenderError::LimitReached("command memory"));
        }
        if rc != 0 {
            return Err(err("frame", rc));
        }
        Ok(SubmitInfo { api_draw_calls: stats.api_draw_calls, gpu_ns: None, wait_ns: stats.wait_ns })
    }
}

impl Drop for Deko3dBackend {
    fn drop(&mut self) {
        // SAFETY: this is the only backend; afterwards the C side is uninitialised.
        unsafe { slynx_render_exit() }
    }
}
