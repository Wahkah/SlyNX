//! Convex volumes of volumes, exits and camera zones (collision.md 5; classes/markers-and-volumes.md).

use formats::collision::ConvexVolume;
use formats::geometry::Affine;

use crate::math::{affine_inverse, dot};

/// Whether a point in the volume's local space is inside (collision.md 5): from node 0, take the negative child when
/// `n · x − d < 0`, else the positive one; when the chosen child is −1 the point is inside if that last test was
/// negative. An empty volume contains nothing.
pub fn contains_local(v: &ConvexVolume, p: [f32; 3]) -> bool {
    let mut node = 0usize;
    // A well-formed tree has no cycles; the bound only keeps corrupt data from looping.
    for _ in 0..=v.nodes.len() {
        let Some(n) = v.nodes.get(node) else { return false };
        let Some(plane) = v.planes.get(n.plane as usize) else { return false };
        let negative = dot(plane.normal, p) - plane.d < 0.0;
        let next = if negative { n.negative } else { n.positive };
        if next < 0 {
            return negative;
        }
        node = next as usize;
    }
    false
}

/// A convex volume placed in the world: the volume and the inverse of its owner's world transform.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedVolume {
    /// The volume in its owner's local space.
    pub volume: ConvexVolume,
    /// World space to the owner's local space.
    pub to_local: Affine,
}

impl PlacedVolume {
    /// Places `volume` with its owner's world transform.
    pub fn new(volume: ConvexVolume, world: &Affine) -> PlacedVolume {
        PlacedVolume { volume, to_local: affine_inverse(world).unwrap_or(Affine::IDENTITY) }
    }

    /// Updates the owner's world transform.
    pub fn set_world(&mut self, world: &Affine) {
        self.to_local = affine_inverse(world).unwrap_or(Affine::IDENTITY);
    }

    /// Whether a world-space point is inside.
    pub fn contains(&self, p: [f32; 3]) -> bool {
        contains_local(&self.volume, self.to_local.transform_point(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::collision::{Plane, VolumeNode};

    /// The box |x|, |y|, |z| < 1 as a chain of six planes: each plane's negative side leads to the next test.
    pub(crate) fn unit_box() -> ConvexVolume {
        let normals =
            [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]];
        let planes = normals.iter().map(|&n| Plane { normal: n, d: 1.0 }).collect();
        let nodes = (0..6)
            .map(|i| VolumeNode { plane: i as u16, negative: if i < 5 { i as i16 + 1 } else { -1 }, positive: -1 })
            .collect();
        ConvexVolume { planes, nodes }
    }

    #[test]
    fn inside_and_outside() {
        let b = unit_box();
        assert!(contains_local(&b, [0.0, 0.0, 0.0]));
        assert!(contains_local(&b, [0.9, -0.9, 0.5]));
        assert!(!contains_local(&b, [1.5, 0.0, 0.0]));
        assert!(!contains_local(&b, [0.0, 0.0, -3.0]));
        assert!(!contains_local(&ConvexVolume::default(), [0.0; 3]));
        let placed = PlacedVolume::new(b, &Affine { cols: crate::math::MAT3_IDENTITY, t: [10.0, 0.0, 0.0] });
        assert!(placed.contains([10.5, 0.0, 0.0]));
        assert!(!placed.contains([0.0, 0.0, 0.0]));
    }
}
