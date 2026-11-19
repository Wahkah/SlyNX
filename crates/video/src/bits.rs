//! Big-endian bit reader over a byte slice, and start-code search (ISO/IEC 13818-2, 5.2 and 6.2.1).

/// Reads bits most significant first. Reading past the end yields zero bits; [`BitReader::overrun`] tells whether
/// that happened, so a caller can reject a truncated unit.
///
/// The next bits are kept left-aligned in a 64-bit window holding at least 32 valid bits at all times, so a peek is
/// a shift and a skip is a shift plus, now and then, one 8-byte load.
#[derive(Clone)]
pub(crate) struct BitReader<'a> {
    data: &'a [u8],
    /// Next byte to load into the window.
    next: usize,
    /// Upcoming bits, left-aligned.
    window: u64,
    /// Valid bits in `window` (at least 32 between calls).
    valid: u32,
    /// Bits consumed so far.
    consumed: usize,
}

impl<'a> BitReader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        let mut br = BitReader { data, next: 0, window: 0, valid: 0, consumed: 0 };
        br.refill();
        br
    }

    #[inline(always)]
    fn refill(&mut self) {
        if let Some(s) = self.data.get(self.next..self.next + 8) {
            let mut b = [0u8; 8];
            b.copy_from_slice(s);
            // Bits below the accepted bytes are the following data; loading them again later ORs the same values.
            self.window |= u64::from_be_bytes(b) >> self.valid;
            let bytes = (63 - self.valid) >> 3;
            self.next += bytes as usize;
            self.valid += bytes * 8;
        } else {
            self.refill_tail();
        }
    }

    #[cold]
    fn refill_tail(&mut self) {
        while self.valid <= 56 {
            let b = self.data.get(self.next).copied().unwrap_or(0);
            self.window |= u64::from(b) << (56 - self.valid);
            self.next += 1;
            self.valid += 8;
        }
    }

    /// The next 32 bits, left-aligned, without consuming them.
    #[inline(always)]
    pub(crate) fn peek32(&self) -> u32 {
        (self.window >> 32) as u32
    }

    /// The next `n` bits (1 to 32) without consuming them.
    #[inline(always)]
    pub(crate) fn peek(&self, n: u32) -> u32 {
        debug_assert!((1..=32).contains(&n));
        self.peek32() >> (32 - n)
    }

    /// Consumes `n` bits (0 to 32).
    #[inline(always)]
    pub(crate) fn skip(&mut self, n: u32) {
        debug_assert!(n <= 32);
        self.window <<= n;
        self.valid -= n;
        self.consumed += n as usize;
        if self.valid < 32 {
            self.refill();
        }
    }

    /// Reads `n` bits (0 to 32).
    #[inline(always)]
    pub(crate) fn read(&mut self, n: u32) -> u32 {
        if n == 0 {
            return 0;
        }
        let v = self.peek(n);
        self.skip(n);
        v
    }

    #[inline(always)]
    pub(crate) fn bit(&mut self) -> bool {
        self.read(1) != 0
    }

    /// Bits consumed so far.
    pub(crate) fn position(&self) -> usize {
        self.consumed
    }

    /// Whether more bits were consumed than the data holds.
    pub(crate) fn overrun(&self) -> bool {
        self.consumed > self.data.len() * 8
    }
}

/// Finds the next start-code prefix `00 00 01` at or after `from`; returns the offset of its first byte.
pub(crate) fn find_start_code(data: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + 3 <= data.len() {
        let c = data[i + 2];
        if c > 1 {
            i += 3;
        } else if c == 1 && data[i] == 0 && data[i + 1] == 0 {
            return Some(i);
        } else {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bits_across_bytes() {
        let data = [0b1010_1100, 0b0101_0011, 0xFF, 0x00, 0x12, 0x34, 0x56, 0x78, 0x9A];
        let mut br = BitReader::new(&data);
        assert_eq!(br.read(1), 1);
        assert_eq!(br.read(3), 0b010);
        assert_eq!(br.read(8), 0b1100_0101);
        assert_eq!(br.peek(4), 0b0011);
        assert_eq!(br.read(20), 0b0011_1111_1111_0000_0000);
        assert_eq!(br.read(32), 0x1234_5678);
        assert_eq!(br.read(8), 0x9A);
        assert!(!br.overrun());
        assert_eq!(br.read(5), 0);
        assert!(br.overrun());
    }

    #[test]
    fn matches_a_naive_reader_for_random_reads() {
        let mut seed = 0x1234_5678_9ABC_DEF0u64;
        let mut rand = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for len in [0usize, 1, 3, 7, 8, 9, 15, 40, 1000] {
            let data: alloc::vec::Vec<u8> = (0..len).map(|_| rand() as u8).collect();
            let naive = |pos: usize, n: u32| -> u32 {
                (0..n as usize).fold(0u32, |acc, i| {
                    let bit = data.get((pos + i) / 8).map_or(0, |b| (b >> (7 - (pos + i) % 8)) & 1);
                    (acc << 1) | u32::from(bit)
                })
            };
            let mut br = BitReader::new(&data);
            let mut pos = 0;
            while pos < len * 8 + 64 {
                let n = (rand() % 33) as u32;
                if n > 0 {
                    assert_eq!(br.peek(n), naive(pos, n), "len {len} pos {pos} peek {n}");
                }
                assert_eq!(br.read(n), naive(pos, n), "len {len} pos {pos} read {n}");
                pos += n as usize;
                assert_eq!(br.position(), pos);
                assert_eq!(br.overrun(), pos > len * 8);
            }
        }
    }

    #[test]
    fn finds_start_codes() {
        let data = [0xFF, 0, 0, 1, 0xB3, 0, 0, 0, 1, 0, 1, 0, 0];
        assert_eq!(find_start_code(&data, 0), Some(1));
        assert_eq!(find_start_code(&data, 2), Some(6));
        assert_eq!(find_start_code(&data, 7), None);
    }
}
