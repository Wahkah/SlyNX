//! The per-frame packet and the code that builds it: culling, sorting, instance records, draw commands, batches.
//!
//! Building a packet touches each live instance once (cull, key), sorts one `u64` key per visible draw and writes
//! one 80-byte record per visible draw. Nothing here depends on the backend.
//!
//! Sort key, most significant first: material order list (5 bits), slot (4 bits: the pass, with two-step blends
//! and stencil volumes split into their steps), mirrored (1 bit), sort value (24 bits, farthest first in
//! [`SortMode::FarToNear`] lists, else 0), texture pool (10 bits), mesh (20 bits).

use crate::math::{Frustum, Mat4, Vec3};
use crate::resources::Resources;
use crate::types::{BlendMode, CullMode, Lighting, Pass, SortMode};
use alloc::vec::Vec;

/// Per-instance record in the instance buffer (std430, 80 bytes). The vertex shader reads it through the
/// instance slot index.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuInstance {
    /// Model matrix rows 0..3 (the affine part), row-major.
    pub rows: [[f32; 4]; 3],
    /// Tint as RGBA8 (`unpackUnorm4x8` in the shader: byte 0 is red).
    pub tint: u32,
    /// Bits 0..7: layer of the texture in the batch's pool; bits 8..23: flags (`INSTANCE_*`); bits 24..31: alpha
    /// cutoff as unorm8.
    pub packed: u32,
    /// For a morphed instance: index of its first vertex in the frame's morph buffer.
    pub morph_base: u32,
    /// For a skinned instance: index of its first matrix in the frame's bone buffer.
    pub bone_base: u32,
    /// For [`INSTANCE_WEIGHTS`]: index of its first vertex's weights in the level's weight buffer.
    pub weights_base: u32,
    /// Bits 0..7: layer of the material's first extra image (T0); bits 8..15: of the second (T2); bits 16..23: fog
    /// factor as unorm8. For screen-space instances: the UV scale's v (f32 bits), with `weights_base` its u.
    /// For outline instances `weights_base` is the first edge point.
    pub layers: u32,
    /// Texture coordinate offset (texture animation).
    pub uv_offset: [f32; 2],
}

impl GpuInstance {
    /// Layer in the texture pool.
    pub fn layer(&self) -> u32 {
        self.packed & 0xff
    }

    pub fn flags(&self) -> u32 {
        (self.packed >> 8) & 0xffff
    }

    /// Alpha-test cutoff, 0..1.
    pub fn alpha_cutoff(&self) -> f32 {
        (self.packed >> 24) as f32 / 255.0
    }

    pub(crate) fn pack(layer: u32, flags: u32, alpha_cutoff: f32) -> u32 {
        (layer & 0xff) | ((flags & 0xffff) << 8) | (unorm8(alpha_cutoff) << 24)
    }
}

fn unorm8(v: f32) -> u32 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u32
}

const _: () = assert!(core::mem::size_of::<GpuInstance>() == 80);

/// Instance flag: Lambert shading with the frame's light.
pub const INSTANCE_LIT: u32 = 1;
/// Instance flag: the transform maps straight to clip space (overlay pass).
pub const INSTANCE_SCREEN: u32 = 2;
/// Instance flag: skinned with the matrices at [`GpuInstance::bone_base`].
pub const INSTANCE_SKINNED: u32 = 4;
/// Instance flag: positions and normals come from the morph buffer at [`GpuInstance::morph_base`].
pub const INSTANCE_MORPHED: u32 = 8;
/// Instance flag: three images blended with per-vertex weights from [`GpuInstance::weights_base`].
pub const INSTANCE_WEIGHTS: u32 = 16;
/// Instance flag: not darkened.
pub const INSTANCE_NO_DARKENING: u32 = 32;
/// Instance flag: not fogged.
pub const INSTANCE_NO_FOG: u32 = 64;
/// Instance flag: flat colour (the tint), no texture.
pub const INSTANCE_FLAT: u32 = 128;
/// Instance flag: outline edges ([`crate::Pass::Outline`]).
pub const INSTANCE_OUTLINE: u32 = 256;
/// Instance flag: sample texel centres (magnified pixel text).
pub const INSTANCE_NEAREST: u32 = 512;
/// Instance flag: a projector receiver draw: the receiving surface again, multiplied by the projected silhouette
/// of the projector whose parameters are at bone row `3 * weights_base` (rendering.md 9.2).
pub const INSTANCE_PROJECT_RECEIVE: u32 = 1024;
/// Instance flag: a shadow caster drawn into the silhouette image, placed by the projector at bone row
/// `3 * weights_base`.
pub const INSTANCE_PROJECT_CAST: u32 = 2048;
/// Instance flag: the movie picture (the YUV planes at fragment texture bindings 2 to 4, [`crate::video`]).
pub const INSTANCE_VIDEO: u32 = 4096;
/// Instance flag: a placed mesh ([`crate::Renderer::add_placed_mesh`]): each vertex is moved by its piece's
/// placement (index in the second stream's bone bytes, records from point `weights_base` of the static point
/// buffer), which also gives its texture layer and tint.
pub const INSTANCE_PLACED: u32 = 8192;

