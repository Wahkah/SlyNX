//! Character-style movement on top of the collision sweeps: collide-and-slide, the ground rule (normal z above
//! 0.7, collision.md 7), and optional step-up and ground snapping.
//!
//! The specs give the ground rule but no step height, snap distance or slope speed rules yet; those are parameters
//! here with no defaults (`None` turns them off), to be filled from the player-moves spec when it gives them.

use alloc::vec::Vec;

use crate::collision::mesh::GROUND_Z;
use crate::collision::world::{CollisionWorld, QueryFilter, SweepHit};
use crate::math::{Vec3, add, dot, length, madd, scale};

/// The moving shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CharacterShape {
    /// A sphere centred on the position.
    Sphere {
        /// Radius.
        radius: f32,
    },
    /// A vertical capsule: the position is the centre of its bottom sphere; the top sphere is `height` above.
    Capsule {
        /// Distance between the two sphere centres.
        height: f32,
        /// Radius.
        radius: f32,
    },
}

impl CharacterShape {
    fn sweep(&self, cw: &CollisionWorld, f: &QueryFilter, p: Vec3, d: Vec3) -> Option<SweepHit> {
        match *self {
            CharacterShape::Sphere { radius } => cw.sphere_sweep(p, radius, d, f),
            CharacterShape::Capsule { height, radius } => cw.capsule_sweep(p, add(p, [0.0, 0.0, height]), radius, d, f),
        }
    }
}

/// Movement settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlideParams {
    /// Most sweeps per move.
    pub max_iterations: u32,
    /// Distance kept from surfaces after a hit.
    pub skin: f32,
    /// Normal z above which a surface is ground (0.7).
    pub ground_z: f32,
    /// Highest step climbed when blocked by a wall-like surface (`None`: no stepping).
    pub step_up: Option<f32>,
    /// Farthest the shape is pulled down to stay on the ground after a move (`None`: no snapping).
    pub snap_down: Option<f32>,
}

impl Default for SlideParams {
    fn default() -> Self {
        SlideParams { max_iterations: 4, skin: 0.05, ground_z: GROUND_Z, step_up: None, snap_down: None }
    }
}

/// What a move ended on or ran into.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlideResult {
    /// Final position.
    pub position: Vec3,
    /// Every surface hit, in order.
    pub hits: Vec<SweepHit>,
    /// The ground under the shape at the end, if any (a ground-like surface within the skin distance).
    pub ground: Option<SweepHit>,
    /// Whether the move climbed a step.
    pub stepped: bool,
}

/// Moves `shape` from `position` by `delta`, sliding along what it hits.
pub fn move_and_slide(
    cw: &CollisionWorld,
    filter: &QueryFilter,
    shape: CharacterShape,
    position: Vec3,
    delta: Vec3,
    params: &SlideParams,
) -> SlideResult {
    let mut out = SlideResult { position, ..Default::default() };
    let mut rest = delta;
    for _ in 0..params.max_iterations {
        let len = length(rest);
        if len <= 1e-6 {
            break;
        }
        let Some(hit) = shape.sweep(cw, filter, out.position, rest) else {
            out.position = add(out.position, rest);
            break;
        };
        // Move up to the contact, keeping the skin distance along the motion.
        let travel = (hit.fraction * len - params.skin).max(0.0);
        out.position = madd(out.position, rest, travel / len);
        out.hits.push(hit);
        let remaining = scale(rest, 1.0 - travel / len);
        if hit.normal[2] <= params.ground_z
            && let Some(h) = params.step_up
            && let Some(p) = try_step(cw, filter, shape, out.position, remaining, h, params)
        {
            out.position = p;
            out.stepped = true;
            break;
        }
        // Slide: remove the part of the remaining motion that goes into the surface.
        let into = dot(remaining, hit.normal);
        rest = if into < 0.0 { madd(remaining, hit.normal, -into) } else { remaining };
    }
    let probe = params.skin * 2.0;
    out.ground = ground_below(cw, filter, shape, out.position, probe, params.ground_z);
    if out.ground.is_none()
        && let Some(d) = params.snap_down
        && let Some(g) = ground_below(cw, filter, shape, out.position, d, params.ground_z)
    {
        out.position = madd(out.position, [0.0, 0.0, -1.0], (g.fraction * d - params.skin).max(0.0));
        out.ground = Some(g);
    }
    out
}

