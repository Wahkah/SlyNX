//! The few floating-point functions the crate needs, written out because `core` has no `exp2`, `sin` or `sqrt` on
//! the targets we build for (`no_std`). All are computed in `f64` and are deterministic on every platform.

const LN2: f64 = core::f64::consts::LN_2;
const PI: f64 = core::f64::consts::PI;

/// Largest integer not above `x` (for `|x| < 2^62`).
pub fn floor(x: f64) -> f64 {
    let t = x as i64 as f64;
    if t > x { t - 1.0 } else { t }
}

/// `2^x`, relative error below 1e-15 for `|x| < 1000`.
pub fn exp2(x: f64) -> f64 {
    if x.is_nan() {
        return x;
    }
    let x = x.clamp(-1000.0, 1000.0);
    let i = floor(x);
    let f = (x - i) * LN2; // 0 <= f < ln 2
    // Taylor series of e^f; 18 terms are far below f64 precision for f < 0.7.
    let mut term = 1.0;
    let mut sum = 1.0;
    for n in 1..18 {
        term *= f / n as f64;
        sum += term;
    }
    let mut scale = 1.0;
    let mut e = i as i32;
    let base = if e < 0 { 0.5 } else { 2.0 };
    e = e.abs();
    let mut b = base;
    while e > 0 {
        if e & 1 != 0 {
            scale *= b;
        }
        b *= b;
        e >>= 1;
    }
    sum * scale
}

/// Frequency ratio of `semitones`: `2^(semitones / 12)`.
pub fn semitone_ratio(semitones: f64) -> f64 {
    exp2(semitones / 12.0)
}

/// Sine of an angle in degrees.
pub fn sin_deg(deg: f64) -> f64 {
    sin(deg * (PI / 180.0))
}

/// Sine of an angle in radians.
pub fn sin(x: f64) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    // Reduce to -pi..pi, then to -pi/2..pi/2 using sin(pi - x) = sin x.
    let two_pi = 2.0 * PI;
    let mut r = x - two_pi * floor(x / two_pi + 0.5);
    if r > PI / 2.0 {
        r = PI - r;
    } else if r < -PI / 2.0 {
        r = -PI - r;
    }
    let r2 = r * r;
    let mut term = r;
    let mut sum = r;
    for n in 1..12 {
        term *= -r2 / ((2 * n) as f64 * (2 * n + 1) as f64);
        sum += term;
    }
    sum
}

/// Square root (Newton's method from a bit-level first guess). Negative input gives 0.
pub fn sqrt(x: f64) -> f64 {
    if x <= 0.0 || x.is_nan() {
        return 0.0;
    }
    if x.is_infinite() {
        return x;
    }
    let mut y = f64::from_bits((x.to_bits() >> 1) + (0x3FF0_0000_0000_0000 >> 1));
    for _ in 0..6 {
        y = 0.5 * (y + x / y);
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp2_matches_known_values() {
        assert_eq!(exp2(0.0), 1.0);
        assert!((exp2(1.0) - 2.0).abs() < 1e-14);
        assert!((exp2(-3.0) - 0.125).abs() < 1e-15);
        assert!((exp2(0.5) - core::f64::consts::SQRT_2).abs() < 1e-14);
        assert!((semitone_ratio(12.0) - 2.0).abs() < 1e-14);
        assert!((semitone_ratio(-24.0) - 0.25).abs() < 1e-15);
        assert!((exp2(10.25) - 1217.7480857627863).abs() < 1e-9);
    }

    #[test]
    fn sin_and_sqrt() {
        assert!(sin_deg(0.0).abs() < 1e-15);
        assert!((sin_deg(90.0) - 1.0).abs() < 1e-14);
        assert!((sin_deg(-90.0) + 1.0).abs() < 1e-14);
        assert!((sin_deg(30.0) - 0.5).abs() < 1e-14);
        assert!((sin_deg(359.0) + sin_deg(1.0)).abs() < 1e-14);
        assert!((sin_deg(720.0 + 45.0) - core::f64::consts::FRAC_1_SQRT_2).abs() < 1e-13);
        assert!((sqrt(2.0) - core::f64::consts::SQRT_2).abs() < 1e-15);
        assert!((sqrt(1e10) - 1e5).abs() < 1e-9);
        assert_eq!(sqrt(-1.0), 0.0);
        assert_eq!(floor(-1.5), -2.0);
        assert_eq!(floor(2.0), 2.0);
    }
}