/// Matrices (three rows each) of a shader light block ([`crate::Renderer::set_shader_light`]).
pub const LIGHT_BLOCK_MATRICES: u32 = 5;

/// [`GpuInstance::layers`] bit: the weights are [`crate::Lighting::TwoColour`]'s two colours.
pub const LAYERS_TWO_COLOUR: u32 = 1 << 24;
/// [`GpuInstance::layers`] bit: the two colours are computed in the vertex shader from a light block in the bone store
/// ([`crate::Renderer::set_shader_light`]; its first matrix in the morph field).
pub const LAYERS_SHADER_LIGHT: u32 = 1 << 25;
/// Instance flags: sample panel image 0 or 1 (fragment texture bindings 5 and 6) instead of the texture.
pub const INSTANCE_PANEL0: u32 = 16384;
pub const INSTANCE_PANEL1: u32 = 32768;

/// One indexed draw. Same layout as OpenGL's `DrawElementsIndirectCommand` and deko3d's
/// `DkDrawIndexedIndirectData`, so the array can go to the GPU as is.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawCmd {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    /// First instance slot; slots `base_instance .. base_instance + instance_count` are this draw's records.
    pub base_instance: u32,
}

const _: () = assert!(core::mem::size_of::<DrawCmd>() == 20);

/// A run of draws sharing fixed-function state, program and texture pool.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Batch {
    /// `STATE_*` bits.
    pub state: u32,
    pub pool: u32,
    pub first_draw: u32,
    pub draw_count: u32,
    /// The pass this batch came from ([`Pass`] as `u32`), for statistics and tests.
    pub pass: u32,
}

/// [`Batch::state`] bits 0..1: depth test.
pub const STATE_DEPTH_LESS: u32 = 0;
pub const STATE_DEPTH_LEQUAL: u32 = 1;
pub const STATE_DEPTH_ALWAYS: u32 = 2;
pub const STATE_DEPTH_MASK: u32 = 3;
/// Depth writes on.
pub const STATE_DEPTH_WRITE: u32 = 1 << 2;
/// Bits 3..5: blending.
pub const STATE_BLEND_ALPHA: u32 = 1 << 3;
pub const STATE_BLEND_ADD: u32 = 2 << 3;
pub const STATE_BLEND_REVERSE_ALPHA: u32 = 3 << 3;
pub const STATE_BLEND_REVERSE_ADD: u32 = 4 << 3;
pub const STATE_BLEND_DEST_ALPHA: u32 = 5 << 3;
pub const STATE_BLEND_INV_DEST_ALPHA: u32 = 6 << 3;
pub const STATE_BLEND_MULTIPLY: u32 = 7 << 3;
pub const STATE_BLEND_MASK: u32 = 7 << 3;
/// No face culling.
pub const STATE_NO_CULL: u32 = 1 << 6;
/// Front faces are clockwise on screen (mirrored instances).
pub const STATE_FRONT_CW: u32 = 1 << 7;
/// Cull front faces instead of back faces.
pub const STATE_CULL_FRONT: u32 = 1 << 8;
/// Bits 9..10: stencil. COUNT: both faces, colour off, increment on back-face depth fail and decrement on
/// front-face depth fail. PAINT: draw only where the stencil is not zero, and zero it.
pub const STATE_STENCIL_COUNT: u32 = 1 << 9;
pub const STATE_STENCIL_PAINT: u32 = 2 << 9;
pub const STATE_STENCIL_MASK: u32 = 3 << 9;
/// Colour writes off.
pub const STATE_NO_COLOR: u32 = 1 << 11;
/// Discard texels below the instance's alpha cutoff (the alpha-tested program).
pub const STATE_ALPHA_TEST: u32 = 1 << 12;
/// Screen-space overlay.
pub const STATE_SCREEN: u32 = 1 << 13;
/// Colour writes to the alpha channel only.
pub const STATE_ALPHA_ONLY: u32 = 1 << 14;

