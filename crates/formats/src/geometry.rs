//! Mesh sets, meshes, submeshes and transforms (geometry.md, transforms.md).
//!
//! Parsing records offsets and counts only; vertex data is read from the world file on demand (it is unaligned in
//! the file, so it is copied out, never borrowed as `f32` slices). [`Submesh::to_gpu`] builds one GPU vertex per
//! strip entry and a `u16` triangle list.

use alloc::vec::Vec;

use crate::error::{Error, Result};
use crate::reader::{Reader, Vec3, le_f32};
use crate::textures::AnimRecord;

/// The float `3.4028235e38` (`FLT_MAX`), used by the data as "unlimited / not set".
pub const FLT_MAX: f32 = f32::MAX;

/// An affine transform: a 3 x 3 matrix stored as three **columns** and a translation. A point `x` maps to
/// `cols[0] * x[0] + cols[1] * x[1] + cols[2] * x[2] + t`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    /// The images of the local X, Y and Z axes.
    pub cols: [Vec3; 3],
    /// The translation.
    pub t: Vec3,
}

impl Default for Affine {
    fn default() -> Self {
        Affine::IDENTITY
    }
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

impl Affine {
    /// The identity.
    pub const IDENTITY: Affine = Affine { cols: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], t: [0.0; 3] };

    /// A transform from stored columns and a translation.
    pub fn new(cols: [Vec3; 3], t: Vec3) -> Self {
        Affine { cols, t }
    }

    /// `M v` (no translation).
    pub fn transform_vector(&self, v: Vec3) -> Vec3 {
        let c = &self.cols;
        [
            c[0][0] * v[0] + c[1][0] * v[1] + c[2][0] * v[2],
            c[0][1] * v[0] + c[1][1] * v[1] + c[2][1] * v[2],
            c[0][2] * v[0] + c[1][2] * v[1] + c[2][2] * v[2],
        ]
    }

    /// `M v + t`.
    pub fn transform_point(&self, v: Vec3) -> Vec3 {
        let r = self.transform_vector(v);
        [r[0] + self.t[0], r[1] + self.t[1], r[2] + self.t[2]]
    }

    /// `self * child`: the transform of a child whose local transform is `child` under a parent `self`
    /// (`Mw = Mp M`, `pw = pp + Mp p`).
    pub fn mul(&self, child: &Affine) -> Affine {
        Affine {
            cols: [
                self.transform_vector(child.cols[0]),
                self.transform_vector(child.cols[1]),
                self.transform_vector(child.cols[2]),
            ],
            t: self.transform_point(child.t),
        }
    }

    /// Determinant of the 3 x 3 part. Negative for a mirroring transform.
    pub fn determinant(&self) -> f32 {
        dot(self.cols[0], cross(self.cols[1], self.cols[2]))
    }

    /// Whether the 3 x 3 part mirrors (negative determinant): triangle winding is reversed under it.
    pub fn is_mirrored(&self) -> bool {
        self.determinant() < 0.0
    }

    /// The inverse transpose of the 3 x 3 part (as columns), for transforming normals. Renormalise after use.
    /// Returns the identity for a singular matrix.
    pub fn normal_matrix(&self) -> [Vec3; 3] {
        let [a, b, c] = self.cols;
        let det = self.determinant();
        if det == 0.0 || !det.is_finite() {
            return Affine::IDENTITY.cols;
        }
        // The cofactor matrix's columns are the cross products of the columns; (M^-1)^T = cof(M) / det.
        let inv = 1.0 / det;
        let s = |v: Vec3| [v[0] * inv, v[1] * inv, v[2] * inv];
        [s(cross(b, c)), s(cross(c, a)), s(cross(a, b))]
    }

    /// Whether any component is non-finite or larger than 1e30 in magnitude (the "degenerate" stored matrices of
    /// classes/placed-object.md 1.1).
    pub fn is_degenerate(&self) -> bool {
        self.cols.iter().flatten().chain(self.t.iter()).any(|x| !x.is_finite() || x.abs() > 1e30)
    }

    /// A 3 x 3 matrix with the degenerate rule applied: identity when any component exceeds 1e30.
    pub fn sanitize_matrix(cols: [Vec3; 3]) -> [Vec3; 3] {
        if cols.iter().flatten().any(|x| !x.is_finite() || x.abs() > 1e30) { Affine::IDENTITY.cols } else { cols }
    }

    /// Column-major 4 x 4 matrix (`[c0, 0, c1, 0, c2, 0, t, 1]`), ready for a GPU uniform.
    pub fn to_mat4(&self) -> [f32; 16] {
        let [a, b, c] = self.cols;
        let t = self.t;
        [a[0], a[1], a[2], 0.0, b[0], b[1], b[2], 0.0, c[0], c[1], c[2], 0.0, t[0], t[1], t[2], 1.0]
    }
}

/// Reads a `mat3` then a `vec3` (stored transform).
pub fn read_transform(r: &mut Reader<'_>) -> Result<Affine> {
    let cols = r.mat3()?;
    let t = r.vec3()?;
    Ok(Affine { cols, t })
}

