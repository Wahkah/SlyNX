//! Level resources on the CPU side: the geometry arena being built, texture pools and materials.
//!
//! Everything is gathered during level load and handed to the backend in one [`LevelUpload`] by
//! [`crate::Renderer::commit`]. After the upload the CPU copies of vertices, indices and texels are dropped; only
//! the small tables the frame builder needs (mesh ranges and bounds, texture locations, materials) stay.

use crate::math::{Vec3, sqrt};
use crate::types::{
    Material, MaterialId, MeshData, MeshId, RenderError, SkinVertex, TextureData, TextureFormat, TextureId, VertexExtra,
};
use alloc::vec::Vec;

/// Most layers in one pool (2D array texture). 256 is the minimum `GL_MAX_ARRAY_TEXTURE_LAYERS` of OpenGL 3.0 and
/// leaves room on every GPU; a full pool simply starts another one with the same key.
pub const MAX_POOL_LAYERS: u32 = 256;
/// Most pools. Pool indices go into sort keys (10 bits).
pub const MAX_POOLS: usize = 1024;
/// Most meshes. Mesh indices go into sort keys (20 bits).
pub const MAX_MESHES: usize = 1 << 20;

/// Where a mesh lives in the arena, and its bounding sphere in model space.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MeshInfo {
    pub first_index: u32,
    pub index_count: u32,
    pub base_vertex: i32,
    pub vertex_count: u32,
    /// For an outline mesh: its first edge point in the outline point buffer.
    pub outline_base: Option<u32>,
    /// For a placed mesh: its first placement record (four points each) in the outline point buffer.
    pub placed_base: Option<u32>,
    pub center: Vec3,
    pub radius: f32,
}

/// One texture pool: an array texture whose layers all share size, mip count and format.
#[derive(Clone, Debug)]
pub(crate) struct PoolInfo {
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub format: TextureFormat,
    pub clamp: bool,
    pub layers: Vec<TextureData>,
}

/// Where a texture lives.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TexLocation {
    pub pool: u16,
    pub layer: u16,
}

/// What [`crate::Backend::upload_level`] receives.
#[derive(Debug)]
pub struct LevelUpload<'a> {
    pub vertices: &'a [crate::Vertex],
    /// One per vertex: the second vertex stream (skinning data, index within the mesh).
    pub extras: &'a [crate::VertexExtra],
    /// Per-instance vertex weights ([`crate::Renderer::add_vertex_weights`]).
    pub weights: &'a [crate::VertexWeights],
    /// Edge points of the outline meshes, four per edge (xyz, w unused).
    pub outline_points: &'a [[f32; 4]],
    pub indices: &'a [u16],
    pub pools: &'a [PoolUpload<'a>],
}

/// One texture pool to create: `layers.len()` layers of `width` x `height` with `mip_levels` levels each.
#[derive(Debug)]
pub struct PoolUpload<'a> {
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub format: TextureFormat,
    /// Clamp-to-edge sampling instead of repeat.
    pub clamp: bool,
    pub layers: &'a [TextureData],
}

#[derive(Default)]
pub(crate) struct Resources {
    pub vertices: Vec<crate::Vertex>,
    pub extras: Vec<crate::VertexExtra>,
    pub indices: Vec<u16>,
    pub meshes: Vec<MeshInfo>,
    pub pools: Vec<PoolInfo>,
    pub textures: Vec<TexLocation>,
    pub materials: Vec<Material>,
    /// Texture coordinate offset of each material (texture animation).
    pub uv_offsets: Vec<[f32; 2]>,
    pub weights: Vec<crate::VertexWeights>,
    pub outline_points: Vec<[f32; 4]>,
    /// Length of the committed weight buffer.
    pub weight_count: u32,
    pub committed: bool,
    pub arena_vertices: u32,
    pub arena_indices: u32,
}

impl Resources {
    fn check_open(&self) -> Result<(), RenderError> {
        if self.committed { Err(RenderError::AlreadyCommitted) } else { Ok(()) }
    }

    pub fn add_mesh(&mut self, mesh: &MeshData<'_>, skin: Option<&[SkinVertex]>) -> Result<MeshId, RenderError> {
        self.check_open()?;
        if skin.is_some_and(|s| s.len() != mesh.vertices.len()) {
            return Err(RenderError::BadMesh("skin data does not match the vertex count"));
        }
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            return Err(RenderError::BadMesh("empty"));
        }
        if mesh.indices.len() % 3 != 0 {
            return Err(RenderError::BadMesh("index count is not a multiple of 3"));
        }
        if mesh.vertices.len() > 65536 {
            return Err(RenderError::BadMesh("more than 65536 vertices; split the mesh"));
        }
        if mesh.indices.iter().any(|&i| i as usize >= mesh.vertices.len()) {
            return Err(RenderError::IndexOutOfRange);
        }
        if self.meshes.len() >= MAX_MESHES {
            return Err(RenderError::LimitReached("meshes"));
        }
        if self.vertices.len() + mesh.vertices.len() > i32::MAX as usize
            || self.indices.len() + mesh.indices.len() > u32::MAX as usize
        {
            return Err(RenderError::LimitReached("arena size"));
        }

