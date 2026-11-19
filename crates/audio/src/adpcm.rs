//! SPU ADPCM (audio.md 2.1): 16-byte blocks of 28 samples, with end, repeat and loop-start flags.
//!
//! [`decode_block`] decodes one block; [`Cursor`] walks a sample block by block the way a voice plays it (following
//! the loop point when a repeating sample ends); [`decode_to_end`] decodes one pass to the end flag, which is what
//! the spec's PCM hashes cover.

use alloc::vec::Vec;

/// Bytes per block.
pub const BLOCK_BYTES: usize = 16;
/// Samples per block.
pub const BLOCK_SAMPLES: usize = 28;

/// Flag bit: the sample ends after this block.
pub const FLAG_END: u8 = 1;
/// Flag bit: with [`FLAG_END`], playback continues at the loop point.
pub const FLAG_REPEAT: u8 = 2;
/// Flag bit: the loop point is the first sample of this block.
pub const FLAG_LOOP_START: u8 = 4;

/// Prediction coefficients per filter number, in 1/64 units.
const FILTERS: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];

/// The two previous output samples carried from block to block (`h1` the most recent).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct History {
    /// Most recent output sample.
    pub h1: i32,
    /// The one before.
    pub h2: i32,
}

/// Decodes the 16-byte `block` into `out`, updating `hist`. Returns the block's flag byte.
///
/// A filter number above 4 (which the game never uses) is treated as filter 0.
pub fn decode_block(block: &[u8; BLOCK_BYTES], hist: &mut History, out: &mut [i16; BLOCK_SAMPLES]) -> u8 {
    let shift = (block[0] & 0x0F) as u32;
    let filter = (block[0] >> 4) as usize;
    let (k1, k2) = FILTERS.get(filter).copied().unwrap_or((0, 0));
    let mut h1 = hist.h1;
    let mut h2 = hist.h2;
    for (i, o) in out.iter_mut().enumerate() {
        let byte = block[2 + i / 2];
        let nibble = if i & 1 == 0 { byte & 0x0F } else { byte >> 4 };
        // Sign-extend the 4-bit code.
        let code = ((nibble as i8) << 4 >> 4) as i32;
        let mut x = (code << 12) >> shift;
        x += (k1 * h1 + k2 * h2 + 32) >> 6;
        let x = x.clamp(-32768, 32767);
        *o = x as i16;
        h2 = h1;
        h1 = x;
    }
    hist.h1 = h1;
    hist.h2 = h2;
    block[1]
}

/// One pass of a sample: from `data[start..]` up to and including the first block with the end flag.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Decoded {
    /// The samples.
    pub pcm: Vec<i16>,
    /// Sample index of the loop point, when a block set one.
    pub loop_start: Option<usize>,
    /// Whether the last block also has the repeat flag (the voice loops instead of stopping).
    pub repeats: bool,
    /// Whether an end flag was found (`false` when the data ran out first).
    pub terminated: bool,
}

/// Decodes from byte offset `start` of `data` to the first end flag (or the end of the data).
pub fn decode_to_end(data: &[u8], start: usize) -> Decoded {
    let mut out = Decoded::default();
    let mut hist = History::default();
    let mut pos = start;
    let mut buf = [0i16; BLOCK_SAMPLES];
    while let Some(block) = data.get(pos..pos + BLOCK_BYTES) {
        let block: &[u8; BLOCK_BYTES] = block.try_into().unwrap_or(&[0; BLOCK_BYTES]);
        let flags = decode_block(block, &mut hist, &mut buf);
        if flags & FLAG_LOOP_START != 0 {
            out.loop_start = Some(out.pcm.len());
        }
        out.pcm.extend_from_slice(&buf);
        pos += BLOCK_BYTES;
        if flags & FLAG_END != 0 {
            out.repeats = flags & FLAG_REPEAT != 0;
            out.terminated = true;
            break;
        }
    }
    out
}

/// Number of bytes from `start` up to and including the first block with the end flag, without decoding.
pub fn sample_len(data: &[u8], start: usize) -> Option<usize> {
    let mut pos = start;
    while let Some(block) = data.get(pos..pos + BLOCK_BYTES) {
        pos += BLOCK_BYTES;
        if block[1] & FLAG_END != 0 {
            return Some(pos - start);
        }
    }
    None
}

