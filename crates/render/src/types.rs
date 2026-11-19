//! Public data types: vertices, mesh and texture data, materials, handles and errors.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// One vertex of the shared geometry arena. 28 bytes; the layout is fixed because both backends' vertex input
/// state and the shaders in `shaders/` describe it.
///
/// | offset | field | GPU format |
/// |---|---|---|
/// | 0 | `pos` | 3 x f32 |
/// | 12 | `uv` | 2 x f32 |
/// | 20 | `normal` | 4 x snorm8 (w unused) |
/// | 24 | `color` | 4 x unorm8 (RGBA) |
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub normal: [i8; 4],
    pub color: [u8; 4],
}

const _: () = assert!(core::mem::size_of::<Vertex>() == 28);

impl Vertex {
    /// Builds a vertex, packing the normal (expected to be unit length or zero) to snorm8.
    pub fn new(pos: [f32; 3], uv: [f32; 2], normal: [f32; 3], color: [u8; 4]) -> Vertex {
        let pack = |v: f32| {
            let c = v.clamp(-1.0, 1.0) * 127.0;
            (if c >= 0.0 { c + 0.5 } else { c - 0.5 }) as i8
        };
        Vertex { pos, uv, normal: [pack(normal[0]), pack(normal[1]), pack(normal[2]), 0], color }
    }
}

/// Skinning data of one vertex: up to four bones (indices into the instance's bone matrices, see
/// [`crate::Renderer::set_bones`]) and their weights. Weights are stored as unorm16 (1 / 65535 steps) and are
/// not renormalised; a vertex whose weights are all zero stays in place.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkinVertex {
    pub bones: [u8; 4],
    pub weights: [u16; 4],
}

impl SkinVertex {
    /// No influence: the vertex is drawn rigidly.
    pub const NONE: SkinVertex = SkinVertex { bones: [0; 4], weights: [0; 4] };

    /// From weights in 0..1 (clamped and rounded to unorm16).
    pub fn new(bones: [u8; 4], weights: [f32; 4]) -> SkinVertex {
        let q = |w: f32| (w.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16;
        SkinVertex { bones, weights: [q(weights[0]), q(weights[1]), q(weights[2]), q(weights[3])] }
    }
}

/// The second vertex stream of the arena, one per arena vertex (16 bytes): skinning data and the vertex's index
/// within its mesh (for morphed instances, whose positions come from the frame's morph buffer).
///
/// | offset | field | GPU format |
/// |---|---|---|
/// | 0 | `bones` | 4 x u8, integer |
/// | 4 | `weights` | 4 x unorm16 |
/// | 12 | `local` | u16, integer |
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VertexExtra {
    pub bones: [u8; 4],
    pub weights: [u16; 4],
    pub local: u16,
    pub pad: u16,
}

const _: () = assert!(core::mem::size_of::<VertexExtra>() == 16);

/// One bone matrix: the three rows of an affine transform (`[m00 m01 m02 tx]`, ...), 48 bytes, the layout of the
/// frame's bone buffer.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneMatrix {
    pub rows: [[f32; 4]; 3],
}

const _: () = assert!(core::mem::size_of::<BoneMatrix>() == 48);

impl BoneMatrix {
    pub const IDENTITY: BoneMatrix =
        BoneMatrix { rows: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]] };
}

/// One morphed vertex in the frame's morph buffer: position and normal, each padded to a `vec4` (std430).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MorphVertex {
    pub position: [f32; 4],
    pub normal: [f32; 4],
}

const _: () = assert!(core::mem::size_of::<MorphVertex>() == 32);

/// Per-vertex weights of an instance drawn with [`Lighting::Palettes`] (8 bytes, unorm8 each): the shadow image's
/// weight (scalar), the mid-tone and highlight images' weights (RGB) and the alpha.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VertexWeights {
    pub shadow: u8,
    pub mid: [u8; 3],
    pub high: [u8; 3],
    pub alpha: u8,
}

const _: () = assert!(core::mem::size_of::<VertexWeights>() == 8);

