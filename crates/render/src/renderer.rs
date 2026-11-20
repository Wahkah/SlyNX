//! The renderer front end: resources, instances, camera and the per-frame call.

use crate::backend::Backend;
use crate::frame::{
    FrameBuilder, FramePacket, FrameStats, FrameUniforms, InstanceRec, ProjectorRef, SILHOUETTE_SIZE, VIEW_CLEAR_COLOR,
    VIEW_CLEAR_DEPTH, VIEW_TARGET_MAIN, VIEW_TARGET_SILHOUETTE, View, ViewMode, default_light,
};
use crate::math::{Frustum, Mat4, Vec3};
use crate::resources::Resources;
use crate::types::{
    BoneMatrix, CullMode, InstanceId, Material, MaterialId, MeshData, MeshId, MorphVertex, RenderError, SkinVertex,
    TextureData, TextureId, VertexWeights,
};
use alloc::vec::Vec;

/// Distance fog: per vertex, `t = clamp((1/near - 1/z) / (1/near - 1/far), 0, 1)` with `z` the distance from the
/// camera, `fog = clamp(t * cap * alpha * instance fog factor, 0, 1)`, then per pixel the colour moves toward
/// `color` by `fog`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub near: f32,
    pub far: f32,
    pub cap: f32,
    /// RGB 0..1 and the strength (alpha) 0..1.
    pub color: [f32; 4],
}

/// A perspective camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn look_at(eye: Vec3, target: Vec3) -> Camera {
        Camera { eye, target, up: Vec3::Y, fov_y: 1.0, near: 0.5, far: 1000.0 }
    }

    pub fn view_proj(&self, aspect: f32, clip: crate::ClipSpace) -> Mat4 {
        Mat4::perspective(self.fov_y, aspect, self.near, self.far, clip) * Mat4::look_at(self.eye, self.target, self.up)
    }
}

/// The renderer. Typical use per level:
///
/// 1. `add_mesh` / `add_texture` / `add_material` for everything the level needs, then `commit` (one upload);
/// 2. `add_instance` for each placed object, `set_transform` as objects move;
/// 3. `render` once per frame;
/// 4. `clear_level` before loading the next level.
pub struct Renderer<B: Backend> {
    backend: B,
    res: Resources,
    instances: Vec<InstanceRec>,
    free: Vec<u32>,
    live: u32,
    builder: FrameBuilder,
    clock: Option<fn() -> u64>,
    cull: CullMode,
    light: [f32; 4],
    fog: Option<Fog>,
    fog_rising: bool,
    /// The performance text ([`crate::perf`]): its capacity (0: off), and the overlay of the committed level.
    perf_chars: usize,
    perf_text: Option<crate::overlay::TextOverlay>,
    darkening: f32,
    /// Outline width in pixels of a 448-line frame (scaled with the target height).
    outline_width: f32,
    /// Bone matrices of all skinned instances; each owns a range ([`InstanceRec::bones`]).
    bone_store: Vec<BoneMatrix>,
    bone_waste: usize,
    /// Morphed vertices of all morphed instances; each owns a range of its mesh's vertex count.
    morph_store: Vec<MorphVertex>,
    morph_waste: usize,
    /// Picture panels drawn this frame, and the panel image each draws into (`None`: the frame).
    panels: Vec<(Panel, Option<u8>)>,
    views: Vec<View>,
    projectors: Vec<Projector>,
    /// The latest movie frame (Y, U, V back to back), its size, and whether it is still to be uploaded.
    video_planes: Vec<u8>,
    video_size: (u32, u32),
    video_new: bool,
}

/// A projector (rendering.md 9.2): casts the silhouette image straight down (-Z) from `origin`. At `d` units below
/// the origin the image covers a square of half-width `r(d)`, from `near_radius` at `near_distance` to `far_radius`
/// at `far_distance`; points beyond either distance are projected as if on that plane (clamped, no fade), and points
/// more than [`PROJECTOR_MARGIN`] units above the near plane are not affected. An orthographic projector (equal radii)
/// neither clamps nor cuts. Receiving pixels are multiplied by `1 - m * strength`, with `m` the silhouette's alpha at
/// the projected point (0 outside the image); pixels with alpha 0.9 or less are not affected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projector {
    pub origin: Vec3,
    pub near_radius: f32,
    pub far_radius: f32,
    pub near_distance: f32,
    pub far_distance: f32,
    /// The projector shader's material alpha.
    pub strength: f32,
    /// Receiver bits it acts on ([`Renderer::set_receiver`]).
    pub receivers: u8,
}

impl Projector {
    /// Half-width of the projected square at `d` units below the origin.
    pub fn radius_at(&self, d: f32) -> f32 {
        let span = self.far_distance - self.near_distance;
        if span.abs() < 1e-6 {
            return self.near_radius;
        }
        let t = ((d - self.near_distance) / span).clamp(0.0, 1.0);
        self.near_radius + (self.far_radius - self.near_radius) * t
    }

    /// Whether the projector is orthographic (equal radii): no clamping, no cut.
    pub fn orthographic(&self) -> bool {
        self.near_radius == self.far_radius
    }

    /// Texture coordinates of world point `p` in the silhouette image (0..1 inside it), or `None` where the effect is
    /// cut (more than the margin above the near plane).
    pub fn project(&self, p: Vec3) -> Option<[f32; 2]> {
        let d = self.origin.z - p.z;
        if !self.orthographic() && d < self.near_distance - PROJECTOR_MARGIN {
            return None;
        }
        let r = self.radius_at(d).max(1e-3);
        Some([(p.x - self.origin.x) / r * 0.5 + 0.5, (p.y - self.origin.y) / r * 0.5 + 0.5])
    }

    /// The parameters as the shaders read them (three rows at the projector's bone index).
    fn rows(&self) -> BoneMatrix {
        let o = self.origin;
        BoneMatrix {
            rows: [
                [o.x, o.y, o.z, self.near_radius],
                [self.near_distance, self.far_distance, self.far_radius, self.strength],
                [if self.orthographic() { f32::MAX } else { PROJECTOR_MARGIN }, 0.0, 0.0, 0.0],
            ],
        }
    }
}

/// One piece of a placed mesh ([`Renderer::add_placed_mesh`]).
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedPiece {
    /// Its vertices in the mesh.
    pub vertices: core::ops::Range<u32>,
    /// Model space to world space.
    pub transform: Mat4,
    pub texture: TextureId,
    /// Multiplies the vertex colour (the material colour of the piece's own material).
    pub tint: [u8; 4],
    /// For [`crate::Lighting::TwoColour`]: the piece's grey image (T0), in the same pool.
    pub grey: Option<TextureId>,
}

/// Most projectors per frame.
pub const MAX_PROJECTORS: usize = 4;
/// How far above its near casting distance (toward the caster) a projector still acts (rendering.md 9.2).
pub const PROJECTOR_MARGIN: f32 = 50.0;

/// A picture panel: the scene drawn again from another camera into a rectangle of the target (Sly's dialogue
/// panels). Screen-space materials ([`Pass::Overlay`]: the UI, wipes, text) are not drawn in panels; the main
/// view's screen-space draws come after all panels, so UI frames around a panel cover it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    pub camera: Camera,
    /// x, y (from the top left), width, height in pixels of the target. Clipped to the target.
    pub rect: [f32; 4],
    /// Background colour of the panel; `None` keeps what the main view drew there (depth is cleared either way).
    pub clear: Option<[f32; 4]>,
}

/// Most picture panels per frame.
pub const MAX_PANELS: usize = 4;

/// Most bone matrices one instance can have (bone indices are 8-bit).
pub const MAX_BONES_PER_INSTANCE: usize = 256;
/// Default outline width: 4.125 pixels of a 448-line frame.
pub const DEFAULT_OUTLINE_WIDTH: f32 = 4.125;
/// Most edges in one outline mesh (four vertices each, 16-bit indices).
pub const MAX_OUTLINE_EDGES: usize = 16384;

