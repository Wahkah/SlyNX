//! The player's input and the world features the context moves use.

use alloc::vec::Vec;

use crate::collision::world::{CollisionWorld, QueryFilter};
use crate::collision::{Ledge, TriMesh};
use crate::math::{Vec3, add, cos, length, mat_mul_vec, normalize, scale, sin, sqrt};

/// One button: held this frame, and pressed this frame (went down).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Button {
    /// Held.
    pub held: bool,
    /// Went down this frame.
    pub pressed: bool,
}

impl Button {
    /// A button pressed this frame (and held).
    pub const PRESSED: Button = Button { held: true, pressed: true };
    /// A button held since an earlier frame.
    pub const HELD: Button = Button { held: true, pressed: false };
}

/// The controller as the player reads it (the object model maps the pad to it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerInput {
    /// Left stick, x right and y up, each in [−1, 1], before the dead zone.
    pub stick: [f32; 2],
    /// Camera heading (radians about +z, 0 = looking along +x): the stick is camera-relative.
    pub camera_yaw: f32,
    /// Jump.
    pub jump: Button,
    /// Attack (sweep; in the air, smash).
    pub attack: Button,
    /// The context button: grab pipes, land on perch points, rails and ropes, catch hooks, flatten, hide.
    pub action: Button,
    /// Duck (and drop from ledges and pipes).
    pub duck: Button,
    /// Rush attack.
    pub rush: Button,
    /// Use the selected power-up.
    pub power: Button,
}

impl PlayerInput {
    /// The stick as a world-space horizontal direction scaled by its deflection (before the dead zone).
    pub fn world_stick(&self) -> Vec3 {
        let (s, c) = (sin(self.camera_yaw), cos(self.camera_yaw));
        let forward = [c, s, 0.0];
        let right = [s, -c, 0.0];
        add(scale(forward, self.stick[1]), scale(right, self.stick[0]))
    }
}

/// A world feature the context moves attach to. The object model builds these from the world's objects (curves,
/// points, hiding places, water) and passes them in every frame.
#[derive(Clone, Debug, PartialEq)]
pub enum FeatureKind {
    /// A vertical (or slanted) pipe from `bottom` to `top`.
    Pipe {
        /// Bottom end.
        bottom: Vec3,
        /// Top end.
        top: Vec3,
    },
    /// A rail walked along (polyline).
    Rail {
        /// Points.
        points: Vec<Vec3>,
    },
    /// A rope walked along like a rail (polyline).
    Rope {
        /// Points.
        points: Vec<Vec3>,
    },
    /// A perch point: a spot the player lands on and stands on.
    Perch {
        /// Position.
        position: Vec3,
    },
    /// A swing hook.
    Hook {
        /// Position.
        position: Vec3,
    },
    /// A hiding place (barrel or basket).
    HidingPlace {
        /// Centre at floor level.
        position: Vec3,
    },
    /// Water: below `surface` inside `min .. max` (x, y).
    Water {
        /// Surface height.
        surface: f32,
        /// Horizontal extent, minimum corner (x, y).
        min: [f32; 2],
        /// Horizontal extent, maximum corner (x, y).
        max: [f32; 2],
    },
}

/// A feature with the caller's id (reported back in events).
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    /// Caller's id.
    pub id: u32,
    /// What it is.
    pub kind: FeatureKind,
}

/// What the player moves in this frame.
#[derive(Clone, Copy)]
pub struct PlayerWorld<'a> {
    /// The collision world (the player's own solids disabled or filtered out).
    pub cw: &'a CollisionWorld,
    /// Filter for the player's queries.
    pub filter: QueryFilter,
    /// Context features.
    pub features: &'a [Feature],
    /// Ledges in world space ([`world_ledges`]).
    pub ledges: &'a [Ledge],
    /// Whether the player stands in shadow (sneaking caps the stick at 0.5).
    pub in_shadow: bool,
    /// Whether the player is in cover (ducking there hides from guards in front).
    pub in_cover: bool,
}

/// Every ledge of every enabled body, in world space.
pub fn world_ledges(cw: &CollisionWorld) -> Vec<Ledge> {
    let mut out = Vec::new();
    for b in cw.bodies().iter().filter(|b| b.enabled) {
        let mesh: &TriMesh = &cw.shapes[b.shape.0 as usize].main;
        for l in mesh.ledges() {
            out.push(Ledge {
                ends: [b.transform.transform_point(l.ends[0]), b.transform.transform_point(l.ends[1])],
                outward: normalize(mat_mul_vec(&b.transform.cols, l.outward)),
                ..l
            });
        }
    }
    out
}

/// The point of segment `a b` closest to `p`, and its parameter in [0, 1].
pub fn closest_on_segment(p: Vec3, a: Vec3, b: Vec3) -> (Vec3, f32) {
    let d = crate::math::sub(b, a);
    let l2 = crate::math::dot(d, d);
    let t = if l2 > 0.0 { (crate::math::dot(crate::math::sub(p, a), d) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (crate::math::madd(a, d, t), t)
}

/// Horizontal distance.
pub fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
    let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
    sqrt(dx * dx + dy * dy)
}

/// Length of a polyline.
pub fn polyline_length(points: &[Vec3]) -> f32 {
    points.windows(2).map(|w| length(crate::math::sub(w[1], w[0]))).sum()
}
