//! Small vector and matrix helpers, with square root and sine/cosine that do not need a C maths library.
//!
//! Vectors are `[f32; 3]` and 3 x 3 matrices are three **columns** (`[Vec3; 3]`), the convention of transforms.md
//! section 1 and of [`formats::geometry::Affine`]. The transcendental functions work in `f64` with plain IEEE
//! operations only, so they give the same bits on every target.

pub use formats::geometry::Affine;
pub use formats::reader::Vec3;

/// A 3 x 3 matrix as three columns.
pub type Mat3 = [Vec3; 3];

/// The identity matrix.
pub const MAT3_IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// `a + b`.
#[inline]
pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// `a - b`.
#[inline]
pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `a * s`.
#[inline]
pub fn scale(a: Vec3, s: f32) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// `a + b * s`.
#[inline]
pub fn madd(a: Vec3, b: Vec3, s: f32) -> Vec3 {
    [a[0] + b[0] * s, a[1] + b[1] * s, a[2] + b[2] * s]
}

/// Component-wise product.
#[inline]
pub fn mul_elem(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

/// Dot product.
#[inline]
pub fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Cross product.
#[inline]
pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// Squared length.
#[inline]
pub fn length_sq(a: Vec3) -> f32 {
    dot(a, a)
}

/// Length.
#[inline]
pub fn length(a: Vec3) -> f32 {
    sqrt(dot(a, a))
}

/// `a / |a|`, or zero for a zero vector.
#[inline]
pub fn normalize(a: Vec3) -> Vec3 {
    let l = length(a);
    if l > 0.0 { scale(a, 1.0 / l) } else { [0.0; 3] }
}

/// Component-wise minimum.
#[inline]
pub fn min3(a: Vec3, b: Vec3) -> Vec3 {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])]
}

/// Component-wise maximum.
#[inline]
pub fn max3(a: Vec3, b: Vec3) -> Vec3 {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]
}

/// `M v`.
#[inline]
pub fn mat_mul_vec(m: &Mat3, v: Vec3) -> Vec3 {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}

/// `A B`.
#[inline]
pub fn mat_mul(a: &Mat3, b: &Mat3) -> Mat3 {
    [mat_mul_vec(a, b[0]), mat_mul_vec(a, b[1]), mat_mul_vec(a, b[2])]
}

/// Transpose.
#[inline]
pub fn transpose(m: &Mat3) -> Mat3 {
    [[m[0][0], m[1][0], m[2][0]], [m[0][1], m[1][1], m[2][1]], [m[0][2], m[1][2], m[2][2]]]
}

/// Determinant.
#[inline]
pub fn determinant(m: &Mat3) -> f32 {
    dot(m[0], cross(m[1], m[2]))
}

/// Inverse, or `None` when the matrix is singular or not finite.
pub fn inverse(m: &Mat3) -> Option<Mat3> {
    let det = determinant(m);
    if det == 0.0 || !det.is_finite() {
        return None;
    }
    // The rows of the inverse are the cross products of the columns, divided by the determinant.
    let inv = 1.0 / det;
    let r0 = scale(cross(m[1], m[2]), inv);
    let r1 = scale(cross(m[2], m[0]), inv);
    let r2 = scale(cross(m[0], m[1]), inv);
    Some(transpose(&[r0, r1, r2]))
}

/// The matrix with column `i` scaled by `s[i]` (`M · diag(s)`).
#[inline]
pub fn scale_columns(m: &Mat3, s: Vec3) -> Mat3 {
    [scale(m[0], s[0]), scale(m[1], s[1]), scale(m[2], s[2])]
}

/// The inverse of an affine transform, or `None` when its 3 x 3 part is singular.
pub fn affine_inverse(a: &Affine) -> Option<Affine> {
    let m = inverse(&a.cols)?;
    let t = mat_mul_vec(&m, a.t);
    Some(Affine { cols: m, t: [-t[0], -t[1], -t[2]] })
}

