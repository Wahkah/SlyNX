//! The world-file compression: an LZ77-style stream with an 8 KiB window (world-file.md section 2).
//!
//! The window is a ring of 8192 bytes, all zero at the start; the write position is the number of bytes output so
//! far, modulo 8192. Copy items name an **absolute** window position, not a distance. The decoder here does not keep
//! a separate ring: window position `p` holds the most recent output byte whose index is congruent to `p` modulo
//! 8192, or zero if no such byte exists yet, so a copy is turned into a distance back into the output and copied from
//! there.

use alloc::vec::Vec;

use crate::error::{Error, Result};

/// Size of the window in bytes.
pub const WINDOW: usize = 8192;
const MASK: usize = WINDOW - 1;

/// Decompresses `input` (exactly the `size` bytes from the descriptor) into a new vector.
pub fn decompress(input: &[u8]) -> Result<Vec<u8>> {
    // Worlds expand by 1.5x to 3x; start at 2x and let the vector grow if needed.
    let mut out = Vec::with_capacity(input.len().saturating_mul(2));
    decompress_into(input, &mut out)?;
    Ok(out)
}

/// Decompresses `input`, appending to `out`. The window starts empty at the current end of `out` (bytes already in
/// `out` are not part of the window).
pub fn decompress_into(input: &[u8], out: &mut Vec<u8>) -> Result<()> {
    let base = out.len();
    let n = input.len();
    let mut i = 0usize;
    while i < n {
        let flags = input[i];
        i += 1;
        // Fast path: eight literals.
        if flags == 0xFF && i + 8 <= n {
            out.extend_from_slice(&input[i..i + 8]);
            i += 8;
            continue;
        }
        for bit in 0..8 {
            if i >= n {
                return Ok(());
            }
            if (flags >> bit) & 1 != 0 {
                out.push(input[i]);
                i += 1;
                continue;
            }
            if i + 2 > n {
                return Err(Error::Corrupt { offset: i, what: "compressed stream ends inside a copy item" });
            }
            let t = input[i] as usize | (input[i + 1] as usize) << 8;
            i += 2;
            let p = t & MASK;
            let len = (t >> 13) + 3;
            let produced = out.len() - base;
            // Distance back from the byte about to be written to the byte that sits at window position `p`.
            let mut d = (produced.wrapping_sub(p)) & MASK;
            if d == 0 {
                d = WINDOW;
            }
            if d <= produced {
                let src = out.len() - d;
                if d >= len {
                    out.extend_from_within(src..src + len);
                } else {
                    // Overlapping copy: repeats the last `d` bytes.
                    for k in 0..len {
                        let b = out[src + k];
                        out.push(b);
                    }
                }
            } else {
                // Starts in the never-written (zero) part of the window.
                for k in 0..len {
                    let idx = produced + k;
                    let b = if idx >= d { out[base + idx - d] } else { 0 };
                    out.push(b);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// A straightforward ring-buffer decoder, written directly from the spec's wording, to cross-check the fast one.
    fn reference(input: &[u8]) -> Vec<u8> {
        let mut ring = [0u8; WINDOW];
        let mut w = 0usize;
        let mut out = Vec::new();
        let mut i = 0;
        while i < input.len() {
            let flags = input[i];
            i += 1;
            for bit in 0..8 {
                if i >= input.len() {
                    break;
                }
                if flags >> bit & 1 == 1 {
                    let b = input[i];
                    i += 1;
                    out.push(b);
                    ring[w] = b;
                    w = (w + 1) % WINDOW;
                } else {
                    let t = input[i] as usize + 256 * input[i + 1] as usize;
                    i += 2;
                    let mut p = t & 0x1FFF;
                    for _ in 0..(t >> 13) + 3 {
                        let b = ring[p];
                        out.push(b);
                        ring[w] = b;
                        w = (w + 1) % WINDOW;
                        p = (p + 1) % WINDOW;
                    }
                }
            }
        }
        out
    }

    fn copy(p: usize, len: usize) -> [u8; 2] {
        let t = p | (len - 3) << 13;
        [t as u8, (t >> 8) as u8]
    }

    #[test]
    fn literals_and_absolute_copy() {
        // Flag 0xFF: 8 literals. Flag 0b1011_1111: 6 literals, a copy, a literal.
        let mut s = vec![0xFF, 1, 2, 3, 4, 5, 6, 7, 8, 0xBF, 9, 10, 11, 12, 13, 14];
        s.extend_from_slice(&copy(2, 3)); // window positions 2, 3, 4 = 3, 4, 5
        s.push(99);
        let out = decompress(&s).unwrap();
        assert_eq!(out, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 3, 4, 5, 99]);
        assert_eq!(out, reference(&s));
    }

    #[test]
    fn copy_from_unwritten_window_reads_zero_then_new_bytes() {
        // Output 2 bytes, then copy 10 bytes from position 8190: two never-written zeros, then output bytes 0.. .
        let mut s = vec![0b0000_0011, 0xAA, 0xBB];
        s.extend_from_slice(&copy(8190, 10));
        let out = decompress(&s).unwrap();
        assert_eq!(out, reference(&s));
        assert_eq!(&out[..4], &[0xAA, 0xBB, 0, 0]);
        assert_eq!(&out[4..8], &[0xAA, 0xBB, 0, 0]);
    }

    #[test]
    fn overlapping_copy_repeats_run() {
        let mut s = vec![0b0000_0001, 0x55];
        s.extend_from_slice(&copy(0, 10)); // source position 0 while writing at 1: a run
        let out = decompress(&s).unwrap();
        assert_eq!(out, vec![0x55; 11]);
    }

    #[test]
    fn copy_at_write_position_reads_oldest_byte() {
        // After 8192 + 3 bytes, position w = 3 holds output byte 3; reading p = w gives that byte (distance 8192).
        let mut s = Vec::new();
        let mut produced = 0u32;
        while produced < 8195 {
            s.push(0xFF);
            for _ in 0..8 {
                s.push((produced % 251) as u8);
                produced += 1;
            }
        }
        // produced is now 8200; w = 8. Copy from p = 8.
        s.push(0b1111_1110);
        s.extend_from_slice(&copy(8, 4));
        let out = decompress(&s).unwrap();
        assert_eq!(out, reference(&s));
        assert_eq!(&out[8200..8204], &out[8..12]);
    }

    #[test]
    fn pseudo_random_streams_match_reference() {
        let mut x = 0x1234_5678u32;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x
        };
        for _ in 0..20 {
            let mut s = Vec::new();
            for _ in 0..3000 {
                let flags = next() as u8;
                s.push(flags);
                for bit in 0..8 {
                    if flags >> bit & 1 == 1 {
                        s.push(next() as u8);
                    } else {
                        let t = next();
                        s.extend_from_slice(&[t as u8, (t >> 8) as u8]);
                    }
                }
            }
            assert_eq!(decompress(&s).unwrap(), reference(&s));
        }
    }

    #[test]
    fn truncated_copy_is_an_error() {
        assert!(decompress(&[0x00, 0x01]).is_err());
        // Input that ends exactly after an item is fine, even mid flag byte.
        assert_eq!(decompress(&[0x01, 7]).unwrap(), [7]);
    }
}