/// Plays a sample block by block, like an SPU voice: decodes 28 samples at a time, remembers the loop point, and
/// on an end flag either jumps to the loop point (repeat flag, history kept) or stops.
///
/// The cursor holds byte offsets only; the caller passes the sample bytes on every call, so no reference is kept.
#[derive(Clone, Copy, Debug)]
pub struct Cursor {
    /// Byte offset of the next block to decode.
    next: usize,
    /// Byte offset of the loop point (the start of the sample until a block sets one).
    loop_at: usize,
    hist: History,
    buf: [i16; BLOCK_SAMPLES],
    /// Next sample of `buf` to hand out; `BLOCK_SAMPLES` when a new block is needed.
    idx: u8,
    /// Flags of the block in `buf`.
    flags: u8,
    ended: bool,
    /// Loop back to the loop point even without the repeat flag (for looped streams).
    wrap: bool,
}

impl Cursor {
    /// A cursor at byte offset `start`.
    pub const fn new(start: usize) -> Cursor {
        Cursor {
            next: start,
            loop_at: start,
            hist: History { h1: 0, h2: 0 },
            buf: [0; BLOCK_SAMPLES],
            idx: BLOCK_SAMPLES as u8,
            flags: 0,
            ended: false,
            wrap: false,
        }
    }

    /// A cursor that also loops a sample whose last block has no repeat flag (or whose data runs out), back to its
    /// loop point (the start, unless a block set one).
    pub const fn looping(start: usize) -> Cursor {
        let mut c = Cursor::new(start);
        c.wrap = true;
        c
    }

    /// Whether the sample has stopped (end flag without repeat, or the data ran out).
    pub fn ended(&self) -> bool {
        self.ended
    }

    /// The next sample, or `None` once the sample has stopped.
    #[inline]
    pub fn next_sample(&mut self, data: &[u8]) -> Option<i16> {
        if self.idx as usize >= BLOCK_SAMPLES && !self.refill(data) {
            return None;
        }
        let s = self.buf[self.idx as usize];
        self.idx += 1;
        Some(s)
    }

