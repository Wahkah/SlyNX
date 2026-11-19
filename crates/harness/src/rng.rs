//! Deterministic random number generator.
//!
//! Apps must take all their randomness from the [`Rng`] in the frame context, so a run with the same seed and the
//! same input script produces the same frames. The generator is xoshiro256** seeded through SplitMix64; both are
//! public-domain algorithms with fixed, documented output, so the sequence never changes between toolchains.

/// xoshiro256** pseudo-random number generator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    /// Creates a generator from a 64-bit seed. Every seed (including 0) gives a valid, distinct sequence.
    pub fn new(seed: u64) -> Self {
        let mut sm = seed;
        let s = [splitmix64(&mut sm), splitmix64(&mut sm), splitmix64(&mut sm), splitmix64(&mut sm)];
        Rng { s }
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Next 32 random bits (the high half of [`Rng::next_u64`]).
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Uniform integer in `0..bound`. Returns 0 when `bound` is 0. Unbiased (rejection sampling).
    pub fn below(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        let zone = u32::MAX - (u32::MAX % bound);
        loop {
            let v = self.next_u32();
            if v < zone {
                return v % bound;
            }
        }
    }

    /// Uniform integer in `lo..=hi` (inclusive). The bounds may be given in either order.
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32 {
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        let span = (hi as i64 - lo as i64 + 1) as u64;
        if span > u32::MAX as u64 {
            return (lo as i64 + (self.next_u64() % span) as i64) as i32;
        }
        (lo as i64 + self.below(span as u32) as i64) as i32
    }

    /// Uniform float in `[0, 1)` with 24 bits of precision.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / (1u32 << 24) as f32)
    }

    /// Uniform float in `[0, 1)` with 53 bits of precision.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// `true` with probability `p` (clamped to `[0, 1]`).
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }

    /// A new generator seeded from this one, for a subsystem that needs its own stream.
    pub fn fork(&mut self) -> Rng {
        Rng::new(self.next_u64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let same = (0..64).filter(|_| a.next_u64() == b.next_u64()).count();
        assert_eq!(same, 0);
    }

    #[test]
    fn known_first_values() {
        // SplitMix64 from seed 0 is a published sequence; this pins our seeding to it.
        let mut sm = 0u64;
        assert_eq!(splitmix64(&mut sm), 0xE220_A839_7B1D_CDAF);
        assert_eq!(splitmix64(&mut sm), 0x6E78_9E6A_A1B9_65F4);
        // Pin the generator's output so an accidental change to it (which would change every recorded run) is caught.
        let mut r = Rng::new(0);
        let first: Vec<u64> = (0..3).map(|_| r.next_u64()).collect();
        assert_eq!(first, vec![0x99EC_5F36_CB75_F2B4, 0xBF6E_1F78_4956_452A, 0x1A5F_849D_4933_E6E0]);
    }

    #[test]
    fn ranges_stay_in_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            assert!(r.below(10) < 10);
            let v = r.range_i32(-3, 3);
            assert!((-3..=3).contains(&v));
            let f = r.next_f32();
            assert!((0.0..1.0).contains(&f));
            let d = r.next_f64();
            assert!((0.0..1.0).contains(&d));
        }
        assert_eq!(r.below(0), 0);
        assert_eq!(r.range_i32(5, 5), 5);
        let full = r.range_i32(i32::MIN, i32::MAX);
        let _ = full;
    }

    #[test]
    fn below_is_roughly_uniform() {
        let mut r = Rng::new(99);
        let mut buckets = [0u32; 8];
        for _ in 0..80_000 {
            buckets[r.below(8) as usize] += 1;
        }
        for b in buckets {
            assert!((9_000..11_000).contains(&b), "bucket count {b}");
        }
    }
}