/// Pose (morph) data of a submesh, as offsets into the world file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoseBlock {
    /// Offset of the pose positions.
    pub positions: usize,
    /// Pose position count.
    pub position_count: u16,
    /// Offset of the pose normals.
    pub normals: usize,
    /// Pose normal count.
    pub normal_count: u16,
    /// Offset of the per-pose index block (`P` groups of `ns` position indices then `ns` normal indices, `u16`).
    pub indices: usize,
}

/// One submesh: one triangle strip with one shader (geometry.md 3.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Submesh {
    /// File offset of the record.
    pub offset: usize,
    /// Bounding-sphere centre.
    pub center: Vec3,
    /// Bounding-sphere radius.
    pub radius: f32,
    /// Position count.
    pub np: u8,
    /// Normal count.
    pub nn: u8,
    /// Colour count.
    pub nc: u8,
    /// UV count.
    pub nt: u8,
    /// Strip length (entries; one GPU vertex each).
    pub ns: u8,
    /// Zero bytes skipped to reach 4-byte alignment (counted from the file start).
    pub padding: u8,
    /// File offset of the position array (the arrays follow back to back).
    pub data: usize,
    /// Shader index into the world's shader table.
    pub shader: u16,
    /// Self-illumination value.
    pub self_illumination: u8,
    /// Bone map length (0 to 4).
    pub bone_count: u8,
    /// Bone map: entry `j` is an index into the mesh set's bone list.
    pub bone_map: [u8; 4],
    /// File offset of the weights (`np * bone_count` `f32`).
    pub weights: usize,
    /// Morph data, when the mesh set has poses.
    pub pose: Option<PoseBlock>,
    /// File offset just past the record.
    pub end: usize,
}

/// Strip entry byte 2 value meaning "no UV".
pub const NO_UV: u8 = 0xFF;
/// Strip entry byte 3 (low 7 bits) value meaning "no colour".
pub const NO_COLOR: u8 = 0x7F;
/// Strip entry byte 3 bit marking a strip break.
pub const STRIP_BREAK: u8 = 0x80;

/// One GPU vertex: 36 bytes, `#[repr(C)]`, no padding.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuVertex {
    /// Position in the owning object's local space (or, for the world mesh, world space).
    pub position: [f32; 3],
    /// Unit normal (zero when the entry has none).
    pub normal: [f32; 3],
    /// Texture coordinate; 0..1 spans the bitmap once. `[0, 0]` when the entry has none.
    pub uv: [f32; 2],
    /// Vertex colour R, G, B, A (0..255 as stored); white when the entry has none.
    pub color: [u8; 4],
}

/// A submesh as a GPU-ready triangle list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuSubmesh {
    /// One vertex per strip entry.
    pub vertices: Vec<GpuVertex>,
    /// Triangle list, counter-clockwise front faces (geometry.md 3.3), indices into `vertices`.
    pub indices: Vec<u16>,
    /// Shader index.
    pub shader: u16,
}

/// The triangles a strip produces (geometry.md 3.3): entry `k >= 2` closes a triangle unless its break bit is set;
/// even `k` gives `(k-2, k-1, k)`, odd `k` gives `(k-2, k, k-1)` (parity is absolute within the submesh).
///
/// Indices are `u16`: entries past index 65,535 produce no triangles (a stored strip has at most 255 entries).
pub fn strip_triangles(entries: &[[u8; 4]]) -> impl Iterator<Item = [u16; 3]> + '_ {
    let usable = entries.len().min(1 << 16);
    entries[..usable].iter().enumerate().skip(2).filter(|(_, e)| e[3] & STRIP_BREAK == 0).map(|(k, _)| {
        let k = k as u16;
        if k % 2 == 0 { [k - 2, k - 1, k] } else { [k - 2, k, k - 1] }
    })
}

impl Submesh {
    fn read(r: &mut Reader<'_>, poses: u8) -> Result<Submesh> {
        let offset = r.pos();
        let center = r.vec3()?;
        let radius = r.f32()?;
        let [np, nn, nc, nt, ns] = r.array::<5>()?;
        let padding = r.align_zero(4)? as u8;
        let data = r.pos();
        r.skip(np as usize * 12 + nn as usize * 12 + nc as usize * 4 + nt as usize * 8 + ns as usize * 4)?;
        let shader = r.u16()?;
        let self_illumination = r.u8()?;
        let bone_count = r.u8()?;
        if bone_count > 4 {
            return Err(Error::Corrupt { offset: r.pos() - 1, what: "submesh bone map longer than 4" });
        }
        let mut bone_map = [0u8; 4];
        bone_map[..bone_count as usize].copy_from_slice(r.bytes(bone_count as usize)?);
        let weights = r.pos();
        r.skip_n(np as usize * bone_count as usize, 4)?;
        let pose = if poses > 0 {
            let position_count = r.u16()?;
            let positions = r.pos();
            r.skip_n(position_count as usize, 12)?;
            let normal_count = r.u16()?;
            let normals = r.pos();
            r.skip_n(normal_count as usize, 12)?;
            let indices = r.pos();
            r.skip_n(poses as usize * ns as usize, 4)?;
            Some(PoseBlock { positions, position_count, normals, normal_count, indices })
        } else {
            None
        };
        let sm = Submesh {
            offset,
            center,
            radius,
            np,
            nn,
            nc,
            nt,
            ns,
            padding,
            data,
            shader,
            self_illumination,
            bone_count,
            bone_map,
            weights,
            pose,
            end: r.pos(),
        };
        sm.check_entries(r.data())?;
        Ok(sm)
    }

