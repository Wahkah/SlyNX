//! The movies' audio stream: an `SShd` header, an `SSbd` header, then the data in alternating left and right blocks,
//! turned into interleaved stereo samples.
//!
//! - Sly 1 (specs/sly1/movies.md 2.2): 16-bit PCM, 512-byte blocks, 48 kHz.
//! - Sly 2 and 3 (specs/sly2/movies.md, specs/sly3/movies.md): SPU ADPCM (specs/sly1/audio.md 2.1), 32-byte blocks,
//!   each channel's bytes one continuous ADPCM stream, 44.1 or 48 kHz.
//!
//! [`Resampler`] converts to the platform's 48 kHz output.

use alloc::vec::Vec;

use crate::Error;

/// Bytes before the audio data (`SShd` header, padding, `SSbd` header).
pub const HEADER_SIZE: usize = 40;
/// `SShd` format: 16-bit signed little-endian PCM.
pub const FORMAT_PCM16: u32 = 1;
/// `SShd` format: SPU ADPCM.
pub const FORMAT_SPU_ADPCM: u32 = 0x10;
/// Bytes per SPU ADPCM block.
pub const ADPCM_BLOCK: usize = 16;
/// Samples per SPU ADPCM block.
pub const ADPCM_SAMPLES: usize = 28;

/// The audio header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioHeader {
    /// [`FORMAT_PCM16`] or [`FORMAT_SPU_ADPCM`].
    pub format: u32,
    pub sample_rate: u32,
    pub channels: u32,
    /// Bytes per channel block.
    pub interleave: u32,
    /// Size of the audio data in bytes (the `SSbd` size).
    pub data_size: u32,
}

impl AudioHeader {
    /// Parses the 40 header bytes.
    pub fn parse(b: &[u8]) -> Result<AudioHeader, Error> {
        if b.len() < HEADER_SIZE {
            return Err(Error::Audio("header truncated"));
        }
        let u = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        if &b[0..4] != b"SShd" || &b[32..36] != b"SSbd" {
            return Err(Error::Audio("missing SShd/SSbd tags"));
        }
        if u(4) != 0x18 {
            return Err(Error::Audio("unexpected SShd size"));
        }
        let h = AudioHeader { format: u(8), sample_rate: u(12), channels: u(16), interleave: u(20), data_size: u(36) };
        let unit = match h.format {
            FORMAT_PCM16 => 2,
            FORMAT_SPU_ADPCM => ADPCM_BLOCK as u32,
            _ => return Err(Error::Unsupported("audio format other than 16-bit PCM or SPU ADPCM")),
        };
        if h.channels != 2 {
            return Err(Error::Unsupported("audio channel count other than 2"));
        }
        if h.interleave == 0 || !h.interleave.is_multiple_of(unit) || h.interleave > 1 << 16 || h.sample_rate == 0 {
            return Err(Error::Audio("bad interleave or sample rate"));
        }
        Ok(h)
    }

    /// Stereo sample frames in the data (whole blocks).
    pub fn frames(&self) -> u64 {
        let per_channel = u64::from(self.data_size) / 2;
        match self.format {
            FORMAT_SPU_ADPCM => per_channel / ADPCM_BLOCK as u64 * ADPCM_SAMPLES as u64,
            _ => per_channel / 2,
        }
    }

    /// Duration in nanoseconds.
    pub fn duration_ns(&self) -> u64 {
        self.frames() * 1_000_000_000 / u64::from(self.sample_rate)
    }
}

/// One channel's SPU ADPCM decoder state: the two previous output samples.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Adpcm {
    h1: i32,
    h2: i32,
}

/// Prediction filters (k1, k2), in 64ths.
const FILTERS: [(i32, i32); 5] = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)];

impl Adpcm {
    /// Decodes one 16-byte block into 28 samples (specs/sly1/audio.md 2.1). The flags byte is not used: movie
    /// audio is one continuous stream per channel.
    pub fn decode_block(&mut self, block: &[u8; ADPCM_BLOCK], out: &mut [i16; ADPCM_SAMPLES]) {
        let shift = u32::from(block[0] & 0x0F);
        // Filters above 4 do not occur in the game; they are read as no prediction.
        let (k1, k2) = FILTERS.get(usize::from(block[0] >> 4)).copied().unwrap_or((0, 0));
        for (i, o) in out.iter_mut().enumerate() {
            let byte = block[2 + i / 2];
            let nibble = if i % 2 == 0 { byte & 0x0F } else { byte >> 4 };
            let code = i32::from((nibble << 4) as i8) >> 4;
            let x = ((code << 12) >> shift) + ((k1 * self.h1 + k2 * self.h2 + 32) >> 6);
            let x = x.clamp(-32768, 32767);
            *o = x as i16;
            self.h2 = self.h1;
            self.h1 = x;
        }
    }
}

