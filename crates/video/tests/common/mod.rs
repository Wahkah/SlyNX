//! Shared checks for the Sly 2 and Sly 3 movie disc tests.

use std::fs::File;
use std::path::Path;

use formats::hash::{Sha256, hex};
use formats::iso::ReadAt;
use formats_sly2::record::FileRecord;
use video::SequenceHeader;
use video::audio::{AudioHeader, AudioStream};
use video::decoder::{Decoded, Decoder};
use video::demux::{Demuxer, PACK_SIZE, Packet};

/// One movie's test vectors.
pub struct Vector {
    pub identity: u32,
    pub sector: u32,
    pub bytes: u32,
    pub width: u32,
    /// bit_rate field in units of 400 bit/s.
    pub bit_rate: u32,
    pub frame_rate_code: u8,
    pub pictures: u64,
    pub audio_rate: u32,
    pub audio_bytes: u32,
    pub sha256: &'static str,
}

/// What one pass over a movie found.
pub struct Checked {
    pub sha256: String,
    pub packs: u64,
    pub pictures: u64,
    pub audio: Option<AudioHeader>,
    pub audio_frames: u64,
    pub first_audio_payload: Vec<u8>,
    /// Left channel samples, when asked for.
    pub left: Vec<i16>,
    pub left_equals_right: bool,
    pub seq: Option<SequenceHeader>,
    /// Frames and slice errors of the first GOP, decoded on its own.
    pub first_gop: (u64, u64, u64),
    /// Frames, slice errors and pictures of the last two GOPs, decoded from the second-to-last GOP header.
    pub last_gops: (u64, u64, u64),
}

const TAIL: usize = 4 << 20;

fn start_codes(es: &[u8], code: u8) -> Vec<usize> {
    es.windows(4).enumerate().filter(|(_, w)| w[..3] == [0, 0, 1] && w[3] == code).map(|(i, _)| i).collect()
}

/// Decodes `es` to the end; returns (frames, slice errors).
fn decode(es: &[u8]) -> (u64, u64) {
    let mut d = Decoder::new();
    d.push(es);
    d.finish();
    let mut frames = 0;
    loop {
        match d.decode().expect("decode") {
            Decoded::Frame(_) => frames += 1,
            Decoded::End => break,
            Decoded::NeedData => unreachable!(),
        }
    }
    (frames, d.stats().slice_errors)
}

