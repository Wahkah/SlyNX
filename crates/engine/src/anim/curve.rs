//! Curve evaluation (animation.md sections 2 and 3) and the Euler rotation of section 4.1.
//!
//! Times are in seconds. Key times are stored as 1/60 s frames and converted with [`key_time`].

use formats::anim::{
    BezierKey, FRAMES_PER_SECOND, HermiteKey, LinearKey, ScalarCurve, TANGENT_STEP, VectorCurve, VectorKey,
};

use crate::math::{Mat3, Vec3, sin_cos};

/// Below this, a key interval or an offset into it counts as zero (animation.md 2.1, 2.4).
pub const EPSILON: f32 = 0.0001;

/// Number of bisection steps that find the Bezier parameter (animation.md 2.4).
pub const BEZIER_STEPS: u32 = 16;

/// Seconds of a key's frame.
#[inline]
pub fn key_time(frame: i16) -> f32 {
    frame as f32 / FRAMES_PER_SECOND
}

/// The segment of a key list that time `t` falls in (animation.md 2.1): `k` is the largest index with `t_k <= t`,
/// limited to `0..=n-2`. Returns `(k, delta, tau)`. Needs at least two keys.
#[inline]
fn locate(n: usize, time_of: impl Fn(usize) -> f32, t: f32) -> (usize, f32, f32) {
    // Binary search for the first key strictly after t, then step back.
    let (mut lo, mut hi) = (0usize, n);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if time_of(mid) <= t {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let k = lo.saturating_sub(1).min(n - 2);
    let tk = time_of(k);
    (k, time_of(k + 1) - tk, t - tk)
}

/// Value and derivative (per second) of a scalar curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Value.
    pub value: f32,
    /// Derivative with respect to time, units per second.
    pub rate: f32,
}

#[inline]
fn hermite_basis(u: f32) -> (f32, f32, f32, f32) {
    let v = 1.0 - u;
    (v * v * v + 3.0 * u * v * v, u * v * v, u * u * v, u * u * u + 3.0 * u * u * v)
}

/// The cubic Hermite value of 2.3 / 3.2 for one component.
#[inline]
fn hermite(g0: f32, m_out: f32, m_in: f32, g1: f32, delta: f32, u: f32) -> f32 {
    let (h0, h1, h2, h3) = hermite_basis(u);
    h0 * g0 + h1 * delta * m_out - h2 * delta * m_in + h3 * g1
}

/// The derivative of 2.3 for one component.
#[inline]
fn hermite_rate(g0: f32, m_out: f32, m_in: f32, g1: f32, delta: f32, u: f32) -> f32 {
    let v = 1.0 - u;
    6.0 * u * v / delta * (g1 - g0) + (1.0 - 4.0 * u + 3.0 * u * u) * m_out + (3.0 * u * u - 2.0 * u) * m_in
}

fn eval_hermite(keys: &[HermiteKey], t: f32) -> Option<Sample> {
    match keys.len() {
        0 => return None,
        1 => return Some(Sample { value: keys[0].value, rate: 0.0 }),
        _ => {}
    }
    let (k, delta, tau) = locate(keys.len(), |i| key_time(keys[i].frame), t);
    let (a, b) = (&keys[k], &keys[k + 1]);
    if a.kind_out == TANGENT_STEP || delta < EPSILON {
        return Some(Sample { value: a.value, rate: 0.0 });
    }
    let u = tau / delta;
    Some(Sample {
        value: hermite(a.value, a.slope_out, b.slope_in, b.value, delta, u),
        rate: hermite_rate(a.value, a.slope_out, b.slope_in, b.value, delta, u),
    })
}

/// `T(w)` of 2.4: the Bezier's time at parameter `w`, with control times 0, `p1`, `p2`, `delta`.
#[inline]
fn bezier_time(p1: f32, p2: f32, delta: f32, w: f32) -> f32 {
    let v = 1.0 - w;
    3.0 * p1 * w * v * v + 3.0 * p2 * w * w * v + delta * w * w * w
}

/// The Bezier parameter whose time is `tau`, by the 16-step bisection of 2.4.
pub fn bezier_parameter(p1: f32, p2: f32, delta: f32, tau: f32) -> f32 {
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    let mut step = 0.5f32;
    for _ in 0..BEZIER_STEPS {
        let m = lo + step;
        let tm = bezier_time(p1, p2, delta, m);
        if tm < tau {
            lo = m;
        } else if tm > tau {
            hi = m;
        } else {
            lo = m;
            hi = m;
            break;
        }
        step *= 0.5;
    }
    (lo + hi) * 0.5
}