/// Per-frame constants (std140 uniform block `Frame` in the shaders), 128 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameUniforms {
    /// Projection * view, column-major.
    pub view_proj: [f32; 16],
    /// xyz: unit direction towards the light, w: ambient level.
    pub light: [f32; 4],
    /// Fog: 1 / near, 1 / near - 1 / far, cap x colour alpha, 1 when fog is on.
    pub fog: [f32; 4],
    /// Fog colour RGB, unused.
    pub fog_color: [f32; 4],
    /// Camera position xyz, darkening factor.
    pub eye: [f32; 4],
    /// Target width and height in pixels, outline width in pixels, unused.
    pub screen: [f32; 4],
}

const _: () = assert!(core::mem::size_of::<FrameUniforms>() == 144);

/// [`View::clear`]: clear depth and stencil inside the viewport.
pub const VIEW_CLEAR_DEPTH: u32 = 1;
/// [`View::clear`]: clear colour inside the viewport to [`View::clear_color`].
pub const VIEW_CLEAR_COLOR: u32 = 2;
/// [`View::target`]: the frame's target.
pub const VIEW_TARGET_MAIN: u32 = 0;
/// [`View::target`]: the projectors' silhouette image ([`SILHOUETTE_SIZE`] square, no depth), sampled by the
/// receiver draws of the views after it.
pub const VIEW_TARGET_SILHOUETTE: u32 = 1;
/// Width and height of the silhouette image (rendering.md 9.2).
pub const SILHOUETTE_SIZE: u32 = 64;
/// [`View::target`]: panel image 0; panel image 1 is the next value. Sampled by instances with
/// [`INSTANCE_PANEL0`] or [`INSTANCE_PANEL1`] in the views after it.
pub const VIEW_TARGET_PANEL0: u32 = 2;
/// Width and height of each panel image (colour and depth).
pub const PANEL_IMAGE_SIZE: u32 = 512;
/// Most views in a frame (`SLYNX_RENDER_MAX_VIEWS` in `c/slynx_render.h`).
pub const MAX_VIEWS: usize = 8;

/// One part of a frame drawn with its own camera into a rectangle of the target: picture panels. Views are drawn
/// in list order after the whole target is cleared; each draws `batches[first_batch..][..batch_count]`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub uniforms: FrameUniforms,
    /// x, y (from the top left), width, height in pixels.
    pub viewport: [u32; 4],
    pub clear_color: [f32; 4],
    /// `VIEW_CLEAR_*` bits.
    pub clear: u32,
    pub first_batch: u32,
    pub batch_count: u32,
    /// `VIEW_TARGET_*`.
    pub target: u32,
}

const _: () = assert!(core::mem::size_of::<View>() == 192);

/// Everything a backend needs to draw one frame.
#[derive(Clone, Copy, Debug)]
pub struct FramePacket<'a> {
    pub uniforms: &'a FrameUniforms,
    /// Instance records, in slot order.
    pub instances: &'a [GpuInstance],
    /// Bone matrices of the skinned instances ([`GpuInstance::bone_base`] indexes it).
    pub bones: &'a [crate::BoneMatrix],
    /// Morphed vertices of the morphed instances ([`GpuInstance::morph_base`] indexes it).
    pub morph: &'a [crate::MorphVertex],
    pub draws: &'a [DrawCmd],
    /// Batches in draw order; each covers `draws[first_draw..][..draw_count]`.
    pub batches: &'a [Batch],
    pub clear: [f32; 4],
    pub cull: CullMode,
    /// Empty: one view, all batches over the whole target with [`FramePacket::uniforms`]. Otherwise the views to
    /// draw, in order (picture panels); together they cover every batch.
    pub views: &'a [View],
    /// A new movie frame to upload before drawing (only on frames where it changed).
    pub video: Option<crate::video::VideoFrame<'a>>,
}

