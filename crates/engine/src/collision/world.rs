//! A collision world: shapes (run-time collision meshes) placed as bodies, a hierarchy over the bodies, and the
//! world-space queries.
//!
//! Body transforms are rigid (transforms.md 5: object transforms are orthonormal), so queries move the query into
//! a body's local space with the inverse transform and distances carry over unchanged.

use alloc::vec::Vec;
use core::cell::Cell;

use formats::collision::{Solid, material_of};
use formats::geometry::Affine;
use formats::options::OptionTable;
use formats::stream::{Layout, PlacementOptions, layout_of};
use formats::world::World;

use super::bvh::Bvh;
use super::geom::{Aabb, inverse_dir};
use super::mesh::{GROUND_Z, MeshContact, TriMesh};
use crate::math::{Vec3, add, affine_inverse, madd, mat_mul_vec, max3, min3, normalize, scale, sub};

/// Identifies a shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShapeId(pub u32);

/// Identifies a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BodyId(pub u32);

/// The collision data of one solid: its collision mesh and, when it has one, its camera mesh (collision.md 3).
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// The collision mesh.
    pub main: TriMesh,
    /// The camera collision mesh, when the solid has a non-empty one.
    pub camera: Option<TriMesh>,
}

impl Shape {
    /// From a parsed solid. Each stored surface is a convex polygon, not only its three corners (specs/sly1
    /// collision.md 1.2): the meshes are covered with each surface's polygon, the convex hull in its plane of its
    /// corners and the ends of every edge naming it (`CollisionMesh::with_polygons`).
    pub fn from_solid(solid: &Solid) -> Shape {
        let polygons = |m: &formats::collision::CollisionMesh| m.with_polygons();
        Shape {
            main: TriMesh::new(&polygons(&solid.mesh), &solid.tree),
            camera: (!solid.camera_mesh.is_empty())
                .then(|| TriMesh::new(&polygons(&solid.camera_mesh), &solid.camera_tree)),
        }
    }

    /// The mesh a query uses.
    pub fn mesh(&self, camera: bool) -> &TriMesh {
        if camera { self.camera.as_ref().unwrap_or(&self.main) } else { &self.main }
    }
}

/// A placed shape.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    /// Its shape.
    pub shape: ShapeId,
    /// Local to world.
    pub transform: Affine,
    inverse: Affine,
    /// Material (option 128 of the solid; 0 by default).
    pub material: i16,
    /// Caller data (for bodies made by [`CollisionWorld::from_world`], the object-stream record).
    pub user: u32,
    /// Whether the solid is fixed (never moves).
    pub fixed: bool,
    /// Disabled bodies are ignored by queries.
    pub enabled: bool,
    bounds: Aabb,
}

impl Body {
    /// World-space bounds.
    pub fn bounds(&self) -> Aabb {
        self.bounds
    }
}

/// What a query looks at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueryFilter {
    /// Ray casts and sweeps also report surfaces met from behind. Overlaps ignore it: contact is "inside the
    /// solid" by the collision tree, from either side (collision.md 7).
    pub two_sided: bool,
    /// Use camera meshes where solids have them.
    pub camera: bool,
    /// Leave this body out (for example the querying character's own body).
    pub ignore: Option<BodyId>,
}

/// A ray hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHit {
    /// Distance from the origin.
    pub distance: f32,
    /// Hit point.
    pub point: Vec3,
    /// Surface normal (world space).
    pub normal: Vec3,
    /// Body.
    pub body: BodyId,
    /// Triangle of the body's mesh.
    pub triangle: u32,
    /// The body's material.
    pub material: i16,
    /// Whether the front (open) side was hit.
    pub front: bool,
}

/// A sweep hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepHit {
    /// Fraction of the motion before contact, in `[0, 1]`.
    pub fraction: f32,
    /// Contact point on the surface.
    pub point: Vec3,
    /// Contact normal, from the surface toward the moving shape.
    pub normal: Vec3,
    /// Body.
    pub body: BodyId,
    /// Triangle.
    pub triangle: u32,
    /// The body's material.
    pub material: i16,
}

/// A ground probe result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundHit {
    /// Drop from the probe point to the ground.
    pub distance: f32,
    /// Ground point.
    pub point: Vec3,
    /// Surface normal.
    pub normal: Vec3,
    /// Material.
    pub material: i16,
    /// Body.
    pub body: BodyId,
    /// Whether the surface counts as ground (normal z above [`GROUND_Z`], collision.md 7).
    pub is_ground: bool,
}