/// Square root, correctly rounded for every finite non-negative input; NaN for negative input.
pub fn sqrt(x: f32) -> f32 {
    if x.is_nan() || x < 0.0 {
        return f32::NAN;
    }
    if x == 0.0 || x.is_infinite() {
        return x;
    }
    sqrt_f64(x as f64) as f32
}

/// Square root in `f64` for positive finite input (Newton's method from a bit-level first guess).
fn sqrt_f64(x: f64) -> f64 {
    // Halving the exponent gives a first guess within a factor of about 1.5.
    let mut y = f64::from_bits((x.to_bits() >> 1) + (1023u64 << 51));
    for _ in 0..6 {
        y = 0.5 * (y + x / y);
    }
    y
}

const FRAC_PI_2_HI: f64 = core::f64::consts::FRAC_PI_2;
const FRAC_PI_2_LO: f64 = 6.123_233_995_736_766e-17;
const FRAC_2_PI: f64 = core::f64::consts::FRAC_2_PI;

/// Sine and cosine of `x` (radians).
pub fn sin_cos(x: f32) -> (f32, f32) {
    if !x.is_finite() {
        return (f32::NAN, f32::NAN);
    }
    let (s, c) = sin_cos_f64(x as f64);
    (s as f32, c as f32)
}

/// `x` rounded to the nearest integer (halves away from zero), for any finite `x`.
fn round_f64(x: f64) -> f64 {
    trunc_f64(if x >= 0.0 { x + 0.5 } else { x - 0.5 })
}

/// `x` with its fraction dropped, for any finite `x`.
fn trunc_f64(x: f64) -> f64 {
    // From 2^52 up every f64 is an integer.
    if x.abs() >= 4_503_599_627_370_496.0 { x } else { (x as i64) as f64 }
}

/// Sine.
pub fn sin(x: f32) -> f32 {
    sin_cos(x).0
}

/// Cosine.
pub fn cos(x: f32) -> f32 {
    sin_cos(x).1
}