    fn check_entries(&self, data: &[u8]) -> Result<()> {
        for (k, e) in self.strip(data).as_chunks::<4>().0.iter().enumerate() {
            let bad = e[0] >= self.np
                || (e[1] != 0xFF && e[1] >= self.nn)
                || (e[2] != NO_UV && e[2] >= self.nt)
                || (e[3] & 0x7F != NO_COLOR && e[3] & 0x7F >= self.nc);
            if bad {
                return Err(Error::Corrupt {
                    offset: self.strip_offset() + k * 4,
                    what: "strip entry index out of range",
                });
            }
        }
        Ok(())
    }

    /// File offset of the normals.
    pub fn normals_offset(&self) -> usize {
        self.data + self.np as usize * 12
    }
    /// File offset of the colours.
    pub fn colors_offset(&self) -> usize {
        self.normals_offset() + self.nn as usize * 12
    }
    /// File offset of the UVs.
    pub fn uvs_offset(&self) -> usize {
        self.colors_offset() + self.nc as usize * 4
    }
    /// File offset of the strip entries.
    pub fn strip_offset(&self) -> usize {
        self.uvs_offset() + self.nt as usize * 8
    }

    fn vec3_at(data: &[u8], off: usize) -> Vec3 {
        // Offsets were bounds-checked when the submesh was parsed.
        [le_f32(data, off).unwrap_or(0.0), le_f32(data, off + 4).unwrap_or(0.0), le_f32(data, off + 8).unwrap_or(0.0)]
    }

    /// Position `i` (< `np`).
    pub fn position(&self, data: &[u8], i: usize) -> Vec3 {
        Self::vec3_at(data, self.data + i * 12)
    }
    /// Normal `i` (< `nn`).
    pub fn normal(&self, data: &[u8], i: usize) -> Vec3 {
        Self::vec3_at(data, self.normals_offset() + i * 12)
    }
    /// Colour `i` (< `nc`), R, G, B, A.
    pub fn color(&self, data: &[u8], i: usize) -> [u8; 4] {
        let o = self.colors_offset() + i * 4;
        data.get(o..o + 4).map_or([0; 4], |c| [c[0], c[1], c[2], c[3]])
    }
    /// UV `i` (< `nt`).
    pub fn uv(&self, data: &[u8], i: usize) -> [f32; 2] {
        let o = self.uvs_offset() + i * 8;
        [le_f32(data, o).unwrap_or(0.0), le_f32(data, o + 4).unwrap_or(0.0)]
    }
    /// The raw strip entries (`ns * 4` bytes).
    pub fn strip<'d>(&self, data: &'d [u8]) -> &'d [u8] {
        let o = self.strip_offset();
        data.get(o..o + self.ns as usize * 4).unwrap_or(&[])
    }
    /// Strip entries as arrays.
    pub fn entries(&self, data: &[u8]) -> Vec<[u8; 4]> {
        self.strip(data).as_chunks::<4>().0.iter().map(|e| [e[0], e[1], e[2], e[3]]).collect()
    }
    /// Weight `j` (< `bone_count`) of position `i`.
    pub fn weight(&self, data: &[u8], i: usize, j: usize) -> f32 {
        le_f32(data, self.weights + (i * self.bone_count as usize + j) * 4).unwrap_or(0.0)
    }
    /// The skinning influences of position `i`: up to four (bone, weight) pairs, the bone given as an index into
    /// the mesh set's bone list (through the bone map, geometry.md 3.1). Unused slots have weight 0 and bone 0.
    pub fn influences(&self, data: &[u8], i: usize) -> ([u8; 4], [f32; 4]) {
        let mut bones = [0u8; 4];
        let mut weights = [0.0f32; 4];
        for j in 0..self.bone_count as usize {
            bones[j] = self.bone_map[j];
            weights[j] = self.weight(data, i, j);
        }
        (bones, weights)
    }

    /// Pose `pose`'s position for strip entry `k`, or `None` where the pose keeps the base position
    /// (geometry.md 3.1: an index outside the pose arrays). `pose` must be below the mesh set's pose count.
    pub fn pose_position(&self, data: &[u8], pose: usize, k: usize) -> Option<Vec3> {
        let pb = self.pose?;
        let at = pb.indices + (pose * self.ns as usize * 2 + k) * 2;
        let idx = crate::reader::le_u16(data, at).ok()? as usize;
        (idx < pb.position_count as usize).then(|| Self::vec3_at(data, pb.positions + idx * 12))
    }

    /// Pose `pose`'s normal for strip entry `k`, or `None` where the pose keeps the base normal.
    pub fn pose_normal(&self, data: &[u8], pose: usize, k: usize) -> Option<Vec3> {
        let pb = self.pose?;
        let at = pb.indices + (pose * self.ns as usize * 2 + self.ns as usize + k) * 2;
        let idx = crate::reader::le_u16(data, at).ok()? as usize;
        (idx < pb.normal_count as usize).then(|| Self::vec3_at(data, pb.normals + idx * 12))
    }

    /// The triangle list (indices of strip entries).
    pub fn triangles(&self, data: &[u8]) -> Vec<[u16; 3]> {
        strip_triangles(&self.entries(data)).collect()
    }
    /// Number of triangles the strip produces.
    pub fn triangle_count(&self, data: &[u8]) -> usize {
        self.strip(data)
            .as_chunks::<4>()
            .0
            .iter()
            .enumerate()
            .filter(|(k, e)| *k >= 2 && e[3] & STRIP_BREAK == 0)
            .count()
    }

    /// Builds the GPU form: one vertex per strip entry and a triangle list.
    pub fn to_gpu(&self, data: &[u8]) -> GpuSubmesh {
        let mut out = GpuSubmesh { vertices: Vec::new(), indices: Vec::new(), shader: self.shader };
        self.append_gpu(data, &mut out.vertices, &mut out.indices);
        out
    }

    /// Appends this submesh's vertices to `vertices` and its triangles (indices relative to the first appended
    /// vertex, so a draw uses a base vertex) to `indices`.
    pub fn append_gpu(&self, data: &[u8], vertices: &mut Vec<GpuVertex>, indices: &mut Vec<u16>) {
        let entries = self.entries(data);
        vertices.reserve(entries.len());
        for e in &entries {
            let normal = if e[1] == 0xFF { [0.0; 3] } else { self.normal(data, e[1] as usize) };
            let uv = if e[2] == NO_UV { [0.0; 2] } else { self.uv(data, e[2] as usize) };
            let ci = e[3] & 0x7F;
            let color = if ci == NO_COLOR { [0xFF; 4] } else { self.color(data, ci as usize) };
            vertices.push(GpuVertex { position: self.position(data, e[0] as usize), normal, uv, color });
        }
        for t in strip_triangles(&entries) {
            indices.extend_from_slice(&t);
        }
    }
}