/// Counters for one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameStats {
    /// Live instances.
    pub instances: u32,
    /// Instance records drawn (instances that passed frustum culling; two-step and volume draws count each step).
    pub visible: u32,
    /// Batches (state groups: state + texture pool).
    pub batches: u32,
    /// Indexed draw commands (one per run of instances sharing a mesh within a batch).
    pub draw_commands: u32,
    /// Draw calls the backend made to its API (one multi-draw per batch on OpenGL, one per command on deko3d).
    pub api_draw_calls: u32,
    pub triangles: u64,
    /// CPU time to cull, sort and build the packet.
    pub cpu_build_ns: u64,
    /// CPU time inside the backend's submit (upload, command recording, submission), excluding fence waits.
    pub cpu_submit_ns: u64,
    /// Time the backend was blocked waiting for the GPU to release a frame slot (GPU-bound time, not CPU work).
    pub wait_ns: u64,
    /// GPU time of an earlier frame, if the backend measures it.
    pub gpu_ns: Option<u64>,
}

impl FrameStats {
    /// Total renderer CPU time in milliseconds.
    pub fn cpu_ms(&self) -> f64 {
        (self.cpu_build_ns + self.cpu_submit_ns) as f64 * 1e-6
    }
}

/// One live instance as the renderer stores it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct InstanceRec {
    pub mesh: u32,
    pub material: u32,
    pub transform: Mat4,
    pub max_scale: f32,
    pub tint: [u8; 4],
    pub visible: bool,
    pub alive: bool,
    /// The transform's determinant is negative.
    pub mirrored: bool,
    /// Bone matrices: range of the renderer's bone store.
    pub bones: Option<(u32, u32)>,
    /// Morphed vertices: first index in the renderer's morph store (the mesh's vertex count long).
    pub morph: Option<u32>,
    /// Bounding sphere in model space replacing the mesh's (animated instances).
    pub sphere: Option<(Vec3, f32)>,
    /// Far-to-near sort key replacing the camera distance.
    pub sort_key: Option<f32>,
    /// First vertex weight in the level's weight buffer.
    pub weights: Option<u32>,
    /// Two colours computed in the shader: the light block's first matrix within the instance's bones.
    pub shader_light: Option<u32>,
    /// Fog factor (1 = full fog).
    pub fog: f32,
    /// Screen-space instances: the texture area `[u0, v0, u1, v1]` the unit quad maps to.
    pub uv_rect: Option<[f32; 4]>,
    /// Flat colour, no texture.
    pub flat: bool,
    /// Sample texel centres.
    pub nearest: bool,
    /// Drawn into the projectors' silhouette image.
    pub caster: bool,
    /// Projector kinds it receives (bit 0x01 shadows, 0x02 light patterns).
    pub receiver: u8,
    /// Draws the movie picture.
    pub video: bool,
    /// Samples a panel image.
    pub panel_image: Option<u8>,
}

#[derive(Clone, Copy)]
struct SortItem {
    key: u64,
    instance: u32,
    /// Projector parameter row (receiver and caster draws), else 0.
    projector: u32,
}

/// A projector as the frame builder needs it: where its parameters are in the bone store, and the region it
/// reaches, for culling the receivers.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ProjectorRef {
    /// First bone matrix of the parameters.
    pub param: u32,
    pub origin: Vec3,
    /// Largest radius of the cone.
    pub max_radius: f32,
    /// Receivers reach from `bottom` to `top` (world z).
    pub top: f32,
    pub bottom: f32,
    /// Receiver bits it acts on.
    pub receivers: u8,
}

