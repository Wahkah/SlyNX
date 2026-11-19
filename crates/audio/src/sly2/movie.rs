//! Movie soundtracks (sly2/movies.md, sly1/movies.md 2.2): the audio stream carried in the movies' private stream
//! (after the demultiplexer has removed the 4-byte sub-stream header) starts with an `SShd` header and an `SSbd`
//! data header. Sly 2 stores SPU ADPCM (format 16) in 32-byte pieces per channel; Sly 1 stores 16-bit PCM
//! (format 1) in 512-byte pieces. [`MovieAudio`] decodes either, incrementally, to interleaved stereo `i16`.

use alloc::vec::Vec;

use crate::Error;
use crate::adpcm::{BLOCK_BYTES, BLOCK_SAMPLES, History, decode_block};

/// Format code: 16-bit little-endian PCM (Sly 1).
pub const FORMAT_PCM16: u32 = 1;
/// Format code: SPU ADPCM (Sly 2).
pub const FORMAT_ADPCM: u32 = 16;
/// Size of the two headers together.
pub const HEADERS_SIZE: usize = 40;

/// The `SShd` and `SSbd` fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MovieAudioHeader {
    /// Format code.
    pub format: u32,
    /// Sample rate.
    pub sample_rate: u32,
    /// Channels (2 in every movie).
    pub channels: u32,
    /// Bytes per channel piece.
    pub interleave: u32,
    /// Size of the audio data.
    pub data_size: u32,
}

impl MovieAudioHeader {
    /// Parses the 40 header bytes.
    pub fn parse(b: &[u8]) -> Result<MovieAudioHeader, Error> {
        if b.len() < HEADERS_SIZE {
            return Err(Error::Truncated { what: "movie audio header", needed: HEADERS_SIZE, len: b.len() });
        }
        let w = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        if &b[0..4] != b"SShd" {
            return Err(Error::BadMagic("SShd"));
        }
        if &b[32..36] != b"SSbd" {
            return Err(Error::BadMagic("SSbd"));
        }
        let h =
            MovieAudioHeader { format: w(8), sample_rate: w(12), channels: w(16), interleave: w(20), data_size: w(36) };
        let unit = if h.format == FORMAT_ADPCM { BLOCK_BYTES as u32 } else { 2 };
        if !(1..=2).contains(&h.channels) || h.interleave == 0 || !h.interleave.is_multiple_of(unit) {
            return Err(Error::Invalid("movie audio channels or interleave"));
        }
        if h.format != FORMAT_ADPCM && h.format != FORMAT_PCM16 {
            return Err(Error::Invalid("movie audio format"));
        }
        Ok(h)
    }

    /// Samples per channel in the whole stream.
    pub fn frames(&self) -> u64 {
        let per_channel = self.data_size as u64 / self.channels as u64;
        if self.format == FORMAT_ADPCM { per_channel / 16 * 28 } else { per_channel / 2 }
    }
}

/// Incremental decoder: push the audio stream in pieces of any size, get interleaved stereo `i16` (a mono stream
/// is duplicated to both sides).
#[derive(Clone, Debug, Default)]
pub struct MovieAudio {
    buf: Vec<u8>,
    header: Option<MovieAudioHeader>,
    used: u64,
    hist: [History; 2],
}

impl MovieAudio {
    /// A decoder waiting for the headers.
    pub fn new() -> MovieAudio {
        MovieAudio::default()
    }

    /// The headers, once read.
    pub fn header(&self) -> Option<&MovieAudioHeader> {
        self.header.as_ref()
    }

