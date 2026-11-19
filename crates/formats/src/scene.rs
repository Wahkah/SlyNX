//! A renderer-ready description of a world's static scene.
//!
//! [`Scene::build`] turns a parsed [`World`] into:
//!
//! - one **vertex arena** ([`GpuVertex`], 36 bytes each) and one **index arena** (`u16`, triangle lists with
//!   counter-clockwise front faces); each [`SceneSubmesh`] is a range of both, drawn with a base vertex;
//! - [`SceneMesh`]es: the real meshes that are drawn at least once, each a range of submeshes, stored once however
//!   many times they are instanced;
//! - [`SceneInstance`]s: one per draw of a mesh, with the full object-to-world transform (object world transform
//!   times instance matrix), a mirrored flag (flip the front-face rule for that draw) and the render pass.
//!   Instance meshes whose matrix holds the `FLT_MAX` "not placed" sentinel are left out and counted in
//!   [`Scene::skipped_degenerate`]; objects whose world position holds it are left out with their descendants;
//! - [`SceneMaterial`]s: one per shader of the world's shader table (index = shader index), with the texture the
//!   shader shows in an unlit render;
//! - [`SceneTexture`]s: the decoded RGBA8 images that materials use (rows in storage order: row 0 is v = 0).
//!
//! Positions are in game space: right-handed, Z up (transforms.md 7). Apply the Y-up conversion of
//! transforms.md 7.1 in the view matrix if wanted.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::ops::Range;

use crate::geometry::{Affine, GpuVertex};
use crate::stream::{PlacementOptions, Record};
use crate::textures::RgbaImage;
use crate::world::World;

/// A range of the arenas drawn with one material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneSubmesh {
    /// First index in [`Scene::indices`].
    pub first_index: u32,
    /// Number of indices (3 per triangle).
    pub index_count: u32,
    /// Added to every index to address [`Scene::vertices`].
    pub base_vertex: u32,
    /// Number of vertices used.
    pub vertex_count: u32,
    /// Material (shader index).
    pub material: u16,
}

/// A real mesh's geometry, in the space of the mesh (its object's local space).
#[derive(Clone, Debug, PartialEq)]
pub struct SceneMesh {
    /// The record that owns the mesh set.
    pub record: u32,
    /// Index of the mesh within the set.
    pub mesh: u32,
    /// Its submeshes, a range of [`Scene::submeshes`].
    pub submeshes: Range<u32>,
    /// Bounding-sphere centre (mesh space).
    pub center: [f32; 3],
    /// Bounding-sphere radius.
    pub radius: f32,
}

/// One draw of a mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneInstance {
    /// Index into [`Scene::meshes`].
    pub mesh: u32,
    /// Mesh space to world space.
    pub transform: Affine,
    /// Whether `transform` mirrors: the triangles' winding is reversed for this draw.
    pub mirrored: bool,
    /// Render pass of the issuing mesh (textures.md 4.1).
    pub pass: u8,
    /// Billboard mode of the issuing mesh.
    pub billboard: u8,
    /// The record placed.
    pub record: u32,
    /// For a clone made by a proxy back-reference, that proxy's record.
    pub clone_of_proxy: Option<u32>,
    /// Index of the mesh that issued the draw in the record's mesh set (an instance mesh, or the real mesh itself):
    /// its pass, sort key, fog factor, flags, billboard mode and texture animation apply to the draw.
    pub issuing_mesh: u32,
}

impl SceneInstance {
    /// The mesh that issued the draw, from the world the scene was built from.
    pub fn issuing<'w>(&self, world: &'w World<'_>) -> Option<&'w crate::geometry::Mesh> {
        world.objects.records.get(self.record as usize)?.mesh_set.as_ref()?.meshes.get(self.issuing_mesh as usize)
    }
}

/// A material: one shader of the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneMaterial {
    /// Shader kind (textures.md 4).
    pub kind: u8,
    /// Blend flags.
    pub blend: u8,
    /// The shader's render pass.
    pub pass: u8,
    /// Material colour R, G, B, A (0..255).
    pub color: [u8; 4],
    /// Index into [`Scene::textures`], when the shader has a usable texture and is used by a drawn submesh.
    pub texture: Option<u32>,
}