/// One outline part (geometry.md 3.2), as offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutlinePart {
    /// File offset.
    pub offset: usize,
    /// Position count.
    pub position_count: u8,
    /// Edge count.
    pub edge_count: u8,
    /// File offset of the edges (4 bytes each).
    pub edges: usize,
    /// File offset just past the record.
    pub end: usize,
}

impl OutlinePart {
    fn read(r: &mut Reader<'_>, poses: u8) -> Result<OutlinePart> {
        let offset = r.pos();
        let k = r.u8()?;
        r.skip(k as usize * 12)?;
        let e = r.u8()?;
        let edges = r.pos();
        r.skip(e as usize * 4)?;
        let nb = r.u8()?;
        r.skip(nb as usize)?;
        r.skip_n(k as usize * nb as usize, 4)?;
        if poses > 0 {
            let pp = r.u16()?;
            r.skip_n(pp as usize, 12)?;
            r.skip_n(poses as usize * k as usize, 2)?;
        }
        Ok(OutlinePart { offset, position_count: k, edge_count: e, edges, end: r.pos() })
    }
}

/// The instance part of a mesh: which earlier mesh it draws and with which matrix.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instance {
    /// Index of the source mesh in the same set (lower than the instance's own index).
    pub source: u16,
    /// The instance matrix and translation, in the object's local space.
    pub matrix: Affine,
}

/// Mesh flag bits (geometry.md 3).
pub mod mesh_flags {
    /// Instance of an earlier mesh.
    pub const INSTANCE: u16 = 0x001;
    /// Zone mask present.
    pub const ZONE_MASK: u16 = 0x002;
    /// Draw-order value present.
    pub const SORT_KEY: u16 = 0x004;
    /// Fog factor present.
    pub const FOG: u16 = 0x008;
    /// Maximum draw distance present.
    pub const MAX_DISTANCE: u16 = 0x010;
    /// Maximum cel-outline distance present.
    pub const OUTLINE_DISTANCE: u16 = 0x020;
    /// Texture-animation record present.
    pub const ANIMATION: u16 = 0x040;
    /// Glint (view-angle fade) record present: a direction and four coefficients ([`Mesh::glint`]).
    pub const HIGHLIGHT: u16 = 0x080;
    /// Vertex/UV distortion record present.
    pub const DISTORTION: u16 = 0x100;
    /// Submesh culling radius present.
    pub const CULL_RADIUS: u16 = 0x200;
    /// Every defined bit.
    pub const ALL: u16 = 0x3FF;
}

