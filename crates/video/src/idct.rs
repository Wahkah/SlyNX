//! 8 x 8 inverse DCT in integer arithmetic, accurate to the requirements of ISO/IEC 13818-2 Annex A (the
//! IEEE 1180-1990 test), checked by the unit tests against a double-precision IDCT.
//!
//! The 2-D transform is two passes of a 1-D transform split into even and odd halves. Each pass works on eight
//! columns at once (one row of the block is a lane vector), so the compiler turns it into SIMD code on both x86-64
//! and AArch64 without intrinsics.

/// Fractional bits of the coefficients: `C[k] = round(2^13 * cos(k pi / 16) / 2)`.
const CBITS: u32 = 13;
const C1: i32 = 4017;
const C2: i32 = 3784;
const C3: i32 = 3406;
const C4: i32 = 2896;
const C5: i32 = 2276;
const C6: i32 = 1567;
const C7: i32 = 799;

/// Extra fractional bits kept between the two passes.
const PASS_BITS: u32 = 4;

type Rows = [[i32; 8]; 8];

/// One 1-D IDCT down the columns: `v[u]` holds frequency `u` for 8 lanes; returns spatial rows, scaled down by
/// `shift` bits with rounding.
#[inline(always)]
fn pass(v: &Rows, shift: u32) -> Rows {
    let rnd = 1i32 << (shift - 1);
    let mut out = [[0i32; 8]; 8];
    for l in 0..8 {
        let (x0, x1, x2, x3) = (v[0][l], v[1][l], v[2][l], v[3][l]);
        let (x4, x5, x6, x7) = (v[4][l], v[5][l], v[6][l], v[7][l]);
        let a0 = (x0 + x4) * C4;
        let a1 = (x0 - x4) * C4;
        let b0 = x2 * C2 + x6 * C6;
        let b1 = x2 * C6 - x6 * C2;
        let e0 = a0 + b0 + rnd;
        let e3 = a0 - b0 + rnd;
        let e1 = a1 + b1 + rnd;
        let e2 = a1 - b1 + rnd;
        let o0 = x1 * C1 + x3 * C3 + x5 * C5 + x7 * C7;
        let o1 = x1 * C3 - x3 * C7 - x5 * C1 - x7 * C5;
        let o2 = x1 * C5 - x3 * C1 + x5 * C7 + x7 * C3;
        let o3 = x1 * C7 - x3 * C5 + x5 * C3 - x7 * C1;
        out[0][l] = (e0 + o0) >> shift;
        out[7][l] = (e0 - o0) >> shift;
        out[1][l] = (e1 + o1) >> shift;
        out[6][l] = (e1 - o1) >> shift;
        out[2][l] = (e2 + o2) >> shift;
        out[5][l] = (e2 - o2) >> shift;
        out[3][l] = (e3 + o3) >> shift;
        out[4][l] = (e3 - o3) >> shift;
    }
    out
}

#[inline(always)]
fn transpose(m: &Rows) -> Rows {
    let mut t = [[0i32; 8]; 8];
    for (r, row) in m.iter().enumerate() {
        for (c, &v) in row.iter().enumerate() {
            t[c][r] = v;
        }
    }
    t
}

/// Inverse DCT of `block` (coefficients in raster order, each in -2048..=2047) in place; the results are saturated
/// to -256..=255.
pub fn idct(block: &mut [i16; 64]) {
    let mut v = [[0i32; 8]; 8];
    for (row, src) in v.iter_mut().zip(block.as_chunks::<8>().0) {
        for (d, &s) in row.iter_mut().zip(src) {
            *d = i32::from(s);
        }
    }
    // Vertical pass (columns are lanes), then the horizontal pass on the transpose.
    let cols = pass(&v, CBITS - PASS_BITS);
    let rows = pass(&transpose(&cols), CBITS + PASS_BITS);
    let out = transpose(&rows);
    for (dst, row) in block.as_chunks_mut::<8>().0.iter_mut().zip(out.iter()) {
        for (d, &s) in dst.iter_mut().zip(row) {
            *d = s.clamp(-256, 255) as i16;
        }
    }
}

