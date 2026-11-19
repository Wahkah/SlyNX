//! The game's random numbers: one deterministic generator per game, seeded once (testing.md, determinism contract).
//! xoshiro128+ state seeded through SplitMix64; floats use the top 24 bits.

/// Deterministic random number generator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    s: [u32; 4],
}

fn splitmix(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl crate::physics::Rng for Rng {
    fn next_f32(&mut self) -> f32 {
        Rng::next_f32(self)
    }
}

impl Rng {
    /// A generator seeded with `seed`.
    pub fn new(seed: u64) -> Rng {
        let mut x = seed;
        let a = splitmix(&mut x);
        let b = splitmix(&mut x);
        let mut s = [a as u32, (a >> 32) as u32, b as u32, (b >> 32) as u32];
        if s == [0; 4] {
            s[0] = 1;
        }
        Rng { s }
    }

    /// The next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        let r = self.s[0].wrapping_add(self.s[3]);
        let t = self.s[1] << 9;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(11);
        r
    }

    /// A float in [0, 1).
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// A float in [lo, hi).
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    /// The state, for hashing.
    pub fn state(&self) -> [u32; 4] {
        self.s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_in_range() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            let x = a.next_f32();
            assert_eq!(x, b.next_f32());
            assert!((0.0..1.0).contains(&x));
        }
        assert_ne!(Rng::new(1).next_u32(), Rng::new(2).next_u32());
        let r = Rng::new(0).range(5.0, 6.0);
        assert!((5.0..6.0).contains(&r));
    }
}
