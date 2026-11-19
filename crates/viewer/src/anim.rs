//! The player character's animation in the viewer: one segment played in a loop on the character's rig, drawn
//! with GPU skinning.
//!
//! Built on the loader thread with the level ([`PlayerAnim::build`]); each frame [`PlayerAnim::update`] advances
//! the playback, poses the rig and writes the result into the renderer: the bone matrices of the skinned parts,
//! the world transforms of the rigid parts hanging from bones, and culling spheres that cover the pose.

use alloc::vec::Vec;

use engine::anim::playback::FiredEvent;
use engine::anim::{Binding, Playback, Rig, Skin, apply_segment};
use engine::math::affine_inverse;
use formats::anim::{RepeatMode, Segment};
use formats::geometry::Affine;
use formats::options::OptionTable;
use formats::world::World;
use render::math::Vec3;
use render::{Backend, BoneMatrix, InstanceId, RenderError, Renderer};

use crate::level::mat4;

/// The segment played on the player character: OID 130, the first required animation segment of the player's
/// lookup list (animation.md 7; 180 frames in Paris). The spec does not name the moves yet; this one keeps the
/// character in place.
pub const PLAYER_SEGMENT_OID: i16 = 130;
/// Class of animation segments.
const SEGMENT_CLASS: i16 = 131;
/// Class of the player character.
const PLAYER_CLASS: i16 = 7;

/// One drawn submesh that belongs to the character.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    /// Index into [`crate::level::PreparedLevel::draws`].
    pub draw: u32,
    /// The rig node of the object that owns the mesh.
    pub node: u32,
    /// The draw's transform relative to that node's world transform (the instance matrix of an instance mesh).
    pub relative: Affine,
    /// For a skinned mesh, its skin in [`PlayerAnim::skins`].
    pub skin: Option<u32>,
    /// Rest bounding sphere in mesh space.
    pub sphere: (Vec3, f32),
}

/// The animated player character.
#[derive(Clone, Debug)]
pub struct PlayerAnim {
    pub rig: Rig,
    pub skins: Vec<Skin>,
    pub segment: Segment,
    pub binding: Binding,
    pub playback: Playback,
    /// World transform of the rig's root's parent.
    pub base: Affine,
    pub parts: Vec<Part>,
    fired: Vec<FiredEvent>,
    /// Per skin: this frame's skinning matrices, and the same as GPU rows.
    matrices: Vec<Vec<Affine>>,
    gpu: Vec<Vec<BoneMatrix>>,
}

/// Where the character's records are: the class-7 record and the proxy holding it (the rig's root).
pub fn find_player(world: &World<'_>) -> Option<(u32, u32)> {
    let s = &world.objects;
    let player = s.records.iter().position(|r| r.class == PLAYER_CLASS && r.in_world)? as u32;
    let root = s.records[player as usize].parent.unwrap_or(player);
    Some((player, root))
}

impl PlayerAnim {
    /// The rig, skins and segment of the player character, if the world has one with the segment. `draw_of`
    /// lists, for every draw the level made, its record and the transform it was drawn with; `spheres` gives each
    /// draw's rest bounding sphere in mesh space.
    pub fn build(world: &World<'_>, draw_of: &[(u32, Affine)], spheres: &[(Vec3, f32)]) -> Option<PlayerAnim> {
        let s = &world.objects;
        let (player, root) = find_player(world)?;
        let world_tf = s.world_transforms(true);
        let mut rig = Rig::from_stream(s, world.data, root);
        let base = s.records[root as usize].transform_parent.map_or(Affine::IDENTITY, |p| world_tf[p as usize]);
        rig.update_world(&base);

        let seg_rec = (root..s.records[root as usize].subtree_end).find(|&k| {
            s.records[k as usize].oid == PLAYER_SEGMENT_OID && s.records[k as usize].class == SEGMENT_CLASS
        })?;
        let segment = Segment::parse(world.data, s.records[seg_rec as usize].offset + 6, &OptionTable::sly1()).ok()?;
        let binding =
            Binding::resolve(&segment, |oid| s.find_nearest(player, oid, false).and_then(|r| rig.node_of_record(r)));

        let mut skins = Vec::new();
        let mut skin_of_node = alloc::collections::BTreeMap::new();
        for (k, n) in rig.nodes.iter().enumerate() {
            let Some(rec) = n.record else { continue };
            let Some(set) = s.records[rec as usize].mesh_set.as_ref().filter(|m| !m.bones.is_empty()) else { continue };
            if let Some(mut skin) = Skin::from_stream(&rig, s, rec, set) {
                skin.capture_bind(&rig.world);
                skin_of_node.insert(k as u32, skins.len() as u32);
                skins.push(skin);
            }
        }

        let mut parts = Vec::new();
        for (d, (record, transform)) in draw_of.iter().enumerate() {
            let Some(node) = rig.node_of_record(*record) else { continue };
            let Some(inv) = affine_inverse(&rig.world[node as usize]) else { continue };
            parts.push(Part {
                draw: d as u32,
                node,
                relative: inv.mul(transform),
                skin: skin_of_node.get(&node).copied(),
                sphere: spheres[d],
            });
        }
        if parts.is_empty() {
            return None;
        }
        let mut fired = Vec::new();
        let mut playback = Playback::start(&segment, 1.0, &mut fired);
        playback.set_repeat(RepeatMode::Loop);
        let gpu = alloc::vec![Vec::new(); skins.len()];
        let matrices = alloc::vec![Vec::new(); skins.len()];
        Some(PlayerAnim { rig, skins, segment, binding, playback, base, parts, fired, matrices, gpu })
    }