/// The ground-like surface within `distance` below the shape.
pub fn ground_below(
    cw: &CollisionWorld,
    filter: &QueryFilter,
    shape: CharacterShape,
    p: Vec3,
    distance: f32,
    ground_z: f32,
) -> Option<SweepHit> {
    shape.sweep(cw, filter, p, [0.0, 0.0, -distance]).filter(|h| h.normal[2] > ground_z)
}

/// Up by `h`, across by `motion`, down by `h + skin`: accepted when it lands on ground.
fn try_step(
    cw: &CollisionWorld,
    filter: &QueryFilter,
    shape: CharacterShape,
    p: Vec3,
    motion: Vec3,
    h: f32,
    params: &SlideParams,
) -> Option<Vec3> {
    let horizontal = [motion[0], motion[1], 0.0];
    let hl = length(horizontal);
    if hl <= 1e-6 {
        return None;
    }
    let up = shape.sweep(cw, filter, p, [0.0, 0.0, h]).map_or(h, |hit| (hit.fraction * h - params.skin).max(0.0));
    let raised = add(p, [0.0, 0.0, up]);
    if let Some(hit) = shape.sweep(cw, filter, raised, horizontal)
        && hit.fraction * hl < params.skin * 2.0
    {
        return None;
    }
    let across = match shape.sweep(cw, filter, raised, horizontal) {
        Some(hit) => madd(raised, horizontal, (hit.fraction * hl - params.skin).max(0.0) / hl),
        None => add(raised, horizontal),
    };
    let drop = up + params.skin;
    let land = ground_below(cw, filter, shape, across, drop, params.ground_z)?;
    Some(madd(across, [0.0, 0.0, -1.0], (land.fraction * drop - params.skin).max(0.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::TriMesh;
    use crate::collision::mesh::tests::step_mesh;
    use crate::collision::world::Shape;
    use formats::geometry::Affine;

    fn world() -> CollisionWorld {
        let (m, t) = step_mesh();
        let mut cw = CollisionWorld::new();
        let s = cw.add_shape(Shape { main: TriMesh::new(&m, &t), camera: None });
        cw.add_body(s, Affine::IDENTITY, 0, 0, true);
        cw.rebuild();
        cw
    }

    #[test]
    fn slides_along_the_wall() {
        let cw = world();
        let f = QueryFilter::default();
        let shape = CharacterShape::Sphere { radius: 0.5 };
        // Moving diagonally into the wall at x = 5 keeps the y motion.
        let r = move_and_slide(&cw, &f, shape, [3.0, 2.0, 0.55], [4.0, 4.0, 0.0], &SlideParams::default());
        assert!((r.position[0] - 4.45).abs() < 0.06, "{:?}", r.position);
        assert!(r.position[1] > 5.0);
        assert!(r.ground.is_some());
        assert!(!r.stepped);
    }

    #[test]
    fn steps_up_when_allowed_and_snaps_down() {
        let cw = world();
        let f = QueryFilter::default();
        let shape = CharacterShape::Sphere { radius: 0.5 };
        let params = SlideParams { step_up: Some(2.5), ..SlideParams::default() };
        let r = move_and_slide(&cw, &f, shape, [3.0, 5.0, 0.55], [4.0, 0.0, 0.0], &params);
        assert!(r.stepped, "{r:?}");
        assert!((r.position[2] - 2.55).abs() < 0.1 && r.position[0] > 5.0, "{:?}", r.position);
        // A step too high for the setting is not climbed.
        let low = SlideParams { step_up: Some(1.0), ..SlideParams::default() };
        let r = move_and_slide(&cw, &f, shape, [3.0, 5.0, 0.55], [4.0, 0.0, 0.0], &low);
        assert!(!r.stepped && r.position[0] < 4.5);
        // Walking off the top edge: snapping pulls the shape down to the floor when it is within reach.
        let snap = SlideParams { snap_down: Some(3.0), ..SlideParams::default() };
        let r = move_and_slide(&cw, &f, shape, [6.0, 5.0, 2.55], [-3.0, 0.0, 0.0], &snap);
        assert!((r.position[2] - 0.55).abs() < 0.1 && r.ground.is_some(), "{:?}", r.position);
        let r = move_and_slide(&cw, &f, shape, [6.0, 5.0, 2.55], [-3.0, 0.0, 0.0], &SlideParams::default());
        assert!(r.ground.is_none());
        // A capsule slides too.
        let cap = CharacterShape::Capsule { height: 0.8, radius: 0.5 };
        let r = move_and_slide(&cw, &f, cap, [3.0, 2.0, 0.55], [4.0, 0.0, 0.0], &SlideParams::default());
        assert!(r.position[0] < 4.5 && r.position[0] > 4.3, "{:?}", r.position);
    }
}
