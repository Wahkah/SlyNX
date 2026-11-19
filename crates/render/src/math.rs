//! Small, deterministic math: 3-vectors, column-major 4x4 matrices (the GLSL layout) and trigonometry.
//!
//! The functions here do not use the platform's libm, so the PC and the Switch compute bit-identical transforms
//! (useful for image comparison) and the crate stays `no_std` on the Switch.

use core::ops::{Add, Mul, Neg, Sub};

pub const PI: f32 = core::f32::consts::PI;
pub const TAU: f32 = core::f32::consts::TAU;
const FRAC_PI_2: f32 = core::f32::consts::FRAC_PI_2;

/// Absolute value.
#[inline]
pub fn abs(x: f32) -> f32 {
    f32::from_bits(x.to_bits() & 0x7fff_ffff)
}

/// Largest integer not above `x`, for |x| < 2^31.
#[inline]
pub fn floor(x: f32) -> f32 {
    let t = x as i32 as f32;
    if t > x { t - 1.0 } else { t }
}

/// Square root (Newton's method from an exponent-halving guess). Returns 0 for non-positive input.
pub fn sqrt(x: f32) -> f32 {
    if x.is_nan() || x == f32::INFINITY {
        return x;
    }
    if x <= 0.0 {
        return 0.0;
    }
    let mut y = f32::from_bits((x.to_bits() >> 1) + 0x1fc0_0000);
    for _ in 0..4 {
        y = 0.5 * (y + x / y);
    }
    y
}

/// Sine and cosine of `x` (radians), absolute error below 1e-6 for |x| up to a few thousand.
pub fn sin_cos(x: f32) -> (f32, f32) {
    // Reduce to [-pi, pi] with 2*pi split in two parts (the high part has few mantissa bits, so k * TAU_HI is exact),
    // then to [-pi/2, pi/2] using sin(pi - r) = sin r and cos(pi - r) = -cos r.
    const TAU_HI: f32 = 6.28125;
    const TAU_LO: f32 = 0.001_935_307_2;
    let k = floor(x / TAU + 0.5);
    let r = (x - k * TAU_HI) - k * TAU_LO;
    let (r, sign) = if r > FRAC_PI_2 {
        (PI - r, -1.0)
    } else if r < -FRAC_PI_2 {
        (-PI - r, -1.0)
    } else {
        (r, 1.0)
    };
    let r2 = r * r;
    // Taylor series to degree 11 (sine) and 12 (cosine); the next terms are below 5e-7 on [-pi/2, pi/2].
    let s = r
        * (1.0
            + r2 * (-1.0 / 6.0
                + r2 * (1.0 / 120.0 + r2 * (-1.0 / 5040.0 + r2 * (1.0 / 362_880.0 + r2 * (-1.0 / 39_916_800.0))))));
    let c = 1.0
        + r2 * (-0.5
            + r2 * (1.0 / 24.0
                + r2 * (-1.0 / 720.0
                    + r2 * (1.0 / 40_320.0 + r2 * (-1.0 / 3_628_800.0 + r2 * (1.0 / 479_001_600.0))))));
    (s, c * sign)
}

pub fn sin(x: f32) -> f32 {
    sin_cos(x).0
}

pub fn cos(x: f32) -> f32 {
    sin_cos(x).1
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    pub const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
    pub const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
    pub const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Vec3 {
        Vec3 { x, y, z }
    }

    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }

    pub fn length(self) -> f32 {
        sqrt(self.dot(self))
    }

    /// Unit vector in the same direction, or zero for a zero vector.
    pub fn normalize(self) -> Vec3 {
        let l = self.length();
        if l > 0.0 { self * (1.0 / l) } else { Vec3::ZERO }
    }

    pub fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

/// Clip-space conventions of a backend. The camera builds its projection to match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipSpace {
    /// Depth range 0..1 (deko3d's `DepthZeroToOne`) instead of OpenGL's -1..1.
    pub depth_zero_to_one: bool,
    /// Normalised y = -1 is the top of the target (deko3d's `OriginUpperLeft`) instead of the bottom.
    pub y_down: bool,
}