/// Turns the audio stream, pushed in pieces of any size, into interleaved stereo `i16` samples at the stream's own
/// rate.
#[derive(Default)]
pub struct AudioStream {
    header: Option<AudioHeader>,
    pending: Vec<u8>,
    adpcm: [Adpcm; 2],
    /// Data bytes taken in so far (header excluded).
    data_seen: u64,
    /// Bytes after the declared data size (ignored).
    pub extra_bytes: u64,
}

impl AudioStream {
    pub fn new() -> AudioStream {
        AudioStream::default()
    }

    pub fn header(&self) -> Option<&AudioHeader> {
        self.header.as_ref()
    }

    /// Data bytes received so far.
    pub fn data_bytes(&self) -> u64 {
        self.data_seen
    }

    /// Appends stream bytes; complete left/right block pairs are converted and appended to `out` as
    /// L, R, L, R, ... samples.
    pub fn push(&mut self, mut data: &[u8], out: &mut Vec<i16>) -> Result<(), Error> {
        if self.header.is_none() {
            let need = HEADER_SIZE - self.pending.len();
            let take = need.min(data.len());
            self.pending.extend_from_slice(&data[..take]);
            data = &data[take..];
            if self.pending.len() < HEADER_SIZE {
                return Ok(());
            }
            self.header = Some(AudioHeader::parse(&self.pending)?);
            self.pending.clear();
        }
        let Some(h) = self.header else { return Ok(()) };
        let left = u64::from(h.data_size).saturating_sub(self.data_seen);
        let take = (data.len() as u64).min(left) as usize;
        self.extra_bytes += (data.len() - take) as u64;
        self.data_seen += take as u64;
        let data = &data[..take];
        let pair = 2 * h.interleave as usize;
        if self.pending.is_empty() {
            // Fast path: whole pairs straight from the input.
            let whole = data.len() / pair * pair;
            self.deinterleave(&h, &data[..whole], out);
            self.pending.extend_from_slice(&data[whole..]);
        } else {
            self.pending.extend_from_slice(data);
            let whole = self.pending.len() / pair * pair;
            let pending = core::mem::take(&mut self.pending);
            self.deinterleave(&h, &pending[..whole], out);
            self.pending = pending;
            self.pending.drain(..whole);
        }
        if self.data_seen == u64::from(h.data_size) {
            self.finish(out);
        }
        Ok(())
    }

    /// Converts a final incomplete block pair (split in half between the channels).
    pub fn finish(&mut self, out: &mut Vec<i16>) {
        let Some(h) = self.header else { return };
        let unit = if h.format == FORMAT_SPU_ADPCM { ADPCM_BLOCK } else { 2 };
        let half = self.pending.len() / (2 * unit) * unit;
        if half > 0 {
            let pending = core::mem::take(&mut self.pending);
            let (l, r) = pending.split_at(half);
            self.convert_pair(&h, l, &r[..half], out);
        }
        self.pending.clear();
    }

    /// Converts whole (left block, right block) pairs.
    fn deinterleave(&mut self, h: &AudioHeader, data: &[u8], out: &mut Vec<i16>) {
        let n = h.interleave as usize;
        out.reserve(data.len() / 2 * if h.format == FORMAT_SPU_ADPCM { 4 } else { 1 });
        for pair in data.chunks_exact(2 * n) {
            let (l, r) = pair.split_at(n);
            self.convert_pair(h, l, r, out);
        }
    }

    /// Converts one left and one right piece of the same length.
    fn convert_pair(&mut self, h: &AudioHeader, l: &[u8], r: &[u8], out: &mut Vec<i16>) {
        if h.format == FORMAT_SPU_ADPCM {
            let mut a = [0i16; ADPCM_SAMPLES];
            let mut b = [0i16; ADPCM_SAMPLES];
            for (lb, rb) in l.as_chunks::<ADPCM_BLOCK>().0.iter().zip(r.as_chunks::<ADPCM_BLOCK>().0) {
                self.adpcm[0].decode_block(lb, &mut a);
                self.adpcm[1].decode_block(rb, &mut b);
                for (&x, &y) in a.iter().zip(&b) {
                    out.push(x);
                    out.push(y);
                }
            }
        } else {
            for (a, b) in l.as_chunks::<2>().0.iter().zip(r.as_chunks::<2>().0) {
                out.push(i16::from_le_bytes(*a));
                out.push(i16::from_le_bytes(*b));
            }
        }
    }
}

/// Streaming linear-interpolation resampler for interleaved stereo `i16` (44.1 kHz movie audio to the platform's
/// 48 kHz output). Deterministic: the same input gives the same output in any piece sizes.
#[derive(Clone, Debug)]
pub struct Resampler {
    /// Input frames per output frame, 32.32 fixed point.
    step: u64,
    /// Position of the next output frame in `buf`, 32.32 fixed point.
    pos: u64,
    buf: Vec<[i16; 2]>,
}

