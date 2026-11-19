//! Geometric primitives: boxes, triangles, closest points, ray and sweep tests against one triangle.
//!
//! Sweeps move a shape by `delta` over a fraction `t` in `[0, 1]`. Triangles are "one-sided" unless a test is
//! asked to be two-sided: collision normals point out of the solid (collision.md 1), so a contact counts only from
//! the open side, and a shape that starts inside a solid can move out of it.

use crate::math::{Vec3, add, cross, dot, length, length_sq, madd, max3, min3, normalize, scale, sqrt, sub};

/// An axis-aligned box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    /// Minimum corner.
    pub min: Vec3,
    /// Maximum corner.
    pub max: Vec3,
}

impl Default for Aabb {
    fn default() -> Self {
        Aabb::EMPTY
    }
}

impl Aabb {
    /// The empty box (union identity).
    pub const EMPTY: Aabb = Aabb { min: [f32::INFINITY; 3], max: [f32::NEG_INFINITY; 3] };

    /// The box of some points.
    pub fn from_points(points: &[Vec3]) -> Aabb {
        points.iter().fold(Aabb::EMPTY, |b, &p| b.with_point(p))
    }

    /// The box grown to contain `p`.
    #[inline]
    pub fn with_point(self, p: Vec3) -> Aabb {
        Aabb { min: min3(self.min, p), max: max3(self.max, p) }
    }

    /// The union of two boxes.
    #[inline]
    pub fn union(self, o: Aabb) -> Aabb {
        Aabb { min: min3(self.min, o.min), max: max3(self.max, o.max) }
    }

    /// The box grown by `r` on every side.
    #[inline]
    pub fn grow(self, r: f32) -> Aabb {
        Aabb { min: sub(self.min, [r; 3]), max: add(self.max, [r; 3]) }
    }

    /// Whether the box is empty.
    pub fn is_empty(&self) -> bool {
        self.min[0] > self.max[0] || self.min[1] > self.max[1] || self.min[2] > self.max[2]
    }

    /// Whether two boxes overlap (touching counts).
    #[inline]
    pub fn overlaps(&self, o: &Aabb) -> bool {
        (0..3).all(|i| self.min[i] <= o.max[i] && o.min[i] <= self.max[i])
    }

    /// Centre.
    #[inline]
    pub fn center(&self) -> Vec3 {
        scale(add(self.min, self.max), 0.5)
    }

    /// Half-size along each axis.
    #[inline]
    pub fn half_extent(&self) -> Vec3 {
        scale(sub(self.max, self.min), 0.5)
    }

    /// Surface area (for split heuristics).
    pub fn area(&self) -> f32 {
        if self.is_empty() {
            return 0.0;
        }
        let e = sub(self.max, self.min);
        2.0 * (e[0] * e[1] + e[1] * e[2] + e[2] * e[0])
    }

    /// The box of this box under an affine transform.
    pub fn transformed(&self, a: &formats::geometry::Affine) -> Aabb {
        if self.is_empty() {
            return *self;
        }
        let c = a.transform_point(self.center());
        let h = self.half_extent();
        let m = &a.cols;
        let mut e = [0.0f32; 3];
        for (i, ei) in e.iter_mut().enumerate() {
            *ei = (m[0][i] * h[0]).abs() + (m[1][i] * h[1]).abs() + (m[2][i] * h[2]).abs();
        }
        Aabb { min: sub(c, e), max: add(c, e) }
    }

    /// The entry distance of the ray `origin + t · dir` (with `inv_dir = 1 / dir` per component) into the box, if it
    /// enters within `[0, t_max]`.
    #[inline]
    pub fn ray_enter(&self, origin: Vec3, inv_dir: Vec3, t_max: f32) -> Option<f32> {
        let mut t0 = 0.0f32;
        let mut t1 = t_max;
        for i in 0..3 {
            let a = (self.min[i] - origin[i]) * inv_dir[i];
            let b = (self.max[i] - origin[i]) * inv_dir[i];
            // NaN (origin on a slab with a zero direction) leaves the interval unchanged through min/max.
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
        (t0 <= t1).then_some(t0)
    }
}

/// `1 / d` per component, with zero components giving infinities of the right sign.
#[inline]
pub fn inverse_dir(d: Vec3) -> Vec3 {
    let inv = |x: f32| if x == 0.0 { f32::INFINITY } else { 1.0 / x };
    [inv(d[0]), inv(d[1]), inv(d[2])]
}

/// A triangle with its plane `normal · x = d`. The normal is `normalise((v1 − v0) × (v2 − v0))` (collision.md 1);
/// zero for a degenerate triangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triangle {
    /// Corners.
    pub v: [Vec3; 3],
    /// Unit normal, pointing out of the solid.
    pub normal: Vec3,
    /// Plane offset.
    pub d: f32,
}