        // Bounding sphere: centre of the axis-aligned box, radius to the farthest vertex.
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in mesh.vertices {
            for k in 0..3 {
                lo[k] = lo[k].min(v.pos[k]);
                hi[k] = hi[k].max(v.pos[k]);
            }
        }
        let center = Vec3::new((lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5);
        let r2 = mesh
            .vertices
            .iter()
            .map(|v| Vec3::new(v.pos[0], v.pos[1], v.pos[2]) - center)
            .fold(0.0f32, |m, d| m.max(d.dot(d)));

        let info = MeshInfo {
            first_index: self.indices.len() as u32,
            index_count: mesh.indices.len() as u32,
            base_vertex: self.vertices.len() as i32,
            vertex_count: mesh.vertices.len() as u32,
            outline_base: None,
            placed_base: None,
            center,
            radius: sqrt(r2),
        };
        self.vertices.extend_from_slice(mesh.vertices);
        self.extras.extend((0..mesh.vertices.len()).map(|i| {
            let s = skin.map_or(SkinVertex::NONE, |s| s[i]);
            VertexExtra { bones: s.bones, weights: s.weights, local: i as u16, pad: 0 }
        }));
        self.indices.extend_from_slice(mesh.indices);
        self.meshes.push(info);
        Ok(MeshId(self.meshes.len() as u32 - 1))
    }

    pub fn add_texture(&mut self, tex: TextureData) -> Result<TextureId, RenderError> {
        self.check_open()?;
        tex.validate()?;
        if self.textures.len() >= u32::MAX as usize {
            return Err(RenderError::LimitReached("textures"));
        }
        let found = self.pools.iter().position(|p| {
            p.width == tex.width
                && p.height == tex.height
                && p.mip_levels == tex.mip_levels
                && p.format == tex.format
                && p.clamp == tex.clamp
                && (p.layers.len() as u32) < MAX_POOL_LAYERS
        });
        let pool = match found {
            Some(p) => p,
            None => {
                if self.pools.len() >= MAX_POOLS {
                    return Err(RenderError::LimitReached("texture pools"));
                }
                self.pools.push(PoolInfo {
                    width: tex.width,
                    height: tex.height,
                    mip_levels: tex.mip_levels,
                    format: tex.format,
                    clamp: tex.clamp,
                    layers: Vec::new(),
                });
                self.pools.len() - 1
            }
        };
        let layer = self.pools[pool].layers.len();
        self.pools[pool].layers.push(tex);
        self.textures.push(TexLocation { pool: pool as u16, layer: layer as u16 });
        Ok(TextureId(self.textures.len() as u32 - 1))
    }

    pub fn check_material(&self, mat: &Material) -> Result<(), RenderError> {
        let pool = |t: crate::TextureId| self.textures.get(t.0 as usize).map(|l| l.pool);
        let main = pool(mat.texture).ok_or(RenderError::InvalidHandle)?;
        if !(0.0..=1.0).contains(&mat.alpha_cutoff) {
            return Err(RenderError::BadTexture("alpha cutoff outside 0..1"));
        }
        if mat.order > 31 {
            return Err(RenderError::LimitReached("material order above 31"));
        }
        if let Some(extra) = mat.palettes {
            for t in extra {
                if pool(t).ok_or(RenderError::InvalidHandle)? != main {
                    return Err(RenderError::BadTexture("palette images must share the texture's pool"));
                }
            }
        }
        if matches!(mat.lighting, crate::types::Lighting::Palettes | crate::types::Lighting::TwoColour)
            && mat.palettes.is_none()
        {
            return Err(RenderError::BadTexture("palette lighting without palette images"));
        }
        Ok(())
    }

    pub fn add_material(&mut self, mat: Material) -> Result<MaterialId, RenderError> {
        self.check_material(&mat)?;
        self.materials.push(mat);
        self.uv_offsets.push([0.0; 2]);
        Ok(MaterialId(self.materials.len() as u32 - 1))
    }

    /// Calls `f` with the upload description, then drops the CPU copies of the bulk data.
    pub fn commit_with(
        &mut self,
        f: impl FnOnce(&LevelUpload<'_>) -> Result<(), RenderError>,
    ) -> Result<(), RenderError> {
        self.check_open()?;
        let pools: Vec<PoolUpload<'_>> = self
            .pools
            .iter()
            .map(|p| PoolUpload {
                width: p.width,
                height: p.height,
                mip_levels: p.mip_levels,
                format: p.format,
                clamp: p.clamp,
                layers: &p.layers,
            })
            .collect();
        f(&LevelUpload {
            vertices: &self.vertices,
            extras: &self.extras,
            weights: &self.weights,
            outline_points: &self.outline_points,
            indices: &self.indices,
            pools: &pools,
        })?;
        self.weight_count = self.weights.len() as u32;
        self.weights = Vec::new();
        self.outline_points = Vec::new();
        drop(pools);
        self.arena_vertices = self.vertices.len() as u32;
        self.arena_indices = self.indices.len() as u32;
        self.vertices = Vec::new();
        self.extras = Vec::new();
        self.indices = Vec::new();
        for p in &mut self.pools {
            p.layers = Vec::new();
        }
        self.committed = true;
        Ok(())
    }
}