impl VertexWeights {
    /// From weights in 0..1 (clamped).
    pub fn new(shadow: f32, mid: [f32; 3], high: [f32; 3], alpha: f32) -> VertexWeights {
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        VertexWeights {
            shadow: q(shadow),
            mid: [q(mid[0]), q(mid[1]), q(mid[2])],
            high: [q(high[0]), q(high[1]), q(high[2])],
            alpha: q(alpha),
        }
    }
}

/// Geometry of one mesh: an indexed triangle list. Indices are relative to `vertices`, so one mesh can have at
/// most 65536 vertices (the arena adds a per-mesh base vertex).
#[derive(Clone, Copy, Debug)]
pub struct MeshData<'a> {
    pub vertices: &'a [Vertex],
    pub indices: &'a [u16],
}

macro_rules! handle {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(pub(crate) u32);

        impl $name {
            /// The handle's index (dense, from 0, in creation order).
            pub fn index(self) -> u32 {
                self.0
            }
        }
    };
}

handle!(
    /// A mesh in the level's geometry arena.
    MeshId
);
handle!(
    /// A texture (one layer of a texture pool).
    TextureId
);
handle!(
    /// A material: texture, pass and constants.
    MaterialId
);
handle!(
    /// A placed object. Ids of removed instances are reused.
    InstanceId
);

/// Texel formats. Only RGBA8 so far; block-compressed formats (BC1/BC2/BC3) slot in here later, each with its own
/// pools, since a pool is one array texture of a single format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    Rgba8,
}

impl TextureFormat {
    pub fn bytes_per_texel(self) -> usize {
        match self {
            TextureFormat::Rgba8 => 4,
        }
    }
}

/// A texture with its mip chain, all levels packed tightly one after another, level 0 first.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub format: TextureFormat,
    pub data: Vec<u8>,
    /// Clamp texture coordinates to the edge instead of repeating (a separate pool; UI glyph sheets).
    pub clamp: bool,
}

impl TextureData {
    /// Largest supported edge length.
    pub const MAX_SIZE: u32 = 4096;

    /// The same texture, sampled with clamp-to-edge.
    pub fn clamped(self) -> TextureData {
        TextureData { clamp: true, ..self }
    }

    /// Number of levels in a full mip chain down to 1x1.
    pub fn full_mip_count(width: u32, height: u32) -> u32 {
        32 - width.max(height).max(1).leading_zeros()
    }

    /// Size of one level.
    pub fn level_size(&self, level: u32) -> (u32, u32) {
        ((self.width >> level).max(1), (self.height >> level).max(1))
    }

    /// Byte range of one level within `data`.
    pub fn level_range(&self, level: u32) -> core::ops::Range<usize> {
        let bpp = self.format.bytes_per_texel();
        let mut off = 0;
        for l in 0..level {
            let (w, h) = self.level_size(l);
            off += w as usize * h as usize * bpp;
        }
        let (w, h) = self.level_size(level);
        off..off + w as usize * h as usize * bpp
    }

    /// Total bytes of `levels` levels.
    fn chain_bytes(width: u32, height: u32, levels: u32, format: TextureFormat) -> usize {
        (0..levels)
            .map(|l| (width >> l).max(1) as usize * (height >> l).max(1) as usize * format.bytes_per_texel())
            .sum()
    }

    /// RGBA8 texture without mips. `pixels` is row-major, top row first.
    pub fn rgba8(width: u32, height: u32, pixels: Vec<u8>) -> Result<TextureData, RenderError> {
        Self::check_size(width, height)?;
        if pixels.len() != width as usize * height as usize * 4 {
            return Err(RenderError::BadTexture("pixel data does not match the size"));
        }
        Ok(TextureData { width, height, mip_levels: 1, format: TextureFormat::Rgba8, data: pixels, clamp: false })
    }

