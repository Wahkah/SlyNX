//! Small float helpers for `no_std` that give the same bits on every platform (no libm).

/// Absolute value.
pub fn abs(x: f32) -> f32 {
    f32::from_bits(x.to_bits() & 0x7FFF_FFFF)
}

/// Rounds toward zero.
pub fn trunc(x: f32) -> f32 {
    // Every f32 of magnitude 2^23 or more is already an integer; NaN and infinities fall through unchanged.
    if abs(x) < 8_388_608.0 { x as i32 as f32 } else { x }
}

/// Rounds toward minus infinity.
pub fn floor(x: f32) -> f32 {
    let t = trunc(x);
    if t > x { t - 1.0 } else { t }
}

fn floor64(x: f64) -> f64 {
    if x.is_nan() || x.abs() >= 4_503_599_627_370_496.0 {
        return x;
    }
    let t = x as i64 as f64;
    if t > x { t - 1.0 } else { t }
}

/// The angle of (x, y) from the +X axis, in -pi..=pi (0 for the origin).
pub fn atan2(y: f32, x: f32) -> f32 {
    use core::f32::consts::{FRAC_PI_2, FRAC_PI_6, PI};
    if x == 0.0 && y == 0.0 {
        return 0.0;
    }
    let (ax, ay) = (abs(x), abs(y));
    let (t, swapped) = if ay > ax { (ax / ay, true) } else { (ay / ax, false) };
    // atan on [0, 1]: above tan(pi/12), shift by pi/6 to keep the series argument small.
    const TAN_PI_12: f32 = 0.267_949_2;
    const SQRT3: f32 = 1.732_050_8;
    let (t, base) = if t > TAN_PI_12 { ((t * SQRT3 - 1.0) / (t + SQRT3), FRAC_PI_6) } else { (t, 0.0) };
    let t2 = t * t;
    let series = t * (1.0 - t2 * (1.0 / 3.0 - t2 * (1.0 / 5.0 - t2 * (1.0 / 7.0 - t2 / 9.0))));
    let mut r = base + series;
    if swapped {
        r = FRAC_PI_2 - r;
    }
    if x < 0.0 {
        r = PI - r;
    }
    if y < 0.0 { -r } else { r }
}

/// Square root (NaN for negative input).
pub fn sqrt(x: f32) -> f32 {
    if x.is_nan() || x < 0.0 {
        return f32::NAN;
    }
    if x == 0.0 || x.is_infinite() {
        return x;
    }
    let xd = x as f64;
    let mut y = f32::from_bits((x.to_bits() >> 1) + 0x1FBD_1DF5) as f64;
    for _ in 0..5 {
        y = 0.5 * (y + xd / y);
    }
    y as f32
}

/// Sine and cosine of `x` radians.
pub fn sin_cos(x: f32) -> (f32, f32) {
    if !x.is_finite() {
        return (f32::NAN, f32::NAN);
    }
    const HALF_PI: f64 = core::f64::consts::FRAC_PI_2;
    let x = x as f64;
    let q = floor64(x / HALF_PI + 0.5);
    let r = x - q * HALF_PI;
    let r2 = r * r;
    // Taylor series to degree 15 / 14; |r| <= pi/4 keeps the error far below f32 precision.
    let mut s = 0.0;
    let mut c = 0.0;
    let mut term_s = r;
    let mut term_c = 1.0;
    for n in 0..8 {
        s += term_s;
        c += term_c;
        let k = (2 * n + 2) as f64;
        term_s *= -r2 / (k * (k + 1.0));
        term_c *= -r2 / ((k - 1.0) * k);
    }
    let (s, c) = (s as f32, c as f32);
    match (q as i64).rem_euclid(4) {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// Dot product of two 3-vectors.
pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Cross product of two 3-vectors.
pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// Length of a 3-vector.
pub fn length(a: [f32; 3]) -> f32 {
    sqrt(dot(a, a))
}

/// `a - b`.
pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// `a * s`.
pub fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// The unit vector along `a`, or zero for a zero vector.
pub fn normalize(a: [f32; 3]) -> [f32; 3] {
    let l = length(a);
    if l > 0.0 && l.is_finite() { scale(a, 1.0 / l) } else { [0.0; 3] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_std() {
        for i in -2000..2000 {
            let x = i as f32 * 0.0137;
            let (s, c) = sin_cos(x);
            assert!((s - x.sin()).abs() < 1e-6, "sin {x}");
            assert!((c - x.cos()).abs() < 1e-6, "cos {x}");
            assert_eq!(floor(x), x.floor());
            assert_eq!(trunc(x), x.trunc());
            let y = (i as f32).abs() * 3.7;
            assert!((sqrt(y) - y.sqrt()).abs() <= y.sqrt() * 1e-6, "sqrt {y}");
        }
        assert!(sqrt(-1.0).is_nan());
        assert_eq!(floor(-0.5), -1.0);
        assert_eq!(floor(1e30), 1e30);
    }
}