impl<B: Backend> Renderer<B> {
    pub fn new(backend: B) -> Renderer<B> {
        Renderer {
            backend,
            res: Resources::default(),
            instances: Vec::new(),
            free: Vec::new(),
            live: 0,
            builder: FrameBuilder::default(),
            clock: None,
            cull: CullMode::Back,
            light: default_light(),
            fog: None,
            fog_rising: false,
            perf_chars: 0,
            perf_text: None,
            darkening: 1.0,
            outline_width: DEFAULT_OUTLINE_WIDTH,
            bone_store: Vec::new(),
            bone_waste: 0,
            morph_store: Vec::new(),
            morph_waste: 0,
            panels: Vec::new(),
            views: Vec::new(),
            projectors: Vec::new(),
            video_planes: Vec::new(),
            video_size: (0, 0),
            video_new: false,
        }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// A monotonic nanosecond clock for the CPU timings in [`FrameStats`]. Without one, timings are zero.
    pub fn set_clock(&mut self, clock: fn() -> u64) {
        self.clock = Some(clock);
        self.backend.set_clock(clock);
    }

    pub fn set_cull_mode(&mut self, cull: CullMode) {
        self.cull = cull;
    }

    pub fn cull_mode(&self) -> CullMode {
        self.cull
    }

    /// Direction towards the light (normalised here) and ambient level 0..1.
    /// Fog for every instance that is not [`Material::no_fog`]; `None` turns it off.
    pub fn set_fog(&mut self, fog: Option<Fog>) {
        self.fog = fog;
    }

    /// Sly 2 and Sly 3's fog (their rendering.md 6): past the far distance it keeps rising toward the maximum's
    /// asymptote (`cap x (1 - near / d) / (1 - near / far)`, clamped to 1) instead of stopping at the cap. Off by
    /// default and after [`Renderer::clear_level`].
    pub fn set_fog_rising(&mut self, rising: bool) {
        self.fog_rising = rising;
    }

    /// The picture panels to draw from now on (at most [`MAX_PANELS`]; an empty list turns them off). See
    /// [`Panel`]. Each panel culls, sorts and draws the scene again, so it costs about as much CPU and GPU time as
    /// the part of the scene its camera sees.
    pub fn set_panels(&mut self, panels: &[Panel]) -> Result<(), RenderError> {
        if panels.len() > MAX_PANELS {
            return Err(RenderError::LimitReached("picture panels"));
        }
        self.panels.clear();
        self.panels.extend(panels.iter().map(|p| (*p, None)));
        Ok(())
    }

    /// Like [`Renderer::set_panels`], each panel drawn into panel image 0 or 1 ([`crate::panel`]) instead of the
    /// frame: the picture is `rect`'s size (at most [`crate::frame::PANEL_IMAGE_SIZE`] each way) from the image's top
    /// left; `rect`'s position is not used.
    pub fn set_image_panels(&mut self, panels: &[(Panel, u8)]) -> Result<(), RenderError> {
        if panels.len() > MAX_PANELS {
            return Err(RenderError::LimitReached("picture panels"));
        }
        self.panels.clear();
        self.panels.extend(panels.iter().map(|&(p, k)| (p, Some(k))));
        Ok(())
    }

    /// The picture panels set with [`Renderer::set_panels`] or [`Renderer::set_image_panels`].
    pub fn panels(&self) -> impl Iterator<Item = &Panel> {
        self.panels.iter().map(|p| &p.0)
    }

    /// The world darkening factor: final colours (before fog) of materials that are not
    /// [`Material::no_darkening`] are multiplied by it. 1 by default.
    pub fn set_darkening(&mut self, k: f32) {
        self.darkening = k.max(0.0);
    }

    pub fn set_light(&mut self, towards_light: Vec3, ambient: f32) {
        let d = towards_light.normalize();
        self.light = [d.x, d.y, d.z, ambient.clamp(0.0, 1.0)];
    }

    pub fn add_mesh(&mut self, mesh: &MeshData<'_>) -> Result<MeshId, RenderError> {
        self.res.add_mesh(mesh, None)
    }

    /// A mesh with skinning data, one [`SkinVertex`] per vertex. Its instances are drawn rigidly until they get
    /// bone matrices ([`Renderer::set_bones`]).
    pub fn add_skinned_mesh(&mut self, mesh: &MeshData<'_>, skin: &[SkinVertex]) -> Result<MeshId, RenderError> {
        self.res.add_mesh(mesh, Some(skin))
    }

    pub fn add_texture(&mut self, tex: TextureData) -> Result<TextureId, RenderError> {
        self.res.add_texture(tex)
    }

    /// Materials may be added before or after `commit`.
    pub fn add_material(&mut self, mat: Material) -> Result<MaterialId, RenderError> {
        self.res.add_material(mat)
    }

    /// Uploads the arena and texture pools in one go and frees their CPU copies.
    pub fn commit(&mut self) -> Result<(), RenderError> {
        // The performance text rides along with every level, once enabled.
        if self.perf_chars > 0 && self.perf_text.is_none() && !self.res.committed {
            self.perf_text = Some(crate::overlay::TextOverlay::new(self, self.perf_chars)?);
        }
        let backend = &mut self.backend;
        self.res.commit_with(|level| backend.upload_level(level))
    }

    /// Draws the performance text ([`crate::perf`]) with every level committed from now on (up to `chars`
    /// characters; the next commit adds it).
    pub fn enable_perf_text(&mut self, chars: usize) {
        self.perf_chars = chars;
    }

    /// Shows `lines` as the performance text in the bottom-left corner (an empty slice hides it). Does nothing until
    /// a level was committed with the text enabled.
    pub fn set_perf_text(&mut self, lines: &[&str]) -> Result<(), RenderError> {
        let Some(mut t) = self.perf_text.take() else { return Ok(()) };
        let (_, h) = self.backend.target_size();
        let scale = if h >= 1000 { 3 } else { 2 };
        let height = lines.len() as u32 * crate::overlay::LINE_HEIGHT * scale;
        let y = h.saturating_sub(height + 8 * scale);
        let result = t.set_text(self, lines, 8 * scale, y, scale, [255, 255, 160, 255]);
        self.perf_text = Some(t);
        result
    }

    /// Forgets all resources and instances (the GPU data is replaced by the next `commit`).
    pub fn clear_level(&mut self) {
        self.res = Resources::default();
        self.instances.clear();
        self.free.clear();
        self.live = 0;
        self.bone_store.clear();
        self.bone_waste = 0;
        self.morph_store.clear();
        self.morph_waste = 0;
        self.fog_rising = false;
        self.perf_text = None;
    }

    /// (vertices, indices) in the committed arena.
    pub fn arena_size(&self) -> (u32, u32) {
        (self.res.arena_vertices, self.res.arena_indices)
    }

    pub fn texture_pool_count(&self) -> usize {
        self.res.pools.len()
    }

    pub fn add_instance(
        &mut self,
        mesh: MeshId,
        material: MaterialId,
        transform: Mat4,
    ) -> Result<InstanceId, RenderError> {
        if mesh.0 as usize >= self.res.meshes.len() || material.0 as usize >= self.res.materials.len() {
            return Err(RenderError::InvalidHandle);
        }
        if self.live >= self.backend.limits().max_instances {
            return Err(RenderError::LimitReached("instances"));
        }
        let rec = InstanceRec {
            mesh: mesh.0,
            material: material.0,
            transform,
            max_scale: transform.max_scale(),
            tint: [255; 4],
            visible: true,
            alive: true,
            mirrored: transform.determinant3() < 0.0,
            bones: None,
            morph: None,
            sphere: None,
            sort_key: None,
            weights: None,
            shader_light: None,
            fog: 1.0,
            uv_rect: None,
            flat: false,
            nearest: false,
            caster: false,
            receiver: 0,
            video: false,
            panel_image: None,
        };
        self.live += 1;
        if let Some(i) = self.free.pop() {
            self.instances[i as usize] = rec;
            Ok(InstanceId(i))
        } else {
            self.instances.push(rec);
            Ok(InstanceId(self.instances.len() as u32 - 1))
        }
    }

    fn rec_mut(&mut self, id: InstanceId) -> Result<&mut InstanceRec, RenderError> {
        match self.instances.get_mut(id.0 as usize) {
            Some(r) if r.alive => Ok(r),
            _ => Err(RenderError::InvalidHandle),
        }
    }

    pub fn remove_instance(&mut self, id: InstanceId) -> Result<(), RenderError> {
        self.clear_bones(id)?;
        self.clear_morph(id)?;
        self.rec_mut(id)?.alive = false;
        self.free.push(id.0);
        self.live -= 1;
        Ok(())
    }

    pub fn set_transform(&mut self, id: InstanceId, transform: Mat4) -> Result<(), RenderError> {
        let r = self.rec_mut(id)?;
        r.transform = transform;
        r.max_scale = transform.max_scale();
        r.mirrored = transform.determinant3() < 0.0;
        Ok(())
    }

    pub fn set_visible(&mut self, id: InstanceId, visible: bool) -> Result<(), RenderError> {
        self.rec_mut(id)?.visible = visible;
        Ok(())
    }

    /// Per-instance colour multiplied with the material tint.
    pub fn set_tint(&mut self, id: InstanceId, tint: [u8; 4]) -> Result<(), RenderError> {
        self.rec_mut(id)?.tint = tint;
        Ok(())
    }

    pub fn set_material(&mut self, id: InstanceId, material: MaterialId) -> Result<(), RenderError> {
        if material.0 as usize >= self.res.materials.len() {
            return Err(RenderError::InvalidHandle);
        }
        self.rec_mut(id)?.material = material.0;
        Ok(())
    }

    pub fn instance_count(&self) -> u32 {
        self.live
    }

    /// Gives an instance this frame's bone matrices (and keeps them until changed): the instance is drawn skinned,
    /// each vertex moved by `M = sum of w_i * bones[idx_i]` (zero weights skipped, all zero: unmoved) before the
    /// instance transform. Call it whenever the pose changes; the same count reuses the instance's range.
    pub fn set_bones(&mut self, id: InstanceId, bones: &[BoneMatrix]) -> Result<(), RenderError> {
        if bones.is_empty() || bones.len() > MAX_BONES_PER_INSTANCE {
            return Err(RenderError::LimitReached("bones per instance"));
        }
        let current = self.rec_mut(id)?.bones;
        let start = match current {
            Some((start, n)) if n as usize == bones.len() => start as usize,
            _ => {
                if let Some((_, n)) = current {
                    self.bone_waste += n as usize;
                }
                let start = self.alloc_bones(bones.len())?;
                self.instances[id.0 as usize].bones = Some((start as u32, bones.len() as u32));
                start
            }
        };
        self.bone_store[start..start + bones.len()].copy_from_slice(bones);
        Ok(())
    }

    /// Lights a [`crate::Lighting::TwoColour`] instance in the vertex shader (per frame, as it moves) instead of
    /// from vertex weights: the light block is matrices `first .. first + LIGHT_BLOCK_MATRICES` of the bones given
    /// with [`Renderer::set_bones`] (rows: shadow colour RGB and the starting fourth channel; material colour RGBA
    /// on 0..255; one-pass flag; then per light, four of them: position or direction and point flag, colour, angular
    /// offset, angular scale, distance factor and in-use flag; `crate::frame` and the shader read it). `None` goes
    /// back to the instance's weights.
    pub fn set_shader_light(&mut self, id: InstanceId, first: Option<u32>) -> Result<(), RenderError> {
        self.rec_mut(id)?.shader_light = first;
        Ok(())
    }

    /// Draws the instance rigidly again.
    pub fn clear_bones(&mut self, id: InstanceId) -> Result<(), RenderError> {
        if let Some((_, n)) = self.rec_mut(id)?.bones.take() {
            self.bone_waste += n as usize;
        }
        Ok(())
    }

    /// Gives an instance its own positions and normals (morph targets applied on the CPU), one per vertex of its
    /// mesh; they replace the mesh's before skinning. Kept until changed or cleared.
    pub fn set_morph(
        &mut self,
        id: InstanceId,
        positions: &[[f32; 3]],
        normals: &[[f32; 3]],
    ) -> Result<(), RenderError> {
        let mesh = self.rec_mut(id)?.mesh;
        let count = self.res.meshes[mesh as usize].vertex_count as usize;
        if positions.len() != count || normals.len() != count {
            return Err(RenderError::BadMesh("morph data does not match the mesh's vertex count"));
        }
        let start = match self.instances[id.0 as usize].morph {
            Some(start) => start as usize,
            None => {
                let start = self.alloc_morph(count)?;
                self.instances[id.0 as usize].morph = Some(start as u32);
                start
            }
        };
        for (d, (p, n)) in self.morph_store[start..start + count].iter_mut().zip(positions.iter().zip(normals)) {
            *d = MorphVertex { position: [p[0], p[1], p[2], 1.0], normal: [n[0], n[1], n[2], 0.0] };
        }
        Ok(())
    }

    /// Draws the instance from its mesh's own positions again.
    pub fn clear_morph(&mut self, id: InstanceId) -> Result<(), RenderError> {
        let mesh = self.rec_mut(id)?.mesh;
        if self.instances[id.0 as usize].morph.take().is_some() {
            self.morph_waste += self.res.meshes[mesh as usize].vertex_count as usize;
        }
        Ok(())
    }

    /// A bounding sphere in model space (before the instance transform) for culling, in place of the mesh's rest
    /// sphere; animated instances need one that covers their pose. `None` goes back to the mesh's.
    pub fn set_bounding_sphere(&mut self, id: InstanceId, sphere: Option<(Vec3, f32)>) -> Result<(), RenderError> {
        self.rec_mut(id)?.sphere = sphere;
        Ok(())
    }

    /// Outline width in pixels of a 448-line frame; the drawn width scales with the target height.
    pub fn set_outline_width(&mut self, px_at_448: f32) {
        self.outline_width = px_at_448.max(0.0);
    }

    /// Adds an outline mesh: one quad per edge, each edge given as four points in mesh space, `[far corner, end,
    /// end, far corner]` (the far corners are the opposite vertices of the two triangles sharing the edge). Its
    /// instances, with a [`crate::Pass::Outline`] material, draw the edges that are silhouettes from the camera
    /// (both far corners on the same side of the projected edge), as lines of the outline width. Before
    /// [`Renderer::commit`] only.
    pub fn add_outline_mesh(&mut self, edges: &[[[f32; 3]; 4]]) -> Result<MeshId, RenderError> {
        if edges.is_empty() || edges.len() > MAX_OUTLINE_EDGES {
            return Err(RenderError::BadMesh("outline edge count out of range"));
        }
        let mut vertices = Vec::with_capacity(edges.len() * 4);
        let mut indices = Vec::with_capacity(edges.len() * 6);
        for (e, pts) in edges.iter().enumerate() {
            // Corners 0 and 1 sit at the first end point, 2 and 3 at the second (for the bounding sphere); the
            // shader moves them out to either side.
            for p in [pts[1], pts[1], pts[2], pts[2]] {
                vertices.push(crate::Vertex::new(p, [0.0; 2], [0.0; 3], [255; 4]));
            }
            let b = (e * 4) as u16;
            indices.extend_from_slice(&[b, b + 1, b + 2, b + 2, b + 1, b + 3]);
        }
        let base = self.res.outline_points.len() as u32;
        let id = self.res.add_mesh(&MeshData { vertices: &vertices, indices: &indices }, None)?;
        self.res.outline_points.extend(edges.iter().flatten().map(|p| [p[0], p[1], p[2], 1.0]));
        self.res.meshes[id.0 as usize].outline_base = Some(base);
        Ok(id)
    }

    /// Several placed pieces of geometry as one mesh, so they draw as one instance (static level geometry merged at
    /// load). Each piece is a vertex range of `mesh` in its own model space with a placement transform, a texture
    /// (all in one pool: the material's) and a tint; the vertex shader applies the placement exactly as it would
    /// an instance transform, so the pixels are the same as drawing the pieces as separate instances. Draw it with
    /// an identity instance transform and a white tint; pieces whose placements mirror go in their own placed mesh,
    /// drawn with [`Renderer::set_mirrored`].
    pub fn add_placed_mesh(&mut self, mesh: &MeshData<'_>, pieces: &[PlacedPiece]) -> Result<MeshId, RenderError> {
        if pieces.is_empty() || pieces.len() > 1 << 24 {
            return Err(RenderError::BadMesh("placed piece count out of range"));
        }
        let pool = |t: TextureId| self.res.textures.get(t.0 as usize).map(|l| l.pool);
        let first = pool(pieces[0].texture).ok_or(RenderError::InvalidHandle)?;
        let mut owner = alloc::vec![u32::MAX; mesh.vertices.len()];
        for (k, p) in pieces.iter().enumerate() {
            if pool(p.texture).ok_or(RenderError::InvalidHandle)? != first
                || p.grey.is_some_and(|g| pool(g) != Some(first))
            {
                return Err(RenderError::BadTexture("placed pieces must share one texture pool"));
            }
            let r = p.vertices.start as usize..p.vertices.end as usize;
            if r.end > mesh.vertices.len() || r.start > r.end {
                return Err(RenderError::BadMesh("placed piece past the mesh's vertices"));
            }
            owner[r].iter_mut().for_each(|o| *o = k as u32);
        }
        if owner.contains(&u32::MAX) {
            return Err(RenderError::BadMesh("a vertex belongs to no placed piece"));
        }
        let skin: Vec<SkinVertex> =
            owner.iter().map(|&k| SkinVertex { bones: k.to_le_bytes(), weights: [0; 4] }).collect();
        let id = self.res.add_mesh(mesh, Some(&skin))?;
        let base = self.res.outline_points.len() as u32;
        // Bounding sphere of the placed positions (the mesh's own sphere is in model space).
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for (v, &k) in mesh.vertices.iter().zip(&owner) {
            let p = pieces[k as usize].transform.transform_point(Vec3::new(v.pos[0], v.pos[1], v.pos[2]));
            for (j, c) in [p.x, p.y, p.z].into_iter().enumerate() {
                lo[j] = lo[j].min(c);
                hi[j] = hi[j].max(c);
            }
        }
        let center = Vec3::new((lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5);
        let mut r2 = 0.0f32;
        for (v, &k) in mesh.vertices.iter().zip(&owner) {
            let d = pieces[k as usize].transform.transform_point(Vec3::new(v.pos[0], v.pos[1], v.pos[2])) - center;
            r2 = r2.max(d.dot(d));
        }
        for p in pieces {
            let layer = self.res.textures[p.texture.0 as usize].layer as f32;
            let grey = p.grey.map_or(0.0, |g| self.res.textures[g.0 as usize].layer as f32);
            let rows = p.transform.affine_rows();
            self.res.outline_points.extend_from_slice(&rows);
            // Texture layer, the tint's RGBA8 bits, the grey image's layer.
            self.res.outline_points.push([layer, f32::from_bits(u32::from_le_bytes(p.tint)), grey, 0.0]);
        }
        let m = &mut self.res.meshes[id.0 as usize];
        m.placed_base = Some(base);
        m.center = center;
        // A little slack so rounding never culls a piece that the GPU would draw.
        m.radius = crate::math::sqrt(r2) * 1.001 + 1e-3;
        Ok(id)
    }

    /// Draws the instance as if its transform mirrored (front faces clockwise on screen): for placed meshes whose
    /// pieces mirror, keeping their triangles as they are.
    pub fn set_mirrored(&mut self, id: InstanceId, mirrored: bool) -> Result<(), RenderError> {
        self.rec_mut(id)?.mirrored = mirrored;
        Ok(())
    }

    /// The texture pool a texture is in (textures of one pool can share a placed mesh).
    pub fn texture_pool(&self, t: TextureId) -> Option<u32> {
        self.res.textures.get(t.0 as usize).map(|l| l.pool as u32)
    }

    /// For a screen-space instance: the texture area `[u0, v0, u1, v1]` its unit quad shows (`None`: all of it).
    pub fn set_uv_rect(&mut self, id: InstanceId, rect: Option<[f32; 4]>) -> Result<(), RenderError> {
        self.rec_mut(id)?.uv_rect = rect;
        Ok(())
    }

    /// Samples texel centres (sharp magnified pixel art under linear filtering).
    pub fn set_nearest(&mut self, id: InstanceId, nearest: bool) -> Result<(), RenderError> {
        self.rec_mut(id)?.nearest = nearest;
        Ok(())
    }

    /// Sets the movie picture shown by [`crate::video::VideoLayer`]: the planes are copied and uploaded with the next
    /// frame. `None` drops the picture (nothing is uploaded).
    pub fn set_video_frame(&mut self, frame: Option<crate::video::VideoFrame<'_>>) -> Result<(), RenderError> {
        match frame {
            Some(f) => {
                f.validate()?;
                self.video_planes.clear();
                self.video_planes.extend_from_slice(f.y);
                self.video_planes.extend_from_slice(f.u);
                self.video_planes.extend_from_slice(f.v);
                self.video_size = (f.width, f.height);
                self.video_new = true;
            }
            None => {
                self.video_new = false;
                self.video_size = (0, 0);
            }
        }
        Ok(())
    }

    /// Makes a screen-space instance sample panel image 0 or 1 ([`crate::panel`]) instead of its texture.
    pub fn set_panel_image(&mut self, id: InstanceId, image: Option<u8>) -> Result<(), RenderError> {
        self.rec_mut(id)?.panel_image = image;
        Ok(())
    }

    /// Makes the instance draw the movie picture (with its screen-space quad's UVs) instead of its texture.
    pub fn set_video_quad(&mut self, id: InstanceId, video: bool) -> Result<(), RenderError> {
        self.rec_mut(id)?.video = video;
        Ok(())
    }

    /// Draws the instance into the projectors' silhouette image (a shadow caster, rendering.md 9.2): flat, with its
    /// vertex alpha, placed by the first projector.
    pub fn set_shadow_caster(&mut self, id: InstanceId, caster: bool) -> Result<(), RenderError> {
        self.rec_mut(id)?.caster = caster;
        Ok(())
    }

    /// The projector kinds the instance receives: a [`Projector`] acts on it when `bits & projector.receivers` is
    /// not zero (Sly 1 mesh flags bit 0x01: shadows). Only opaque and alpha-tested materials receive.
    pub fn set_receiver(&mut self, id: InstanceId, bits: u8) -> Result<(), RenderError> {
        self.rec_mut(id)?.receiver = bits;
        Ok(())
    }

    /// The projectors from now on (at most [`MAX_PROJECTORS`]; an empty list turns them off). They all project the
    /// one silhouette image the shadow casters are drawn into ([`Renderer::set_shadow_caster`]).
    pub fn set_projectors(&mut self, projectors: &[Projector]) -> Result<(), RenderError> {
        if projectors.len() > MAX_PROJECTORS {
            return Err(RenderError::LimitReached("projectors"));
        }
        self.projectors.clear();
        self.projectors.extend_from_slice(projectors);
        Ok(())
    }

    /// Draws the instance in its flat tint colour, without sampling the texture.
    pub fn set_flat(&mut self, id: InstanceId, flat: bool) -> Result<(), RenderError> {
        self.rec_mut(id)?.flat = flat;
        Ok(())
    }

    /// The key of a [`crate::SortMode::FarToNear`] list (larger first) in place of the squared camera distance;
    /// `None` goes back to the distance.
    pub fn set_sort_key(&mut self, id: InstanceId, key: Option<f32>) -> Result<(), RenderError> {
        self.rec_mut(id)?.sort_key = key;
        Ok(())
    }

    /// The instance's fog factor (1 by default; 0 turns fog off for it).
    pub fn set_fog_factor(&mut self, id: InstanceId, factor: f32) -> Result<(), RenderError> {
        self.rec_mut(id)?.fog = factor.clamp(0.0, 1.0);
        Ok(())
    }

    /// Adds per-vertex weights for one instance drawn with [`crate::Lighting::Palettes`] (one per vertex of its
    /// mesh, in order) to the level's weight buffer; returns the base to give [`Renderer::set_weights`]. Before
    /// [`Renderer::commit`] only.
    pub fn add_vertex_weights(&mut self, weights: &[VertexWeights]) -> Result<u32, RenderError> {
        if self.res.committed {
            return Err(RenderError::AlreadyCommitted);
        }
        if self.res.weights.len() + weights.len() > u32::MAX as usize {
            return Err(RenderError::LimitReached("vertex weights"));
        }
        let base = self.res.weights.len() as u32;
        self.res.weights.extend_from_slice(weights);
        Ok(base)
    }

    /// Uses the weights at `base` (from [`Renderer::add_vertex_weights`]) for this instance.
    pub fn set_weights(&mut self, id: InstanceId, base: Option<u32>) -> Result<(), RenderError> {
        let mesh = self.rec_mut(id)?.mesh;
        if let Some(b) = base {
            let count = self.res.meshes[mesh as usize].vertex_count as u64;
            let len = if self.res.committed { self.res.weight_count } else { self.res.weights.len() as u32 };
            if b as u64 + count > len as u64 {
                return Err(RenderError::InvalidHandle);
            }
        }
        self.instances[id.0 as usize].weights = base;
        Ok(())
    }

    /// Changes a material (texture animation: another frame's images); every instance using it follows from the
    /// next frame.
    pub fn set_material_images(
        &mut self,
        material: MaterialId,
        texture: TextureId,
        palettes: Option<[TextureId; 2]>,
    ) -> Result<(), RenderError> {
        let mut m = *self.res.materials.get(material.0 as usize).ok_or(RenderError::InvalidHandle)?;
        m.texture = texture;
        // `None` keeps the material's palette images.
        m.palettes = palettes.or(m.palettes);
        self.res.check_material(&m)?;
        self.res.materials[material.0 as usize] = m;
        Ok(())
    }

    /// Offsets every texture coordinate of a material (texture animation: scrolling).
    pub fn set_material_uv_offset(&mut self, material: MaterialId, offset: [f32; 2]) -> Result<(), RenderError> {
        *self.res.uv_offsets.get_mut(material.0 as usize).ok_or(RenderError::InvalidHandle)? = offset;
        Ok(())
    }

    /// The material, as added or last changed.
    pub fn material(&self, material: MaterialId) -> Option<&Material> {
        self.res.materials.get(material.0 as usize)
    }

    fn alloc_bones(&mut self, n: usize) -> Result<usize, RenderError> {
        let max = self.backend.limits().max_bones as usize;
        if self.bone_store.len() + n > max || self.bone_waste > self.bone_store.len() / 2 {
            // Compact: keep only the live ranges, in instance order.
            let mut store = Vec::with_capacity(self.bone_store.len() - self.bone_waste + n);
            for rec in self.instances.iter_mut().filter(|r| r.alive) {
                if let Some((start, count)) = rec.bones.as_mut() {
                    let from = *start as usize;
                    *start = store.len() as u32;
                    store.extend_from_slice(&self.bone_store[from..from + *count as usize]);
                }
            }
            self.bone_store = store;
            self.bone_waste = 0;
        }
        if self.bone_store.len() + n > max {
            return Err(RenderError::LimitReached("bone matrices"));
        }
        let start = self.bone_store.len();
        self.bone_store.resize(start + n, BoneMatrix::IDENTITY);
        Ok(start)
    }

    fn alloc_morph(&mut self, n: usize) -> Result<usize, RenderError> {
        let max = self.backend.limits().max_morph_vertices as usize;
        if self.morph_store.len() + n > max || self.morph_waste > self.morph_store.len() / 2 {
            let mut store = Vec::with_capacity(self.morph_store.len() - self.morph_waste + n);
            for rec in self.instances.iter_mut().filter(|r| r.alive) {
                if let Some(start) = rec.morph.as_mut() {
                    let from = *start as usize;
                    let count = self.res.meshes[rec.mesh as usize].vertex_count as usize;
                    *start = store.len() as u32;
                    store.extend_from_slice(&self.morph_store[from..from + count]);
                }
            }
            self.morph_store = store;
            self.morph_waste = 0;
        }
        if self.morph_store.len() + n > max {
            return Err(RenderError::LimitReached("morphed vertices"));
        }
        let start = self.morph_store.len();
        self.morph_store.resize(start + n, MorphVertex::default());
        Ok(start)
    }

    /// The frame constants of a view of `w` x `h` pixels.
    fn uniforms(&self, view_proj: &Mat4, e: Vec3, w: f32, h: f32) -> FrameUniforms {
        let fog = match self.fog {
            Some(f) if f.near > 0.0 && f.far > f.near => {
                [1.0 / f.near, 1.0 / f.near - 1.0 / f.far, f.cap * f.color[3], if self.fog_rising { 2.0 } else { 1.0 }]
            }
            _ => [0.0; 4],
        };
        let fog_color = self.fog.map_or([0.0; 4], |f| [f.color[0], f.color[1], f.color[2], 0.0]);
        FrameUniforms {
            view_proj: view_proj.m,
            light: self.light,
            fog,
            fog_color,
            eye: [e.x, e.y, e.z, self.darkening],
            screen: [w, h, self.outline_width * h / 448.0, 0.0],
        }
    }

    /// Culls, sorts and batches the instances, then has the backend draw the frame.
    pub fn render(&mut self, camera: &Camera, clear: [f32; 4]) -> Result<FrameStats, RenderError> {
        if !self.res.committed {
            return Err(RenderError::NotCommitted);
        }
        let now = || self.clock.map_or(0, |c| c());
        let t0 = now();

        let (w, h) = self.backend.target_size();
        let clip = self.backend.clip_space();
        let view_proj = camera.view_proj(w.max(1) as f32 / h.max(1) as f32, clip);
        let frustum = Frustum::from_view_proj(&view_proj, clip);
        let uniforms = self.uniforms(&view_proj, camera.eye, w as f32, h as f32);
        self.views.clear();
        let view = |uniforms, viewport, first_batch, batch_count| View {
            uniforms,
            viewport,
            clear_color: [0.0; 4],
            clear: 0,
            first_batch,
            batch_count,
            target: VIEW_TARGET_MAIN,
        };
        // Projector parameters go after the instances' bone matrices for this frame only.
        let bone_count = self.bone_store.len();
        let mut refs =
            [ProjectorRef { param: 0, origin: Vec3::ZERO, max_radius: 0.0, top: 0.0, bottom: 0.0, receivers: 0 };
                MAX_PROJECTORS];
        for (k, p) in self.projectors.iter().enumerate() {
            refs[k] = ProjectorRef {
                param: (bone_count + k) as u32,
                origin: p.origin,
                max_radius: p.near_radius.abs().max(p.far_radius.abs()),
                // Nothing fades below the far plane: the reach goes all the way down.
                top: if p.orthographic() { f32::MAX } else { p.origin.z - p.near_distance + PROJECTOR_MARGIN },
                bottom: f32::MIN,
                receivers: p.receivers,
            };
        }
        let refs = &refs[..self.projectors.len()];
        if bone_count + refs.len() > self.backend.limits().max_bones as usize {
            return Err(RenderError::LimitReached("bone matrices"));
        }
        self.builder.clear();
        let (mut visible, mut triangles) = (0, 0);
        let casters = !refs.is_empty() && self.instances.iter().any(|r| r.alive && r.visible && r.caster);
        if casters {
            // The silhouette image first: the shadow casters seen by the first projector.
            let (v, t) = self.builder.add_view(
                &self.res,
                &self.instances,
                &frustum,
                camera.eye,
                ViewMode::Casters { param: refs[0].param },
            );
            visible += v;
            triangles += t;
            let n = self.builder.batches.len() as u32;
            let s = SILHOUETTE_SIZE;
            let mut sv = view(uniforms, [0, 0, s, s], 0, n);
            sv.clear = VIEW_CLEAR_COLOR;
            sv.target = VIEW_TARGET_SILHOUETTE;
            self.views.push(sv);
        }
        let main_first = self.builder.batches.len() as u32;
        let (v, t) = self.builder.add_view(&self.res, &self.instances, &frustum, camera.eye, ViewMode::All(refs));
        visible += v;
        triangles += t;
        if !self.panels.is_empty() || casters {
            // Main view up to its screen-space draws, then the panels, then the main view's screen-space draws.
            let main_end = self.builder.batches.len() as u32;
            let split = self.builder.batches[main_first as usize..]
                .iter()
                .position(|b| b.pass == crate::types::Pass::Overlay as u32)
                .map_or(main_end, |k| main_first + k as u32);
            let full = [0, 0, w, h];
            self.views.push(view(uniforms, full, main_first, split - main_first));
            for (p, image) in &self.panels {
                let (x0, y0, x1, y1) = match image {
                    Some(_) => {
                        let s = crate::frame::PANEL_IMAGE_SIZE as f32;
                        (0, 0, p.rect[2].clamp(0.0, s) as u32, p.rect[3].clamp(0.0, s) as u32)
                    }
                    None => (
                        p.rect[0].clamp(0.0, w as f32) as u32,
                        p.rect[1].clamp(0.0, h as f32) as u32,
                        (p.rect[0] + p.rect[2]).clamp(0.0, w as f32) as u32,
                        (p.rect[1] + p.rect[3]).clamp(0.0, h as f32) as u32,
                    ),
                };
                if x1 <= x0 || y1 <= y0 {
                    continue;
                }
                let (pw, ph) = (x1 - x0, y1 - y0);
                let vp = p.camera.view_proj(pw as f32 / ph as f32, clip);
                let fr = Frustum::from_view_proj(&vp, clip);
                let first = self.builder.batches.len() as u32;
                let (v, t) =
                    self.builder.add_view(&self.res, &self.instances, &fr, p.camera.eye, ViewMode::World(refs));
                visible += v;
                triangles += t;
                let mut pv = view(
                    self.uniforms(&vp, p.camera.eye, pw as f32, ph as f32),
                    [x0, y0, pw, ph],
                    first,
                    self.builder.batches.len() as u32 - first,
                );
                pv.clear = VIEW_CLEAR_DEPTH | if p.clear.is_some() { VIEW_CLEAR_COLOR } else { 0 };
                pv.clear_color = p.clear.unwrap_or([0.0; 4]);
                if let Some(k) = *image {
                    // An image has no main view under it: always cleared.
                    pv.target = crate::frame::VIEW_TARGET_PANEL0 + (k as u32 & 1);
                    pv.clear |= VIEW_CLEAR_COLOR;
                }
                self.views.push(pv);
            }
            self.views.push(view(uniforms, full, split, main_end - split));
        }
        // Views with nothing to draw or clear are left out.
        self.views.retain(|v| v.batch_count > 0 || v.clear != 0);
        let limits = self.backend.limits();
        if self.builder.draws.len() as u32 > limits.max_draws {
            return Err(RenderError::LimitReached("draws per frame"));
        }
        if self.builder.batches.len() as u32 > limits.max_batches {
            return Err(RenderError::LimitReached("batches per frame"));
        }
        for p in &self.projectors {
            self.bone_store.push(p.rows());
        }
        let t1 = now();

        let packet = FramePacket {
            uniforms: &uniforms,
            instances: &self.builder.instances,
            bones: &self.bone_store,
            morph: &self.morph_store,
            draws: &self.builder.draws,
            batches: &self.builder.batches,
            clear,
            cull: self.cull,
            views: &self.views,
            video: if self.video_new {
                let (w, h) = self.video_size;
                let (cw, ch) = crate::video::VideoFrame::chroma_size(w, h);
                let (ys, cs) = ((w * h) as usize, (cw * ch) as usize);
                Some(crate::video::VideoFrame {
                    width: w,
                    height: h,
                    y: &self.video_planes[..ys],
                    u: &self.video_planes[ys..ys + cs],
                    v: &self.video_planes[ys + cs..ys + 2 * cs],
                })
            } else {
                None
            },
        };
        let info = self.backend.submit(&packet);
        self.bone_store.truncate(bone_count);
        if info.is_ok() {
            self.video_new = false;
        }
        let info = info?;
        let t2 = now();

        Ok(FrameStats {
            instances: self.live,
            visible,
            batches: self.builder.batches.len() as u32,
            draw_commands: self.builder.draws.len() as u32,
            api_draw_calls: info.api_draw_calls,
            triangles,
            cpu_build_ns: t1.saturating_sub(t0),
            cpu_submit_ns: t2.saturating_sub(t1).saturating_sub(info.wait_ns),
            wait_ns: info.wait_ns,
            gpu_ns: info.gpu_ns,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NullBackend;
    use crate::types::{BoneMatrix, Pass, SkinVertex, Vertex, VertexWeights};
    use alloc::vec;

    fn tri_mesh(r: &mut Renderer<NullBackend>, size: f32) -> MeshId {
        let v = [
            Vertex::new([-size, 0.0, 0.0], [0.0, 0.0], [0.0, 0.0, 1.0], [255; 4]),
            Vertex::new([size, 0.0, 0.0], [1.0, 0.0], [0.0, 0.0, 1.0], [255; 4]),
            Vertex::new([0.0, size, 0.0], [0.5, 1.0], [0.0, 0.0, 1.0], [255; 4]),
        ];
        r.add_mesh(&MeshData { vertices: &v, indices: &[0, 1, 2] }).unwrap()
    }

    fn tex(r: &mut Renderer<NullBackend>, size: u32) -> TextureId {
        let px = vec![200u8; (size * size * 4) as usize];
        r.add_texture(TextureData::rgba8_with_mips(size, size, px).unwrap()).unwrap()
    }

    fn cam() -> Camera {
        Camera::look_at(Vec3::new(0.0, 0.0, 20.0), Vec3::ZERO)
    }

    #[test]
    fn arena_and_pools() {
        let mut r = Renderer::new(NullBackend::new(640, 480));
        let a = tri_mesh(&mut r, 1.0);
        let b = tri_mesh(&mut r, 2.0);
        assert_eq!((a.index(), b.index()), (0, 1));
        let t0 = tex(&mut r, 8);
        let t1 = tex(&mut r, 8);
        let t2 = tex(&mut r, 16);
        assert_eq!(r.texture_pool_count(), 2);
        assert_eq!(r.res.textures[t1.0 as usize].layer, 1);
        assert_eq!(r.res.textures[t2.0 as usize].pool, 1);
        let _ = t0;
        r.commit().unwrap();
        assert_eq!(r.backend().uploads, 1);
        assert_eq!(r.backend().vertex_count, 6);
        assert_eq!(r.backend().index_count, 6);
        assert_eq!(r.backend().pool_layers, vec![2, 1]);
        assert_eq!(r.arena_size(), (6, 6));
        assert_eq!(
            r.add_mesh(&MeshData { vertices: &[Vertex::default(); 3], indices: &[0, 1, 2] }),
            Err(RenderError::AlreadyCommitted)
        );
        let m = r.res.meshes[1];
        assert_eq!((m.first_index, m.base_vertex), (3, 3));
    }

    #[test]
    fn bad_meshes() {
        let mut r = Renderer::new(NullBackend::new(64, 64));
        let v = [Vertex::default(); 3];
        assert!(r.add_mesh(&MeshData { vertices: &v, indices: &[0, 1] }).is_err());
        assert_eq!(r.add_mesh(&MeshData { vertices: &v, indices: &[0, 1, 3] }), Err(RenderError::IndexOutOfRange));
        assert!(r.add_mesh(&MeshData { vertices: &[], indices: &[] }).is_err());
    }

    #[test]
    fn render_needs_commit() {
        let mut r = Renderer::new(NullBackend::new(64, 64));
        assert_eq!(r.render(&cam(), [0.0; 4]), Err(RenderError::NotCommitted));
    }

    #[test]
    fn batching_groups_by_pass_pool_and_mesh() {
        let mut r = Renderer::new(NullBackend::new(640, 480));
        let ma = tri_mesh(&mut r, 1.0);
        let mb = tri_mesh(&mut r, 1.0);
        let small = tex(&mut r, 8);
        let small2 = tex(&mut r, 8);
        let big = tex(&mut r, 16);
        r.commit().unwrap();
        let op_small = r.add_material(Material::opaque(small)).unwrap();
        let op_small2 = r.add_material(Material::opaque(small2)).unwrap();
        let op_big = r.add_material(Material::opaque(big)).unwrap();
        let alpha = r
            .add_material(Material { alpha_cutoff: 0.5, ..Material::opaque(small).with_pass(Pass::AlphaTest) })
            .unwrap();
        let blend = r
            .add_material(Material { tint: [255, 255, 255, 128], ..Material::opaque(big).with_pass(Pass::Blend) })
            .unwrap();

        let at = |x: f32, z: f32| Mat4::translation(Vec3::new(x, 0.0, z));
        // Opaque, pool 0: mesh a x3 (two materials sharing the pool), mesh b x1.
        r.add_instance(ma, op_small, at(-3.0, 0.0)).unwrap();
        r.add_instance(mb, op_small, at(-1.0, 0.0)).unwrap();
        r.add_instance(ma, op_small2, at(1.0, 0.0)).unwrap();
        r.add_instance(ma, op_small, at(3.0, 0.0)).unwrap();
        // Opaque, pool 1.
        r.add_instance(ma, op_big, at(0.0, 2.0)).unwrap();
        // Alpha-tested.
        r.add_instance(mb, alpha, at(0.0, -2.0)).unwrap();
        // Blended: near one added first, must be drawn last.
        let near = r.add_instance(ma, blend, at(0.0, 5.0)).unwrap();
        let far = r.add_instance(mb, blend, at(0.0, -5.0)).unwrap();
        // Culled (behind the camera) and hidden.
        r.add_instance(ma, op_small, at(0.0, 40.0)).unwrap();
        let hidden = r.add_instance(ma, op_small, at(0.0, 0.0)).unwrap();
        r.set_visible(hidden, false).unwrap();

        let s = r.render(&cam(), [0.0; 4]).unwrap();
        assert_eq!(s.instances, 10);
        assert_eq!(s.visible, 8);
        assert_eq!(s.triangles, 8);
        let nb = r.backend();
        let batches: Vec<(u32, u32, u32)> = nb.last_batches.iter().map(|b| (b.pass, b.pool, b.draw_count)).collect();
        // Blended draws split by pool only when the pool changes; both use pool 1 here, but they are different
        // meshes so they are two draws in one batch.
        assert_eq!(batches, vec![(1, 0, 2), (1, 1, 1), (2, 0, 1), (3, 1, 2)]);
        assert_eq!(s.batches, 4);
        assert_eq!(s.draw_commands, 6);
        assert_eq!(s.api_draw_calls, 4);
        let d = &nb.last_draws;
        assert_eq!((d[0].instance_count, d[0].base_instance, d[0].first_index), (3, 0, 0));
        assert_eq!((d[1].instance_count, d[1].base_instance, d[1].base_vertex), (1, 3, 3));
        // Instance records follow slot order; the second record of draw 0 is the small2 material (layer 1).
        let layers: Vec<u32> = nb.last_instances[..3].iter().map(|i| i.layer()).collect();
        assert_eq!(layers, vec![0, 1, 0]);
        // Alpha cutoff only on the alpha-tested record.
        assert_eq!(nb.last_instances[5].alpha_cutoff(), 128.0 / 255.0);
        assert_eq!(nb.last_instances[0].alpha_cutoff(), 0.0);
        // Back to front: the far blended instance (z = -5) comes first.
        assert_eq!(nb.last_instances[6].rows[2][3], -5.0);
        assert_eq!(nb.last_instances[7].rows[2][3], 5.0);
        assert_eq!(nb.last_instances[7].tint.to_le_bytes(), [255, 255, 255, 128]);

        // Moving the near blended instance behind the far one swaps the order.
        r.set_transform(near, at(0.0, -8.0)).unwrap();
        r.render(&cam(), [0.0; 4]).unwrap();
        assert_eq!(r.backend().last_instances[6].rows[2][3], -8.0);
        let _ = far;
    }

    #[test]
    fn mirrored_additive_and_overlay() {
        use crate::frame::*;
        use crate::types::{BlendMode, Lighting};
        let mut r = Renderer::new(NullBackend::new(640, 480));
        let m = tri_mesh(&mut r, 1.0);
        let t = tex(&mut r, 8);
        r.commit().unwrap();
        let lit = r.add_material(Material::opaque(t)).unwrap();
        let unlit = r.add_material(Material { lighting: Lighting::Unlit, ..Material::opaque(t) }).unwrap();
        let add = r
            .add_material(Material { blend: BlendMode::Additive, ..Material::opaque(t).with_pass(Pass::Blend) })
            .unwrap();
        let alpha = r.add_material(Material::opaque(t).with_pass(Pass::Blend)).unwrap();
        let overlay = r.add_material(Material::opaque(t).with_pass(Pass::Overlay)).unwrap();

        let mirror = Mat4::scale(Vec3::new(-1.0, 1.0, 1.0));
        r.add_instance(m, lit, Mat4::IDENTITY).unwrap();
        let flipped = r.add_instance(m, unlit, mirror).unwrap();
        r.add_instance(m, alpha, Mat4::translation(Vec3::new(0.0, 0.0, -4.0))).unwrap();
        r.add_instance(m, add, Mat4::translation(Vec3::new(0.0, 0.0, -2.0))).unwrap();
        // Far outside the frustum in world terms, but overlay instances are never culled.
        r.add_instance(m, overlay, Mat4::translation(Vec3::new(0.0, 0.0, 500.0))).unwrap();

        let s = r.render(&cam(), [0.0; 4]).unwrap();
        assert_eq!(s.visible, 5);
        let nb = r.backend();
        let batches: Vec<(u32, u32)> =
            nb.last_batches.iter().map(|b| (b.pass, b.state & (STATE_FRONT_CW | STATE_BLEND_MASK))).collect();
        // Opaque unmirrored, opaque mirrored, blended far (alpha), blended near (additive), overlay.
        assert_eq!(
            batches,
            vec![(1, 0), (1, STATE_FRONT_CW), (3, STATE_BLEND_ALPHA), (3, STATE_BLEND_ADD), (6, STATE_BLEND_ALPHA)]
        );
        let flags: Vec<u32> = nb.last_instances.iter().map(|i| i.flags()).collect();
        let screen = INSTANCE_SCREEN | INSTANCE_NO_FOG | INSTANCE_NO_DARKENING;
        assert_eq!(flags, vec![INSTANCE_LIT, 0, INSTANCE_LIT, INSTANCE_LIT, screen]);
        assert_eq!(nb.last_batches[0].state, STATE_DEPTH_LESS | STATE_DEPTH_WRITE);
        assert_eq!(nb.last_batches[4].state & STATE_DEPTH_MASK, STATE_DEPTH_ALWAYS);

        // Un-mirroring the instance merges it back into the first draw.
        r.set_transform(flipped, Mat4::translation(Vec3::new(1.0, 0.0, 0.0))).unwrap();
        r.render(&cam(), [0.0; 4]).unwrap();
        let nb = r.backend();
        assert_eq!(nb.last_batches.len(), 4);
        assert_eq!(nb.last_draws[0].instance_count, 2);
    }

    #[test]
    fn lists_two_step_volumes_and_sort_keys() {
        use crate::frame::*;
        use crate::types::{BlendMode, Lighting, SortMode};
        let mut r = Renderer::new(NullBackend::new(640, 480));
        let m = tri_mesh(&mut r, 1.0);
        let t = tex(&mut r, 8);
        let t0 = tex(&mut r, 8);
        let t2 = tex(&mut r, 8);
        let odd = tex(&mut r, 16);
        let base = r.add_vertex_weights(&[VertexWeights::new(0.25, [0.5; 3], [0.0; 3], 1.0); 3]).unwrap();
        r.commit().unwrap();
        assert!(r.add_vertex_weights(&[VertexWeights::default()]).is_err());
        // Palette images must share the texture's pool.
        let bad = Material { lighting: Lighting::Palettes, palettes: Some([t0, odd]), ..Material::opaque(t) };
        assert!(r.add_material(bad).is_err());
        let sky = Material { order: 0, blend: BlendMode::None, ..Material::opaque(t).with_pass(Pass::Background) };
        let sky = r.add_material(sky).unwrap();
        let lit = Material { lighting: Lighting::Palettes, palettes: Some([t0, t2]), order: 4, ..Material::opaque(t) };
        let lit = r.add_material(lit).unwrap();
        let glass =
            Material { two_step: true, alpha_cutoff: 0.9, order: 16, ..Material::opaque(t).with_pass(Pass::Blend) };
        let glass = r.add_material(glass).unwrap();
        let shadow = Material { order: 9, ..Material::opaque(t).with_pass(Pass::Volume) };
        let shadow = r.add_material(shadow).unwrap();
        let post = r.add_material(Material::opaque(t).with_pass(Pass::Post)).unwrap();
        assert_eq!(r.material(glass).unwrap().sort, SortMode::FarToNear);

        let at = |z: f32| Mat4::translation(Vec3::new(0.0, 0.0, z));
        let p = r.add_instance(m, post, at(0.0)).unwrap();
        let g_near = r.add_instance(m, glass, at(5.0)).unwrap();
        let g_far = r.add_instance(m, glass, at(-5.0)).unwrap();
        r.add_instance(m, shadow, at(0.0)).unwrap();
        let w = r.add_instance(m, lit, at(0.0)).unwrap();
        r.set_weights(w, Some(base)).unwrap();
        assert!(r.set_weights(w, Some(base + 1)).is_err());
        r.set_fog_factor(w, 0.5).unwrap();
        let s1 = r.add_instance(m, sky, at(0.0)).unwrap();
        r.set_material_uv_offset(lit, [0.25, -0.5]).unwrap();

        r.render(&cam(), [0.0; 4]).unwrap();
        let nb = r.backend();
        let order: Vec<(u32, u32)> =
            nb.last_batches.iter().map(|b| (b.pass, b.state & (STATE_DEPTH_WRITE | STATE_STENCIL_MASK))).collect();
        // Background (list 0), palettes opaque (4), volume count + paint (9), two-step glass (16), post (30).
        assert_eq!(
            order,
            vec![
                (0, 0),
                (1, STATE_DEPTH_WRITE),
                (4, STATE_STENCIL_COUNT),
                (4, STATE_STENCIL_PAINT),
                (3, STATE_DEPTH_WRITE),
                (3, 0),
                (5, 0),
            ]
        );
        // Two-step: the prepass is alpha-tested with depth writes, both steps sorted far to near.
        assert!(nb.last_batches[4].state & STATE_ALPHA_TEST != 0);
        assert_eq!(nb.last_batches[4].draw_count, 1);
        assert_eq!(nb.last_draws[nb.last_batches[4].first_draw as usize].instance_count, 2);
        let recs = &nb.last_instances;
        assert_eq!(recs.len(), 9);
        assert_eq!((recs[1].flags() & INSTANCE_WEIGHTS, recs[1].weights_base), (INSTANCE_WEIGHTS, base));
        assert_eq!((recs[1].layers & 0xff, (recs[1].layers >> 8) & 0xff), (1, 2));
        assert_eq!((recs[1].layers >> 16) & 0xff, 128);
        assert_eq!(recs[1].uv_offset, [0.25, -0.5]);
        assert!((recs[4].alpha_cutoff() - 0.9).abs() < 0.003);
        assert_eq!(recs[4].rows[2][3], -5.0);
        assert_eq!(recs[6].rows[2][3], -5.0);

        // An explicit sort key overrides the distance: the near glass now draws first.
        r.set_sort_key(g_near, Some(1e9)).unwrap();
        r.render(&cam(), [0.0; 4]).unwrap();
        assert_eq!(r.backend().last_instances[4].rows[2][3], 5.0);
        let _ = (p, g_far, s1);
    }

    #[test]
    fn skinned_and_rigid_instances_share_draws() {
        use crate::frame::{INSTANCE_LIT, INSTANCE_MORPHED, INSTANCE_SKINNED};
        let mut r = Renderer::new(NullBackend::new(640, 480));
        let v = [
            Vertex::new([-1.0, 0.0, 0.0], [0.0, 0.0], [0.0, 0.0, 1.0], [255; 4]),
            Vertex::new([1.0, 0.0, 0.0], [1.0, 0.0], [0.0, 0.0, 1.0], [255; 4]),
            Vertex::new([0.0, 1.0, 0.0], [0.5, 1.0], [0.0, 0.0, 1.0], [255; 4]),
        ];
        let skin = [
            SkinVertex::new([0, 0, 0, 0], [1.0, 0.0, 0.0, 0.0]),
            SkinVertex::new([1, 0, 0, 0], [0.25, 0.75, 0.0, 0.0]),
            SkinVertex::NONE,
        ];
        assert!(r.add_skinned_mesh(&MeshData { vertices: &v, indices: &[0, 1, 2] }, &skin[..2]).is_err());
        let m = r.add_skinned_mesh(&MeshData { vertices: &v, indices: &[0, 1, 2] }, &skin).unwrap();
        let t = tex(&mut r, 4);
        r.commit().unwrap();
        // The second stream: skinning data and each vertex's index in its mesh.
        let x = &r.backend().extras;
        assert_eq!(x.len(), 3);
        assert_eq!((x[1].bones, x[1].weights, x[1].local), ([1, 0, 0, 0], [16384, 49151, 0, 0], 1));
        let mat = r.add_material(Material::opaque(t)).unwrap();
        let rigid = r.add_instance(m, mat, Mat4::IDENTITY).unwrap();
        let a = r.add_instance(m, mat, Mat4::translation(Vec3::new(2.0, 0.0, 0.0))).unwrap();
        let b = r.add_instance(m, mat, Mat4::translation(Vec3::new(-2.0, 0.0, 0.0))).unwrap();
        let mut bone = BoneMatrix::IDENTITY;
        bone.rows[0][3] = 5.0;
        r.set_bones(a, &[bone, BoneMatrix::IDENTITY]).unwrap();
        r.set_bones(b, &[BoneMatrix::IDENTITY; 3]).unwrap();
        r.set_morph(b, &[[0.0; 3]; 3], &[[0.0, 0.0, 1.0]; 3]).unwrap();
        assert!(r.set_morph(a, &[[0.0; 3]; 2], &[[0.0; 3]; 2]).is_err());
        assert!(r.set_bones(a, &[]).is_err());

        let s = r.render(&cam(), [0.0; 4]).unwrap();
        // One mesh: one draw for the rigid, skinned and morphed instances together.
        assert_eq!((s.visible, s.draw_commands, s.batches), (3, 1, 1));
        let nb = r.backend();
        assert_eq!(nb.last_bones.len(), 5);
        assert_eq!(nb.last_bones[0].rows[0][3], 5.0);
        assert_eq!(nb.last_morph.len(), 3);
        assert_eq!(nb.last_morph[0].normal, [0.0, 0.0, 1.0, 0.0]);
        let recs = &nb.last_instances;
        assert_eq!(recs[0].flags(), INSTANCE_LIT);
        assert_eq!((recs[1].flags(), recs[1].bone_base), (INSTANCE_LIT | INSTANCE_SKINNED, 0));
        assert_eq!((recs[2].flags(), recs[2].bone_base), (INSTANCE_LIT | INSTANCE_SKINNED | INSTANCE_MORPHED, 2));
        assert_eq!(recs[2].morph_base, 0);

        // Same count: the range is reused. Another count: a new range, the old one compacted away.
        r.set_bones(a, &[BoneMatrix::IDENTITY; 2]).unwrap();
        assert_eq!(r.bone_store.len(), 5);
        r.set_bones(a, &[BoneMatrix::IDENTITY; 4]).unwrap();
        r.set_bones(b, &[BoneMatrix::IDENTITY; 1]).unwrap();
        assert!(r.bone_store.len() <= 9);
        r.clear_bones(a).unwrap();
        r.remove_instance(b).unwrap();
        r.render(&cam(), [0.0; 4]).unwrap();
        assert_eq!(r.backend().last_instances.len(), 2);
        assert!(r.backend().last_instances.iter().all(|i| i.flags() == INSTANCE_LIT));
        let _ = rigid;
    }

    #[test]
    fn instance_bounding_sphere_replaces_the_mesh_sphere() {
        let mut r = Renderer::new(NullBackend::new(640, 480));
        let m = tri_mesh(&mut r, 1.0);
        let t = tex(&mut r, 4);
        r.commit().unwrap();
        let mat = r.add_material(Material::opaque(t)).unwrap();
        // Behind the camera: culled with the mesh's sphere.
        let id = r.add_instance(m, mat, Mat4::translation(Vec3::new(0.0, 0.0, 30.0))).unwrap();
        assert_eq!(r.render(&cam(), [0.0; 4]).unwrap().visible, 0);
        // An animated extent reaching back in front of the camera keeps it.
        r.set_bounding_sphere(id, Some((Vec3::new(0.0, 0.0, -20.0), 5.0))).unwrap();
        assert_eq!(r.render(&cam(), [0.0; 4]).unwrap().visible, 1);
        r.set_bounding_sphere(id, None).unwrap();
        assert_eq!(r.render(&cam(), [0.0; 4]).unwrap().visible, 0);
    }

    #[test]
    fn instance_slots_are_reused() {
        let mut r = Renderer::new(NullBackend::new(64, 64));
        let m = tri_mesh(&mut r, 1.0);
        let t = tex(&mut r, 4);
        r.commit().unwrap();
        let mat = r.add_material(Material::opaque(t)).unwrap();
        let a = r.add_instance(m, mat, Mat4::IDENTITY).unwrap();
        let b = r.add_instance(m, mat, Mat4::IDENTITY).unwrap();
        r.remove_instance(a).unwrap();
        assert_eq!(r.set_transform(a, Mat4::IDENTITY), Err(RenderError::InvalidHandle));
        assert_eq!(r.remove_instance(a), Err(RenderError::InvalidHandle));
        let c = r.add_instance(m, mat, Mat4::IDENTITY).unwrap();
        assert_eq!(c, a);
        assert_ne!(b, c);
        assert_eq!(r.instance_count(), 2);
        assert!(r.add_instance(MeshId(5), mat, Mat4::IDENTITY).is_err());
    }

    #[test]
    fn uniforms_and_cull_reach_the_backend() {
        let mut r = Renderer::new(NullBackend::new(64, 64));
        let m = tri_mesh(&mut r, 1.0);
        let t = tex(&mut r, 4);
        r.commit().unwrap();
        let mat = r.add_material(Material::opaque(t)).unwrap();
        r.add_instance(m, mat, Mat4::IDENTITY).unwrap();
        r.set_cull_mode(CullMode::None);
        r.render(&cam(), [0.0; 4]).unwrap();
        let u = r.backend().last_uniforms.unwrap();
        let vp = cam().view_proj(1.0, crate::ClipSpace::GL);
        assert_eq!(u.view_proj, vp.m);
        assert_eq!(r.backend().last_cull, CullMode::None);
    }

    #[test]
    fn panels_draw_between_the_world_and_the_ui() {
        let mut r = Renderer::new(NullBackend::new(640, 480));
        let m = tri_mesh(&mut r, 1.0);
        let t = tex(&mut r, 8);
        let world = r.add_material(Material::opaque(t)).unwrap();
        let ui = r.add_material(Material { order: 30, ..Material::opaque(t).with_pass(Pass::Overlay) }).unwrap();
        r.add_instance(m, world, Mat4::IDENTITY).unwrap();
        r.add_instance(m, ui, Mat4::IDENTITY).unwrap();
        r.commit().unwrap();
        let s = r.render(&cam(), [0.0; 4]).unwrap();
        assert_eq!(s.visible, 2);
        assert!(r.backend().last_views.is_empty());

        let panel = Panel {
            camera: Camera::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO),
            rect: [400.0, 300.0, 200.0, 250.0],
            clear: Some([0.0, 0.0, 0.0, 1.0]),
        };
        r.set_panels(&[panel]).unwrap();
        let s = r.render(&cam(), [0.0; 4]).unwrap();
        // The world twice, the UI once.
        assert_eq!(s.visible, 3);
        let nb = r.backend();
        let v = &nb.last_views;
        assert_eq!(v.len(), 3);
        // Main world, then the panel (clipped to the target, own camera and size), then the main UI.
        assert_eq!((v[0].first_batch, v[0].batch_count, v[0].clear), (0, 1, 0));
        assert_eq!(v[1].viewport, [400, 300, 200, 180]);
        assert_eq!(v[1].clear, VIEW_CLEAR_DEPTH | VIEW_CLEAR_COLOR);
        assert_eq!(v[1].uniforms.screen[..2], [200.0, 180.0]);
        assert_eq!(nb.last_batches[v[1].first_batch as usize].pass, Pass::Opaque as u32);
        assert_eq!(v[1].batch_count, 1);
        assert_eq!(nb.last_batches[v[2].first_batch as usize].pass, Pass::Overlay as u32);
        assert_eq!(v[2].viewport, [0, 0, 640, 480]);
        let covered: u32 = v.iter().map(|v| v.batch_count).sum();
        assert_eq!(covered as usize, nb.last_batches.len());
        assert!(r.set_panels(&[panel; MAX_PANELS + 1]).is_err());
        r.set_panels(&[]).unwrap();
        r.render(&cam(), [0.0; 4]).unwrap();
        assert!(r.backend().last_views.is_empty());
    }

    #[test]
    fn projectors_draw_casters_then_receivers() {
        let mut r = Renderer::new(NullBackend::new(640, 480));
        let m = tri_mesh(&mut r, 1.0);
        let t = tex(&mut r, 8);
        let mat = r.add_material(Material::opaque(t)).unwrap();
        let ground = r.add_instance(m, mat, Mat4::IDENTITY).unwrap();
        let far = r.add_instance(m, mat, Mat4::translation(Vec3::new(500.0, 0.0, 0.0))).unwrap();
        let caster = r.add_instance(m, mat, Mat4::translation(Vec3::new(0.0, 0.0, 5.0))).unwrap();
        r.commit().unwrap();
        r.set_receiver(ground, 1).unwrap();
        r.set_receiver(far, 1).unwrap();
        r.set_shadow_caster(caster, true).unwrap();
        let p = Projector {
            origin: Vec3::new(0.0, 0.0, 10.0),
            near_radius: 150.0,
            far_radius: 10.0,
            near_distance: 0.0,
            far_distance: 500.0,
            strength: 0.5,
            receivers: 1,
        };
        // Projection: at the origin's height the square is 150 wide each way; 500 below, 10.
        assert_eq!(p.project(Vec3::new(75.0, 0.0, 10.0)), Some([0.75, 0.5]));
        assert_eq!(p.radius_at(250.0), 80.0);
        // Below the far plane: clamped there, no fade; more than 50 above the near plane: cut.
        assert_eq!(p.project(Vec3::new(5.0, 0.0, 10.0 - 2000.0)), Some([0.75, 0.5]));
        assert_eq!(p.project(Vec3::new(0.0, 0.0, 10.0 + 51.0)), None);
        assert!(p.project(Vec3::new(0.0, 0.0, 10.0 + 49.0)).is_some());
        r.set_projectors(&[p]).unwrap();
        let cam = Camera::look_at(Vec3::new(0.0, -50.0, 50.0), Vec3::ZERO);
        let s = r.render(&cam, [0.0; 4]).unwrap();
        let nb = r.backend();
        // The silhouette view first (the caster), then the main view.
        assert_eq!(nb.last_views.len(), 2);
        assert_eq!(nb.last_views[0].target, VIEW_TARGET_SILHOUETTE);
        assert_eq!(nb.last_views[0].viewport, [0, 0, SILHOUETTE_SIZE, SILHOUETTE_SIZE]);
        let flags: Vec<u32> = nb.last_instances.iter().map(|i| i.flags()).collect();
        let cast = flags.iter().filter(|f| *f & crate::frame::INSTANCE_PROJECT_CAST != 0).count();
        let recv = flags.iter().filter(|f| *f & crate::frame::INSTANCE_PROJECT_RECEIVE != 0).count();
        // One caster record; one receiver record (the far instance is out of reach).
        assert_eq!((cast, recv), (1, 1));
        // The projector's parameters follow the bone matrices for this frame only.
        assert_eq!(nb.last_bones.len(), 1);
        assert_eq!(nb.last_bones[0].rows[0], [0.0, 0.0, 10.0, 150.0]);
        assert!(s.visible >= 4);
        let receiver = nb.last_instances.iter().find(|i| i.flags() & crate::frame::INSTANCE_PROJECT_RECEIVE != 0);
        assert_eq!(receiver.unwrap().weights_base, 0);
        assert!(r.set_projectors(&[p; MAX_PROJECTORS + 1]).is_err());
        r.set_projectors(&[]).unwrap();
        r.render(&cam, [0.0; 4]).unwrap();
        assert!(r.backend().last_views.is_empty());
        assert!(r.backend().last_bones.is_empty());
    }

    #[test]
    fn masked_areas_draw_in_two_steps() {
        use crate::frame::*;
        let mut r = Renderer::new(NullBackend::new(640, 480));
        let m = tri_mesh(&mut r, 1.0);
        let t = tex(&mut r, 8);
        let write = r.add_material(Material { order: 13, ..Material::opaque(t).with_pass(Pass::MaskWrite) }).unwrap();
        let fill = r.add_material(Material { order: 15, ..Material::opaque(t).with_pass(Pass::MaskFill) }).unwrap();
        r.add_instance(m, write, Mat4::IDENTITY).unwrap();
        r.add_instance(m, fill, Mat4::IDENTITY).unwrap();
        r.commit().unwrap();
        r.set_cull_mode(CullMode::Back);
        r.render(&cam(), [0.0; 4]).unwrap();
        let states: Vec<u32> = r.backend().last_batches.iter().map(|b| b.state).collect();
        // Back faces writing alpha only, front faces alpha over; then fill by inverse destination alpha, alpha over.
        assert_eq!(
            states,
            [
                STATE_DEPTH_LEQUAL | STATE_CULL_FRONT | STATE_ALPHA_ONLY,
                STATE_DEPTH_LEQUAL | STATE_BLEND_ALPHA,
                STATE_DEPTH_LEQUAL | STATE_BLEND_INV_DEST_ALPHA,
                STATE_DEPTH_LEQUAL | STATE_BLEND_ALPHA,
            ]
        );
    }
}