impl ClipSpace {
    /// OpenGL defaults: depth -1..1, y up.
    pub const GL: ClipSpace = ClipSpace { depth_zero_to_one: false, y_down: false };
    /// deko3d with `DepthZeroToOne | OriginUpperLeft`.
    pub const DEKO3D: ClipSpace = ClipSpace { depth_zero_to_one: true, y_down: true };
}

/// 4x4 matrix, column-major: element (row r, column c) is `m[c * 4 + r]`, the layout GLSL expects.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub m: [f32; 16],
}

impl Default for Mat4 {
    fn default() -> Mat4 {
        Mat4::IDENTITY
    }
}

impl Mat4 {
    pub const IDENTITY: Mat4 =
        Mat4 { m: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0] };

    #[inline]
    pub fn get(&self, row: usize, col: usize) -> f32 {
        self.m[col * 4 + row]
    }

    pub fn translation(t: Vec3) -> Mat4 {
        let mut r = Mat4::IDENTITY;
        r.m[12] = t.x;
        r.m[13] = t.y;
        r.m[14] = t.z;
        r
    }

    pub fn scale(s: Vec3) -> Mat4 {
        let mut r = Mat4::IDENTITY;
        r.m[0] = s.x;
        r.m[5] = s.y;
        r.m[10] = s.z;
        r
    }

    /// Rotation by `angle` radians around the unit vector `axis` (right-handed).
    pub fn rotation(axis: Vec3, angle: f32) -> Mat4 {
        let a = axis.normalize();
        let (s, c) = sin_cos(angle);
        let t = 1.0 - c;
        Mat4 {
            m: [
                t * a.x * a.x + c,
                t * a.x * a.y + s * a.z,
                t * a.x * a.z - s * a.y,
                0.0,
                t * a.x * a.y - s * a.z,
                t * a.y * a.y + c,
                t * a.y * a.z + s * a.x,
                0.0,
                t * a.x * a.z + s * a.y,
                t * a.y * a.z - s * a.x,
                t * a.z * a.z + c,
                0.0,
                0.0,
                0.0,
                0.0,
                1.0,
            ],
        }
    }

    /// Translation * rotation * scale, the usual object transform.
    pub fn trs(t: Vec3, axis: Vec3, angle: f32, s: Vec3) -> Mat4 {
        let mut r = Mat4::rotation(axis, angle);
        for i in 0..3 {
            r.m[i] *= s.x;
            r.m[4 + i] *= s.y;
            r.m[8 + i] *= s.z;
        }
        r.m[12] = t.x;
        r.m[13] = t.y;
        r.m[14] = t.z;
        r
    }

    /// Right-handed view matrix: the camera at `eye` looks at `target`, the view looks down -z.
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
        let f = (target - eye).normalize();
        let s = f.cross(up).normalize();
        let u = s.cross(f);
        Mat4 {
            m: [
                s.x,
                u.x,
                -f.x,
                0.0,
                s.y,
                u.y,
                -f.y,
                0.0,
                s.z,
                u.z,
                -f.z,
                0.0,
                -s.dot(eye),
                -u.dot(eye),
                f.dot(eye),
                1.0,
            ],
        }
    }

    /// Right-handed perspective projection for the given clip-space convention.
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32, clip: ClipSpace) -> Mat4 {
        let (s, c) = sin_cos(fov_y * 0.5);
        let f = c / s;
        let mut m = [0.0; 16];
        m[0] = f / aspect;
        m[5] = if clip.y_down { -f } else { f };
        m[11] = -1.0;
        if clip.depth_zero_to_one {
            m[10] = far / (near - far);
            m[14] = far * near / (near - far);
        } else {
            m[10] = (far + near) / (near - far);
            m[14] = 2.0 * far * near / (near - far);
        }
        Mat4 { m }
    }

    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        let m = &self.m;
        Vec3::new(
            m[0] * p.x + m[4] * p.y + m[8] * p.z + m[12],
            m[1] * p.x + m[5] * p.y + m[9] * p.z + m[13],
            m[2] * p.x + m[6] * p.y + m[10] * p.z + m[14],
        )
    }

    /// Full homogeneous transform of (p, 1).
    pub fn transform_point4(&self, p: Vec3) -> [f32; 4] {
        let m = &self.m;
        [
            m[0] * p.x + m[4] * p.y + m[8] * p.z + m[12],
            m[1] * p.x + m[5] * p.y + m[9] * p.z + m[13],
            m[2] * p.x + m[6] * p.y + m[10] * p.z + m[14],
            m[3] * p.x + m[7] * p.y + m[11] * p.z + m[15],
        ]
    }

    /// The top three rows, row-major: the layout of an affine transform in the instance buffer.
    pub fn affine_rows(&self) -> [[f32; 4]; 3] {
        let m = &self.m;
        [[m[0], m[4], m[8], m[12]], [m[1], m[5], m[9], m[13]], [m[2], m[6], m[10], m[14]]]
    }

    /// Determinant of the upper-left 3x3 part. Negative when the matrix mirrors (triangle winding reverses).
    pub fn determinant3(&self) -> f32 {
        let m = &self.m;
        m[0] * (m[5] * m[10] - m[9] * m[6]) - m[4] * (m[1] * m[10] - m[9] * m[2]) + m[8] * (m[1] * m[6] - m[5] * m[2])
    }

    /// Largest length of the three basis columns: how much the matrix can stretch a bounding sphere.
    pub fn max_scale(&self) -> f32 {
        let m = &self.m;
        let l = |i: usize| m[i] * m[i] + m[i + 1] * m[i + 1] + m[i + 2] * m[i + 2];
        sqrt(l(0).max(l(4)).max(l(8)))
    }
}