/// What a view draws.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ViewMode<'p> {
    /// Everything, with the projectors' receiver draws.
    All(&'p [ProjectorRef]),
    /// Everything but screen-space materials (picture panels), with the receiver draws.
    World(&'p [ProjectorRef]),
    /// Only the shadow casters, flat, placed by the projector parameters at `param` (the silhouette image).
    Casters { param: u32 },
}

/// Draw slots inside one order list, in drawing order.
mod slot {
    pub const BACKGROUND: u64 = 0;
    pub const OPAQUE: u64 = 1;
    pub const ALPHA_TEST: u64 = 2;
    pub const BLEND_PRE: u64 = 3;
    pub const BLEND: u64 = 4;
    pub const VOLUME_COUNT: u64 = 5;
    pub const VOLUME_PAINT: u64 = 6;
    pub const POST: u64 = 7;
    pub const OVERLAY: u64 = 8;
    pub const OUTLINE: u64 = 9;
    pub const MASK_BACK: u64 = 10;
    pub const MASK_FRONT: u64 = 11;
    pub const MASK_FILL: u64 = 12;
    pub const MASK_OVER: u64 = 13;
    pub const PROJECT: u64 = 14;
    pub const CAST: u64 = 15;
}

const ORDER_SHIFT: u32 = 59;
const SLOT_SHIFT: u32 = 55;
const SLOT_MASK: u64 = 0xf;
const MIRROR_SHIFT: u32 = 54;
const VALUE_SHIFT: u32 = 30;
const POOL_SHIFT: u32 = 20;
const MESH_MASK: u64 = (1 << 20) - 1;
const POOL_MASK: u64 = (1 << 10) - 1;

/// Reusable buffers for building packets; allocations are kept between frames.
#[derive(Default)]
pub(crate) struct FrameBuilder {
    items: Vec<SortItem>,
    pub instances: Vec<GpuInstance>,
    pub draws: Vec<DrawCmd>,
    pub batches: Vec<Batch>,
}

fn mul_u8(a: u8, b: u8) -> u8 {
    ((a as u32 * b as u32 + 127) / 255) as u8
}

fn blend_state(b: BlendMode) -> u32 {
    match b {
        BlendMode::None => 0,
        BlendMode::Alpha => STATE_BLEND_ALPHA,
        BlendMode::Additive => STATE_BLEND_ADD,
        BlendMode::ReverseAlpha => STATE_BLEND_REVERSE_ALPHA,
        BlendMode::ReverseAdd => STATE_BLEND_REVERSE_ADD,
        BlendMode::DestAlpha => STATE_BLEND_DEST_ALPHA,
        BlendMode::InvDestAlpha => STATE_BLEND_INV_DEST_ALPHA,
        BlendMode::Multiply => STATE_BLEND_MULTIPLY,
    }
}

/// Fixed-function state of a draw slot.
fn slot_state(s: u64, blend: BlendMode) -> u32 {
    match s {
        slot::BACKGROUND => STATE_DEPTH_ALWAYS | blend_state(blend),
        slot::OPAQUE => STATE_DEPTH_LESS | STATE_DEPTH_WRITE,
        slot::ALPHA_TEST => STATE_DEPTH_LESS | STATE_DEPTH_WRITE | STATE_ALPHA_TEST,
        slot::BLEND_PRE => STATE_DEPTH_LESS | STATE_DEPTH_WRITE | STATE_ALPHA_TEST | blend_state(blend),
        slot::BLEND => STATE_DEPTH_LESS | blend_state(blend),
        slot::VOLUME_COUNT => STATE_DEPTH_LESS | STATE_NO_CULL | STATE_NO_COLOR | STATE_STENCIL_COUNT,
        slot::VOLUME_PAINT => STATE_DEPTH_ALWAYS | STATE_CULL_FRONT | STATE_STENCIL_PAINT | blend_state(blend),
        slot::POST => STATE_DEPTH_ALWAYS | blend_state(blend),
        slot::OUTLINE => STATE_DEPTH_LEQUAL | STATE_DEPTH_WRITE | STATE_NO_CULL | STATE_BLEND_ALPHA,
        slot::MASK_BACK => STATE_DEPTH_LEQUAL | STATE_CULL_FRONT | STATE_ALPHA_ONLY,
        slot::MASK_FRONT | slot::MASK_OVER => STATE_DEPTH_LEQUAL | STATE_BLEND_ALPHA,
        slot::MASK_FILL => STATE_DEPTH_LEQUAL | STATE_BLEND_INV_DEST_ALPHA,
        slot::PROJECT => STATE_DEPTH_LEQUAL | STATE_ALPHA_TEST | STATE_BLEND_MULTIPLY,
        slot::CAST => STATE_DEPTH_ALWAYS | STATE_NO_CULL,
        _ => STATE_DEPTH_ALWAYS | STATE_NO_CULL | STATE_BLEND_ALPHA | STATE_SCREEN,
    }
}

/// The slots a pass draws in.
fn slots(pass: Pass, two_step: bool) -> &'static [u64] {
    match pass {
        Pass::Background => &[slot::BACKGROUND],
        Pass::Opaque => &[slot::OPAQUE],
        Pass::AlphaTest => &[slot::ALPHA_TEST],
        Pass::Blend if two_step => &[slot::BLEND_PRE, slot::BLEND],
        Pass::Blend => &[slot::BLEND],
        Pass::Volume => &[slot::VOLUME_COUNT, slot::VOLUME_PAINT],
        Pass::Post => &[slot::POST],
        Pass::Overlay => &[slot::OVERLAY],
        Pass::Outline => &[slot::OUTLINE],
        Pass::MaskWrite => &[slot::MASK_BACK, slot::MASK_FRONT],
        Pass::MaskFill => &[slot::MASK_FILL, slot::MASK_OVER],
    }
}