/// One overlap found by [`CollisionWorld::sphere_overlaps`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Overlap {
    /// Body.
    pub body: BodyId,
    /// Triangle.
    pub triangle: u32,
    /// Push-out direction (world).
    pub normal: Vec3,
    /// Penetration depth.
    pub depth: f32,
    /// Closest point on the surface.
    pub point: Vec3,
}

/// Shapes, bodies and the hierarchy over the bodies.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollisionWorld {
    /// Shapes.
    pub shapes: Vec<Shape>,
    bodies: Vec<Body>,
    boxes: Vec<Aabb>,
    top: Bvh,
}

fn body_bounds(shape: &Shape, t: &Affine) -> Aabb {
    let b = shape.main.bounds;
    let b = match &shape.camera {
        Some(c) => b.union(c.bounds),
        None => b,
    };
    b.transformed(t)
}

impl CollisionWorld {
    /// An empty world.
    pub fn new() -> CollisionWorld {
        CollisionWorld::default()
    }

    /// Adds a shape.
    pub fn add_shape(&mut self, shape: Shape) -> ShapeId {
        self.shapes.push(shape);
        ShapeId(self.shapes.len() as u32 - 1)
    }

    /// Places a shape. Call [`Self::rebuild`] after adding bodies (queries before that do not see them).
    pub fn add_body(&mut self, shape: ShapeId, transform: Affine, material: i16, user: u32, fixed: bool) -> BodyId {
        let inverse = affine_inverse(&transform).unwrap_or(Affine::IDENTITY);
        let bounds = body_bounds(&self.shapes[shape.0 as usize], &transform);
        self.bodies.push(Body { shape, transform, inverse, material, user, fixed, enabled: true, bounds });
        self.boxes.push(bounds);
        BodyId(self.bodies.len() as u32 - 1)
    }

    /// Rebuilds the hierarchy over the bodies.
    pub fn rebuild(&mut self) {
        self.top = Bvh::build(&self.boxes);
    }

    /// Moves a body and refits the hierarchy.
    pub fn set_transform(&mut self, body: BodyId, transform: Affine) {
        self.set_transform_deferred(body, transform);
        self.refit();
    }

    /// Moves a body without refitting; call [`Self::refit`] once after moving several.
    pub fn set_transform_deferred(&mut self, body: BodyId, transform: Affine) {
        let b = &mut self.bodies[body.0 as usize];
        b.transform = transform;
        b.inverse = affine_inverse(&transform).unwrap_or(Affine::IDENTITY);
        b.bounds = body_bounds(&self.shapes[b.shape.0 as usize], &transform);
        self.boxes[body.0 as usize] = b.bounds;
    }

    /// Refits the hierarchy to the bodies' current bounds.
    pub fn refit(&mut self) {
        self.top.refit(&self.boxes);
    }

    /// Enables or disables a body.
    pub fn set_enabled(&mut self, body: BodyId, enabled: bool) {
        self.bodies[body.0 as usize].enabled = enabled;
    }

    /// A body.
    pub fn body(&self, id: BodyId) -> &Body {
        &self.bodies[id.0 as usize]
    }