/// One mesh of a mesh set (geometry.md 3).
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// File offset of the flag word.
    pub offset: usize,
    /// The flag word.
    pub flags: u16,
    /// Present for instance meshes.
    pub instance: Option<Instance>,
    /// Zone mask (bit 0x002).
    pub zone_mask: Option<u32>,
    /// Submesh culling radius (bit 0x200).
    pub cull_radius: Option<f32>,
    /// Draw-order value `g` (bit 0x004); the effective sort key is `g * |g|`.
    pub sort_key: Option<f32>,
    /// Fog factor (bit 0x008).
    pub fog: Option<f32>,
    /// Maximum draw distance (bit 0x010).
    pub max_distance: Option<f32>,
    /// Maximum outline distance (bit 0x020).
    pub outline_distance: Option<f32>,
    /// Texture animation (bit 0x040).
    pub animation: Option<AnimRecord>,
    /// Bounding-sphere centre.
    pub center: Vec3,
    /// Bounding-sphere radius.
    pub radius: f32,
    /// Object id of the mesh.
    pub oid: i16,
    /// Billboard mode (0 none, 1 face the camera).
    pub billboard: u8,
    /// Render pass (textures.md 4.1).
    pub pass: u8,
    /// Mesh flags byte (not decoded).
    pub mesh_flags: u8,
    /// Submeshes (empty for an instance).
    pub submeshes: Vec<Submesh>,
    /// Outline parts (empty for an instance).
    pub outlines: Vec<OutlinePart>,
    /// File offset just past the mesh.
    pub end: usize,
}

/// The view-angle fade ("glint") of a mesh with flag 0x080 (geometry.md 3; rendering.md 6.2): a direction and
/// four coefficients; the mesh's alpha is multiplied by `clamp(c0 + x·(c1 + x·(c2 + x·c3)), 0, 1)` with
/// `x = |direction_world · camera_forward|`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glint {
    /// Direction, in the mesh's space.
    pub direction: Vec3,
    /// Coefficients c0 to c3.
    pub c: [f32; 4],
}

impl Glint {
    /// The alpha factor for `x = |direction_world · camera_forward|`.
    pub fn factor(&self, x: f32) -> f32 {
        let c = &self.c;
        (c[0] + x * (c[1] + x * (c[2] + x * c[3]))).clamp(0.0, 1.0)
    }
}

impl Mesh {
    /// The mesh's glint record (flag 0x080), read from `data` (the world the mesh was parsed from); `None` without
    /// the flag.
    pub fn glint(&self, data: &[u8]) -> Result<Option<Glint>> {
        if self.flags & mesh_flags::HIGHLIGHT == 0 {
            return Ok(None);
        }
        let mut r = Reader::at(data, self.offset + 2);
        let has = |b: u16| self.flags & b != 0;
        if has(mesh_flags::INSTANCE) {
            r.skip(2 + 48)?;
        }
        for b in [
            mesh_flags::ZONE_MASK,
            mesh_flags::CULL_RADIUS,
            mesh_flags::SORT_KEY,
            mesh_flags::FOG,
            mesh_flags::MAX_DISTANCE,
            mesh_flags::OUTLINE_DISTANCE,
        ] {
            if has(b) {
                r.skip(4)?;
            }
        }
        if has(mesh_flags::ANIMATION) {
            AnimRecord::read(&mut r)?;
        }
        let direction = r.vec3()?;
        Ok(Some(Glint { direction, c: [r.f32()?, r.f32()?, r.f32()?, r.f32()?] }))
    }

    fn read(r: &mut Reader<'_>, index: usize, poses: u8) -> Result<Mesh> {
        let offset = r.pos();
        let flags = r.u16()?;
        if flags & !mesh_flags::ALL != 0 {
            return Err(Error::Corrupt { offset, what: "unknown mesh flag bits" });
        }
        let has = |b: u16| flags & b != 0;
        let instance = if has(mesh_flags::INSTANCE) {
            let source = r.i16()?;
            if source < 0 || source as usize >= index {
                return Err(Error::Corrupt { offset, what: "instance source is not an earlier mesh" });
            }
            let cols = r.mat3()?;
            let t = r.vec3()?;
            Some(Instance { source: source as u16, matrix: Affine { cols, t } })
        } else {
            None
        };
        let opt_u32 = |r: &mut Reader<'_>, b| if has(b) { r.u32().map(Some) } else { Ok(None) };
        let opt_f32 = |r: &mut Reader<'_>, b| if has(b) { r.f32().map(Some) } else { Ok(None) };
        let zone_mask = opt_u32(r, mesh_flags::ZONE_MASK)?;
        let cull_radius = opt_f32(r, mesh_flags::CULL_RADIUS)?;
        let sort_key = opt_f32(r, mesh_flags::SORT_KEY)?;
        let fog = opt_f32(r, mesh_flags::FOG)?;
        let max_distance = opt_f32(r, mesh_flags::MAX_DISTANCE)?;
        let outline_distance = opt_f32(r, mesh_flags::OUTLINE_DISTANCE)?;
        let animation = if has(mesh_flags::ANIMATION) { Some(AnimRecord::read(r)?) } else { None };
        if has(mesh_flags::HIGHLIGHT) {
            r.skip(28)?;
        }
        if has(mesh_flags::DISTORTION) {
            r.skip(2)?;
            if r.i8()? != -1 {
                r.skip(16 + 48)?;
            }
            r.skip(3)?;
        }
        let center = r.vec3()?;
        let radius = r.f32()?;
        let oid = r.i16()?;
        let [billboard, pass, mesh_flags_byte] = r.array::<3>()?;
        let mut submeshes = Vec::new();
        let mut outlines = Vec::new();
        if instance.is_none() {
            let s = r.u16()?;
            submeshes.reserve(s as usize);
            for _ in 0..s {
                submeshes.push(Submesh::read(r, poses)?);
            }
            let o = r.u16()?;
            for _ in 0..o {
                outlines.push(OutlinePart::read(r, poses)?);
            }
        }
        Ok(Mesh {
            offset,
            flags,
            instance,
            zone_mask,
            cull_radius,
            sort_key,
            fog,
            max_distance,
            outline_distance,
            animation,
            center,
            radius,
            oid,
            billboard,
            pass,
            mesh_flags: mesh_flags_byte,
            submeshes,
            outlines,
            end: r.pos(),
        })
    }
}

