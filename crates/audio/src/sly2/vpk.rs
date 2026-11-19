//! `VPK ` stream files (sly2/audio.md 3): music and other long sounds, several mono ADPCM channels interleaved in
//! 8 KiB blocks after a 2048-byte header.

use alloc::vec::Vec;

use crate::Error;

/// The magic as a little-endian u32 (` KPV` as stored).
pub const MAGIC: u32 = 0x5650_4B20;
/// Size of the header.
pub const HEADER_SIZE: usize = 0x800;
/// Most channels accepted.
pub const MAX_CHANNELS: usize = 8;

/// One channel's settings from the header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VpkChannel {
    /// Volume (127).
    pub volume: u8,
    /// Group number (the music layer, 0..3; 5 for a stereo pair of an other stream).
    pub group: u8,
    /// Pan in degrees (0 centre, 270 left, 90 right).
    pub pan: u16,
}

/// The header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VpkHeader {
    /// ADPCM bytes per channel.
    pub bytes_per_channel: u32,
    /// Offset of the data.
    pub data_offset: u32,
    /// Interleave block size per channel.
    pub interleave: u32,
    /// Sample rate.
    pub sample_rate: u32,
    /// Channels.
    pub channels: Vec<VpkChannel>,
}

impl VpkHeader {
    /// Parses the header.
    pub fn parse(b: &[u8]) -> Result<VpkHeader, Error> {
        let w = |o: usize| -> Result<u32, Error> {
            b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])).ok_or(Error::Truncated {
                what: "VPK header",
                needed: o + 4,
                len: b.len(),
            })
        };
        if w(0)? != MAGIC {
            return Err(Error::BadMagic("VPK "));
        }
        let c = w(20)? as usize;
        if c == 0 || c > MAX_CHANNELS {
            return Err(Error::Invalid("VPK channel count"));
        }
        let mut channels = Vec::with_capacity(c);
        for i in 0..c {
            let v = w(24 + 4 * i)?;
            channels.push(VpkChannel { volume: v as u8, group: (v >> 8) as u8, pan: (v >> 16) as u16 });
        }
        let h = VpkHeader {
            bytes_per_channel: w(4)?,
            data_offset: w(8)?,
            interleave: w(12)?,
            sample_rate: w(16)?,
            channels,
        };
        if h.interleave == 0 || h.interleave % 16 != 0 || h.sample_rate == 0 || h.sample_rate > 192_000 {
            return Err(Error::Invalid("VPK interleave or sample rate"));
        }
        Ok(h)
    }

    /// Seconds per channel.
    pub fn duration(&self) -> f32 {
        (self.bytes_per_channel / 16 * 28) as f32 / self.sample_rate as f32
    }
}

/// A stream file split into contiguous per-channel ADPCM, ready for the mixer (which loops each channel).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VpkStream {
    /// The header.
    pub header: VpkHeader,
    /// Each channel's ADPCM, `bytes_per_channel` long (shorter if the file is cut).
    pub channels: Vec<Vec<u8>>,
}

impl VpkStream {
    /// De-interleaves a whole stream file.
    pub fn parse(file: &[u8]) -> Result<VpkStream, Error> {
        let header = VpkHeader::parse(file)?;
        let n = header.channels.len();
        let il = header.interleave as usize;
        let per = header.bytes_per_channel as usize;
        let data = file.get(header.data_offset as usize..).ok_or(Error::Invalid("VPK data offset"))?;
        let mut channels: Vec<Vec<u8>> = (0..n).map(|_| Vec::with_capacity(per.min(data.len()))).collect();
        let mut pos = 0;
        'outer: while channels.iter().any(|c| c.len() < per) {
            // A file that is not padded to whole rounds (Sly 3) splits its last round evenly between the channels.
            let rest = data.len().saturating_sub(pos);
            let size = if rest >= il * n { il } else { rest / n / 16 * 16 };
            if size == 0 {
                break;
            }
            for ch in channels.iter_mut() {
                let Some(block) = data.get(pos..pos + size) else {
                    break 'outer;
                };
                let take = block.len().min(per - ch.len().min(per));
                ch.extend_from_slice(&block[..take]);
                pos += size;
            }
        }
        Ok(VpkStream { header, channels })
    }

    /// Channel count.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;

    /// A stream file with `c` channels of `per` bytes each; channel `k`'s bytes are all `k + 1`.
    pub(crate) fn file(c: usize, per: usize, il: usize, rate: u32) -> Vec<u8> {
        let mut f = vec![0u8; HEADER_SIZE];
        let put = |f: &mut Vec<u8>, o: usize, v: u32| f[o..o + 4].copy_from_slice(&v.to_le_bytes());
        put(&mut f, 0, MAGIC);
        put(&mut f, 4, per as u32);
        put(&mut f, 8, HEADER_SIZE as u32);
        put(&mut f, 12, il as u32);
        put(&mut f, 16, rate);
        put(&mut f, 20, c as u32);
        for k in 0..c {
            put(&mut f, 24 + 4 * k, 127 | (k as u32) << 8);
        }
        let blocks = per.div_ceil(il);
        for _ in 0..blocks {
            for k in 0..c {
                f.extend(core::iter::repeat_n(k as u8 + 1, il));
            }
        }
        f
    }

    #[test]
    fn deinterleaves() {
        let s = VpkStream::parse(&file(3, 80, 32, 32_000)).unwrap();
        assert_eq!(s.channel_count(), 3);
        for (k, ch) in s.channels.iter().enumerate() {
            assert_eq!(ch.len(), 80, "padding beyond the channel length is dropped");
            assert!(ch.iter().all(|&b| b == k as u8 + 1));
        }
        assert_eq!(s.header.channels[2], VpkChannel { volume: 127, group: 2, pan: 0 });
        assert!((s.header.duration() - 5.0 * 28.0 / 32_000.0).abs() < 1e-6);
        let mut bad = file(1, 16, 16, 32_000);
        bad[0] = 0;
        assert!(VpkStream::parse(&bad).is_err());
        // A last round shorter than a full one is split evenly between the channels.
        let f = file(2, 64, 32, 32_000);
        let s = VpkStream::parse(&f[..HEADER_SIZE + 96]).unwrap();
        assert_eq!((s.channels[0].len(), s.channels[1].len()), (48, 48));
    }
}