    /// Advances the animation by `dt` seconds and poses the rig.
    pub fn advance(&mut self, dt: f32) {
        self.fired.clear();
        self.playback.advance(&self.segment, dt, &mut self.fired);
        apply_segment(&self.segment, &self.binding, self.playback.time, &mut self.rig);
        self.rig.update_world(&self.base);
        for ((skin, m), out) in self.skins.iter().zip(self.matrices.iter_mut()).zip(self.gpu.iter_mut()) {
            skin.matrices(&self.rig.world, m);
            out.clear();
            out.extend(m.iter().map(bone_rows));
        }
    }

    /// Writes the pose into the renderer: `instances[d]` is the instance of draw `d`.
    pub fn apply<B: Backend>(&mut self, r: &mut Renderer<B>, instances: &[InstanceId]) -> Result<(), RenderError> {
        for p in &self.parts {
            let Some(&id) = instances.get(p.draw as usize) else { continue };
            let world = self.rig.world[p.node as usize].mul(&p.relative);
            r.set_transform(id, mat4(&world))?;
            if let Some(k) = p.skin {
                let bones = &self.gpu[k as usize];
                if bones.is_empty() {
                    continue;
                }
                r.set_bones(id, bones)?;
                r.set_bounding_sphere(id, Some(posed_sphere(p.sphere, &self.matrices[k as usize])))?;
            }
        }
        Ok(())
    }
}

/// The three rows of an affine transform (columns + translation), the renderer's bone layout.
pub fn bone_rows(a: &Affine) -> BoneMatrix {
    let c = &a.cols;
    BoneMatrix {
        rows: [
            [c[0][0], c[1][0], c[2][0], a.t[0]],
            [c[0][1], c[1][1], c[2][1], a.t[1]],
            [c[0][2], c[1][2], c[2][2], a.t[2]],
        ],
    }
}

/// A sphere around everything the skinning matrices can make of the rest sphere `(c, r)`: for each matrix `S`,
/// the centre moves to `S c` and the radius grows by how far `S` can stretch it (column lengths).
pub fn posed_sphere((c, r): (Vec3, f32), matrices: &[Affine]) -> (Vec3, f32) {
    let mut reach = 0.0f32;
    for m in matrices {
        let moved = m.transform_point([c.x, c.y, c.z]);
        let d = Vec3::new(moved[0] - c.x, moved[1] - c.y, moved[2] - c.z).length();
        let stretch = m.cols.iter().map(|v| Vec3::new(v[0], v[1], v[2]).length()).fold(1.0f32, f32::max);
        reach = reach.max(d + r * (stretch - 1.0));
    }
    (c, r + reach)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bone_rows_match_the_engine() {
        let a = Affine::new([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]], [10.0, 11.0, 12.0]);
        assert_eq!(bone_rows(&a).rows, engine::anim::BoneMatrix::from_affine(&a).rows);
    }

    #[test]
    fn posed_sphere_covers_moved_bones() {
        let id = Affine::IDENTITY;
        assert_eq!(posed_sphere((Vec3::ZERO, 2.0), &[id]), (Vec3::ZERO, 2.0));
        let mut moved = id;
        moved.t = [3.0, 4.0, 0.0];
        let (c, r) = posed_sphere((Vec3::new(1.0, 0.0, 0.0), 2.0), &[id, moved]);
        assert_eq!(c, Vec3::new(1.0, 0.0, 0.0));
        assert!((r - 7.0).abs() < 1e-5);
        let mut scaled = id;
        scaled.cols[0] = [2.0, 0.0, 0.0];
        assert!(posed_sphere((Vec3::ZERO, 1.0), &[scaled]).1 >= 2.0);
    }
}