/// All the geometry of one drawable object (geometry.md 2).
#[derive(Clone, Debug, PartialEq)]
pub struct MeshSet {
    /// File offset.
    pub offset: usize,
    /// Relight flag.
    pub relight: u8,
    /// Second header byte (not decoded).
    pub unknown: u8,
    /// Object ids of the bones.
    pub bones: Vec<i16>,
    /// Initial pose weights (one per pose).
    pub pose_weights: Vec<f32>,
    /// The meshes.
    pub meshes: Vec<Mesh>,
    /// File offset just past the set.
    pub end: usize,
}

/// One thing to draw from a mesh set: the real mesh whose submeshes are drawn, and the transform from that mesh's
/// space to the object's space (identity for a real mesh, the instance matrix for an instance).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshDraw {
    /// Index of the mesh that issued the draw (real or instance).
    pub mesh: usize,
    /// Index of the real mesh whose submeshes are drawn.
    pub geometry: usize,
    /// Mesh space to object space.
    pub transform: Affine,
    /// Render pass of the issuing mesh.
    pub pass: u8,
}

impl MeshSet {
    /// Parses a mesh set at the reader's position (the reader must cover the whole world file, because submesh
    /// padding counts from the file start).
    pub fn read(r: &mut Reader<'_>) -> Result<MeshSet> {
        let offset = r.pos();
        let relight = r.u8()?;
        let unknown = r.u8()?;
        let b = r.u8()?;
        let mut bones = Vec::with_capacity(b as usize);
        for _ in 0..b {
            bones.push(r.i16()?);
        }
        let p = r.u8()?;
        let mut pose_weights = Vec::with_capacity(p as usize);
        for _ in 0..p {
            pose_weights.push(r.f32()?);
        }
        let g = r.u16()?;
        let mut meshes = Vec::with_capacity(g as usize);
        for i in 0..g as usize {
            meshes.push(Mesh::read(r, i, p)?);
        }
        let set = MeshSet { offset, relight, unknown, bones, pose_weights, meshes, end: r.pos() };
        for m in &set.meshes {
            if let Some(inst) = m.instance
                && set.meshes[inst.source as usize].instance.is_some()
            {
                return Err(Error::Corrupt { offset: m.offset, what: "instance of an instance" });
            }
            for sm in &m.submeshes {
                if sm.bone_map[..sm.bone_count as usize].iter().any(|&j| j as usize >= set.bones.len()) {
                    return Err(Error::Corrupt { offset: sm.offset, what: "bone map index out of range" });
                }
            }
        }
        Ok(set)
    }

    /// Parses a mesh set at `offset` of the world file `data`.
    pub fn parse(data: &[u8], offset: usize) -> Result<MeshSet> {
        MeshSet::read(&mut Reader::at(data, offset))
    }

    /// Pose count `P`.
    pub fn pose_count(&self) -> u8 {
        self.pose_weights.len() as u8
    }

    /// Whether the set draws anything: a mesh with a submesh, or an instance mesh (worlds.md "drawable").
    pub fn is_drawable(&self) -> bool {
        self.meshes.iter().any(|m| m.instance.is_some() || !m.submeshes.is_empty())
    }

    /// Every draw of the set: real meshes with the identity, instances with their matrix.
    pub fn draws(&self) -> impl Iterator<Item = MeshDraw> + '_ {
        self.meshes.iter().enumerate().map(|(i, m)| match m.instance {
            Some(inst) => MeshDraw { mesh: i, geometry: inst.source as usize, transform: inst.matrix, pass: m.pass },
            None => MeshDraw { mesh: i, geometry: i, transform: Affine::IDENTITY, pass: m.pass },
        })
    }
}

/// Geometry totals in the form of geometry.md 7.1.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeometryStats {
    /// Mesh sets read.
    pub mesh_sets: usize,
    /// Mesh sets with at least one mesh.
    pub nonempty_sets: usize,
    /// Meshes (real and instance).
    pub meshes: usize,
    /// Instance meshes.
    pub instances: usize,
    /// Submeshes of real meshes.
    pub submeshes: usize,
    /// Strip entries (= GPU vertices).
    pub strip_entries: usize,
    /// Stored positions.
    pub positions: usize,
    /// Triangles from real meshes, instances not multiplied.
    pub triangles: usize,
    /// Outline parts.
    pub outline_parts: usize,
}

