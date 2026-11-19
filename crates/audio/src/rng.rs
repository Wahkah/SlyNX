//! A small deterministic random generator (xorshift64*) for the random steps, intermittent sounds and choices.

/// Pseudo-random numbers; the same seed gives the same sequence on every platform.
#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);

impl Rng {
    /// A generator from `seed` (0 is replaced by a fixed non-zero value).
    pub const fn new(seed: u64) -> Rng {
        Rng(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    /// The next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    /// A uniform integer in `lo..=hi` (the bounds may come in either order).
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        let span = (hi as i64 - lo as i64 + 1) as u64;
        (lo as i64 + (self.next_u32() as u64 % span) as i64) as i32
    }

    /// A uniform float in `0..1`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// A uniform float between `lo` and `hi`.
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        let mut r = Rng::new(1);
        let mut seen = [false; 5];
        for _ in 0..1000 {
            let v = r.range_i32(3, 7);
            assert!((3..=7).contains(&v));
            seen[(v - 3) as usize] = true;
            let f = r.range_f32(-1.0, 1.0);
            assert!((-1.0..1.0).contains(&f));
        }
        assert!(seen.iter().all(|&s| s));
        assert_eq!(r.range_i32(5, 5), 5);
        assert!((-3..=2).contains(&r.range_i32(2, -3)));
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        assert_eq!(a.next_u32(), b.next_u32());
    }
}