/// The IDCT of a block whose only non-zero coefficient is the DC term: every sample is `dc / 8`, rounded.
#[inline(always)]
pub fn idct_dc(dc: i16) -> i16 {
    ((i32::from(dc) + 4) >> 3).clamp(-256, 255) as i16
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    fn cos_table() -> [[f64; 8]; 8] {
        // c[x][u] = C(u)/2 cos((2x+1) u pi / 16)
        let mut c = [[0.0; 8]; 8];
        for (x, row) in c.iter_mut().enumerate() {
            for (u, v) in row.iter_mut().enumerate() {
                let cu = if u == 0 { core::f64::consts::FRAC_1_SQRT_2 } else { 1.0 };
                *v = cu / 2.0 * (((2 * x + 1) as f64) * (u as f64) * core::f64::consts::PI / 16.0).cos();
            }
        }
        c
    }

    fn reference_idct(c: &[[f64; 8]; 8], f: &[i16; 64]) -> [f64; 64] {
        let mut tmp = [0.0; 64];
        for y in 0..8 {
            for u in 0..8 {
                tmp[y * 8 + u] = (0..8).map(|v| c[y][v] * f64::from(f[v * 8 + u])).sum();
            }
        }
        let mut out = [0.0; 64];
        for y in 0..8 {
            for x in 0..8 {
                out[y * 8 + x] = (0..8).map(|u| c[x][u] * tmp[y * 8 + u]).sum();
            }
        }
        out
    }

    fn reference_fdct(c: &[[f64; 8]; 8], p: &[i32; 64]) -> [f64; 64] {
        let mut tmp = [0.0; 64];
        for y in 0..8 {
            for u in 0..8 {
                tmp[y * 8 + u] = (0..8).map(|x| c[x][u] * f64::from(p[y * 8 + x])).sum();
            }
        }
        let mut out = [0.0; 64];
        for v in 0..8 {
            for u in 0..8 {
                out[v * 8 + u] = (0..8).map(|y| c[y][v] * tmp[y * 8 + u]).sum();
            }
        }
        out
    }

    fn round(x: f64) -> f64 {
        // Rounds half away from zero.
        x.signum() * (x.abs() + 0.5).floor()
    }

    /// The generator of IEEE 1180-1990 (a linear congruential generator scaled into [-l, h]).
    struct Gen(u64);
    impl Gen {
        fn next(&mut self, l: i64, h: i64) -> i32 {
            self.0 = (self.0.wrapping_mul(1_103_515_245).wrapping_add(12345)) & 0xFFFF_FFFF;
            let i = (self.0 & 0x7FFF_FFFE) as f64;
            let z = i / f64::from(0x7FFF_FFFFu32);
            let x = z * (l + h + 1) as f64;
            let j = x as i64;
            let mut v = j - l;
            if v > h {
                v = h;
            }
            v as i32
        }
    }

    struct Stats {
        peak: i32,
        pmse: f64,
        omse: f64,
        pme: f64,
        ome: f64,
    }

    /// Runs the IEEE 1180 procedure for one input range and sign.
    fn run(l: i64, h: i64, sign: i32, blocks: usize) -> Stats {
        let c = cos_table();
        let mut g = Gen(1);
        let mut err = [0i64; 64];
        let mut sq = [0i64; 64];
        let mut peak = 0;
        for _ in 0..blocks {
            let mut p = [0i32; 64];
            for v in p.iter_mut() {
                *v = g.next(l, h) * sign;
            }
            let f = reference_fdct(&c, &p);
            let mut coef = [0i16; 64];
            for (d, &s) in coef.iter_mut().zip(f.iter()) {
                *d = round(s).clamp(-2048.0, 2047.0) as i16;
            }
            let reference = reference_idct(&c, &coef);
            let mut test = coef;
            idct(&mut test);
            for i in 0..64 {
                let r = round(reference[i]).clamp(-256.0, 255.0) as i32;
                let e = i32::from(test[i]) - r;
                peak = peak.max(e.abs());
                err[i] += i64::from(e);
                sq[i] += i64::from(e * e);
            }
        }
        let n = blocks as f64;
        Stats {
            peak,
            pmse: sq.iter().map(|&s| s as f64 / n).fold(0.0, f64::max),
            omse: sq.iter().sum::<i64>() as f64 / (64.0 * n),
            pme: err.iter().map(|&s| (s as f64 / n).abs()).fold(0.0, f64::max),
            ome: (err.iter().sum::<i64>() as f64 / (64.0 * n)).abs(),
        }
    }

    #[test]
    fn meets_ieee_1180_accuracy() {
        for (l, h) in [(256, 255), (5, 5), (300, 300)] {
            for sign in [1, -1] {
                let s = run(l, h, sign, 10_000);
                let what = std::format!("range -{l}..{h} sign {sign}");
                assert!(s.peak <= 1, "{what}: peak error {}", s.peak);
                assert!(s.pmse <= 0.06, "{what}: per-pixel mse {}", s.pmse);
                assert!(s.omse <= 0.02, "{what}: overall mse {}", s.omse);
                assert!(s.pme <= 0.015, "{what}: per-pixel mean error {}", s.pme);
                assert!(s.ome <= 0.0015, "{what}: overall mean error {}", s.ome);
            }
        }
    }

    #[test]
    fn zero_in_zero_out() {
        let mut b = [0i16; 64];
        idct(&mut b);
        assert_eq!(b, [0; 64]);
    }

    #[test]
    fn single_coefficients_within_one() {
        // Every position with a range of values, including the extremes, against the reference.
        let c = cos_table();
        let values: Vec<i16> = [-2048, -2047, -1000, -301, -64, -7, -1, 1, 2, 7, 63, 300, 1001, 2047].into();
        for pos in 0..64 {
            for &v in &values {
                let mut b = [0i16; 64];
                b[pos] = v;
                let r = reference_idct(&c, &b);
                idct(&mut b);
                for i in 0..64 {
                    let want = round(r[i]).clamp(-256.0, 255.0) as i32;
                    assert!((i32::from(b[i]) - want).abs() <= 1, "pos {pos} value {v}");
                }
            }
        }
    }

    #[test]
    fn dc_only_matches_reference() {
        let c = cos_table();
        for dc in -2048..2048i16 {
            let mut b = [0i16; 64];
            b[0] = dc;
            let r = reference_idct(&c, &b);
            let want = round(r[0]).clamp(-256.0, 255.0) as i16;
            assert!((idct_dc(dc) - want).abs() <= 1, "dc {dc}");
            let mut full = b;
            idct(&mut full);
            assert!((full[0] - idct_dc(dc)).abs() <= 1, "dc {dc}");
        }
    }

    #[test]
    fn extreme_blocks_do_not_overflow() {
        for v in [2047i16, -2048] {
            let mut b = [v; 64];
            idct(&mut b);
            let mut alt = [0i16; 64];
            for (i, x) in alt.iter_mut().enumerate() {
                *x = if (i + i / 8) % 2 == 0 { 2047 } else { -2048 };
            }
            idct(&mut alt);
        }
    }

    #[test]
    fn constants_match_cosines() {
        let k = [C1, C2, C3, C4, C5, C6, C7];
        for (i, &c) in k.iter().enumerate() {
            let exact = ((i + 1) as f64 * core::f64::consts::PI / 16.0).cos() / 2.0 * f64::from(1 << CBITS);
            assert!((f64::from(c) - exact).abs() <= 0.5, "C{}", i + 1);
        }
    }
}