impl Triangle {
    /// From three corners.
    pub fn new(v0: Vec3, v1: Vec3, v2: Vec3) -> Triangle {
        let normal = normalize(cross(sub(v1, v0), sub(v2, v0)));
        Triangle { v: [v0, v1, v2], normal, d: dot(normal, v0) }
    }

    /// Whether the triangle has no area.
    pub fn is_degenerate(&self) -> bool {
        self.normal == [0.0; 3]
    }

    /// Signed distance of `p` from the plane.
    #[inline]
    pub fn signed_distance(&self, p: Vec3) -> f32 {
        dot(self.normal, p) - self.d
    }

    /// The box of the triangle.
    pub fn aabb(&self) -> Aabb {
        Aabb::from_points(&self.v)
    }
}

/// The point of triangle `t` closest to `p`.
pub fn closest_point_on_triangle(p: Vec3, t: &Triangle) -> Vec3 {
    let [a, b, c] = t.v;
    let ab = sub(b, a);
    let ac = sub(c, a);
    let ap = sub(p, a);
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return madd(a, ab, v);
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return madd(a, ac, w);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return madd(b, sub(c, b), w);
    }
    let denom = va + vb + vc;
    if denom == 0.0 {
        return a;
    }
    let v = vb / denom;
    let w = vc / denom;
    add(a, add(scale(ab, v), scale(ac, w)))
}

/// Closest points of segments `p0 p1` and `q0 q1`: `(point on the first, point on the second)`.
pub fn closest_points_segments(p0: Vec3, p1: Vec3, q0: Vec3, q1: Vec3) -> (Vec3, Vec3) {
    let d1 = sub(p1, p0);
    let d2 = sub(q1, q0);
    let r = sub(p0, q0);
    let a = dot(d1, d1);
    let e = dot(d2, d2);
    let f = dot(d2, r);
    let clamp01 = |x: f32| x.clamp(0.0, 1.0);
    let (s, t);
    if a <= 1e-12 && e <= 1e-12 {
        return (p0, q0);
    }
    if a <= 1e-12 {
        s = 0.0;
        t = clamp01(f / e);
    } else {
        let c = dot(d1, r);
        if e <= 1e-12 {
            t = 0.0;
            s = clamp01(-c / a);
        } else {
            let b = dot(d1, d2);
            let denom = a * e - b * b;
            let mut s0 = if denom > 1e-12 * a * e { clamp01((b * f - c * e) / denom) } else { 0.0 };
            let mut t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t0 = 0.0;
                s0 = clamp01(-c / a);
            } else if t0 > 1.0 {
                t0 = 1.0;
                s0 = clamp01((b - c) / a);
            }
            s = s0;
            t = t0;
        }
    }
    (madd(p0, d1, s), madd(q0, d2, t))
}

/// Ray (or segment) `origin + t · dir` against a triangle, for `t` in `[0, t_max]`. Returns `(t, front)`; `front`
/// is true when the ray meets the side the normal points to. One-sided tests skip back faces.
#[inline]
pub fn ray_triangle(origin: Vec3, dir: Vec3, tri: &Triangle, t_max: f32, two_sided: bool) -> Option<(f32, bool)> {
    let e1 = sub(tri.v[1], tri.v[0]);
    let e2 = sub(tri.v[2], tri.v[0]);
    let p = cross(dir, e2);
    let det = dot(e1, p);
    // det > 0: the ray goes against the normal (front face).
    let front = det > 0.0;
    if det == 0.0 || (!two_sided && !front) {
        return None;
    }
    let inv = 1.0 / det;
    let s = sub(origin, tri.v[0]);
    let u = dot(s, p) * inv;
    const SLACK: f32 = 1e-6;
    if !(-SLACK..=1.0 + SLACK).contains(&u) {
        return None;
    }
    let q = cross(s, e1);
    let v = dot(dir, q) * inv;
    if v < -SLACK || u + v > 1.0 + SLACK {
        return None;
    }
    let t = dot(e2, q) * inv;
    (t >= 0.0 && t <= t_max).then_some((t, front))
}

