//! A small, stable hasher for app state hashes.
//!
//! `std`'s default hasher is not guaranteed to stay the same between Rust releases, so state hashes that are stored
//! or compared across runs use 64-bit FNV-1a instead.

/// 64-bit FNV-1a hasher.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fnv64(u64);

impl Default for Fnv64 {
    fn default() -> Self {
        Self::new()
    }
}

impl Fnv64 {
    const OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01B3;

    /// A hasher in its initial state.
    pub fn new() -> Self {
        Fnv64(Self::OFFSET)
    }

    /// Feeds bytes.
    pub fn write(&mut self, bytes: &[u8]) -> &mut Self {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
        self
    }

    /// Feeds a `u32` (little-endian).
    pub fn write_u32(&mut self, v: u32) -> &mut Self {
        self.write(&v.to_le_bytes())
    }

    /// Feeds a `u64` (little-endian).
    pub fn write_u64(&mut self, v: u64) -> &mut Self {
        self.write(&v.to_le_bytes())
    }

    /// Feeds an `f32` by its bit pattern (so `-0.0` and `0.0` hash differently, and NaNs by payload).
    pub fn write_f32(&mut self, v: f32) -> &mut Self {
        self.write_u32(v.to_bits())
    }

    /// The hash so far.
    pub fn finish(&self) -> u64 {
        self.0
    }

    /// Hashes a byte slice in one call.
    pub fn hash(bytes: &[u8]) -> u64 {
        Fnv64::new().write(bytes).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors() {
        assert_eq!(Fnv64::hash(b""), 0xCBF2_9CE4_8422_2325);
        assert_eq!(Fnv64::hash(b"a"), 0xAF63_DC4C_8601_EC8C);
        assert_eq!(Fnv64::hash(b"foobar"), 0x8594_4171_F739_67E8);
    }
}