    #[cold]
    fn refill(&mut self, data: &[u8]) -> bool {
        if self.ended {
            return false;
        }
        // The flags of the block just finished take effect now.
        if self.flags & FLAG_END != 0 {
            if self.flags & FLAG_REPEAT != 0 || self.wrap {
                self.next = self.loop_at;
            } else {
                self.ended = true;
                return false;
            }
        }
        if self.next + BLOCK_BYTES > data.len()
            && self.wrap
            && self.loop_at + BLOCK_BYTES <= data.len()
            && self.next != self.loop_at
        {
            self.next = self.loop_at;
        }
        let Some(block) = data.get(self.next..self.next + BLOCK_BYTES) else {
            self.ended = true;
            return false;
        };
        let block: &[u8; BLOCK_BYTES] = block.try_into().unwrap_or(&[0; BLOCK_BYTES]);
        self.flags = decode_block(block, &mut self.hist, &mut self.buf);
        if self.flags & FLAG_LOOP_START != 0 {
            self.loop_at = self.next;
        }
        self.next += BLOCK_BYTES;
        self.idx = 0;
        true
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;

    /// Encodes one block from raw parts (test helper).
    pub(crate) fn block(shift: u8, filter: u8, flags: u8, codes: &[i8; 28]) -> [u8; 16] {
        let mut b = [0u8; 16];
        b[0] = (filter << 4) | shift;
        b[1] = flags;
        for (i, &c) in codes.iter().enumerate() {
            let n = (c as u8) & 0x0F;
            b[2 + i / 2] |= if i & 1 == 0 { n } else { n << 4 };
        }
        b
    }

    #[test]
    fn filter_zero_is_scaled_codes() {
        let mut codes = [0i8; 28];
        codes[0] = 7;
        codes[1] = -8;
        codes[2] = 1;
        codes[3] = -1;
        let b = block(12, 0, 0, &codes);
        let mut h = History::default();
        let mut out = [0; 28];
        decode_block(&b, &mut h, &mut out);
        assert_eq!(&out[..4], &[7, -8, 1, -1]);
        let b = block(0, 0, 0, &codes);
        decode_block(&b, &mut h, &mut out);
        assert_eq!(&out[..4], &[28672, -32768, 4096, -4096]);
    }

    #[test]
    fn prediction_and_clamping() {
        let mut codes = [0i8; 28];
        codes[0] = 7;
        let b = block(0, 1, 0, &codes);
        let mut h = History::default();
        let mut out = [0; 28];
        decode_block(&b, &mut h, &mut out);
        // 28672, then (60 * 28672 + 32) >> 6 = 26880, then 25200 ...
        assert_eq!(out[0], 28672);
        assert_eq!(out[1], 26880);
        assert_eq!(out[2], ((60 * 26880 + 32) >> 6) as i16);
        // Filter 4 with a large history clamps.
        let mut h = History { h1: 32767, h2: 0 };
        let b = block(0, 4, 0, &[7; 28]);
        decode_block(&b, &mut h, &mut out);
        assert_eq!(out[0], 32767);
        // Negative codes with filter 2: arithmetic shifts round toward minus infinity.
        let mut h = History { h1: -3, h2: 5 };
        let b = block(12, 2, 0, &[-1; 28]);
        decode_block(&b, &mut h, &mut out);
        assert_eq!(out[0] as i32, -1 + ((115 * -3 + -52 * 5 + 32) >> 6));
    }

    #[test]
    fn decode_to_end_stops_at_end_flag() {
        let mut data = vec![];
        data.extend_from_slice(&block(12, 0, 0, &[1; 28]));
        data.extend_from_slice(&block(12, 0, FLAG_LOOP_START, &[2; 28]));
        data.extend_from_slice(&block(12, 0, FLAG_END | FLAG_REPEAT, &[3; 28]));
        data.extend_from_slice(&block(12, 0, 0, &[4; 28]));
        let d = decode_to_end(&data, 0);
        assert_eq!(d.pcm.len(), 84);
        assert_eq!(d.loop_start, Some(28));
        assert!(d.repeats && d.terminated);
        assert_eq!(sample_len(&data, 0), Some(48));
        assert_eq!(sample_len(&data, 48), None);
    }

    #[test]
    fn cursor_loops_and_stops() {
        let mut data = vec![];
        data.extend_from_slice(&block(12, 0, 0, &[1; 28]));
        data.extend_from_slice(&block(12, 0, FLAG_LOOP_START, &[2; 28]));
        data.extend_from_slice(&block(12, 0, FLAG_END | FLAG_REPEAT, &[3; 28]));
        let mut c = Cursor::new(0);
        let got: Vec<i16> = (0..28 * 5).map(|_| c.next_sample(&data).unwrap()).collect();
        assert_eq!(got[0], 1);
        assert_eq!(got[28], 2);
        assert_eq!(got[56], 3);
        assert_eq!(got[84], 2);
        assert_eq!(got[112], 3);
        // Without the repeat flag the voice stops.
        data[16 * 2 + 1] = FLAG_END;
        let mut c = Cursor::new(0);
        for _ in 0..84 {
            assert!(c.next_sample(&data).is_some());
        }
        assert_eq!(c.next_sample(&data), None);
        assert!(c.ended());
        // Running off the data stops too.
        let mut c = Cursor::new(32);
        for _ in 0..28 {
            c.next_sample(&data[..48]).unwrap();
        }
        assert_eq!(c.next_sample(&data[..40]), None);
        // A looping cursor wraps a one-shot sample, and data that runs out.
        let mut c = Cursor::looping(0);
        for _ in 0..84 {
            c.next_sample(&data).unwrap();
        }
        assert_eq!(c.next_sample(&data), Some(2));
        let mut c = Cursor::looping(0);
        for _ in 0..28 {
            c.next_sample(&data[..16]).unwrap();
        }
        assert_eq!(c.next_sample(&data[..16]), Some(1));
    }
}