/// A sweep contact with one triangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// Fraction of the motion at first contact, in `[0, 1]`.
    pub t: f32,
    /// Contact normal (from the triangle toward the moving shape).
    pub normal: Vec3,
    /// Contact point on the triangle.
    pub point: Vec3,
}

/// Whether point `p` (on the triangle's plane) is inside the triangle, with a small tolerance.
fn inside_triangle(p: Vec3, tri: &Triangle) -> bool {
    let n = tri.normal;
    for i in 0..3 {
        let a = tri.v[i];
        let b = tri.v[(i + 1) % 3];
        let edge_n = cross(sub(b, a), n);
        // edge_n points out of the triangle for counter-clockwise corners.
        if dot(edge_n, sub(p, a)) > 1e-4 * length(sub(b, a)) {
            return false;
        }
    }
    true
}

/// Smallest root in `[0, 1]` of `a t² + 2 b t + c = 0` with `c > 0` and `b < 0` (approaching).
#[inline]
fn first_root(a: f32, b: f32, c: f32) -> Option<f32> {
    if a <= 1e-12 || b >= 0.0 {
        return None;
    }
    let disc = b * b - a * c;
    if disc < 0.0 {
        return None;
    }
    let t = (-b - sqrt(disc)) / a;
    (0.0..=1.0).contains(&t).then_some(t)
}

/// Sphere of radius `r` at `center` moving by `delta`, against a triangle (face, edges and corners).
pub fn sphere_sweep_triangle(center: Vec3, r: f32, delta: Vec3, tri: &Triangle, two_sided: bool) -> Option<Contact> {
    if tri.is_degenerate() {
        return None;
    }
    // Work relative to the first corner to keep magnitudes small.
    let o = tri.v[0];
    let c = sub(center, o);
    let v = [[0.0; 3], sub(tri.v[1], o), sub(tri.v[2], o)];
    let local = Triangle { v, normal: tri.normal, d: 0.0 };
    let n = tri.normal;
    let dist0 = dot(n, c);
    let dn = dot(n, delta);
    if !two_sided && dist0 < -r {
        return None;
    }
    let side = if dist0 >= 0.0 { 1.0 } else { -1.0 };
    let sn = scale(n, side);
    let (sd, sdn) = (dist0 * side, dn * side);
    // Face.
    if sd <= r {
        let q = madd(c, n, -dist0);
        if inside_triangle(q, &local) {
            if (side > 0.0 || two_sided) && sdn < 0.0 {
                return Some(Contact { t: 0.0, normal: sn, point: add(q, o) });
            }
            return None;
        }
    } else if sdn < 0.0 {
        let t = (r - sd) / sdn;
        if t <= 1.0 {
            let q = madd(madd(c, delta, t), sn, -r);
            if inside_triangle(q, &local) {
                return Some(Contact { t, normal: sn, point: add(q, o) });
            }
        }
    }
    // Edges and corners: the earliest contact.
    let mut best: Option<(f32, Vec3)> = None;
    let mut consider = |t: f32, p: Vec3| {
        if best.is_none_or(|(bt, _)| t < bt) {
            best = Some((t, p));
        }
    };
    let dd = dot(delta, delta);
    for i in 0..3 {
        let p0 = v[i];
        let e = sub(v[(i + 1) % 3], p0);
        let m = sub(c, p0);
        let ee = dot(e, e);
        let ed = dot(e, delta);
        let em = dot(e, m);
        let a = ee * dd - ed * ed;
        let b = ee * dot(m, delta) - em * ed;
        let cq = ee * (dot(m, m) - r * r) - em * em;
        if cq < 0.0 {
            let s = em / ee;
            if (0.0..=1.0).contains(&s) {
                consider(0.0, madd(p0, e, s));
            }
            continue;
        }
        if let Some(t) = first_root(a, b, cq) {
            let s = (em + t * ed) / ee;
            if (0.0..=1.0).contains(&s) {
                consider(t, madd(p0, e, s));
            }
        }
    }
    for &p in &v {
        let m = sub(c, p);
        let cq = dot(m, m) - r * r;
        if cq < 0.0 {
            consider(0.0, p);
            continue;
        }
        if let Some(t) = first_root(dd, dot(m, delta), cq) {
            consider(t, p);
        }
    }
    let (t, p) = best?;
    let at = madd(c, delta, t);
    let normal = normalize(sub(at, p));
    let normal = if normal == [0.0; 3] { sn } else { normal };
    if !two_sided && dot(normal, n) < -1e-3 {
        return None;
    }
    if t == 0.0 && dot(delta, normal) >= 0.0 {
        return None;
    }
    Some(Contact { t, normal, point: add(p, o) })
}