/// A positive float's bits, which order like the value, cut to 24 bits.
fn order_bits(v: f32) -> u64 {
    (v.max(0.0).to_bits() >> 8) as u64 & 0xff_ffff
}

impl FrameBuilder {
    /// Empties the records, draws and batches.
    pub fn clear(&mut self) {
        self.instances.clear();
        self.draws.clear();
        self.batches.clear();
    }

    /// Culls and sorts the instances for one view and appends their records, draws and batches to the ones already
    /// built ([`ViewMode`]; [`FrameBuilder::clear`] starts a frame). Returns (records added, triangles).
    pub fn add_view(
        &mut self,
        res: &Resources,
        recs: &[InstanceRec],
        frustum: &Frustum,
        eye: Vec3,
        mode: ViewMode<'_>,
    ) -> (u32, u64) {
        self.items.clear();
        let (world_only, projectors) = match mode {
            ViewMode::All(p) => (false, p),
            ViewMode::World(p) => (true, p),
            ViewMode::Casters { param } => {
                for (i, rec) in recs.iter().enumerate() {
                    if rec.alive && rec.visible && rec.caster {
                        let pool = res.textures[res.materials[rec.material as usize].texture.0 as usize].pool as u64;
                        let key = (slot::CAST << SLOT_SHIFT)
                            | ((pool & POOL_MASK) << POOL_SHIFT)
                            | (rec.mesh as u64 & MESH_MASK);
                        self.items.push(SortItem { key, instance: i as u32, projector: param });
                    }
                }
                return self.emit(res, recs);
            }
        };

        for (i, rec) in recs.iter().enumerate() {
            if !rec.alive || !rec.visible {
                continue;
            }
            let mesh = &res.meshes[rec.mesh as usize];
            let mat = &res.materials[rec.material as usize];
            if world_only && mat.pass == Pass::Overlay {
                continue;
            }
            let pool = res.textures[mat.texture.0 as usize].pool as u64;
            let mut value = 0u64;
            if mat.pass != Pass::Overlay {
                // Screen space is never culled.
                let (center, radius) = rec.sphere.unwrap_or((mesh.center, mesh.radius));
                let c = rec.transform.transform_point(center);
                let r = radius * rec.max_scale;
                if !frustum.sphere_visible(c, r) {
                    continue;
                }
                if mat.sort == SortMode::FarToNear {
                    let d = rec.sort_key.unwrap_or_else(|| {
                        let v = c - eye;
                        v.dot(v)
                    });
                    // Farthest first: invert.
                    value = !order_bits(d) & 0xff_ffff;
                }
            }
            let mirrored = rec.mirrored && mat.pass != Pass::Overlay;
            let base = ((mat.order as u64 & 0x1f) << ORDER_SHIFT)
                | ((mirrored as u64) << MIRROR_SHIFT)
                | (value << VALUE_SHIFT)
                | ((pool & POOL_MASK) << POOL_SHIFT)
                | (rec.mesh as u64 & MESH_MASK);
            for &s in slots(mat.pass, mat.two_step) {
                self.items.push(SortItem { key: base | (s << SLOT_SHIFT), instance: i as u32, projector: 0 });
            }
            // Receivers: opaque and cut-out surfaces inside a projector's reach get one more draw per projector, at
            // the end of their own list.
            if rec.receiver != 0 && matches!(mat.pass, Pass::Opaque | Pass::AlphaTest) && !projectors.is_empty() {
                let (center, radius) = rec.sphere.unwrap_or((mesh.center, mesh.radius));
                let c = rec.transform.transform_point(center);
                let r = radius * rec.max_scale;
                for p in projectors.iter().filter(|p| p.receivers & rec.receiver != 0) {
                    let (dx, dy) = (c.x - p.origin.x, c.y - p.origin.y);
                    let reach = p.max_radius + r;
                    if dx * dx + dy * dy <= reach * reach && c.z - r <= p.top && c.z + r >= p.bottom {
                        let key = ((mat.order as u64 & 0x1f) << ORDER_SHIFT)
                            | (slot::PROJECT << SLOT_SHIFT)
                            | ((mirrored as u64) << MIRROR_SHIFT)
                            | ((pool & POOL_MASK) << POOL_SHIFT)
                            | (rec.mesh as u64 & MESH_MASK);
                        self.items.push(SortItem { key, instance: i as u32, projector: p.param });
                    }
                }
            }
        }
        self.emit(res, recs)
    }