    /// RGBA8 texture with a full mip chain made by 2x2 box filtering (edges of odd sizes are clamped).
    pub fn rgba8_with_mips(width: u32, height: u32, pixels: Vec<u8>) -> Result<TextureData, RenderError> {
        let base = Self::rgba8(width, height, pixels)?;
        let levels = Self::full_mip_count(width, height);
        let mut data = base.data;
        data.reserve(Self::chain_bytes(width, height, levels, TextureFormat::Rgba8) - data.len());
        let mut src_off = 0;
        for l in 1..levels {
            let (sw, sh) = ((width >> (l - 1)).max(1) as usize, (height >> (l - 1)).max(1) as usize);
            let (dw, dh) = ((width >> l).max(1) as usize, (height >> l).max(1) as usize);
            for y in 0..dh {
                for x in 0..dw {
                    let (x0, y0) = ((2 * x).min(sw - 1), (2 * y).min(sh - 1));
                    let (x1, y1) = ((2 * x + 1).min(sw - 1), (2 * y + 1).min(sh - 1));
                    for c in 0..4 {
                        let px = |xx: usize, yy: usize| data[src_off + (yy * sw + xx) * 4 + c] as u32;
                        let sum = px(x0, y0) + px(x1, y0) + px(x0, y1) + px(x1, y1);
                        data.push(((sum + 2) / 4) as u8);
                    }
                }
            }
            src_off += sw * sh * 4;
        }
        Ok(TextureData { width, height, mip_levels: levels, format: TextureFormat::Rgba8, data, clamp: false })
    }

    fn check_size(width: u32, height: u32) -> Result<(), RenderError> {
        if width == 0 || height == 0 || width > Self::MAX_SIZE || height > Self::MAX_SIZE {
            return Err(RenderError::BadTexture("size out of range"));
        }
        Ok(())
    }

    /// Checks that `data` holds exactly `mip_levels` levels.
    pub fn validate(&self) -> Result<(), RenderError> {
        Self::check_size(self.width, self.height)?;
        if self.mip_levels == 0 || self.mip_levels > Self::full_mip_count(self.width, self.height) {
            return Err(RenderError::BadTexture("bad mip level count"));
        }
        if self.data.len() != Self::chain_bytes(self.width, self.height, self.mip_levels, self.format) {
            return Err(RenderError::BadTexture("data length does not match the mip chain"));
        }
        Ok(())
    }
}

/// Render pass: the fixed-function state a material draws with. Within one [`Material::order`] list the passes
/// are drawn in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u32)]
pub enum Pass {
    /// Backdrops: no depth test, no depth writes, blending as [`Material::blend`] says. Give it the lowest order so
    /// everything else covers it.
    Background = 0,
    /// Depth test and write, no blending, no discard (keeps early depth testing on).
    Opaque = 1,
    /// Like opaque, but texels with alpha below the material's cutoff are discarded.
    AlphaTest = 2,
    /// Depth test without write, blending ([`BlendMode`]). With [`Material::two_step`], each draw is preceded (as a
    /// list) by one with depth writes that keeps only texels at or above the cutoff.
    Blend = 3,
    /// Stencil volume (blob shadows, light pools): marks where scene surfaces lie inside the closed mesh, then
    /// paints the material's colour there ([`BlendMode`]).
    Volume = 4,
    /// Drawn after the scene in world space with no depth test or writes, so never hidden (blending per
    /// [`Material::blend`]).
    Post = 5,
    /// Screen-space overlay (text, stats, UI): the instance transform maps straight to clip space (no camera, no
    /// culling), no depth test, alpha blending, no face culling. Drawn last.
    Overlay = 6,
    /// Silhouette edges of outline meshes ([`crate::Renderer::add_outline_mesh`]): depth test less-or-equal with
    /// writes, alpha over, a fixed pixel width; the tint is the colour.
    Outline = 7,
    /// Masked areas, step 1 (Sly 1 pass 10): each mesh twice, depth less-or-equal without writes: its back faces
    /// writing only the destination alpha (set to the source alpha), then its front faces with alpha over.
    MaskWrite = 8,
    /// Masked areas, step 3 (Sly 1 pass 12): each mesh twice, depth less-or-equal without writes: first
    /// `src * (1 - dst_alpha) + dst * dst_alpha`, then alpha over.
    MaskFill = 9,
}

impl Pass {
    pub const ALL: [Pass; 10] = [
        Pass::Background,
        Pass::Opaque,
        Pass::AlphaTest,
        Pass::Blend,
        Pass::Volume,
        Pass::Post,
        Pass::Overlay,
        Pass::Outline,
        Pass::MaskWrite,
        Pass::MaskFill,
    ];