/// A decoded texture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneTexture {
    /// Bitmap index.
    pub bitmap: u32,
    /// Palette index (none for 32-bit bitmaps).
    pub palette: Option<u32>,
    /// The image, rows in storage order.
    pub image: RgbaImage,
}

/// The static scene of one world.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    /// Vertex arena.
    pub vertices: Vec<GpuVertex>,
    /// Index arena (relative to each submesh's base vertex).
    pub indices: Vec<u16>,
    /// Submesh ranges.
    pub submeshes: Vec<SceneSubmesh>,
    /// Meshes.
    pub meshes: Vec<SceneMesh>,
    /// Draws.
    pub instances: Vec<SceneInstance>,
    /// Materials, indexed by shader index.
    pub materials: Vec<SceneMaterial>,
    /// Textures.
    pub textures: Vec<SceneTexture>,
    /// Textures that failed to decode: (bitmap, palette).
    pub texture_errors: Vec<(u32, Option<u32>)>,
    /// Instance-mesh draws left out because their instance matrix holds the "not placed" sentinel (a component of
    /// magnitude above 1e30; 201 instance meshes on the disc). Objects that are not placed never reach the scene
    /// (see [`crate::stream::ObjectStream::placements`]).
    pub skipped_degenerate: u32,
}

/// Options for [`Scene::build`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneOptions {
    /// Placement options (clones, ancestor overrides).
    pub placement: PlacementOptions,
    /// Decode the textures the drawn materials use.
    pub decode_textures: bool,
}

impl Default for SceneOptions {
    fn default() -> Self {
        SceneOptions { placement: PlacementOptions::default(), decode_textures: true }
    }
}

impl Scene {
    /// Builds the scene of `world`.
    pub fn build(world: &World<'_>, opts: SceneOptions) -> Scene {
        let data = world.data;
        let records: &[Record] = &world.objects.records;
        let mut scene = Scene::default();
        let mut mesh_ids: BTreeMap<(u32, u32), u32> = BTreeMap::new();
        let mut used_shaders: Vec<bool> = alloc::vec![false; world.layout.materials.shaders.len()];

        for p in world.objects.placements(opts.placement) {
            let Some(set) = records[p.record as usize].mesh_set.as_ref() else { continue };
            for d in set.draws() {
                let transform = p.world.mul(&d.transform);
                // Instance matrices stored at FLT_MAX magnitude mean "not placed" (transforms.md 4).
                if d.transform.is_degenerate() || transform.is_degenerate() {
                    scene.skipped_degenerate += 1;
                    continue;
                }
                let key = (p.record, d.geometry as u32);
                let mesh_index = match mesh_ids.get(&key) {
                    Some(&m) => m,
                    None => {
                        let m = &set.meshes[d.geometry];
                        let first = scene.submeshes.len() as u32;
                        for sm in &m.submeshes {
                            let base_vertex = scene.vertices.len() as u32;
                            let first_index = scene.indices.len() as u32;
                            sm.append_gpu(data, &mut scene.vertices, &mut scene.indices);
                            scene.submeshes.push(SceneSubmesh {
                                first_index,
                                index_count: scene.indices.len() as u32 - first_index,
                                base_vertex,
                                vertex_count: scene.vertices.len() as u32 - base_vertex,
                                material: sm.shader,
                            });
                            if let Some(u) = used_shaders.get_mut(sm.shader as usize) {
                                *u = true;
                            }
                        }
                        let id = scene.meshes.len() as u32;
                        scene.meshes.push(SceneMesh {
                            record: p.record,
                            mesh: d.geometry as u32,
                            submeshes: first..scene.submeshes.len() as u32,
                            center: m.center,
                            radius: m.radius,
                        });
                        mesh_ids.insert(key, id);
                        id
                    }
                };
                scene.instances.push(SceneInstance {
                    mesh: mesh_index,
                    transform,
                    mirrored: transform.is_mirrored(),
                    pass: d.pass,
                    billboard: set.meshes[d.mesh].billboard,
                    record: p.record,
                    clone_of_proxy: p.clone_of_proxy,
                    issuing_mesh: d.mesh as u32,
                });
            }
        }

        let tables = &world.layout.materials;
        let payload = world.payload();
        let mut tex_ids: BTreeMap<(usize, Option<usize>), Option<u32>> = BTreeMap::new();
        for (i, sh) in tables.shaders.iter().enumerate() {
            let mut texture = None;
            if opts.decode_textures
                && used_shaders[i]
                && let Some((b, p)) = tables.shader_texture(i)
            {
                texture = *tex_ids.entry((b, p)).or_insert_with(|| {
                    let pal = p.map(|p| &tables.palettes[p]);
                    match payload.decode(&tables.bitmaps[b], pal) {
                        Ok(image) => {
                            scene.textures.push(SceneTexture { bitmap: b as u32, palette: p.map(|p| p as u32), image });
                            Some(scene.textures.len() as u32 - 1)
                        }
                        Err(_) => {
                            scene.texture_errors.push((b as u32, p.map(|p| p as u32)));
                            None
                        }
                    }
                });
            }
            scene.materials.push(SceneMaterial {
                kind: sh.kind,
                blend: sh.blend,
                pass: sh.pass,
                color: sh.color,
                texture,
            });
        }
        scene
    }