fn eval_bezier(keys: &[BezierKey], t: f32) -> Option<Sample> {
    match keys.len() {
        0 => return None,
        1 => return Some(Sample { value: keys[0].value, rate: 0.0 }),
        _ => {}
    }
    let (k, delta, tau) = locate(keys.len(), |i| key_time(keys[i].frame), t);
    let (a, b) = (&keys[k], &keys[k + 1]);
    if a.kind_out == TANGENT_STEP || delta < EPSILON || tau < EPSILON {
        return Some(Sample { value: a.value, rate: 0.0 });
    }
    if delta - tau < EPSILON {
        return Some(Sample { value: b.value, rate: 0.0 });
    }
    let p1 = a.len_out;
    let p2 = delta - b.len_in;
    let w = bezier_parameter(p1, p2, delta, tau);
    let (g0, g1, g2, g3) = (a.value, a.value + a.rise_out, b.value - b.drop_in, b.value);
    let v = 1.0 - w;
    let value = v * v * v * g0 + 3.0 * w * v * v * g1 + 3.0 * w * w * v * g2 + w * w * w * g3;
    // dG/dw over dT/dw.
    let dg = 3.0 * (v * v * (g1 - g0) + 2.0 * w * v * (g2 - g1) + w * w * (g3 - g2));
    let dt = 3.0 * (v * v * p1 + 2.0 * w * v * (p2 - p1) + w * w * (delta - p2));
    let rate = if dt.abs() > 1e-12 { dg / dt } else { 0.0 };
    Some(Sample { value, rate })
}

fn eval_linear(keys: &[LinearKey], t: f32) -> Option<Sample> {
    match keys.len() {
        0 => return None,
        1 => return Some(Sample { value: keys[0].value, rate: 0.0 }),
        _ => {}
    }
    let (k, delta, tau) = locate(keys.len(), |i| key_time(keys[i].frame), t);
    let (a, b) = (keys[k].value, keys[k + 1].value);
    if delta < EPSILON {
        return Some(Sample { value: a, rate: 0.0 });
    }
    let u = tau / delta;
    Some(Sample { value: (1.0 - u) * a + u * b, rate: (b - a) / delta })
}

/// Value and rate of a scalar curve at `t`, or `None` for the blend placeholders (kinds 3 and 4, whose value a
/// run-time blend supplies) and for curves without keys.
pub fn sample_scalar(curve: &ScalarCurve, t: f32) -> Option<Sample> {
    match curve {
        ScalarCurve::Hermite(k) => eval_hermite(k, t),
        ScalarCurve::Bezier(k) => eval_bezier(k, t),
        ScalarCurve::Linear(k) => eval_linear(k, t),
        ScalarCurve::TwistBlend | ScalarCurve::PoseBlend => None,
    }
}

/// The value of a scalar curve at `t` (see [`sample_scalar`]).
pub fn eval_scalar(curve: &ScalarCurve, t: f32) -> Option<f32> {
    sample_scalar(curve, t).map(|s| s.value)
}

/// Value and rate of a vector curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VectorSample {
    /// Value.
    pub value: Vec3,
    /// Derivative with respect to time, units per second.
    pub rate: Vec3,
}

fn eval_vector_hermite(keys: &[VectorKey], t: f32) -> Option<VectorSample> {
    match keys.len() {
        0 => return None,
        1 => return Some(VectorSample { value: keys[0].value, rate: [0.0; 3] }),
        _ => {}
    }
    let (k, delta, tau) = locate(keys.len(), |i| key_time(keys[i].frame), t);
    let (a, b) = (&keys[k], &keys[k + 1]);
    if delta < EPSILON {
        return Some(VectorSample { value: a.value, rate: [0.0; 3] });
    }
    let u = tau / delta;
    let mut out = VectorSample { value: [0.0; 3], rate: [0.0; 3] };
    for c in 0..3 {
        out.value[c] = hermite(a.value[c], a.tangent_out[c], b.tangent_in[c], b.value[c], delta, u);
        out.rate[c] = hermite_rate(a.value[c], a.tangent_out[c], b.tangent_in[c], b.value[c], delta, u);
    }
    Some(out)
}