fn sin_cos_f64(x: f64) -> (f64, f64) {
    // Reduce to r in [-pi/4, pi/4] with x = r + k pi/2 (two-part pi/2 keeps r accurate for game-sized angles).
    let q = x * FRAC_2_PI;
    let kf = round_f64(q);
    // Exact for |x| up to about 2^20 radians. Beyond that an f32 angle has no fractional precision left and the
    // reduction is approximate; the remainder is kept in range so the result stays on the unit circle.
    let r = ((x - kf * FRAC_PI_2_HI) - kf * FRAC_PI_2_LO)
        .clamp(-core::f64::consts::FRAC_PI_2, core::f64::consts::FRAC_PI_2);
    // Quadrant: kf mod 4, from integers held exactly in f64 (kf beyond 2^53 is a multiple of 4).
    let k = (kf - 4.0 * trunc_f64(kf / 4.0)) as i64;
    let r2 = r * r;
    // Taylor series to the 17th / 16th power: error below 1e-17 on the reduced range.
    let mut s = 0.0;
    let mut c = 0.0;
    let mut term_s = r;
    let mut term_c = 1.0;
    for n in 0..9 {
        s += term_s;
        c += term_c;
        let a = (2 * n + 2) as f64;
        let b = (2 * n + 3) as f64;
        term_s *= -r2 / (a * b);
        term_c *= -r2 / ((a - 1.0) * a);
    }
    match k.rem_euclid(4) {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// `x` modulo `m` (for `m > 0`), in `[0, m)`. Uses no floating-point remainder call.
#[inline]
pub fn rem_euclid(x: f32, m: f32) -> f32 {
    let (xd, md) = (x as f64, m as f64);
    let r = xd - md * trunc_f64(xd / md);
    let r = if r < 0.0 { r + md } else { r };
    let r = r as f32;
    // A tiny negative remainder can round up to m.
    if r >= m || r < 0.0 { 0.0 } else { r }
}

/// `e^x`. Range reduction by powers of two of `ln 2`, then a Taylor series, in `f64`.
pub fn exp(x: f32) -> f32 {
    if x.is_nan() {
        return f32::NAN;
    }
    if x > 88.8 {
        return f32::INFINITY;
    }
    if x < -104.0 {
        return 0.0;
    }
    let x = x as f64;
    let ln2 = core::f64::consts::LN_2;
    let k = if x >= 0.0 { (x / ln2 + 0.5) as i64 } else { (x / ln2 - 0.5) as i64 };
    let r = x - k as f64 * ln2;
    let mut term = 1.0f64;
    let mut sum = 1.0f64;
    for n in 1..20 {
        term *= r / n as f64;
        sum += term;
    }
    // 2^k by the exponent bits (k is within the f64 normal range here).
    (sum * f64::from_bits(((1023 + k) as u64) << 52)) as f32
}

/// `atan(x)` for `x` in `[0, 1]`, in `f64`.
fn atan_unit(x: f64) -> f64 {
    // atan x = pi/6 + atan((x sqrt3 - 1) / (x + sqrt3)) moves x into |x| <= tan(pi/12).
    let sqrt3 = 1.732_050_807_568_877_2;
    let (base, y) = if x > 0.267_949_192_431_122_7 {
        (core::f64::consts::FRAC_PI_6, (x * sqrt3 - 1.0) / (x + sqrt3))
    } else {
        (0.0, x)
    };
    let y2 = y * y;
    let mut term = y;
    let mut sum = 0.0;
    for n in 0..24 {
        sum += term / (2 * n + 1) as f64;
        term *= -y2;
    }
    base + sum
}

/// `atan2(y, x)` in radians, in `(-pi, pi]`.
pub fn atan2(y: f32, x: f32) -> f32 {
    let (y, x) = (y as f64, x as f64);
    if x == 0.0 && y == 0.0 {
        return 0.0;
    }
    let (ay, ax) = (y.abs(), x.abs());
    let a = if ay <= ax { atan_unit(ay / ax) } else { core::f64::consts::FRAC_PI_2 - atan_unit(ax / ay) };
    let a = if x < 0.0 { core::f64::consts::PI - a } else { a };
    (if y < 0.0 { -a } else { a }) as f32
}

/// `acos(x)` for x in [−1, 1] (clamped), radians.
pub fn acos(x: f32) -> f32 {
    let x = x.clamp(-1.0, 1.0);
    atan2(sqrt((1.0 - x * x).max(0.0)), x)
}

/// The rotation by angle `|r|` about the axis `r / |r|` (transforms.md 6); the identity for `|r| <= 0.0001`.
pub fn axis_angle(r: Vec3) -> Mat3 {
    let a = length(r);
    if a <= 0.0001 {
        return MAT3_IDENTITY;
    }
    let [x, y, z] = scale(r, 1.0 / a);
    let (s, k) = sin_cos(a);
    let t = 1.0 - k;
    [
        [t * x * x + k, t * x * y + s * z, t * x * z - s * y],
        [t * x * y - s * z, t * y * y + k, t * y * z + s * x],
        [t * x * z + s * y, t * y * z - s * x, t * z * z + k],
    ]
}

/// The rotation vector (axis times angle, angle in `[0, pi]`) of a rotation matrix.
pub fn rotation_vector(m: &Mat3) -> Vec3 {
    // Columns: m[c][r]. sin(angle) * axis = (R21 - R12, R02 - R20, R10 - R01) / 2 in row-column notation.
    let v = [(m[1][2] - m[2][1]) * 0.5, (m[2][0] - m[0][2]) * 0.5, (m[0][1] - m[1][0]) * 0.5];
    let c = ((m[0][0] + m[1][1] + m[2][2]) - 1.0) * 0.5;
    let s = length(v);
    let angle = atan2(s, c);
    if s > 1e-4 {
        return scale(v, angle / s);
    }
    if c > 0.0 {
        // Near the identity: angle ~ s.
        return v;
    }
    // Near a half turn: the axis is the column of R + I with the largest norm.
    let cols =
        [[m[0][0] + 1.0, m[0][1], m[0][2]], [m[1][0], m[1][1] + 1.0, m[1][2]], [m[2][0], m[2][1], m[2][2] + 1.0]];
    let best = cols.iter().copied().fold([0.0f32; 3], |b, c| if length_sq(c) > length_sq(b) { c } else { b });
    scale(normalize(best), angle)
}

/// The nearest-ish orthonormal matrix (Gram-Schmidt on the columns, keeping the first column's direction).
pub fn orthonormalize(m: &Mat3) -> Mat3 {
    let x = normalize(m[0]);
    let y = normalize(sub(m[1], scale(x, dot(x, m[1]))));
    let z = cross(x, y);
    [x, y, z]
}

/// Clamps `x` to `[lo, hi]`.
#[inline]
pub fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    x.max(lo).min(hi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqrt_matches_std() {
        for i in 0..20_000u32 {
            let x = f32::from_bits(0x0080_0000 + i * 104_729);
            assert_eq!(sqrt(x), x.sqrt(), "{x}");
        }
        for x in [0.0f32, 1.0, 2.0, 1e-30, 3.0e38, 0.25] {
            assert_eq!(sqrt(x), x.sqrt());
        }
        assert!(sqrt(-1.0).is_nan());
    }

    #[test]
    fn sin_cos_stay_finite_for_huge_angles() {
        for x in [1e7f32, 3.0e9, -7.5e15, 1e30, f32::MAX, -f32::MAX] {
            let (s, c) = sin_cos(x);
            assert!(s.is_finite() && c.is_finite(), "{x}");
            assert!((s * s + c * c - 1.0).abs() < 1e-5, "{x}: {s} {c}");
        }
        // Accurate where an f32 angle still has fractional precision.
        let x = 1_000_000.5f32;
        assert!((sin(x) - (x as f64).sin() as f32).abs() < 1e-5);
    }

    #[test]
    fn sin_cos_match_std() {
        let mut x = -100.0f32;
        while x < 100.0 {
            let (s, c) = sin_cos(x);
            assert!((s - x.sin()).abs() <= 1e-6, "sin {x}");
            assert!((c - x.cos()).abs() <= 1e-6, "cos {x}");
            x += 0.0137;
        }
    }

    #[test]
    fn matrix_inverse() {
        let m = [[2.0, 0.5, 0.0], [0.0, 1.0, 0.3], [0.1, 0.0, 3.0]];
        let i = inverse(&m).unwrap();
        let p = mat_mul(&m, &i);
        for (a, b) in p.iter().flatten().zip(MAT3_IDENTITY.iter().flatten()) {
            assert!((a - b).abs() < 1e-6);
        }
        assert!(inverse(&[[0.0; 3]; 3]).is_none());
        assert_eq!(rem_euclid(-0.25, 1.0), 0.75);
        assert_eq!(rem_euclid(2.5, 1.0), 0.5);
    }

    #[test]
    fn exp_atan2_and_rotation_vectors() {
        let mut x = -20.0f32;
        while x < 20.0 {
            let e = exp(x);
            assert!((e - x.exp()).abs() <= 2e-7 * x.exp(), "exp {x}");
            x += 0.173;
        }
        for (y, x) in [(1.0f32, 2.0f32), (-3.0, 0.5), (0.2, -4.0), (-1.0, -1.0), (0.0, -1.0), (5.0, 0.0)] {
            assert!((atan2(y, x) - y.atan2(x)).abs() < 1e-6, "{y} {x}");
        }
        for r in [[0.3f32, -0.2, 0.9], [0.0, 0.0, 3.1], [1.0, 1.0, 0.0], [0.0, 1e-6, 0.0]] {
            let back = rotation_vector(&axis_angle(r));
            let want = if length(r) <= 0.0001 { [0.0; 3] } else { r };
            assert!(length(sub(back, want)) < 2e-4, "{r:?} -> {back:?}");
        }
        let m = orthonormalize(&[[1.0, 0.01, 0.0], [0.0, 1.0, 0.02], [0.0, 0.0, 1.0]]);
        assert!((determinant(&m) - 1.0).abs() < 1e-6);
        let a = Affine { cols: m, t: [1.0, 2.0, 3.0] };
        let r = a.mul(&affine_inverse(&a).unwrap());
        assert!(r.t.iter().all(|v| v.abs() < 1e-5));
    }
}