    /// Every body.
    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }

    fn usable(&self, i: u32, filter: &QueryFilter) -> Option<&Body> {
        let b = &self.bodies[i as usize];
        (b.enabled && filter.ignore != Some(BodyId(i))).then_some(b)
    }

    /// The nearest hit of a ray from `origin` along `dir` (any length; normalised here) within `max_distance`.
    pub fn ray_cast(&self, origin: Vec3, dir: Vec3, max_distance: f32, filter: &QueryFilter) -> Option<RayHit> {
        let dir = normalize(dir);
        let inv = inverse_dir(dir);
        let limit = Cell::new(max_distance);
        let mut best: Option<RayHit> = None;
        self.top.walk(
            |b| b.ray_enter(origin, inv, limit.get()).is_some(),
            |items| {
                for &i in items {
                    let Some(body) = self.usable(i, filter) else { continue };
                    if body.bounds.ray_enter(origin, inv, limit.get()).is_none() {
                        continue;
                    }
                    let mesh = self.shapes[body.shape.0 as usize].mesh(filter.camera);
                    let lo = body.inverse.transform_point(origin);
                    let ld = mat_mul_vec(&body.inverse.cols, dir);
                    if let Some(h) = mesh.ray_cast(lo, ld, limit.get(), filter.two_sided) {
                        limit.set(h.t);
                        let n = mat_mul_vec(&body.transform.cols, mesh.normal(h.triangle));
                        best = Some(RayHit {
                            distance: h.t,
                            point: madd(origin, dir, h.t),
                            normal: if h.front { n } else { scale(n, -1.0) },
                            body: BodyId(i),
                            triangle: h.triangle,
                            material: body.material,
                            front: h.front,
                        });
                    }
                }
                true
            },
        );
        best
    }

    /// Whether anything blocks the segment `from .. to` (line of sight).
    pub fn segment_blocked(&self, from: Vec3, to: Vec3, filter: &QueryFilter) -> bool {
        let d = sub(to, from);
        let len = crate::math::length(d);
        if len == 0.0 {
            return false;
        }
        let dir = scale(d, 1.0 / len);
        let inv = inverse_dir(dir);
        let mut hit = false;
        self.top.walk(
            |b| b.ray_enter(from, inv, len).is_some(),
            |items| {
                for &i in items {
                    let Some(body) = self.usable(i, filter) else { continue };
                    let mesh = self.shapes[body.shape.0 as usize].mesh(filter.camera);
                    let lo = body.inverse.transform_point(from);
                    let ld = mat_mul_vec(&body.inverse.cols, dir);
                    if mesh.ray_blocked(lo, ld, len, filter.two_sided) {
                        hit = true;
                        return false;
                    }
                }
                true
            },
        );
        hit
    }

    fn sweep(
        &self,
        start: Aabb,
        delta: Vec3,
        filter: &QueryFilter,
        test: impl Fn(&TriMesh, &Affine) -> Option<MeshContact>,
    ) -> Option<SweepHit> {
        let swept = Aabb { min: min3(start.min, add(start.min, delta)), max: max3(start.max, add(start.max, delta)) };
        let mut best: Option<SweepHit> = None;
        self.top.walk(
            |b| b.overlaps(&swept),
            |items| {
                for &i in items {
                    let Some(body) = self.usable(i, filter) else { continue };
                    if !body.bounds.overlaps(&swept) {
                        continue;
                    }
                    let mesh = self.shapes[body.shape.0 as usize].mesh(filter.camera);
                    if let Some(c) = test(mesh, &body.inverse)
                        && best.is_none_or(|b| c.contact.t < b.fraction)
                    {
                        best = Some(SweepHit {
                            fraction: c.contact.t,
                            point: body.transform.transform_point(c.contact.point),
                            normal: mat_mul_vec(&body.transform.cols, c.contact.normal),
                            body: BodyId(i),
                            triangle: c.triangle,
                            material: body.material,
                        });
                    }
                }
                true
            },
        );
        best
    }

    /// The first contact of a sphere moving by `delta`.
    pub fn sphere_sweep(&self, center: Vec3, radius: f32, delta: Vec3, filter: &QueryFilter) -> Option<SweepHit> {
        let start = Aabb { min: center, max: center }.grow(radius);
        self.sweep(start, delta, filter, |mesh, inv| {
            let c = inv.transform_point(center);
            let d = mat_mul_vec(&inv.cols, delta);
            mesh.sphere_sweep(c, radius, d, filter.two_sided)
        })
    }

    /// The first contact of a capsule (segment `a b`, radius `radius`) moving by `delta`.
    pub fn capsule_sweep(&self, a: Vec3, b: Vec3, radius: f32, delta: Vec3, filter: &QueryFilter) -> Option<SweepHit> {
        let start = Aabb { min: min3(a, b), max: max3(a, b) }.grow(radius);
        self.sweep(start, delta, filter, |mesh, inv| {
            let la = inv.transform_point(a);
            let lb = inv.transform_point(b);
            let d = mat_mul_vec(&inv.cols, delta);
            mesh.capsule_sweep(la, lb, radius, d, filter.two_sided)
        })
    }

    /// Every surface a sphere overlaps, appended to `out`.
    pub fn sphere_overlaps(&self, center: Vec3, radius: f32, filter: &QueryFilter, out: &mut Vec<Overlap>) {
        let q = Aabb { min: center, max: center }.grow(radius);
        let mut local = Vec::new();
        self.top.walk(
            |b| b.overlaps(&q),
            |items| {
                for &i in items {
                    let Some(body) = self.usable(i, filter) else { continue };
                    if !body.bounds.overlaps(&q) {
                        continue;
                    }
                    let mesh = self.shapes[body.shape.0 as usize].mesh(filter.camera);
                    local.clear();
                    mesh.sphere_overlaps(body.inverse.transform_point(center), radius, &mut local);
                    out.extend(local.iter().map(|&(t, n, depth, p)| Overlap {
                        body: BodyId(i),
                        triangle: t,
                        normal: mat_mul_vec(&body.transform.cols, n),
                        depth,
                        point: body.transform.transform_point(p),
                    }));
                }
                true
            },
        );
    }

    /// The first body whose solid (collision tree) contains `p`. Only bodies whose bounds contain `p` are tested:
    /// a tree's inside can reach past its surfaces (below a floor, for example), but contact only matters near them.
    pub fn point_inside(&self, p: Vec3, filter: &QueryFilter) -> Option<BodyId> {
        let q = Aabb { min: p, max: p };
        let mut found = None;
        self.top.walk(
            |b| b.overlaps(&q),
            |items| {
                for &i in items {
                    let Some(body) = self.usable(i, filter) else { continue };
                    if !body.bounds.overlaps(&q) {
                        continue;
                    }
                    let mesh = self.shapes[body.shape.0 as usize].mesh(filter.camera);
                    if mesh.point_inside(body.inverse.transform_point(p)) {
                        found = Some(BodyId(i));
                        return false;
                    }
                }
                true
            },
        );
        found
    }

    /// The surface below `point` within `max_drop` (a ray straight down, −Z), with its normal and material.
    pub fn ground_probe(&self, point: Vec3, max_drop: f32, filter: &QueryFilter) -> Option<GroundHit> {
        let h = self.ray_cast(point, [0.0, 0.0, -1.0], max_drop, filter)?;
        Some(GroundHit {
            distance: h.distance,
            point: h.point,
            normal: h.normal,
            material: h.material,
            body: h.body,
            is_ground: h.normal[2] > GROUND_Z,
        })
    }

    /// The ground below `point` within `max_drop` by the collision trees' one-sided rule ([`TriMesh::tree_floor`]):
    /// the nearest floor of any body whose bounds the drop crosses, with the deciding surface's normal. For floors
    /// whose stored surfaces do not cover the point (an outline stored incompletely, Sly 3's v_apt surface 401).
    pub fn tree_ground_probe(&self, point: Vec3, max_drop: f32, filter: &QueryFilter) -> Option<GroundHit> {
        let end = add(point, [0.0, 0.0, -max_drop]);
        let q = Aabb { min: min3(point, end), max: max3(point, end) };
        let mut best: Option<GroundHit> = None;
        self.top.walk(
            |b| b.overlaps(&q),
            |items| {
                for &i in items {
                    let Some(body) = self.usable(i, filter) else { continue };
                    if !body.bounds.overlaps(&q) {
                        continue;
                    }
                    let mesh = self.shapes[body.shape.0 as usize].mesh(filter.camera);
                    let lp = body.inverse.transform_point(point);
                    let ld = normalize(mat_mul_vec(&body.inverse.cols, [0.0, 0.0, -1.0]));
                    let scale_z = crate::math::length(mat_mul_vec(&body.inverse.cols, [0.0, 0.0, max_drop]));
                    let Some((d_local, tri)) = mesh.tree_floor(lp, ld, scale_z) else { continue };
                    let distance = d_local * max_drop / scale_z.max(1e-6);
                    if best.as_ref().is_none_or(|b| distance < b.distance) {
                        let mut n = mat_mul_vec(&body.transform.cols, mesh.normal(tri));
                        if n[2] < 0.0 {
                            n = scale(n, -1.0);
                        }
                        best = Some(GroundHit {
                            distance,
                            point: madd(point, [0.0, 0.0, -1.0], distance),
                            normal: n,
                            material: body.material,
                            body: BodyId(i),
                            is_ground: n[2] > GROUND_Z,
                        });
                    }
                }
                true
            },
        );
        best
    }

    /// The ground below a sphere: a sphere sweep straight down by `max_drop`, which also finds ledges and slopes
    /// the centre ray would miss.
    pub fn ground_probe_sphere(
        &self,
        center: Vec3,
        radius: f32,
        max_drop: f32,
        filter: &QueryFilter,
    ) -> Option<GroundHit> {
        let h = self.sphere_sweep(center, radius, [0.0, 0.0, -max_drop], filter)?;
        Some(GroundHit {
            distance: h.fraction * max_drop,
            point: h.point,
            normal: h.normal,
            material: h.material,
            body: h.body,
            is_ground: h.normal[2] > GROUND_Z,
        })
    }

    /// Builds the collision world of a parsed world: one body per placed solid in the world hierarchy (classes with
    /// the solid or conveyor layout, outside stock templates), proxy clones included, at its world transform, with
    /// its material (option 128). Solids with an empty collision mesh and no camera mesh are left out. Each body's
    /// `user` is its object-stream record; clones of one source share one shape.
    pub fn from_world(world: &World<'_>, table: &OptionTable) -> formats::Result<CollisionWorld> {
        let records = &world.objects.records;
        let is_solid =
            |r: &formats::stream::Record| matches!(layout_of(r.class), Some(Layout::Solid | Layout::Conveyor));
        let placements = world.objects.placements_where(PlacementOptions::default(), &is_solid);
        let mut cw = CollisionWorld::new();
        let mut shape_of: alloc::collections::BTreeMap<u32, Option<(ShapeId, i16, bool)>> = Default::default();
        for p in placements {
            let entry = match shape_of.get(&p.record) {
                Some(e) => *e,
                None => {
                    let r = &records[p.record as usize];
                    let solid = Solid::parse(world.data, r.offset + 6)?;
                    let e = if solid.mesh.is_empty() && solid.camera_mesh.is_empty() {
                        None
                    } else {
                        let material = match r.options {
                            Some(o) => material_of(world.data, &o, table)?,
                            None => 0,
                        };
                        Some((cw.add_shape(Shape::from_solid(&solid)), material, solid.is_fixed()))
                    };
                    shape_of.insert(p.record, e);
                    e
                }
            };
            if let Some((shape, material, fixed)) = entry {
                cw.add_body(shape, p.world, material, p.record, fixed);
            }
        }
        cw.rebuild();
        Ok(cw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::mesh::tests::step_mesh;

    fn world() -> CollisionWorld {
        let (m, t) = step_mesh();
        let mut cw = CollisionWorld::new();
        let s = cw.add_shape(Shape { main: TriMesh::new(&m, &t), camera: None });
        cw.add_body(s, Affine::IDENTITY, 1, 10, true);
        // A second copy turned half a turn about z and moved: its wall is at x = 15, facing +x.
        let turned = Affine { cols: [[-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0]], t: [20.0, 10.0, 0.0] };
        cw.add_body(s, turned, 2, 11, false);
        cw.rebuild();
        cw
    }

    #[test]
    fn world_queries_find_the_nearest_body() {
        let mut cw = world();
        let f = QueryFilter::default();
        let h = cw.ray_cast([2.0, 2.0, 5.0], [0.0, 0.0, -2.0], 100.0, &f).unwrap();
        assert_eq!((h.distance, h.body, h.material), (5.0, BodyId(0), 1));
        assert_eq!(h.normal, [0.0, 0.0, 1.0]);
        // The turned copy's wall is at x = 15, facing +x.
        let h = cw.ray_cast([25.0, 5.0, 1.0], [-1.0, 0.0, 0.0], 100.0, &f).unwrap();
        assert_eq!(h.body, BodyId(1));
        assert!((h.distance - 10.0).abs() < 1e-5);
        assert!((h.normal[0] - 1.0).abs() < 1e-6);
        let s = cw.sphere_sweep([25.0, 5.0, 1.0], 0.5, [-20.0, 0.0, 0.0], &f).unwrap();
        assert!((s.fraction - 0.475).abs() < 1e-5 && s.body == BodyId(1));
        let g = cw.ground_probe([18.0, 5.0, 10.0], 100.0, &f).unwrap();
        assert!(g.is_ground && (g.distance - 10.0).abs() < 1e-5 && g.material == 2);
        let g = cw.ground_probe_sphere([2.0, 5.0, 3.0], 0.5, 10.0, &f).unwrap();
        assert!((g.distance - 2.5).abs() < 1e-5);
        assert!(cw.segment_blocked([25.0, 5.0, 1.0], [5.0, 5.0, 1.0], &f));
        assert!(!cw.segment_blocked([25.0, 5.0, 1.0], [16.0, 5.0, 1.0], &f));
        let ignore = QueryFilter { ignore: Some(BodyId(1)), ..f };
        assert!(cw.ray_cast([25.0, 5.0, 1.0], [-1.0, 0.0, 0.0], 100.0, &ignore).is_none());
        // Moving the second body away.
        let far = Affine { t: [1000.0, 0.0, 0.0], ..cw.body(BodyId(1)).transform };
        cw.set_transform(BodyId(1), far);
        assert!(cw.ray_cast([25.0, 5.0, 1.0], [-1.0, 0.0, 0.0], 100.0, &f).is_none());
        let mut ov = Vec::new();
        cw.sphere_overlaps([3.0, 1.0, 0.25], 0.5, &f, &mut ov);
        assert_eq!(ov.len(), 1);
        assert!((ov[0].depth - 0.25).abs() < 1e-6);
        assert_eq!(cw.point_inside([8.0, 5.0, 1.0], &f), Some(BodyId(0)));
        assert_eq!(cw.point_inside([2.0, 5.0, 1.0], &f), None);
    }
}