/// Value and rate of a vector curve at `t`, or `None` for the blend placeholder (kind 2) and curves without keys.
/// Per-component curves keep the base value (rate 0) in components without a curve or with a blend placeholder.
pub fn sample_vector(curve: &VectorCurve, t: f32) -> Option<VectorSample> {
    match curve {
        VectorCurve::Components { base, curves, .. } => {
            let mut out = VectorSample { value: *base, rate: [0.0; 3] };
            for (c, curve) in curves.iter().enumerate() {
                if let Some(s) = curve.as_ref().and_then(|cv| sample_scalar(cv, t)) {
                    out.value[c] = s.value;
                    out.rate[c] = s.rate;
                }
            }
            Some(out)
        }
        VectorCurve::Hermite(k) => eval_vector_hermite(k, t),
        VectorCurve::Blend => None,
    }
}

/// The value of a vector curve at `t` (see [`sample_vector`]).
pub fn eval_vector(curve: &VectorCurve, t: f32) -> Option<Vec3> {
    sample_vector(curve, t).map(|s| s.value)
}

/// The rotation `Rz(z) · Ry(y) · Rx(x)` of Euler angles (x, y, z) in radians, as columns (animation.md 4.1).
pub fn euler_zyx(angles: Vec3) -> Mat3 {
    let (sx, cx) = sin_cos(angles[0]);
    let (sy, cy) = sin_cos(angles[1]);
    let (sz, cz) = sin_cos(angles[2]);
    [
        [cy * cz, cy * sz, -sy],
        [sx * sy * cz - cx * sz, sx * sy * sz + cx * cz, sx * cy],
        [cx * sy * cz + sx * sz, cx * sy * sz - sx * cz, cx * cy],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::{MAT3_IDENTITY, mat_mul};
    use alloc::vec;

    fn hk(frame: i16, value: f32, kin: i8, kout: i8, sin: f32, sout: f32) -> HermiteKey {
        HermiteKey { frame, value, kind_in: kin, kind_out: kout, slope_in: sin, slope_out: sout }
    }

    #[test]
    fn hermite_vector_of_the_spec() {
        // animation.md 9, segment 0x251A0B: z rotation at t = 0.5 s is 3.25423.
        let c = ScalarCurve::Hermite(vec![
            hk(0, 0.0, 2, 2, -9.42478, 7.50489),
            hk(60, 7.50492, 1, 1, 11.49073, 11.49073),
            hk(180, 50.68658, 1, 1, 40.93951, 40.93951),
        ]);
        let v = eval_scalar(&c, 0.5).unwrap();
        assert!((v - 3.25423).abs() < 2e-5, "{v}");
        let r = euler_zyx([0.0, 0.0, v]);
        let want = [[-0.99366, -0.11240, 0.0], [0.11240, -0.99366, 0.0], [0.0, 0.0, 1.0]];
        for (a, b) in r.iter().flatten().zip(want.iter().flatten()) {
            assert!((a - b).abs() < 2e-5);
        }
        // Ends of the segments and the derivative against a finite difference.
        assert_eq!(eval_scalar(&c, 0.0), Some(0.0));
        assert!((eval_scalar(&c, 1.0).unwrap() - 7.50492).abs() < 1e-5);
        let s = sample_scalar(&c, 1.7).unwrap();
        let h = 1e-3;
        let fd = (eval_scalar(&c, 1.7 + h).unwrap() - eval_scalar(&c, 1.7 - h).unwrap()) / (2.0 * h);
        assert!((s.rate - fd).abs() < 1e-2 * fd.abs().max(1.0), "{} {}", s.rate, fd);
    }

    #[test]
    fn weighted_bezier_vector_of_the_spec() {
        // animation.md 9, segment 0x265546. The handle lengths are stored as whole frames (5/60, 10/60 s); the
        // spec's values follow its 16-step bisection, which ends 2^-17 away from the exact parameter here.
        let c = ScalarCurve::Bezier(vec![
            BezierKey {
                frame: 0,
                value: 0.0,
                kind_in: 3,
                kind_out: 3,
                len_in: 10.0 / 60.0,
                drop_in: 0.0,
                len_out: 5.0 / 60.0,
                rise_out: 0.0,
            },
            BezierKey {
                frame: 15,
                value: 2.85681,
                kind_in: 4,
                kind_out: 4,
                len_in: 5.0 / 60.0,
                drop_in: -0.12084,
                len_out: 0.22222,
                rise_out: -0.32224,
            },
        ]);
        let a = eval_scalar(&c, 0.0625).unwrap();
        let b = eval_scalar(&c, 0.125).unwrap();
        assert!((a - 0.46334).abs() < 4e-5, "{a}");
        assert!((b - 1.47372).abs() < 4e-5, "{b}");
        assert_eq!(eval_scalar(&c, 0.00001), Some(0.0));
        assert_eq!(eval_scalar(&c, 0.25), Some(2.85681));
    }

    #[test]
    fn vector_hermite_vector_of_the_spec() {
        // animation.md 9, segment 0x27AC4A.
        let t0 = [-34.1395, 72.7469, -155.2521];
        let p1 = [-53.6977, -14.7839, -20.2897];
        let c = VectorCurve::Hermite(vec![
            VectorKey { frame: 0, value: [-38.9039, -21.6148, 12.2804], tangent_in: t0, tangent_out: t0 },
            VectorKey { frame: 25, value: p1, tangent_in: [0.0; 3], tangent_out: [0.0; 3] },
            VectorKey { frame: 90, value: p1, tangent_in: [0.0; 3], tangent_out: [0.0; 3] },
        ]);
        let close = |a: Vec3, b: Vec3| a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 2e-4);
        assert!(close(eval_vector(&c, 0.0).unwrap(), [-38.9039, -21.6148, 12.2804]));
        assert!(close(eval_vector(&c, 0.208333).unwrap(), [-48.0789, -14.4105, -12.0907]));
        assert!(close(eval_vector(&c, 0.958333).unwrap(), p1));
    }

    #[test]
    fn step_key_holds() {
        // animation.md 9, segment 0x3B7332: the key at frame 180 holds 203.81694 until frame 181.
        let c = ScalarCurve::Hermite(vec![
            hk(0, 0.0, 1, 1, 0.0, 0.0),
            hk(180, 203.81694, 1, TANGENT_STEP, 0.0, 0.0),
            hk(181, 300.0, 1, 1, 0.0, 0.0),
            hk(530, 300.0, 1, 1, 0.0, 0.0),
        ]);
        assert_eq!(eval_scalar(&c, 3.00833), Some(203.81694));
        assert_eq!(eval_scalar(&c, 181.0 / 60.0), Some(300.0));
        // At the last key the last segment is used, so a step there holds the previous value (2.1, 2.3).
        let held = ScalarCurve::Hermite(vec![hk(0, 1.0, 1, TANGENT_STEP, 0.0, 0.0), hk(60, 2.0, 1, 1, 0.0, 0.0)]);
        assert_eq!(eval_scalar(&held, 1.0), Some(1.0));
    }

    #[test]
    fn linear_and_degenerate_keys() {
        let c = ScalarCurve::Linear(vec![LinearKey { frame: 0, value: 1.0 }, LinearKey { frame: 30, value: 3.0 }]);
        let s = sample_scalar(&c, 0.25).unwrap();
        assert_eq!((s.value, s.rate), (2.0, 4.0));
        // Two keys at the same time: use the first value.
        let c = ScalarCurve::Linear(vec![
            LinearKey { frame: 0, value: 1.0 },
            LinearKey { frame: 10, value: 5.0 },
            LinearKey { frame: 10, value: 9.0 },
        ]);
        assert_eq!(eval_scalar(&c, 10.0 / 60.0), Some(5.0));
        assert_eq!(eval_scalar(&ScalarCurve::TwistBlend, 0.0), None);
        let comp = VectorCurve::Components { mask: 2, base: [1.0, 2.0, 3.0], curves: [None, Some(c), None] };
        assert_eq!(eval_vector(&comp, 0.0), Some([1.0, 1.0, 3.0]));
    }

    #[test]
    fn euler_order_is_z_y_x() {
        let (x, y, z) = (0.3f32, -0.7f32, 1.1f32);
        let rx = [[1.0, 0.0, 0.0], [0.0, x.cos(), x.sin()], [0.0, -x.sin(), x.cos()]];
        let ry = [[y.cos(), 0.0, -y.sin()], [0.0, 1.0, 0.0], [y.sin(), 0.0, y.cos()]];
        let rz = [[z.cos(), z.sin(), 0.0], [-z.sin(), z.cos(), 0.0], [0.0, 0.0, 1.0]];
        let want = mat_mul(&rz, &mat_mul(&ry, &rx));
        let got = euler_zyx([x, y, z]);
        for (a, b) in got.iter().flatten().zip(want.iter().flatten()) {
            assert!((a - b).abs() < 1e-6);
        }
        assert_eq!(euler_zyx([0.0; 3]), MAT3_IDENTITY);
    }
}