    /// Sorts the view's items and appends their records, draws and batches.
    fn emit(&mut self, res: &Resources, recs: &[InstanceRec]) -> (u32, u64) {
        let first_record = self.instances.len() as u32;
        // Ties broken by instance index, so the order is fully defined. Unstable sort: no scratch allocation.
        self.items
            .sort_unstable_by_key(|it| ((it.key as u128) << 64) | ((it.instance as u128) << 32) | it.projector as u128);

        let mut triangles = 0u64;
        let mut cur_batch: Option<(u32, u32)> = None; // (state, pool)
        let mut prev_mesh = u32::MAX;
        for it in &self.items {
            let rec = &recs[it.instance as usize];
            let mat = &res.materials[rec.material as usize];
            let loc = res.textures[mat.texture.0 as usize];
            let mesh = &res.meshes[rec.mesh as usize];
            let s = (it.key >> SLOT_SHIFT) & SLOT_MASK;
            let mut state = slot_state(s, mat.blend);
            if (it.key >> MIRROR_SHIFT) & 1 != 0 {
                state |= STATE_FRONT_CW;
            }
            let pool = loc.pool as u32;

            if cur_batch != Some((state, pool)) {
                cur_batch = Some((state, pool));
                self.batches.push(Batch {
                    state,
                    pool,
                    first_draw: self.draws.len() as u32,
                    draw_count: 0,
                    pass: mat.pass as u32,
                });
                prev_mesh = u32::MAX;
            }
            let slot_index = self.instances.len() as u32;
            if rec.mesh == prev_mesh {
                let d = self.draws.last_mut().expect("a draw exists after the first instance of a mesh");
                d.instance_count += 1;
            } else {
                self.draws.push(DrawCmd {
                    index_count: mesh.index_count,
                    instance_count: 1,
                    first_index: mesh.first_index,
                    base_vertex: mesh.base_vertex,
                    base_instance: slot_index,
                });
                self.batches.last_mut().expect("batch pushed above").draw_count += 1;
                prev_mesh = rec.mesh;
            }
            triangles += mesh.index_count as u64 / 3;

            let t = [
                mul_u8(mat.tint[0], rec.tint[0]),
                mul_u8(mat.tint[1], rec.tint[1]),
                mul_u8(mat.tint[2], rec.tint[2]),
                mul_u8(mat.tint[3], rec.tint[3]),
            ];
            let mut flags = match mat.pass {
                Pass::Overlay => INSTANCE_SCREEN | INSTANCE_NO_FOG | INSTANCE_NO_DARKENING,
                Pass::Outline => INSTANCE_OUTLINE | INSTANCE_FLAT | INSTANCE_NO_FOG | INSTANCE_NO_DARKENING,
                _ if mat.lighting == Lighting::Lambert => INSTANCE_LIT,
                _ => 0,
            };
            if rec.flat {
                flags |= INSTANCE_FLAT;
            }
            if rec.nearest {
                flags |= INSTANCE_NEAREST;
            }
            if rec.video {
                flags |= INSTANCE_VIDEO;
            }
            match rec.panel_image {
                Some(0) => flags |= INSTANCE_PANEL0,
                Some(_) => flags |= INSTANCE_PANEL1,
                None => {}
            }
            if mat.no_darkening {
                flags |= INSTANCE_NO_DARKENING;
            }
            if mat.no_fog {
                flags |= INSTANCE_NO_FOG;
            }
            let (mut bone_base, mut morph_base, mut weights_base) = (0, 0, 0);
            if let Some((start, _)) = rec.bones {
                flags |= INSTANCE_SKINNED;
                bone_base = start;
            }
            if let Some(start) = rec.morph {
                flags |= INSTANCE_MORPHED;
                morph_base = start;
            }
            let mut layers = 0;
            if matches!(mat.lighting, Lighting::Palettes | Lighting::TwoColour)
                && let (Some(w), Some([t0, t2])) = (rec.weights, mat.palettes)
            {
                flags |= INSTANCE_WEIGHTS;
                weights_base = w;
                let l = |t: crate::TextureId| res.textures[t.0 as usize].layer as u32 & 0xff;
                layers = l(t0) | (l(t2) << 8);
                if mat.lighting == Lighting::TwoColour {
                    layers |= LAYERS_TWO_COLOUR;
                }
            }
            // The light block's index rides in the morph field, so a morphed or placed instance (which use that
            // field) never takes the shader light; it keeps its weights.
            if mat.lighting == Lighting::TwoColour
                && rec.morph.is_none()
                && mesh.placed_base.is_none()
                && let (Some(k), Some((start, n)), Some([t0, t2])) = (rec.shader_light, rec.bones, mat.palettes)
                && k + LIGHT_BLOCK_MATRICES <= n
            {
                flags |= INSTANCE_WEIGHTS;
                morph_base = start + k;
                let l = |t: crate::TextureId| res.textures[t.0 as usize].layer as u32 & 0xff;
                layers = l(t0) | (l(t2) << 8) | LAYERS_TWO_COLOUR | LAYERS_SHADER_LIGHT;
            }
            layers |= unorm8(rec.fog) << 16;
            let cutoff = if state & STATE_ALPHA_TEST != 0 { mat.alpha_cutoff } else { 0.0 };
            let mut uv = res.uv_offsets.get(rec.material as usize).copied().unwrap_or([0.0; 2]);
            if mat.pass == Pass::Overlay {
                // Screen space: the unit quad's UVs map to a texture area (whole texture by default).
                let r = rec.uv_rect.unwrap_or([0.0, 0.0, 1.0, 1.0]);
                uv = [r[0], r[1]];
                weights_base = (r[2] - r[0]).to_bits();
                layers = (r[3] - r[1]).to_bits();
            }
            if let Some(base) = mesh.outline_base {
                flags |= INSTANCE_OUTLINE;
                weights_base = base;
            }
            if let Some(base) = mesh.placed_base {
                // The placements take the weights field; weights move to the morph field (never morphed).
                flags |= INSTANCE_PLACED;
                morph_base = weights_base;
                weights_base = base;
            }
            let mut t = t;
            if s == slot::PROJECT || s == slot::CAST {
                // The surface's shape and alpha only; the projector's parameters in the weights field.
                let keep = INSTANCE_SKINNED | INSTANCE_MORPHED | INSTANCE_FLAT;
                flags = (flags & keep) | INSTANCE_NO_FOG | INSTANCE_NO_DARKENING;
                flags |= if s == slot::CAST { INSTANCE_PROJECT_CAST | INSTANCE_FLAT } else { INSTANCE_PROJECT_RECEIVE };
                weights_base = it.projector;
                if s == slot::CAST {
                    t = [255, 255, 255, 255];
                }
            }
            self.instances.push(GpuInstance {
                rows: rec.transform.affine_rows(),
                tint: u32::from_le_bytes(t),
                packed: GpuInstance::pack(loc.layer as u32, flags, cutoff),
                morph_base,
                bone_base,
                weights_base,
                layers,
                uv_offset: uv,
            });
        }
        (self.instances.len() as u32 - first_record, triangles)
    }
}

/// Default light: from above and slightly in front, 35% ambient.
pub(crate) fn default_light() -> [f32; 4] {
    let d = Vec3::new(0.35, 1.0, 0.55).normalize();
    [d.x, d.y, d.z, 0.35]
}