    /// Total triangles drawn (every instance counted).
    pub fn drawn_triangles(&self) -> u64 {
        self.instances
            .iter()
            .map(|i| {
                let m = &self.meshes[i.mesh as usize];
                self.submeshes[m.submeshes.start as usize..m.submeshes.end as usize]
                    .iter()
                    .map(|s| s.index_count as u64 / 3)
                    .sum::<u64>()
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic world: outer sections with one palette, one bitmap and one shader, the synthetic object
    /// stream, padding and the texture payload.
    fn synthetic_world() -> Vec<u8> {
        let mut w = crate::world::tests::outer(32 + 64);
        // Replace the empty material tables (last 10 bytes) with real ones.
        w.0.truncate(w.0.len() - 10);
        w.u16(1).u32(1).u16(16).u16(1).u32(0);
        w.u16(1).u16(8).u16(8).u32(1).u8(0x14).u8(2).u16(1).u32(32).u32(64);
        w.u16(1).u16(0);
        w.u8(1).u8(2).i16(1).bytes(&[255; 8]).u32(1).u16(0xFFFF).u8(3).u8(1).u16(0);
        w.i16(1).i16(0).u8(1).u8(1).u16(0).u16(0);
        w.u16(0);
        crate::stream::tests::synthetic_stream_into(&mut w);
        w.0.resize(w.0.len().div_ceil(16) * 16, 0);
        let mut payload = alloc::vec![0u8; 96];
        payload[0..4].copy_from_slice(&[9, 8, 7, 0x80]);
        payload.extend_from_slice(&[0xAB; 7]); // unreferenced tail
        w.bytes(&payload);
        w.0
    }

    #[test]
    fn builds_scene_from_synthetic_world() {
        let data = synthetic_world();
        let world = World::load(&data).unwrap();
        assert_eq!(world.tail_bytes(), 7);
        let scene = Scene::build(&world, SceneOptions::default());
        // Placements: world mesh, 3, 5, 6, 8 and the clone of 8: six draws of five distinct meshes.
        assert_eq!(scene.instances.len(), 6);
        assert_eq!(scene.meshes.len(), 5);
        assert_eq!(scene.submeshes.len(), 5);
        assert_eq!(scene.vertices.len(), 20);
        assert_eq!(scene.indices.len(), 30);
        assert_eq!(scene.drawn_triangles(), 12);
        assert_eq!(scene.materials.len(), 1);
        let t = scene.materials[0].texture.unwrap() as usize;
        assert_eq!(&scene.textures[t].image.pixels[..4], &[9, 8, 7, 255]);
        let clone = scene.instances.last().unwrap();
        assert_eq!(clone.transform.t, [0., 200., 2.]);
        assert!(!clone.mirrored);
    }
}