    pub fn from_u32(v: u32) -> Option<Pass> {
        Pass::ALL.get(v as usize).copied()
    }

    /// The default [`Material::order`]: backgrounds first, overlays last, the rest in between.
    pub const fn default_order(self) -> u8 {
        match self {
            Pass::Background => 0,
            Pass::Post => 30,
            Pass::Overlay => 31,
            _ => 16,
        }
    }
}

/// How a material combines with the framebuffer (blended, background, volume and post passes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BlendMode {
    /// No blending: the colour replaces the framebuffer's.
    None,
    /// `src * src_alpha + dst * (1 - src_alpha)`.
    #[default]
    Alpha,
    /// `src * src_alpha + dst`.
    Additive,
    /// `src * (1 - src_alpha) + dst * src_alpha`.
    ReverseAlpha,
    /// `src + dst * src_alpha`.
    ReverseAdd,
    /// By destination alpha: `src * dst_alpha + dst * (1 - dst_alpha)`.
    DestAlpha,
    /// By inverse destination alpha: `src * (1 - dst_alpha) + dst * dst_alpha`.
    InvDestAlpha,
    /// `dst * src` (projected shadows).
    Multiply,
}

/// The order of draws inside one list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SortMode {
    /// Grouped by texture pool and mesh, for the fewest draws. The default for every pass but [`Pass::Blend`].
    #[default]
    Grouped,
    /// Farthest first: by the instance's sort key ([`crate::Renderer::set_sort_key`]) when it has one, otherwise
    /// the squared distance from the camera to its bounding-sphere centre. Ties keep instance order.
    FarToNear,
}

/// How a material is shaded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lighting {
    /// Texture x vertex colour x tint as they are.
    #[default]
    Unlit,
    /// Lambert from the frame's light direction plus ambient ([`crate::Renderer::set_light`]).
    Lambert,
    /// Three images blended per vertex with weights the caller computed ([`crate::Renderer::add_vertex_weights`]):
    /// `rgb = T0 * wS + T1 * wM + T2 * wH`, `a = weight alpha * T1.a`. The texture is T1; T0 and T2 are
    /// [`Material::palettes`], which must be in the same pool (same size and mip count). An instance without
    /// weights draws T1 x vertex colour.
    Palettes,
    /// Sly 2's two colours per vertex (its rendering.md 2.4 and 3.2), from [`crate::Renderer::add_vertex_weights`]
    /// with `shadow` = A's alpha, `mid` = A, `high` = B, `alpha` = B's alpha, each on the 0x80 = 1.0 scale:
    /// `rgb = min(T0 x A, 1) + min(T1 x B, 1) x T1.a x B.a`, `a = T0.a x A.a`. T0 is the first of
    /// [`Material::palettes`] (the grey image); T1 the texture. An instance without weights draws T1 x vertex colour.
    TwoColour,
}

/// Which triangles to discard. Front faces are counter-clockwise as seen by the camera (in a right-handed world).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum CullMode {
    None = 0,
    #[default]
    Back = 1,
    Front = 2,
}

/// Surface description shared by many instances.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    pub texture: TextureId,
    pub pass: Pass,
    /// Draw-order list: lists are drawn in increasing order, and inside a list by pass ([`Pass`] order).
    /// 0 to 31; [`Pass::default_order`] by default.
    pub order: u8,
    /// Multiplies the texture and vertex colour (RGBA8).
    pub tint: [u8; 4],
    /// For [`Pass::AlphaTest`] and two-step blends: texels with alpha below this are discarded.
    pub alpha_cutoff: f32,
    /// For the blended, background, volume and post passes: how the colour is blended.
    pub blend: BlendMode,
    pub lighting: Lighting,
    /// For [`Pass::Blend`]: draw first with depth writes, keeping texels at or above [`Material::alpha_cutoff`],
    /// then blended without depth writes.
    pub two_step: bool,
    pub sort: SortMode,
    /// For [`Lighting::Palettes`]: the images weighted by wS and wH (T0 and T2).
    pub palettes: Option<[TextureId; 2]>,
    /// Not affected by the frame's darkening factor ([`crate::Renderer::set_darkening`]).
    pub no_darkening: bool,
    /// Not fogged.
    pub no_fog: bool,
}