    /// Feeds stream bytes; appends decoded frames (L, R) to `out`.
    pub fn push(&mut self, data: &[u8], out: &mut Vec<i16>) -> Result<(), Error> {
        self.buf.extend_from_slice(data);
        let mut start = 0;
        if self.header.is_none() {
            if self.buf.len() < HEADERS_SIZE {
                return Ok(());
            }
            self.header = Some(MovieAudioHeader::parse(&self.buf)?);
            start = HEADERS_SIZE;
        }
        let Some(h) = self.header else { return Ok(()) };
        let piece = h.interleave as usize;
        let unit = piece * h.channels as usize;
        while self.buf.len() - start >= unit && self.used < h.data_size as u64 {
            let group = &self.buf[start..start + unit];
            let n = if h.format == FORMAT_ADPCM { piece / BLOCK_BYTES * BLOCK_SAMPLES } else { piece / 2 };
            let first = out.len();
            out.resize(first + n * 2, 0);
            for ch in 0..h.channels as usize {
                let src = &group[ch * piece..(ch + 1) * piece];
                let mut k = 0;
                if h.format == FORMAT_ADPCM {
                    let mut pcm = [0i16; BLOCK_SAMPLES];
                    for block in src.as_chunks::<BLOCK_BYTES>().0 {
                        decode_block(block, &mut self.hist[ch], &mut pcm);
                        for &s in &pcm {
                            out[first + k * 2 + ch] = s;
                            k += 1;
                        }
                    }
                } else {
                    for s in src.as_chunks::<2>().0 {
                        out[first + k * 2 + ch] = i16::from_le_bytes(*s);
                        k += 1;
                    }
                }
            }
            if h.channels == 1 {
                for k in 0..n {
                    out[first + k * 2 + 1] = out[first + k * 2];
                }
            }
            start += unit;
            self.used += unit as u64;
        }
        self.buf.drain(..start);
        Ok(())
    }

    /// Whether all of the data has been decoded.
    pub fn done(&self) -> bool {
        self.header.is_some_and(|h| self.used >= h.data_size as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adpcm::tests::block;
    use alloc::vec;

    fn headers(format: u32, interleave: u32, size: u32) -> Vec<u8> {
        let mut h = vec![0u8; HEADERS_SIZE];
        h[0..4].copy_from_slice(b"SShd");
        h[4..8].copy_from_slice(&0x18u32.to_le_bytes());
        h[8..12].copy_from_slice(&format.to_le_bytes());
        h[12..16].copy_from_slice(&44_100u32.to_le_bytes());
        h[16..20].copy_from_slice(&2u32.to_le_bytes());
        h[20..24].copy_from_slice(&interleave.to_le_bytes());
        h[24..32].fill(0xFF);
        h[32..36].copy_from_slice(b"SSbd");
        h[36..40].copy_from_slice(&size.to_le_bytes());
        h
    }

    #[test]
    fn adpcm_pieces_of_32_bytes() {
        // Left: codes 1, right: codes -1, two blocks per piece, two groups.
        let mut s = headers(FORMAT_ADPCM, 32, 128);
        for _ in 0..2 {
            s.extend_from_slice(&block(12, 0, 0, &[1; 28]));
            s.extend_from_slice(&block(12, 0, 0, &[1; 28]));
            s.extend_from_slice(&block(12, 0, 0, &[-1; 28]));
            s.extend_from_slice(&block(12, 0, 0, &[-1; 28]));
        }
        let mut d = MovieAudio::new();
        let mut out = Vec::new();
        // Pushed in odd pieces.
        for chunk in s.chunks(7) {
            d.push(chunk, &mut out).unwrap();
        }
        assert!(d.done());
        assert_eq!(out.len(), 2 * 112);
        assert!(out.chunks(2).all(|f| f == [1, -1]));
        assert_eq!(d.header().unwrap().frames(), 112);
    }

    #[test]
    fn pcm_pieces() {
        let mut s = headers(FORMAT_PCM16, 4, 8);
        for v in [1i16, 2, -1, -2] {
            s.extend_from_slice(&v.to_le_bytes());
        }
        let mut d = MovieAudio::new();
        let mut out = Vec::new();
        d.push(&s, &mut out).unwrap();
        assert_eq!(out, [1, -1, 2, -2]);
        assert!(MovieAudioHeader::parse(&headers(7, 4, 8)).is_err());
        assert!(MovieAudioHeader::parse(&s[..20]).is_err());
    }
}