impl GeometryStats {
    /// Adds one mesh set.
    pub fn add(&mut self, data: &[u8], set: &MeshSet) {
        self.mesh_sets += 1;
        if !set.meshes.is_empty() {
            self.nonempty_sets += 1;
        }
        for m in &set.meshes {
            self.meshes += 1;
            if m.instance.is_some() {
                self.instances += 1;
            }
            self.outline_parts += m.outlines.len();
            for sm in &m.submeshes {
                self.submeshes += 1;
                self.strip_entries += sm.ns as usize;
                self.positions += sm.np as usize;
                self.triangles += sm.triangle_count(data);
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;

    /// Little helper to build synthetic world bytes.
    #[derive(Default)]
    pub(crate) struct W(pub Vec<u8>);
    impl W {
        pub fn u8(&mut self, v: u8) -> &mut Self {
            self.0.push(v);
            self
        }
        pub fn i8(&mut self, v: i8) -> &mut Self {
            self.u8(v as u8)
        }
        pub fn u16(&mut self, v: u16) -> &mut Self {
            self.0.extend_from_slice(&v.to_le_bytes());
            self
        }
        pub fn i16(&mut self, v: i16) -> &mut Self {
            self.0.extend_from_slice(&v.to_le_bytes());
            self
        }
        pub fn u32(&mut self, v: u32) -> &mut Self {
            self.0.extend_from_slice(&v.to_le_bytes());
            self
        }
        pub fn f32(&mut self, v: f32) -> &mut Self {
            self.0.extend_from_slice(&v.to_le_bytes());
            self
        }
        pub fn v3(&mut self, v: Vec3) -> &mut Self {
            self.f32(v[0]).f32(v[1]).f32(v[2])
        }
        pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
            self.0.extend_from_slice(b);
            self
        }
        pub fn zeros(&mut self, n: usize) -> &mut Self {
            self.0.resize(self.0.len() + n, 0);
            self
        }
        pub fn ident(&mut self) -> &mut Self {
            self.v3([1., 0., 0.]).v3([0., 1., 0.]).v3([0., 0., 1.])
        }
        pub fn align4(&mut self) -> &mut Self {
            while self.0.len() % 4 != 0 {
                self.0.push(0);
            }
            self
        }
        /// A quad submesh: 4 positions, 1 normal, 1 colour, 4 UVs, 4 strip entries; one bone if `bone`.
        pub fn quad_submesh(&mut self, shader: u16, bone: bool) -> &mut Self {
            self.v3([0.5, 0.5, 0.]).f32(1.0).bytes(&[4, 1, 1, 4, 4]).align4();
            for p in [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 0.]] {
                self.v3(p);
            }
            self.v3([0., 0., 1.]).bytes(&[10, 20, 30, 0xFF]);
            for uv in [[0., 0.], [1., 0.], [0., 1.], [1., 1.]] {
                self.f32(uv[0]).f32(uv[1]);
            }
            self.bytes(&[0, 0, 0, 0x80, 1, 0, 1, 0x80, 2, 0, NO_UV, 0x7F, 3, 0, 3, 0x00]);
            self.u16(shader).u8(0);
            if bone {
                self.u8(1).u8(0).f32(1.).f32(1.).f32(1.).f32(1.);
            } else {
                self.u8(0);
            }
            self
        }
    }

    #[test]
    fn strip_rule_uses_absolute_parity() {
        let e = |b: u8| [0, 0, 0, b];
        let entries = [e(0x80), e(0x80), e(0), e(0), e(0x81), e(0x80), e(0), e(0)];
        let tris: Vec<_> = strip_triangles(&entries).collect();
        assert_eq!(tris, vec![[0, 1, 2], [1, 3, 2], [4, 5, 6], [5, 7, 6]]);
        // A break at an odd position keeps the parity: the next triangle at k = 5 is odd.
        let entries = [e(0x80), e(0x80), e(0), e(0x80), e(0x80), e(0)];
        let tris: Vec<_> = strip_triangles(&entries).collect();
        assert_eq!(tris, vec![[0, 1, 2], [3, 5, 4]]);
    }

    #[test]
    fn affine_composition_and_normals() {
        // 90 degrees about Z, then translate.
        let rz = Affine::new([[0., 1., 0.], [-1., 0., 0.], [0., 0., 1.]], [10., 0., 0.]);
        assert_eq!(rz.transform_point([1., 0., 0.]), [10., 1., 0.]);
        let child = Affine::new(Affine::IDENTITY.cols, [0., 2., 0.]);
        let w = rz.mul(&child);
        assert_eq!(w.t, [8., 0., 0.]);
        assert_eq!(w.cols, rz.cols);
        let s = Affine::new([[2., 0., 0.], [0., 1., 0.], [0., 0., 1.]], [0.; 3]);
        assert_eq!(s.normal_matrix()[0], [0.5, 0., 0.]);
        let mirror = Affine::new([[-1., 0., 0.], [0., 1., 0.], [0., 0., 1.]], [0.; 3]);
        assert!(mirror.is_mirrored() && !rz.is_mirrored());
        let bad = Affine::new([[1., 0., 0.], [-f32::MAX; 3], [-f32::MAX; 3]], [0.; 3]);
        assert!(bad.is_degenerate());
        assert_eq!(Affine::sanitize_matrix(bad.cols), Affine::IDENTITY.cols);
        assert_eq!(rz.to_mat4()[12], 10.0);
    }

    fn sample_set(start_pad: usize) -> (Vec<u8>, usize) {
        let mut w = W::default();
        w.zeros(start_pad);
        let at = w.0.len();
        // relight, unknown, 1 bone (oid 77), no poses, 3 meshes.
        w.u8(0).u8(1).u8(1).i16(77).u8(0).u16(3);
        // Mesh 0: real, flags ZONE_MASK | MAX_DISTANCE.
        w.u16(0x012).u32(0xF).f32(FLT_MAX);
        w.v3([0.5, 0.5, 0.]).f32(1.).i16(5).bytes(&[0, 3, 0x13]);
        w.u16(1).quad_submesh(7, true).u16(0);
        // Mesh 1: instance of 0, mirrored, with zone mask and culling radius.
        w.u16(0x203).i16(0).v3([-1., 0., 0.]).v3([0., 1., 0.]).v3([0., 0., 1.]).v3([5., 0., 0.]);
        w.u32(1).f32(2.0);
        w.v3([4.5, 0.5, 0.]).f32(1.).i16(6).bytes(&[1, 3, 0x10]);
        // Mesh 2: real, highlight effect + distortion record + animation, one outline part.
        w.u16(0x1C0).u16(1).i16(9).u16(0).zeros(24); // animation kind 1
        w.zeros(28); // highlight effect
        w.i16(3).i8(-1).bytes(&[1, 2, 3]); // warp without matrix
        w.v3([0.; 3]).f32(1.).i16(8).bytes(&[0, 13, 0x20]);
        w.u16(1).quad_submesh(8, false);
        w.u16(1).u8(2).v3([0.; 3]).v3([1.; 3]).u8(1).bytes(&[0, 1, 1, 0]).u8(0);
        w.u8(0xEE); // a byte after the set
        (w.0, at)
    }

    #[test]
    fn parses_mesh_set_with_instances_and_padding() {
        for pad in 0..4 {
            let (data, at) = sample_set(pad);
            let set = MeshSet::parse(&data, at).unwrap();
            assert_eq!(data[set.end], 0xEE);
            assert_eq!(set.bones, [77]);
            assert_eq!(set.meshes.len(), 3);
            let m0 = &set.meshes[0];
            assert_eq!((m0.zone_mask, m0.max_distance, m0.pass, m0.oid), (Some(0xF), Some(FLT_MAX), 3, 5));
            let sm = &m0.submeshes[0];
            assert_eq!(sm.data % 4, 0);
            assert_eq!((sm.np, sm.ns, sm.shader, sm.bone_count), (4, 4, 7, 1));
            assert_eq!(sm.position(&data, 3), [1., 1., 0.]);
            assert_eq!(sm.weight(&data, 2, 0), 1.0);
            assert_eq!(sm.triangles(&data), vec![[0, 1, 2], [1, 3, 2]]);
            let gpu = sm.to_gpu(&data);
            assert_eq!(gpu.vertices.len(), 4);
            assert_eq!(gpu.indices, [0, 1, 2, 1, 3, 2]);
            assert_eq!(gpu.vertices[1].uv, [1., 0.]);
            assert_eq!(gpu.vertices[1].color, [10, 20, 30, 0xFF]);
            assert_eq!(gpu.vertices[2].uv, [0., 0.]); // no UV
            assert_eq!(gpu.vertices[2].color, [0xFF; 4]); // no colour
            assert_eq!(gpu.vertices[0].normal, [0., 0., 1.]);
            let inst = set.meshes[1].instance.unwrap();
            assert_eq!(inst.source, 0);
            assert!(inst.matrix.is_mirrored());
            assert_eq!(set.meshes[1].cull_radius, Some(2.0));
            let m2 = &set.meshes[2];
            assert_eq!(m2.animation.unwrap().kind, 1);
            assert_eq!(m2.outlines.len(), 1);
            assert_eq!(m2.outlines[0].edge_count, 1);
            let draws: Vec<_> = set.draws().collect();
            assert_eq!(draws.iter().map(|d| d.geometry).collect::<Vec<_>>(), [0, 0, 2]);
            let mut stats = GeometryStats::default();
            stats.add(&data, &set);
            assert_eq!(
                stats,
                GeometryStats {
                    mesh_sets: 1,
                    nonempty_sets: 1,
                    meshes: 3,
                    instances: 1,
                    submeshes: 2,
                    strip_entries: 8,
                    positions: 8,
                    triangles: 4,
                    outline_parts: 1
                }
            );
        }
    }

    #[test]
    fn rejects_bad_references() {
        let (mut data, at) = sample_set(0);
        // Make mesh 1's instance source point at itself.
        let set = MeshSet::parse(&data, at).unwrap();
        let src = set.meshes[1].offset + 2;
        data[src] = 1;
        assert!(MeshSet::parse(&data, at).is_err());
        // Strip entry with an out-of-range position index.
        let (mut data, at) = sample_set(0);
        let strip = set.meshes[0].submeshes[0].strip_offset();
        data[strip] = 9;
        assert!(MeshSet::parse(&data, at).is_err());
        // Unknown mesh flag bits.
        let (mut data, at) = sample_set(0);
        data[set.meshes[0].offset + 1] |= 0x40;
        assert!(MeshSet::parse(&data, at).is_err());
    }
}
