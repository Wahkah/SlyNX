//! Convex volumes: the inside test of collision.md 5, and volumes placed in a world.

use alloc::vec::Vec;

use formats::collision::ConvexVolume;
use formats::geometry::Affine;
use formats::world::World;

use crate::math::{Vec3, affine_inverse, dot};

/// Whether `p` (in the volume's local space) is inside: start at node 0; go to the negative child when
/// `n · p − d < 0`, else to the positive child; when the chosen child is −1 the point is inside if that last test
/// was negative (collision.md 5). A volume without nodes contains nothing.
pub fn point_in_convex(volume: &ConvexVolume, p: Vec3) -> bool {
    let mut node = 0usize;
    // A well-formed tree reaches a leaf in at most one step per node; the bound guards against cycles.
    for _ in 0..=volume.nodes.len() {
        let Some(n) = volume.nodes.get(node) else { return false };
        let plane = &volume.planes[n.plane as usize];
        let negative = dot(plane.normal, p) - plane.d < 0.0;
        let next = if negative { n.negative } else { n.positive };
        if next < 0 {
            return negative;
        }
        node = next as usize;
    }
    false
}

/// A convex volume placed in the world.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedVolume {
    /// The object-stream record that owns it (volume, exit, camera zone or flash volume).
    pub record: u32,
    /// The volume, in the owner's local space.
    pub volume: ConvexVolume,
    /// Local to world.
    pub transform: Affine,
    inverse: Affine,
}

impl PlacedVolume {
    /// Places a volume.
    pub fn new(record: u32, volume: ConvexVolume, transform: Affine) -> PlacedVolume {
        let inverse = affine_inverse(&transform).unwrap_or(Affine::IDENTITY);
        PlacedVolume { record, volume, transform, inverse }
    }

    /// Moves the volume.
    pub fn set_transform(&mut self, transform: Affine) {
        self.transform = transform;
        self.inverse = affine_inverse(&transform).unwrap_or(Affine::IDENTITY);
    }

    /// Whether a world-space point is inside.
    pub fn contains(&self, p: Vec3) -> bool {
        point_in_convex(&self.volume, self.inverse.transform_point(p))
    }
}

/// Every convex volume of the world hierarchy (outside stock templates, placed), at its owner's world transform.
pub fn world_volumes(world: &World<'_>) -> formats::Result<Vec<PlacedVolume>> {
    let s = &world.objects;
    let wt = s.world_transforms(true);
    let placed = s.placed_flags(&wt);
    let mut out = Vec::new();
    for v in &s.volumes {
        let i = v.record as usize;
        let r = &s.records[i];
        if !r.in_world || r.template || !placed[i] {
            continue;
        }
        out.push(PlacedVolume::new(v.record, ConvexVolume::parse(world.data, v.offset)?, wt[i]));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use formats::collision::{Plane, VolumeNode};

    /// The unit cube [0, 1]³ as six planes with outward normals in a chain of negative children.
    fn cube() -> ConvexVolume {
        let planes = vec![
            Plane { normal: [1.0, 0.0, 0.0], d: 1.0 },
            Plane { normal: [-1.0, 0.0, 0.0], d: 0.0 },
            Plane { normal: [0.0, 1.0, 0.0], d: 1.0 },
            Plane { normal: [0.0, -1.0, 0.0], d: 0.0 },
            Plane { normal: [0.0, 0.0, 1.0], d: 1.0 },
            Plane { normal: [0.0, 0.0, -1.0], d: 0.0 },
        ];
        let nodes = (0..6)
            .map(|i| VolumeNode { plane: i as u16, negative: if i < 5 { i + 1 } else { -1 }, positive: -1 })
            .collect();
        ConvexVolume { planes, nodes }
    }

    #[test]
    fn inside_test() {
        let c = cube();
        assert!(point_in_convex(&c, [0.5, 0.5, 0.5]));
        assert!(!point_in_convex(&c, [1.5, 0.5, 0.5]));
        assert!(!point_in_convex(&c, [0.5, 0.5, -0.1]));
        assert!(!point_in_convex(&ConvexVolume::default(), [0.0; 3]));
        let v = PlacedVolume::new(0, c, Affine { cols: crate::math::MAT3_IDENTITY, t: [10.0, 0.0, 0.0] });
        assert!(v.contains([10.5, 0.5, 0.5]));
        assert!(!v.contains([0.5, 0.5, 0.5]));
    }
}