/// Distance between segment `p0 p1` and a triangle: `(distance, point on the segment, point on the triangle)`.
pub fn segment_triangle_distance(p0: Vec3, p1: Vec3, tri: &Triangle) -> (f32, Vec3, Vec3) {
    let d = sub(p1, p0);
    if let Some((t, _)) = ray_triangle(p0, d, tri, 1.0, true) {
        let p = madd(p0, d, t);
        return (0.0, p, p);
    }
    let mut best = (f32::INFINITY, p0, p0);
    let mut take = |a: Vec3, b: Vec3| {
        let dist = length_sq(sub(a, b));
        if dist < best.0 {
            best = (dist, a, b);
        }
    };
    take(p0, closest_point_on_triangle(p0, tri));
    take(p1, closest_point_on_triangle(p1, tri));
    for i in 0..3 {
        let (a, b) = closest_points_segments(p0, p1, tri.v[i], tri.v[(i + 1) % 3]);
        take(a, b);
    }
    (sqrt(best.0), best.1, best.2)
}

/// Golden-section steps used to find the closest approach of a capsule sweep.
const GOLDEN_STEPS: u32 = 32;
/// Bisection steps used to find the first contact of a capsule sweep.
const ROOT_STEPS: u32 = 24;

/// Capsule (segment `a b`, radius `r`) moving by `delta`, against a triangle. The distance from the moving segment
/// to the triangle is a convex function of the fraction, so its minimum is found by golden-section search and the
/// first contact by bisection before it. The returned fraction is the last one found without contact (within
/// 2^-24 of the contact).
pub fn capsule_sweep_triangle(
    a: Vec3,
    b: Vec3,
    r: f32,
    delta: Vec3,
    tri: &Triangle,
    two_sided: bool,
) -> Option<Contact> {
    if tri.is_degenerate() {
        return None;
    }
    let o = tri.v[0];
    let local = Triangle { v: [[0.0; 3], sub(tri.v[1], o), sub(tri.v[2], o)], normal: tri.normal, d: 0.0 };
    let (a, b) = (sub(a, o), sub(b, o));
    let n = tri.normal;
    if !two_sided && dot(n, a) < -r && dot(n, b) < -r {
        return None;
    }
    let at = |t: f32| segment_triangle_distance(madd(a, delta, t), madd(b, delta, t), &local);
    let finish = |t: f32, seg: Vec3, on_tri: Vec3| -> Option<Contact> {
        let normal = normalize(sub(seg, on_tri));
        let normal =
            if normal == [0.0; 3] { if dot(n, sub(a, on_tri)) >= 0.0 { n } else { scale(n, -1.0) } } else { normal };
        if !two_sided && dot(normal, n) < -1e-3 {
            return None;
        }
        if t == 0.0 && dot(delta, normal) >= 0.0 {
            return None;
        }
        Some(Contact { t, normal, point: add(on_tri, o) })
    };
    let (d0, s0, q0) = at(0.0);
    if d0 <= r {
        return finish(0.0, s0, q0);
    }
    if d0 - length(delta) > r {
        return None;
    }
    // Closest approach.
    const INV_PHI: f32 = 0.618_034;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    let mut x1 = hi - INV_PHI * (hi - lo);
    let mut x2 = lo + INV_PHI * (hi - lo);
    let mut f1 = at(x1).0;
    let mut f2 = at(x2).0;
    let mut t_min = 1.0;
    let (d1, ..) = at(1.0);
    if d1 > r {
        for _ in 0..GOLDEN_STEPS {
            if f1.min(f2) <= r {
                break;
            }
            if f1 < f2 {
                hi = x2;
                x2 = x1;
                f2 = f1;
                x1 = hi - INV_PHI * (hi - lo);
                f1 = at(x1).0;
            } else {
                lo = x1;
                x1 = x2;
                f1 = f2;
                x2 = lo + INV_PHI * (hi - lo);
                f2 = at(x2).0;
            }
        }
        if f1.min(f2) > r {
            return None;
        }
        t_min = if f1 <= r { x1 } else { x2 };
    }
    // First contact: f(lo) > r, f(hi) <= r.
    let (mut lo, mut hi) = (0.0f32, t_min);
    for _ in 0..ROOT_STEPS {
        let mid = 0.5 * (lo + hi);
        if at(mid).0 > r {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let (_, s, q) = at(hi);
    finish(lo, s, q).map(|c| Contact { t: lo, ..c })
}

/// The deepest overlap of a sphere with a triangle: `(normal from the triangle to the centre, depth, point)`, or
/// `None` when they do not touch (or, one-sided, when the centre is behind the plane).
pub fn sphere_triangle_overlap(center: Vec3, r: f32, tri: &Triangle, two_sided: bool) -> Option<(Vec3, f32, Vec3)> {
    if tri.is_degenerate() {
        return None;
    }
    if !two_sided && tri.signed_distance(center) < 0.0 {
        return None;
    }
    let q = closest_point_on_triangle(center, tri);
    let d = sub(center, q);
    let l2 = dot(d, d);
    if l2 >= r * r {
        return None;
    }
    let l = sqrt(l2);
    let normal = if l > 1e-6 { scale(d, 1.0 / l) } else { tri.normal };
    Some((normal, r - l, q))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor() -> Triangle {
        Triangle::new([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0])
    }

    #[test]
    fn ray_hits_front_only_unless_two_sided() {
        let t = floor();
        assert_eq!(t.normal, [0.0, 0.0, 1.0]);
        let hit = ray_triangle([1.0, 1.0, 5.0], [0.0, 0.0, -1.0], &t, 10.0, false).unwrap();
        assert_eq!(hit, (5.0, true));
        assert!(ray_triangle([1.0, 1.0, -5.0], [0.0, 0.0, 1.0], &t, 10.0, false).is_none());
        assert_eq!(ray_triangle([1.0, 1.0, -5.0], [0.0, 0.0, 1.0], &t, 10.0, true), Some((5.0, false)));
        assert!(ray_triangle([1.0, 1.0, 5.0], [0.0, 0.0, -1.0], &t, 4.0, false).is_none());
        assert!(ray_triangle([9.0, 9.0, 5.0], [0.0, 0.0, -1.0], &t, 10.0, false).is_none());
    }

    #[test]
    fn closest_points() {
        let t = floor();
        assert_eq!(closest_point_on_triangle([1.0, 1.0, 3.0], &t), [1.0, 1.0, 0.0]);
        assert_eq!(closest_point_on_triangle([-1.0, -1.0, 0.0], &t), [0.0, 0.0, 0.0]);
        assert_eq!(closest_point_on_triangle([5.0, -3.0, 0.0], &t), [5.0, 0.0, 0.0]);
        let p = closest_point_on_triangle([8.0, 8.0, 0.0], &t);
        assert!((p[0] - 5.0).abs() < 1e-5 && (p[1] - 5.0).abs() < 1e-5);
        let (a, b) = closest_points_segments([0.0, 0.0, 1.0], [2.0, 0.0, 1.0], [1.0, -1.0, 0.0], [1.0, 1.0, 0.0]);
        assert_eq!((a, b), ([1.0, 0.0, 1.0], [1.0, 0.0, 0.0]));
        let (d, ..) = segment_triangle_distance([1.0, 1.0, 2.0], [1.0, 1.0, 5.0], &t);
        assert_eq!(d, 2.0);
        let (d, ..) = segment_triangle_distance([1.0, 1.0, 2.0], [1.0, 1.0, -5.0], &t);
        assert_eq!(d, 0.0);
    }

    #[test]
    fn sphere_sweep_face_edge_corner() {
        let t = floor();
        let c = sphere_sweep_triangle([2.0, 2.0, 5.0], 1.0, [0.0, 0.0, -10.0], &t, false).unwrap();
        assert!((c.t - 0.4).abs() < 1e-6 && c.normal == [0.0, 0.0, 1.0]);
        // Edge along x (y = 0), approached from -y at z = 0.5.
        let c = sphere_sweep_triangle([5.0, -5.0, 0.5], 1.0, [0.0, 10.0, 0.0], &t, false).unwrap();
        let expect = (5.0 - (1.0f32 - 0.25).sqrt()) / 10.0;
        assert!((c.t - expect).abs() < 1e-5, "{}", c.t);
        assert!(c.normal[1] < 0.0 && c.normal[2] > 0.0);
        // Corner at the origin, approached diagonally.
        let c = sphere_sweep_triangle([-5.0, -5.0, 0.0], 1.0, [10.0, 10.0, 0.0], &t, false).unwrap();
        let expect = (50.0f32.sqrt() - 1.0) / 200.0f32.sqrt();
        assert!((c.t - expect).abs() < 1e-5);
        // From behind: no contact one-sided; a two-sided test hits the back face.
        assert!(sphere_sweep_triangle([2.0, 2.0, -5.0], 1.0, [0.0, 0.0, 10.0], &t, false).is_none());
        let c = sphere_sweep_triangle([2.0, 2.0, -5.0], 1.0, [0.0, 0.0, 10.0], &t, true).unwrap();
        assert_eq!(c.normal, [0.0, 0.0, -1.0]);
        // Starting in contact and moving away: nothing.
        assert!(sphere_sweep_triangle([2.0, 2.0, 0.5], 1.0, [0.0, 0.0, 1.0], &t, false).is_none());
        assert_eq!(sphere_sweep_triangle([2.0, 2.0, 0.5], 1.0, [0.0, 0.0, -1.0], &t, false).unwrap().t, 0.0);
    }

    #[test]
    fn capsule_sweep_matches_sphere_sweep_for_a_point_segment() {
        let t = floor();
        for (c, d) in [
            ([2.0, 2.0, 5.0], [0.0, 0.0, -10.0]),
            ([5.0, -5.0, 0.5], [0.0, 10.0, 0.0]),
            ([-5.0, -5.0, 0.0], [10.0, 10.0, 0.0]),
            ([3.0, 3.0, 3.0], [-1.0, 2.0, -6.0]),
        ] {
            let s = sphere_sweep_triangle(c, 1.0, d, &t, false).unwrap();
            let k = capsule_sweep_triangle(c, c, 1.0, d, &t, false).unwrap();
            assert!((s.t - k.t).abs() < 1e-5, "{c:?}: {} {}", s.t, k.t);
        }
        // A horizontal capsule falling onto the floor's edge region.
        let k = capsule_sweep_triangle([-3.0, 5.0, 4.0], [3.0, 5.0, 4.0], 1.0, [0.0, 0.0, -8.0], &t, false).unwrap();
        assert!((k.t - 3.0 / 8.0).abs() < 1e-5);
        assert!(
            capsule_sweep_triangle([20.0, 20.0, 4.0], [21.0, 20.0, 4.0], 1.0, [0.0, 0.0, -8.0], &t, false).is_none()
        );
    }

    #[test]
    fn overlap_and_boxes() {
        let t = floor();
        let (n, depth, _) = sphere_triangle_overlap([1.0, 1.0, 0.5], 1.0, &t, false).unwrap();
        assert_eq!(n, [0.0, 0.0, 1.0]);
        assert!((depth - 0.5).abs() < 1e-6);
        assert!(sphere_triangle_overlap([1.0, 1.0, -0.5], 1.0, &t, false).is_none());
        let b = t.aabb();
        assert_eq!(b.ray_enter([1.0, 1.0, 5.0], inverse_dir([0.0, 0.0, -1.0]), 10.0), Some(5.0));
        assert_eq!(b.ray_enter([20.0, 1.0, 5.0], inverse_dir([0.0, 0.0, -1.0]), 10.0), None);
        let a = formats::geometry::Affine {
            cols: [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]],
            t: [1.0, 0.0, 0.0],
        };
        let tb = b.transformed(&a);
        assert!((tb.min[0] + 9.0).abs() < 1e-5 && (tb.max[1] - 10.0).abs() < 1e-5);
    }
}