impl Material {
    /// An opaque, Lambert-lit material with no tint.
    pub fn opaque(texture: TextureId) -> Material {
        Material {
            texture,
            pass: Pass::Opaque,
            order: Pass::Opaque.default_order(),
            tint: [255; 4],
            alpha_cutoff: 0.0,
            blend: BlendMode::Alpha,
            lighting: Lighting::Lambert,
            two_step: false,
            sort: SortMode::Grouped,
            palettes: None,
            no_darkening: false,
            no_fog: false,
        }
    }

    /// The same material in another pass, with that pass's default order and sort mode.
    pub fn with_pass(self, pass: Pass) -> Material {
        let sort = if pass == Pass::Blend { SortMode::FarToNear } else { SortMode::Grouped };
        Material { pass, order: pass.default_order(), sort, ..self }
    }
}

/// Errors from the renderer and its backends.
#[derive(Clone, Debug, PartialEq)]
pub enum RenderError {
    /// A mesh with no vertices or indices, or an index count that is not a multiple of 3.
    BadMesh(&'static str),
    /// An index refers past the mesh's vertices.
    IndexOutOfRange,
    /// A texture's size, format or data is wrong.
    BadTexture(&'static str),
    /// A handle that does not belong to this renderer (or a removed instance).
    InvalidHandle,
    /// Resources can only be added before [`crate::Renderer::commit`]; call `clear_level` first.
    AlreadyCommitted,
    /// [`crate::Renderer::render`] before [`crate::Renderer::commit`].
    NotCommitted,
    /// A fixed limit (instances, draws, arena size, pool count) was reached.
    LimitReached(&'static str),
    /// The backend failed; the message says how.
    Backend(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::BadMesh(why) => write!(f, "bad mesh: {why}"),
            RenderError::IndexOutOfRange => write!(f, "mesh index out of range"),
            RenderError::BadTexture(why) => write!(f, "bad texture: {why}"),
            RenderError::InvalidHandle => write!(f, "invalid handle"),
            RenderError::AlreadyCommitted => write!(f, "resources already committed"),
            RenderError::NotCommitted => write!(f, "resources not committed"),
            RenderError::LimitReached(what) => write!(f, "limit reached: {what}"),
            RenderError::Backend(msg) => write!(f, "backend error: {msg}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn normal_packing() {
        let v = Vertex::new([0.0; 3], [0.0; 2], [1.0, -1.0, 0.0], [0; 4]);
        assert_eq!(v.normal, [127, -127, 0, 0]);
    }

    #[test]
    fn mip_chain() {
        assert_eq!(TextureData::full_mip_count(1, 1), 1);
        assert_eq!(TextureData::full_mip_count(64, 64), 7);
        assert_eq!(TextureData::full_mip_count(128, 32), 8);
        // 4x2: level 1 is 2x1, level 2 is 1x1.
        let mut px = vec![0u8; 4 * 2 * 4];
        for (i, p) in px.chunks_mut(4).enumerate() {
            p.copy_from_slice(&[(i * 10) as u8, 0, 0, 255]);
        }
        let t = TextureData::rgba8_with_mips(4, 2, px).unwrap();
        assert_eq!(t.mip_levels, 3);
        t.validate().unwrap();
        assert_eq!(t.level_range(1), 32..40);
        assert_eq!(t.level_range(2), 40..44);
        // Level 1 texel 0 averages texels (0,0),(1,0),(0,1),(1,1) = 0,10,40,50 -> 25.
        assert_eq!(t.data[32], 25);
        // Level 2 averages level 1's two texels with the clamped edge.
        let l1 = (t.data[32] as u32, t.data[36] as u32);
        assert_eq!(t.data[40] as u32, (2 * l1.0 + 2 * l1.1 + 2) / 4);
    }

    #[test]
    fn bad_textures() {
        assert!(TextureData::rgba8(0, 4, vec![]).is_err());
        assert!(TextureData::rgba8(2, 2, vec![0; 15]).is_err());
        let mut t = TextureData::rgba8(2, 2, vec![0; 16]).unwrap();
        t.mip_levels = 2;
        assert!(t.validate().is_err());
    }
}