impl Resampler {
    /// A resampler from `from` to `to` frames per second.
    pub fn new(from: u32, to: u32) -> Resampler {
        Resampler { step: (u64::from(from) << 32) / u64::from(to.max(1)), pos: 0, buf: Vec::new() }
    }

    /// Converts the interleaved stereo samples in `input`; output frames whose next input frame has not arrived wait
    /// for the next call (or [`Resampler::finish`]).
    pub fn push(&mut self, input: &[i16], out: &mut Vec<i16>) {
        self.buf.extend(input.as_chunks::<2>().0.iter().copied());
        while ((self.pos >> 32) as usize) + 1 < self.buf.len() {
            self.emit(out);
        }
        let used = ((self.pos >> 32) as usize).min(self.buf.len());
        self.buf.drain(..used);
        self.pos -= (used as u64) << 32;
    }

    /// Converts what is left, holding the last input frame.
    pub fn finish(&mut self, out: &mut Vec<i16>) {
        while ((self.pos >> 32) as usize) < self.buf.len() {
            self.emit(out);
        }
        self.buf.clear();
        self.pos = 0;
    }

    fn emit(&mut self, out: &mut Vec<i16>) {
        let i = (self.pos >> 32) as usize;
        let frac = (self.pos & 0xFFFF_FFFF) as i64;
        let a = self.buf[i];
        let b = *self.buf.get(i + 1).unwrap_or(&a);
        for c in 0..2 {
            let (x, y) = (i64::from(a[c]), i64::from(b[c]));
            out.push((x + (((y - x) * frac) >> 32)) as i16);
        }
        self.pos += self.step;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn header_with(format: u32, rate: u32, interleave: u32, data_size: u32) -> Vec<u8> {
        let mut h = Vec::new();
        h.extend_from_slice(b"SShd");
        for v in [0x18u32, format, rate, 2, interleave] {
            h.extend_from_slice(&v.to_le_bytes());
        }
        h.extend_from_slice(&[0xFF; 8]);
        h.extend_from_slice(b"SSbd");
        h.extend_from_slice(&data_size.to_le_bytes());
        h
    }

    fn header(data_size: u32) -> Vec<u8> {
        header_with(FORMAT_PCM16, 48_000, 0x200, data_size)
    }

    #[test]
    fn deinterleaves_blocks_in_any_pieces() {
        // Two block pairs: left samples count up from 0, right samples count down from -1.
        let mut data = Vec::new();
        for pair in 0..2i16 {
            for k in 0..256i16 {
                data.extend_from_slice(&(pair * 256 + k).to_le_bytes());
            }
            for k in 0..256i16 {
                data.extend_from_slice(&(-1 - (pair * 256 + k)).to_le_bytes());
            }
        }
        let mut stream = header(data.len() as u32);
        stream.extend_from_slice(&data);
        stream.extend_from_slice(&[1, 2, 3]); // beyond the declared size
        for piece in [1usize, 7, 1000, stream.len()] {
            let mut a = AudioStream::new();
            let mut out = Vec::new();
            for c in stream.chunks(piece) {
                a.push(c, &mut out).unwrap();
            }
            assert_eq!(out.len(), 1024, "piece {piece}");
            for i in 0..512 {
                assert_eq!(out[2 * i], i as i16);
                assert_eq!(out[2 * i + 1], -1 - i as i16);
            }
            assert_eq!(a.extra_bytes, 3);
            let h = a.header().unwrap();
            assert_eq!((h.sample_rate, h.channels, h.interleave, h.frames()), (48_000, 2, 512, 512));
        }
    }

    #[test]
    fn rejects_bad_headers() {
        let mut h = header(0);
        h[0] = b'X';
        assert!(AudioHeader::parse(&h).is_err());
        let mut h = header(0);
        h[8] = 2;
        assert!(matches!(AudioHeader::parse(&h), Err(Error::Unsupported(_))));
        assert!(AudioHeader::parse(&header(0)[..39]).is_err());
        assert_eq!(AudioHeader::parse(&header(192_000)).unwrap().duration_ns(), 1_000_000_000);
        // ADPCM pieces must be whole blocks.
        assert!(AudioHeader::parse(&header_with(FORMAT_SPU_ADPCM, 44_100, 24, 0)).is_err());
        let h = AudioHeader::parse(&header_with(FORMAT_SPU_ADPCM, 44_100, 32, 64)).unwrap();
        assert_eq!(h.frames(), 56);
    }

    /// Reference decoder written from the formula, sample by sample, for the block decoder to match.
    fn reference_block(block: &[u8; 16], h: &mut (i32, i32)) -> Vec<i16> {
        let s = u32::from(block[0] & 15);
        let f = usize::from(block[0] >> 4);
        let (k1, k2) = [(0, 0), (60, 0), (115, -52), (98, -55), (122, -60)][f];
        let mut out = Vec::new();
        for i in 0..28 {
            let n = (block[2 + i / 2] >> (4 * (i % 2))) & 15;
            let c = if n >= 8 { i32::from(n) - 16 } else { i32::from(n) };
            let mut x = (c * 4096) >> s;
            x += (k1 * h.0 + k2 * h.1 + 32) >> 6;
            let x = x.clamp(-32768, 32767);
            out.push(x as i16);
            h.1 = h.0;
            h.0 = x;
        }
        out
    }

    #[test]
    fn adpcm_blocks_match_the_formula() {
        let mut seed = 0x9E37_79B9u32;
        let mut rand = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        let mut dec = Adpcm::default();
        let mut href = (0, 0);
        for _ in 0..2000 {
            let mut block = [0u8; 16];
            for b in block.iter_mut() {
                *b = rand() as u8;
            }
            block[0] = (block[0] % 5) << 4 | (block[0] % 13);
            let mut out = [0i16; 28];
            dec.decode_block(&block, &mut out);
            assert_eq!(out.to_vec(), reference_block(&block, &mut href));
        }
        // A known block: shift 12, filter 0: codes 1, -1, 7, -8 give 1, -1, 7, -8.
        let mut block = [0u8; 16];
        block[0] = 12;
        block[2] = 0xF1;
        block[3] = 0x87;
        let mut out = [0i16; 28];
        Adpcm::default().decode_block(&block, &mut out);
        assert_eq!(&out[..4], &[1, -1, 7, -8]);
    }

    #[test]
    fn adpcm_stream_keeps_each_channel_continuous() {
        // Four pairs of 32-byte pieces: left and right each one continuous ADPCM stream.
        let mut seed = 7u32;
        let mut data = Vec::new();
        let mut left = Vec::new();
        let mut right = Vec::new();
        for _ in 0..4 {
            for side in [&mut left, &mut right] {
                for _ in 0..32 {
                    seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                    let mut b = (seed >> 16) as u8;
                    if side.len() % 16 == 0 {
                        b = (b % 5) << 4 | (b % 13);
                    }
                    side.push(b);
                    data.push(b);
                }
            }
        }
        let decode = |bytes: &[u8]| {
            let mut d = Adpcm::default();
            let mut all = Vec::new();
            for blk in bytes.as_chunks::<16>().0 {
                let mut o = [0i16; 28];
                d.decode_block(blk, &mut o);
                all.extend_from_slice(&o);
            }
            all
        };
        let (l, r) = (decode(&left), decode(&right));
        let mut stream = header_with(FORMAT_SPU_ADPCM, 44_100, 32, data.len() as u32);
        stream.extend_from_slice(&data);
        for piece in [1usize, 5, 64, stream.len()] {
            let mut a = AudioStream::new();
            let mut out = Vec::new();
            for c in stream.chunks(piece) {
                a.push(c, &mut out).unwrap();
            }
            assert_eq!(out.len(), 2 * l.len(), "piece {piece}");
            for i in 0..l.len() {
                assert_eq!((out[2 * i], out[2 * i + 1]), (l[i], r[i]), "piece {piece} sample {i}");
            }
        }
    }

    #[test]
    fn resampler_rates_and_pieces() {
        // A ramp at 44.1 kHz, 1 s: 48,000 frames out (within one), the ramp preserved, in any piece sizes.
        let input: Vec<i16> = (0..44_100).flat_map(|i| [(i % 30_000) as i16, -((i % 30_000) as i16)]).collect();
        let mut whole = Vec::new();
        let mut r = Resampler::new(44_100, 48_000);
        r.push(&input, &mut whole);
        r.finish(&mut whole);
        assert!((whole.len() / 2).abs_diff(48_000) <= 1, "{}", whole.len() / 2);
        for (k, f) in whole.as_chunks::<2>().0.iter().enumerate().take(29_000) {
            let want = k as f64 * 44_100.0 / 48_000.0;
            assert!((f64::from(f[0]) - want).abs() <= 1.0, "frame {k}");
            assert!((i32::from(f[1]) + i32::from(f[0])).abs() <= 1, "frame {k}");
        }
        for piece in [2usize, 6, 882, 10_000] {
            let mut out = Vec::new();
            let mut r = Resampler::new(44_100, 48_000);
            for c in input.chunks(piece) {
                r.push(c, &mut out);
            }
            r.finish(&mut out);
            assert_eq!(out, whole, "piece {piece}");
        }
        // Same rate: unchanged.
        let mut out = Vec::new();
        let mut r = Resampler::new(48_000, 48_000);
        r.push(&input[..2000], &mut out);
        r.finish(&mut out);
        assert_eq!(out, &input[..2000]);
    }
}
