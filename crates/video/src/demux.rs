//! MPEG-2 program stream demultiplexing (ISO/IEC 13818-1, 2.5) as used by the game's PSS movies
//! (specs/sly1/movies.md 2): video in stream `0xE0`, audio in private stream 1 sub-stream `0xFF`.

use alloc::vec::Vec;
use core::ops::Range;

use crate::Error;

/// Size of every pack in the game's movie files.
pub const PACK_SIZE: usize = 16_384;

/// Stream id of the video stream.
pub const VIDEO_STREAM: u8 = 0xE0;
/// Stream id of private stream 1, which carries the audio.
pub const PRIVATE_STREAM_1: u8 = 0xBD;
/// The audio sub-stream id (the first byte of each private stream 1 payload).
pub const AUDIO_SUBSTREAM: u8 = 0xFF;
/// Bytes at the start of each audio payload that are not audio (`FF A0 00 00`).
pub const AUDIO_PAYLOAD_HEADER: usize = 4;

const PACK_START: u8 = 0xBA;
const END_CODE: u8 = 0xB9;
const SYSTEM_HEADER: u8 = 0xBB;
const PADDING: u8 = 0xBE;

/// A payload taken out of the stream.
#[derive(Debug, PartialEq, Eq)]
pub enum Packet<'a> {
    /// Video elementary-stream bytes, with the packet's presentation time stamp (90 kHz) if it has one.
    Video { data: &'a [u8], pts: Option<u64> },
    /// Audio stream bytes (the 4-byte sub-stream header removed).
    Audio(&'a [u8]),
}

/// Counters of what the demultiplexer has seen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DemuxStats {
    pub packs: u64,
    pub system_headers: u64,
    pub padding_packets: u64,
    pub video_packets: u64,
    pub audio_packets: u64,
    /// Packets of other streams (ignored).
    pub other_packets: u64,
    pub video_bytes: u64,
    pub audio_bytes: u64,
    pub end_code: bool,
}

/// A streaming program-stream parser: push bytes in pieces of any size, take packets out.
#[derive(Default)]
pub struct Demuxer {
    buf: Vec<u8>,
    pos: usize,
    /// Stream offset of `buf[0]`.
    base: u64,
    stats: DemuxStats,
}

impl Demuxer {
    pub fn new() -> Demuxer {
        Demuxer::default()
    }

    pub fn push(&mut self, data: &[u8]) {
        if self.pos > 0 {
            self.buf.drain(..self.pos);
            self.base += self.pos as u64;
            self.pos = 0;
        }
        self.buf.extend_from_slice(data);
    }

    pub fn stats(&self) -> &DemuxStats {
        &self.stats
    }

    /// Whether the end code has been read.
    pub fn ended(&self) -> bool {
        self.stats.end_code
    }

    /// Bytes buffered and not yet parsed.
    pub fn buffered(&self) -> usize {
        self.buf.len() - self.pos
    }

    /// The next payload, or `None` when the buffered data ends inside a packet (or after the end code).
    pub fn next_packet(&mut self) -> Result<Option<Packet<'_>>, Error> {
        loop {
            match self.parse_one()? {
                Parsed::NeedData => return Ok(None),
                Parsed::Skip => {}
                Parsed::Video(r, pts) => return Ok(Some(Packet::Video { data: &self.buf[r], pts })),
                Parsed::Audio(r) => return Ok(Some(Packet::Audio(&self.buf[r]))),
            }
        }
    }

    fn corrupt(&self, what: &'static str) -> Error {
        Error::Container { offset: self.base + self.pos as u64, what }
    }

    fn parse_one(&mut self) -> Result<Parsed, Error> {
        if self.stats.end_code {
            return Ok(Parsed::NeedData);
        }
        let b = &self.buf[self.pos..];
        if b.len() < 4 {
            return Ok(Parsed::NeedData);
        }
        if b[..3] != [0, 0, 1] {
            return Err(self.corrupt("expected a start code"));
        }
        let id = b[3];
        match id {
            PACK_START => {
                if b.len() < 14 {
                    return Ok(Parsed::NeedData);
                }
                if b[4] >> 6 != 1 {
                    return Err(self.corrupt("not an MPEG-2 pack header"));
                }
                let len = 14 + usize::from(b[13] & 7);
                if b.len() < len {
                    return Ok(Parsed::NeedData);
                }
                self.stats.packs += 1;
                self.pos += len;
                Ok(Parsed::Skip)
            }
            END_CODE => {
                self.stats.end_code = true;
                self.pos += 4;
                Ok(Parsed::NeedData)
            }
            SYSTEM_HEADER..=0xFF => {
                if b.len() < 6 {
                    return Ok(Parsed::NeedData);
                }
                let len = 6 + usize::from(u16::from_be_bytes([b[4], b[5]]));
                if b.len() < len {
                    return Ok(Parsed::NeedData);
                }
                let start = self.pos;
                self.pos += len;
                let has_pes_header =
                    !matches!(id, 0xBC | SYSTEM_HEADER | PADDING | 0xBF | 0xF0 | 0xF1 | 0xF2 | 0xF8 | 0xFF);
                match id {
                    SYSTEM_HEADER => self.stats.system_headers += 1,
                    PADDING => self.stats.padding_packets += 1,
                    _ => {}
                }
                if !has_pes_header || (id != VIDEO_STREAM && id != PRIVATE_STREAM_1) {
                    if has_pes_header {
                        self.stats.other_packets += 1;
                    }
                    return Ok(Parsed::Skip);
                }
                let p = &self.buf[start..start + len];
                if len < 9 || p[6] >> 6 != 2 {
                    self.pos = start;
                    return Err(self.corrupt("not an MPEG-2 PES header"));
                }
                let payload = 9 + usize::from(p[8]);
                if payload > len {
                    self.pos = start;
                    return Err(self.corrupt("PES header longer than the packet"));
                }
                if id == VIDEO_STREAM {
                    let pts = (p[7] & 0x80 != 0 && payload >= 14).then(|| read_timestamp(&p[9..14]));
                    self.stats.video_packets += 1;
                    self.stats.video_bytes += (len - payload) as u64;
                    return Ok(Parsed::Video(start + payload..start + len, pts));
                }
                if payload >= len || p[payload] != AUDIO_SUBSTREAM {
                    self.stats.other_packets += 1;
                    return Ok(Parsed::Skip);
                }
                if len - payload < AUDIO_PAYLOAD_HEADER {
                    self.pos = start;
                    return Err(self.corrupt("audio payload shorter than its header"));
                }
                self.stats.audio_packets += 1;
                self.stats.audio_bytes += (len - payload - AUDIO_PAYLOAD_HEADER) as u64;
                Ok(Parsed::Audio(start + payload + AUDIO_PAYLOAD_HEADER..start + len))
            }
            _ => Err(self.corrupt("unexpected start code in a program stream")),
        }
    }
}