/// Reads, hashes and demultiplexes a whole movie, decodes all its audio and its first and last GOPs.
pub fn check(path: &Path, f: &FileRecord, keep_left: bool) -> Checked {
    let mut file = File::open(path).expect("open image");
    let mut sha = Sha256::new();
    let mut demux = Demuxer::new();
    let mut audio = AudioStream::new();
    let mut samples = Vec::new();
    let mut out = Checked {
        sha256: String::new(),
        packs: 0,
        pictures: 0,
        audio: None,
        audio_frames: 0,
        first_audio_payload: Vec::new(),
        left: Vec::new(),
        left_equals_right: true,
        seq: None,
        first_gop: (0, 0, 0),
        last_gops: (0, 0, 0),
    };
    // The elementary stream: the first 2 MiB, and a sliding window over the last few MiB.
    let mut head: Vec<u8> = Vec::new();
    let mut tail: Vec<u8> = Vec::new();
    let mut boundary = [0xFFu8; 3];
    let mut buf = vec![0u8; 1 << 20];
    let mut done = 0u64;
    while done < u64::from(f.size) {
        let n = (u64::from(f.size) - done).min(buf.len() as u64) as usize;
        file.read_at(f.offset() + done, &mut buf[..n]).expect("read");
        done += n as u64;
        sha.update(&buf[..n]);
        demux.push(&buf[..n]);
        while let Some(p) = demux.next_packet().expect("demux") {
            match p {
                Packet::Video { data, .. } => {
                    let mut joined = boundary.to_vec();
                    joined.extend_from_slice(data);
                    out.pictures += joined.windows(4).filter(|w| w == &[0, 0, 1, 0]).count() as u64;
                    boundary.copy_from_slice(&joined[joined.len() - 3..]);
                    if head.len() < 2 << 20 {
                        head.extend_from_slice(data);
                    }
                    tail.extend_from_slice(data);
                    if tail.len() > 2 * TAIL {
                        tail.drain(..tail.len() - TAIL);
                    }
                }
                Packet::Audio(a) => {
                    if out.first_audio_payload.is_empty() {
                        out.first_audio_payload = a[..24.min(a.len())].to_vec();
                    }
                    samples.clear();
                    audio.push(a, &mut samples).expect("audio");
                    out.audio_frames += samples.len() as u64 / 2;
                    for pair in samples.as_chunks::<2>().0 {
                        out.left_equals_right &= pair[0] == pair[1];
                        if keep_left {
                            out.left.push(pair[0]);
                        }
                    }
                }
            }
        }
    }
    samples.clear();
    audio.finish(&mut samples);
    out.audio_frames += samples.len() as u64 / 2;
    out.sha256 = hex(&sha.finish());
    out.packs = demux.stats().packs;
    out.audio = audio.header().copied();

    // First GOP: from the start to the second GOP header.
    let gops = start_codes(&head, 0xB8);
    assert!(gops.len() >= 2, "first GOP longer than 2 MiB");
    let first = &head[..gops[1]];
    let (frames, errors) = decode(first);
    out.first_gop = (frames, errors, start_codes(first, 0x00).len() as u64);
    // The sequence header (and its extensions) from the start, then the last two GOPs.
    let mut d = Decoder::new();
    d.push(&head[..gops[0]]);
    d.finish();
    while !matches!(d.decode().expect("sequence"), Decoded::End) {}
    out.seq = d.sequence().cloned();
    let tg = start_codes(&tail, 0xB8);
    assert!(tg.len() >= 2, "fewer than two GOPs in the tail window");
    let from = tg[tg.len() - 2];
    let mut es = head[..gops[0]].to_vec();
    es.extend_from_slice(&tail[from..]);
    let (frames, errors) = decode(&es);
    out.last_gops = (frames, errors, start_codes(&tail[from..], 0x00).len() as u64);
    out
}

/// Asserts a movie against its vector (the audio duration within 0.1 s of the video's when `durations`).
pub fn assert_vector(id: &str, f: &FileRecord, v: &Vector, c: &Checked) {
    assert_eq!((f.identity, f.sector, f.size), (v.identity, v.sector, v.bytes), "{id}");
    assert_eq!(c.sha256, v.sha256, "{id}");
    assert_eq!(u64::from(v.bytes), c.packs * PACK_SIZE as u64 + 4, "{id}: whole packs and the end code");
    assert_eq!(c.pictures, v.pictures, "{id}");
    let s = c.seq.as_ref().expect("sequence header");
    assert_eq!((s.width, s.height, s.profile_and_level, s.aspect_ratio_information), (v.width, 480, 0x48, 2), "{id}");
    assert_eq!((s.frame_rate_code, s.bit_rate, s.vbv_buffer_size), (v.frame_rate_code, v.bit_rate, 112), "{id}");
    assert!(!s.progressive_sequence && s.chroma_format == 1, "{id}");
    let h = c.audio.unwrap_or_else(|| panic!("{id}: no audio header"));
    assert_eq!((h.format, h.channels, h.interleave), (0x10, 2, 0x20), "{id}: SPU ADPCM, 32-byte interleave");
    assert_eq!((h.sample_rate, h.data_size), (v.audio_rate, v.audio_bytes), "{id}");
    assert_eq!(c.audio_frames, h.frames(), "{id}: every sample decoded");
    assert_eq!(&c.first_audio_payload[..4], b"SShd", "{id}");
    // First and last GOPs decode without a slice error and give a frame per picture.
    let (frames, errors, pictures) = c.first_gop;
    assert_eq!((errors, frames), (0, pictures), "{id}: first GOP");
    assert!(pictures >= 10, "{id}: first GOP of {pictures} pictures");
    let (frames, errors, pictures) = c.last_gops;
    assert_eq!((errors, frames), (0, pictures), "{id}: last GOPs");
}