impl Mul for Mat4 {
    type Output = Mat4;
    fn mul(self, b: Mat4) -> Mat4 {
        let mut r = [0.0; 16];
        for c in 0..4 {
            for row in 0..4 {
                let mut s = 0.0;
                for k in 0..4 {
                    s += self.m[k * 4 + row] * b.m[c * 4 + k];
                }
                r[c * 4 + row] = s;
            }
        }
        Mat4 { m: r }
    }
}

/// Six normalised planes (a, b, c, d) with the inside where a*x + b*y + c*z + d >= 0.
#[derive(Clone, Copy, Debug)]
pub struct Frustum {
    pub planes: [[f32; 4]; 6],
}

impl Frustum {
    /// Extracts the planes of a view-projection matrix (Gribb and Hartmann's method).
    pub fn from_view_proj(vp: &Mat4, clip: ClipSpace) -> Frustum {
        let row = |r: usize| [vp.get(r, 0), vp.get(r, 1), vp.get(r, 2), vp.get(r, 3)];
        let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
        let add = |a: [f32; 4], b: [f32; 4]| [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]];
        let sub = |a: [f32; 4], b: [f32; 4]| [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]];
        let near = if clip.depth_zero_to_one { r2 } else { add(r3, r2) };
        let mut planes = [add(r3, r0), sub(r3, r0), add(r3, r1), sub(r3, r1), near, sub(r3, r2)];
        for p in &mut planes {
            let l = sqrt(p[0] * p[0] + p[1] * p[1] + p[2] * p[2]);
            if l > 0.0 {
                for v in p.iter_mut() {
                    *v /= l;
                }
            }
        }
        Frustum { planes }
    }

    /// False if the sphere is entirely outside one plane.
    #[inline]
    pub fn sphere_visible(&self, c: Vec3, radius: f32) -> bool {
        self.planes.iter().all(|p| p[0] * c.x + p[1] * c.y + p[2] * c.z + p[3] >= -radius)
    }

    /// Signed distance from the near plane (larger is farther); used to sort blended draws back to front.
    #[inline]
    pub fn depth(&self, c: Vec3) -> f32 {
        let p = &self.planes[4];
        p[0] * c.x + p[1] * c.y + p[2] * c.z + p[3]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, eps: f32) -> bool {
        abs(a - b) <= eps
    }

    #[test]
    fn sqrt_matches_std() {
        for &x in &[0.0f32, 1e-6, 0.25, 1.0, 2.0, 3.0, 100.0, 12345.678, 1e10] {
            assert!(close(sqrt(x), x.sqrt(), x.sqrt() * 1e-6 + 1e-12), "{x}");
        }
        assert_eq!(sqrt(-1.0), 0.0);
    }

    #[test]
    fn trig_matches_std() {
        let mut x = -50.0f32;
        while x < 50.0 {
            let (s, c) = sin_cos(x);
            assert!(close(s, x.sin(), 2e-6), "sin {x}: {s} vs {}", x.sin());
            assert!(close(c, x.cos(), 2e-6), "cos {x}: {c} vs {}", x.cos());
            x += 0.0173;
        }
    }

    #[test]
    fn floor_works() {
        assert_eq!(floor(1.5), 1.0);
        assert_eq!(floor(-1.5), -2.0);
        assert_eq!(floor(-2.0), -2.0);
        assert_eq!(floor(0.0), 0.0);
    }

    #[test]
    fn rotation_and_multiply() {
        let r = Mat4::rotation(Vec3::Z, PI / 2.0);
        let p = r.transform_point(Vec3::X);
        assert!(close(p.x, 0.0, 1e-6) && close(p.y, 1.0, 1e-6));
        let t = Mat4::translation(Vec3::new(1.0, 2.0, 3.0));
        let p = (t * r).transform_point(Vec3::X);
        assert!(close(p.x, 1.0, 1e-6) && close(p.y, 3.0, 1e-6) && close(p.z, 3.0, 1e-6));
        let trs = Mat4::trs(Vec3::new(1.0, 2.0, 3.0), Vec3::Z, PI / 2.0, Vec3::new(2.0, 2.0, 2.0));
        let p = trs.transform_point(Vec3::X);
        assert!(close(p.x, 1.0, 1e-5) && close(p.y, 4.0, 1e-5));
        assert!(close(trs.max_scale(), 2.0, 1e-5));
    }

    #[test]
    fn projection_maps_near_and_far() {
        for clip in [ClipSpace::GL, ClipSpace::DEKO3D] {
            let p = Mat4::perspective(1.0, 1.5, 0.5, 100.0, clip);
            let n = p.transform_point4(Vec3::new(0.0, 0.0, -0.5));
            let f = p.transform_point4(Vec3::new(0.0, 0.0, -100.0));
            let lo = if clip.depth_zero_to_one { 0.0 } else { -1.0 };
            assert!(close(n[2] / n[3], lo, 1e-5));
            assert!(close(f[2] / f[3], 1.0, 1e-5));
            let up = p.transform_point4(Vec3::new(0.0, 1.0, -2.0));
            assert_eq!(up[1] > 0.0, !clip.y_down);
        }
    }

    #[test]
    fn frustum_culls() {
        for clip in [ClipSpace::GL, ClipSpace::DEKO3D] {
            let view = Mat4::look_at(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y);
            let vp = Mat4::perspective(1.0, 1.0, 1.0, 50.0, clip) * view;
            let f = Frustum::from_view_proj(&vp, clip);
            assert!(f.sphere_visible(Vec3::ZERO, 1.0));
            assert!(!f.sphere_visible(Vec3::new(0.0, 0.0, 20.0), 1.0), "behind the camera");
            assert!(!f.sphere_visible(Vec3::new(0.0, 0.0, -100.0), 1.0), "beyond far");
            assert!(!f.sphere_visible(Vec3::new(100.0, 0.0, 0.0), 1.0), "off to the side");
            assert!(f.sphere_visible(Vec3::new(0.0, 0.0, -39.5), 1.0), "touching far");
            assert!(f.depth(Vec3::new(0.0, 0.0, -10.0)) > f.depth(Vec3::ZERO));
        }
    }
}