enum Parsed {
    NeedData,
    Skip,
    Video(Range<usize>, Option<u64>),
    Audio(Range<usize>),
}

/// Reads a 33-bit PTS or DTS field (5 bytes with marker bits).
fn read_timestamp(b: &[u8]) -> u64 {
    (u64::from(b[0] >> 1) & 7) << 30
        | u64::from(b[1]) << 22
        | (u64::from(b[2]) >> 1) << 15
        | u64::from(b[3]) << 7
        | u64::from(b[4]) >> 1
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;

    /// Builds a pack header (MPEG-2, no stuffing).
    pub(crate) fn pack_header() -> Vec<u8> {
        vec![0, 0, 1, 0xBA, 0x44, 0, 4, 0, 4, 1, 0, 0xFF, 0x53, 0xF8]
    }

    /// Builds a PES packet with an optional PTS.
    pub(crate) fn pes(id: u8, payload: &[u8], pts: Option<u64>) -> Vec<u8> {
        let mut hdr = [0x81u8, 0];
        let mut opt = Vec::new();
        if let Some(t) = pts {
            hdr[1] = 0x80;
            opt = vec![
                0x21 | ((t >> 29) as u8 & 0x0E),
                (t >> 22) as u8,
                ((t >> 14) as u8) | 1,
                (t >> 7) as u8,
                ((t << 1) as u8) | 1,
            ];
        }
        let len = 3 + opt.len() + payload.len();
        let mut v = vec![0, 0, 1, id, (len >> 8) as u8, len as u8, hdr[0], hdr[1], opt.len() as u8];
        v.extend_from_slice(&opt);
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn demuxes_video_and_audio_across_pushes() {
        let mut s = pack_header();
        s.extend_from_slice(&[0, 0, 1, 0xBB, 0, 2, 0xAA, 0xBB]);
        s.extend(pes(0xE0, &[1, 2, 3, 4, 5], Some(123_456_789)));
        s.extend(pes(0xBD, &[0xFF, 0xA0, 0, 0, 9, 8, 7], None));
        s.extend(pes(0xBD, &[0x80, 1, 2, 3], None)); // another sub-stream: ignored
        s.extend_from_slice(&[0, 0, 1, 0xBE, 0, 3, 0xFF, 0xFF, 0xFF]);
        s.extend(pack_header());
        s.extend(pes(0xE0, &[6, 7], None));
        s.extend(pes(0xC0, &[1], None));
        s.extend_from_slice(&[0, 0, 1, 0xB9]);
        // Feed one byte at a time.
        let mut d = Demuxer::new();
        let mut video = Vec::new();
        let mut audio = Vec::new();
        let mut pts = Vec::new();
        for &b in &s {
            d.push(&[b]);
            while let Some(p) = d.next_packet().unwrap() {
                match p {
                    Packet::Video { data, pts: t } => {
                        video.extend_from_slice(data);
                        pts.push(t);
                    }
                    Packet::Audio(a) => audio.extend_from_slice(a),
                }
            }
        }
        assert_eq!(video, [1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(audio, [9, 8, 7]);
        assert_eq!(pts, [Some(123_456_789), None]);
        let st = d.stats();
        assert_eq!((st.packs, st.system_headers, st.padding_packets), (2, 1, 1));
        assert_eq!((st.video_packets, st.audio_packets, st.other_packets), (2, 1, 2));
        assert_eq!((st.video_bytes, st.audio_bytes), (7, 3));
        assert!(st.end_code);
    }

    #[test]
    fn rejects_garbage() {
        let mut d = Demuxer::new();
        d.push(&[1, 2, 3, 4, 5]);
        assert!(matches!(d.next_packet(), Err(Error::Container { offset: 0, .. })));
        let mut d = Demuxer::new();
        let mut s = pack_header();
        s[4] = 0x21; // MPEG-1 pack
        d.push(&s);
        assert!(d.next_packet().is_err());
        let mut d = Demuxer::new();
        d.push(&[0, 0, 1, 0xE0, 0, 3, 0x81, 0x80, 9]);
        assert!(d.next_packet().is_err());
    }
}
