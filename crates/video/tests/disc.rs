//! Tests against the player's own Sly 1 disc image (specs/sly1/movies.md test vectors). They skip, and pass, when
//! `SLYNX_DISC_SLY1` is not set. Movies are read straight from the image in chunks, several at once.

use std::fs::File;
use std::path::{Path, PathBuf};

use formats::disc::Disc;
use formats::hash::{Sha256, hex};
use formats::iso::ReadAt;
use harness::golden::{Game, require_disc};
use video::audio::{AudioHeader, AudioStream};
use video::decoder::{Decoded, Decoder, Parallel};
use video::demux::{Demuxer, PACK_SIZE, Packet};
use video::movies::{Region, movie_table};
use video::{MoviePlayer, PictureType, PlayerStatus};

/// One row of movies.md 5: identity, LBA, size, packs, pictures, audio data bytes, video elementary-stream bytes,
/// SHA-256 of the file.
struct Vector {
    identity: u32,
    lba: u32,
    size: u32,
    packs: u64,
    pictures: u64,
    audio: u64,
    video: u64,
    sha256: &'static str,
}

#[rustfmt::skip]
const VECTORS: [Vector; 17] = [
    Vector { identity: 0x344, lba: 0xF6A14, size: 11_223_044, packs: 685, pictures: 409, audio: 2_621_440, video: 8_521_535, sha256: "3fb688e6833fd27311e2530ed5e6bb9dff47792fcf84708b5294327c84568b41" },
    Vector { identity: 0x340, lba: 0x4FFF, size: 89_325_572, packs: 5_452, pictures: 3_259, audio: 20_855_808, video: 67_955_913, sha256: "ba48dd230268519f222c597ed967b89b2412d1391074ea11575e14f0fb36a9b4" },
    Vector { identity: 0x341, lba: 0x3D3A9, size: 90_456_068, packs: 5_521, pictures: 3_300, audio: 21_122_048, video: 68_810_930, sha256: "8224bc26ac498cf3a45e5548cb1575f4ac0e7e3e96c22b728813abcb1aa94bf3" },
    Vector { identity: 0x33C, lba: 0x10678E, size: 353_697_796, packs: 21_588, pictures: 12_900, audio: 82_642_944, video: 269_022_833, sha256: "ce1e8842f9ac907bffe459f30036e425e6c82e9c44fa107c95184b1e5ad390da" },
    Vector { identity: 0x33F, lba: 0x17588, size: 225_787_908, packs: 13_781, pictures: 8_235, audio: 52_757_504, video: 171_726_249, sha256: "d78255226ac699b77eac4e7324f31f6ef372987fd0cc5a43c89e2097e57a0996" },
    Vector { identity: 0x345, lba: 0x67BF1, size: 50_872_324, packs: 3_105, pictures: 1_856, audio: 11_878_400, video: 38_697_515, sha256: "3200cda7efec117a1abca8127e555c4b20ac078cfdaea9c2842f34736b16e3d5" },
    Vector { identity: 0x346, lba: 0xD9EEC, size: 37_715_972, packs: 2_302, pictures: 1_376, audio: 8_806_400, video: 28_687_515, sha256: "457fb6b0a33be3781219cf434e202e59256b2de1dc7394bd315ce6d5a867ed9f" },
    Vector { identity: 0x33D, lba: 0x799DF, size: 74_596_356, packs: 4_553, pictures: 2_721, audio: 17_415_168, video: 56_736_368, sha256: "963f00f75f161a6ae6f021e6dc927c52d22dfa7183db9946e6b082ae58ff9eaf" },
    Vector { identity: 0x33E, lba: 0x5CA8C, size: 38_502_404, packs: 2_350, pictures: 1_405, audio: 8_985_600, video: 29_292_289, sha256: "5ff11b9d307652acbfbec612966172d2a78707e081e16d618073177ee831df49" },
    Vector { identity: 0x347, lba: 0xF0F6C, size: 46_710_788, packs: 2_851, pictures: 1_703, audio: 10_931_200, video: 35_506_825, sha256: "113df4f4de3f19cd56cb5510121d62b93d1a953dbf25c3d8d8a4c31e647d24fd" },
    Vector { identity: 0x348, lba: 0x32431, size: 25_214_980, packs: 1_539, pictures: 920, audio: 5_888_000, video: 19_178_012, sha256: "f63cc2f54be1f04a0423010335c33b6e6a1903798536e05b436c07a78733c82b" },
    Vector { identity: 0x342, lba: 0xE98F3, size: 62_111_748, packs: 3_791, pictures: 2_266, audio: 14_502_912, video: 47_247_723, sha256: "ce102b637e4128436dc25255635efa551c6396f727c166304815c024fc8a4605" },
    Vector { identity: 0x343, lba: 0x100C92, size: 42_270_724, packs: 2_580, pictures: 1_546, audio: 9_779_200, video: 32_232_724, sha256: "aa7b8b010543b412b93788cc1a0742b224d53569a2c99c10606a6fde038805af" },
    Vector { identity: 0x336, lba: 0xD5B7D, size: 29_868_036, packs: 1_823, pictures: 1_089, audio: 6_977_536, video: 22_702_367, sha256: "edeaff63f77715039285e69b4c8b9aef6a1de6d0e9893c2d31a5c8730f4adc05" },
    Vector { identity: 0x33A, lba: 0x13D429, size: 88_932_356, packs: 5_428, pictures: 3_243, audio: 20_783_104, video: 67_622_243, sha256: "1738dfa6d07fd19e5ad71e5d79abd020bfb6d82acc542db2e5c7816bfd07a2d4" },
    Vector { identity: 0x33B, lba: 0x617F0, size: 39_878_660, packs: 2_434, pictures: 1_455, audio: 9_312_256, video: 30_334_998, sha256: "1dd9973dfe5e8816dc444ca440c7c1ccccbfd86c2e988566bb4f72edb74c797d" },
    Vector { identity: 0x338, lba: 0x513F6, size: 93_700_100, packs: 5_719, pictures: 3_417, audio: 21_899_264, video: 71_250_871, sha256: "fd70a97b96f42c661bd80d9792dbd9e67c72c4696f36530d018c6cdd18572b58" },
];

/// movies.md 1.2: the game's titles of table entries 1 to 16, and each movie's duration in seconds.
const TITLES: [&str; 16] = [
    "The Secret Police File",
    "The Tokyo Police File",
    "The Making Of Sly Cooper",
    "Commercials & Outtakes",
    "Tide of Terror",
    "Raleigh Hung out to Dry",
    "Sunset Snake Eyes",
    "Muggshot Busted",
    "Vicious Voodoo",
    "Mz Ruby Vexed",
    "Fire in the Sky",
    "The Panda King on Ice",
    "The Cold Heart of Hate",
    "Ten Seconds",
    "The Thievius Raccoonus",
    "Credits",
];
const DURATIONS: [f64; 17] =
    [13.65, 108.7, 110.1, 430.4, 274.8, 61.9, 45.9, 90.8, 46.9, 56.9, 30.7, 75.6, 51.6, 36.3, 108.2, 48.5, 114.0];

/// movies.md 5: the second copies referenced from worlds (identity, LBA).
const SECOND_COPIES: [(u32, u32); 13] = [
    (0x340, 0x6EDB0),
    (0x344, 0xC8F03),
    (0x347, 0xF89FC),
    (0x345, 0x1147F),
    (0x342, 0x132E51),
    (0x33D, 0x82FA2),
    (0x336, 0x148568),
    (0x33C, 0x90D55),
    (0x341, 0xDE6DD),
    (0x346, 0x8C2BE),
    (0x33E, 0x4899E),
    (0x348, 0x4DD03),
    (0x343, 0x3547A),
];

const SECTOR: u64 = 2048;
const CHUNK: usize = 1 << 20;

/// Reads `size` bytes at sector `lba` of the image in chunks.
fn read_chunks(path: &Path, lba: u32, size: u32, mut f: impl FnMut(&[u8])) {
    let mut file = File::open(path).expect("open image");
    let mut buf = vec![0u8; CHUNK];
    let mut done = 0u64;
    while done < u64::from(size) {
        let n = (u64::from(size) - done).min(CHUNK as u64) as usize;
        file.read_at(u64::from(lba) * SECTOR + done, &mut buf[..n]).expect("read");
        f(&buf[..n]);
        done += n as u64;
    }
}

fn sha256_hex(path: &Path, lba: u32, size: u32) -> String {
    let mut h = Sha256::new();
    read_chunks(path, lba, size, |c| {
        h.update(c);
    });
    hex(&h.finish())
}

/// Runs `f` for every vector on its own thread and returns the results in table order.
fn per_movie<T: Send>(f: impl Fn(&Vector) -> T + Sync) -> Vec<T> {
    std::thread::scope(|s| {
        let hs: Vec<_> = VECTORS.iter().map(|v| s.spawn(|| f(v))).collect();
        hs.into_iter().map(|h| h.join().expect("movie thread")).collect()
    })
}

fn disc(test: &str) -> Option<PathBuf> {
    require_disc(Game::Sly1, test)
}

#[test]
fn movie_table_and_titles() {
    let Some(path) = disc("movie_table_and_titles") else { return };
    let d = Disc::open_path(&path).expect("open disc");
    let table = movie_table(&d.executable).expect("movie table");
    assert_eq!(table.len(), 17);
    for (e, v) in table.iter().zip(&VECTORS) {
        assert_eq!(e.identity(), v.identity, "entry {}", e.index);
        assert_eq!(e.descriptor.sector, v.lba, "entry {}", e.index);
        assert_eq!(e.descriptor.size, v.size, "entry {}", e.index);
        if e.index == 0 {
            assert_eq!(e.title, None);
        } else {
            assert_eq!(e.title.as_deref(), Some(TITLES[e.index - 1]));
        }
    }
}

struct Container {
    packs: u64,
    video_bytes: u64,
    audio_bytes: u64,
    audio: Option<AudioHeader>,
    audio_samples: u64,
    pictures: u64,
    first_pack: Vec<u8>,
    first_audio: Vec<u8>,
    end_code: bool,
    sha256: String,
}

/// Hashes and demultiplexes a whole movie in one pass.
fn container(path: &Path, v: &Vector) -> Container {
    let mut sha = Sha256::new();
    let mut demux = Demuxer::new();
    let mut audio = AudioStream::new();
    let mut samples = Vec::new();
    let mut audio_samples = 0u64;
    let mut first_pack = Vec::new();
    let mut first_audio = Vec::new();
    // Picture start codes, counted across packet boundaries.
    let mut tail = [0xFFu8; 3];
    let mut pictures = 0u64;
    read_chunks(path, v.lba, v.size, |chunk| {
        sha.update(chunk);
        if first_pack.is_empty() {
            first_pack = chunk[..14].to_vec();
        }
        demux.push(chunk);
        while let Some(p) = demux.next_packet().expect("demux") {
            match p {
                Packet::Video { data, .. } => {
                    let mut joined = tail.to_vec();
                    joined.extend_from_slice(data);
                    pictures += joined.windows(4).filter(|w| w == &[0, 0, 1, 0]).count() as u64;
                    let n = joined.len();
                    tail.copy_from_slice(&joined[n - 3..]);
                }
                Packet::Audio(a) => {
                    if first_audio.is_empty() {
                        first_audio = a[..24.min(a.len())].to_vec();
                    }
                    samples.clear();
                    audio.push(a, &mut samples).expect("audio");
                    audio_samples += samples.len() as u64;
                }
            }
        }
    });
    let st = *demux.stats();
    Container {
        packs: st.packs,
        video_bytes: st.video_bytes,
        audio_bytes: audio.data_bytes(),
        audio: audio.header().copied(),
        audio_samples,
        pictures,
        first_pack,
        first_audio,
        end_code: st.end_code,
        sha256: hex(&sha.finish()),
    }
}

#[test]
fn container_vectors() {
    let Some(path) = disc("container_vectors") else { return };
    let results = per_movie(|v| container(&path, v));
    for ((i, v), c) in VECTORS.iter().enumerate().zip(&results) {
        let id = format!("movie {i} ({:#x})", v.identity);
        assert_eq!(c.sha256, v.sha256, "{id}");
        assert_eq!(c.packs, v.packs, "{id}");
        assert_eq!(u64::from(v.size), c.packs * PACK_SIZE as u64 + 4, "{id}: whole packs plus the end code");
        assert!(c.end_code, "{id}");
        assert_eq!(c.video_bytes, v.video, "{id}");
        assert_eq!(c.pictures, v.pictures, "{id}");
        let h = c.audio.unwrap_or_else(|| panic!("{id}: no audio header"));
        assert_eq!((h.format, h.sample_rate, h.channels, h.interleave), (1, 48_000, 2, 0x200), "{id}");
        assert_eq!(u64::from(h.data_size), v.audio, "{id}");
        assert_eq!(c.audio_bytes, v.audio, "{id}: audio bytes after the headers");
        assert_eq!(c.audio_samples, v.audio / 2, "{id}: 16-bit samples, both channels");
        // The table's durations are rounded from the audio's or the video's length (pictures / 29.97); the two
        // agree within 0.12 s (0.118 s for 0x340), except 0x343 whose audio is 0.65 s shorter.
        let audio_s = h.duration_ns() as f64 / 1e9;
        let video_s = v.pictures as f64 * 1001.0 / 30000.0;
        let off = (audio_s - DURATIONS[i]).abs().min((video_s - DURATIONS[i]).abs());
        assert!(off < 0.1, "{id}: table {} s, audio {audio_s} s, video {video_s} s", DURATIONS[i]);
        let tolerance = if v.identity == 0x343 { 0.7 } else { 0.12 };
        assert!((audio_s - video_s).abs() <= tolerance, "{id}: audio {audio_s} s, video {video_s} s");
    }
    // Byte checks on the logo movie.
    let logo = &results[0];
    assert_eq!(logo.first_pack, [0x00, 0x00, 0x01, 0xBA, 0x44, 0x00, 0x04, 0x00, 0x04, 0x01, 0x00, 0xFF, 0x53, 0xF8]);
    // After the 4-byte sub-stream header: "SShd", size 0x18, format 1, 48000 Hz, 2 channels, interleave 0x200.
    assert_eq!(
        logo.first_audio,
        [
            0x53, 0x53, 0x68, 0x64, 0x18, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80, 0xBB, 0x00, 0x00, 0x02, 0x00,
            0x00, 0x00, 0x00, 0x02, 0x00, 0x00
        ]
    );
}

#[test]
fn second_copies_match() {
    let Some(path) = disc("second_copies_match") else { return };
    let hashes: Vec<(u32, String)> = std::thread::scope(|s| {
        let hs: Vec<_> = SECOND_COPIES
            .iter()
            .map(|&(identity, lba)| {
                let v = VECTORS.iter().find(|v| v.identity == identity).expect("identity in the table");
                let path = &path;
                s.spawn(move || (identity, sha256_hex(path, lba, v.size)))
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("thread")).collect()
    });
    for (identity, h) in hashes {
        let v = VECTORS.iter().find(|v| v.identity == identity).expect("identity");
        assert_eq!(h, v.sha256, "second copy of {identity:#x}");
    }
}

/// FNV-1a over the visible planes of every frame, in display order.
#[derive(Default)]
struct FrameHash(u64);

impl FrameHash {
    fn add(&mut self, f: &video::Frame) {
        if self.0 == 0 {
            self.0 = 0xCBF2_9CE4_8422_2325;
        }
        for plane in [&f.y, &f.cb, &f.cr] {
            for &b in plane.iter() {
                self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x100_0000_01B3);
            }
        }
    }
}

struct Decode {
    frames: u64,
    types: [u64; 3],
    errors: u64,
    first_error: Option<video::Error>,
    hash: u64,
    in_order: bool,
    frame_layout_ok: bool,
    seq: Option<video::SequenceHeader>,
    gops: u64,
    closed_gops: u64,
    broken_links: u64,
    unsupported_modes: u64,
}

/// Decodes a whole movie, streaming it through the demultiplexer as a player would.
fn decode(path: &Path, v: &Vector, parallel: Option<Box<dyn Parallel>>) -> Decode {
    let mut demux = Demuxer::new();
    let mut dec = Decoder::new();
    dec.set_parallel(parallel);
    let mut out = Decode {
        frames: 0,
        types: [0; 3],
        errors: 0,
        first_error: None,
        hash: 0,
        in_order: true,
        frame_layout_ok: true,
        seq: None,
        gops: 0,
        closed_gops: 0,
        broken_links: 0,
        unsupported_modes: 0,
    };
    let mut hash = FrameHash::default();
    let mut take = |dec: &mut Decoder, out: &mut Decode| loop {
        match dec.decode().expect("decode") {
            Decoded::Frame(f) => {
                out.in_order &= f.info.display_index == out.frames;
                out.frame_layout_ok &= !f.info.field_pictures && f.info.top_field_first;
                out.types[f.info.picture_type as usize - 1] += 1;
                out.frames += 1;
                hash.add(f);
            }
            Decoded::NeedData => return,
            Decoded::End => return,
        }
    };
    read_chunks(path, v.lba, v.size, |chunk| {
        demux.push(chunk);
        while let Some(p) = demux.next_packet().expect("demux") {
            if let Packet::Video { data, .. } = p {
                dec.push(data);
            }
        }
        take(&mut dec, &mut out);
    });
    dec.finish();
    take(&mut dec, &mut out);
    let s = dec.stats();
    out.errors = s.slice_errors;
    out.first_error = s.first_error.clone();
    out.seq = dec.sequence().cloned();
    (out.gops, out.closed_gops, out.broken_links) = (s.gops, s.closed_gops, s.broken_links);
    out.unsupported_modes = s.mb.field_motion + s.mb.dual_prime + s.mb.sixteen_by_eight;
    out.hash = hash.0;
    out
}

#[test]
fn decode_every_movie() {
    let Some(path) = disc("decode_every_movie") else { return };
    let results = per_movie(|v| decode(&path, v, None));
    for ((i, v), r) in VECTORS.iter().enumerate().zip(&results) {
        let id = format!("movie {i} ({:#x})", v.identity);
        assert_eq!(r.errors, 0, "{id}: {:?}", r.first_error);
        assert_eq!(r.frames, v.pictures, "{id}");
        assert!(r.in_order, "{id}: display indices");
        assert!(r.frame_layout_ok, "{id}: frame pictures, interlaced, top field first");
        // One GOP per I picture, about one I picture in 15 (movies.md 2.1). Only the first GOP is marked closed; the
        // others are open (their leading B pictures predict from the previous GOP), and no link is broken.
        assert_eq!(r.gops, r.types[0], "{id}");
        assert!(r.types[0].abs_diff(v.pictures / 15) <= 1, "{id}: {} I pictures", r.types[0]);
        assert_eq!((r.closed_gops, r.broken_links), (1, 0), "{id}");
        let s = r.seq.as_ref().expect("sequence header");
        assert_eq!((s.width, s.height), (640, 480), "{id}");
        assert_eq!(s.profile_and_level, 0x48, "{id}");
        assert_eq!((s.aspect_ratio_information, s.frame_rate_code), (2, 4), "{id}");
        assert_eq!((s.bit_rate, s.vbv_buffer_size), (12_500, 112), "{id}");
        assert_eq!(s.chroma_format, 1, "{id}");
        assert!(!s.progressive_sequence, "{id}");
        assert_eq!(r.unsupported_modes, 0, "{id}: the movies use frame prediction only");
    }
    // movies.md 2.1: I : P : B = 28 : 109 : 272 in the logo movie.
    assert_eq!(results[0].types, [28, 109, 272]);
}

/// Splits jobs over scoped threads, like the `threads` feature's runner (written here so the test needs no
/// feature).
struct TestThreads(usize);

impl Parallel for TestThreads {
    fn run(&self, jobs: &mut [&mut (dyn FnMut() + Send)]) {
        std::thread::scope(|s| {
            for job in jobs.iter_mut() {
                s.spawn(job);
            }
        });
    }
    fn width(&self) -> usize {
        self.0
    }
}

#[test]
fn parallel_slices_give_the_same_frames() {
    let Some(path) = disc("parallel_slices_give_the_same_frames") else { return };
    let v = &VECTORS[0];
    let serial = decode(&path, v, None);
    for n in [2, 3, 7] {
        let par = decode(&path, v, Some(Box::new(TestThreads(n))));
        assert_eq!(par.errors, 0);
        assert_eq!(par.frames, serial.frames);
        assert_eq!(par.hash, serial.hash, "{n} bands");
    }
}

fn logo_player(path: &Path) -> MoviePlayer<File> {
    let v = &VECTORS[0];
    let file = File::open(path).expect("open image");
    MoviePlayer::new(Region::new(file, u64::from(v.lba) * SECTOR, u64::from(v.size)))
}

#[test]
fn player_plays_the_logo_against_its_audio_clock() {
    let Some(path) = disc("player_plays_the_logo_against_its_audio_clock") else { return };
    let mut p = logo_player(&path);
    let h = p.audio_header().expect("read").expect("audio header");
    assert_eq!(h.sample_rate, 48_000);
    assert_eq!(p.duration_ns().expect("read"), Some(13_653_333_333));
    // A game running at 60 Hz: each frame the audio device takes 800 sample frames, and the clock is the audio
    // played so far.
    let mut played = 0u64;
    let mut buf = vec![0i16; 1600];
    let mut shown = Vec::new();
    let mut status = PlayerStatus::Playing;
    for _ in 0..60 * 20 {
        let n = p.read_audio(&mut buf).expect("audio");
        played += n as u64 / 2;
        let clock = played * 1_000_000_000 / 48_000;
        status = p.update(clock).expect("update");
        if let Some(f) = p.frame()
            && shown.last() != Some(&f.info.display_index)
        {
            // Never shown before it is due.
            assert!(p.frame_time_ns(f.info.display_index) <= clock);
            shown.push(f.info.display_index);
        }
        if status == PlayerStatus::Ended {
            break;
        }
    }
    assert_eq!(status, PlayerStatus::Ended);
    // The audio runs 6 ms past the last frame; the rest is still there to read.
    loop {
        let n = p.read_audio(&mut buf).expect("audio");
        if n == 0 {
            break;
        }
        played += n as u64 / 2;
    }
    assert_eq!(played, VECTORS[0].audio / 4, "every audio sample frame delivered");
    assert_eq!(shown.first(), Some(&0));
    assert_eq!(shown.last(), Some(&408));
    assert!(shown.windows(2).all(|w| w[0] < w[1]));
    // At 60 updates per second every frame of a 29.97 fps movie gets shown.
    assert_eq!(shown.len(), 409);
    assert_eq!(p.frames_dropped(), 0);
    assert_eq!(p.decoder_stats().slice_errors, 0);
}

#[test]
fn player_catches_up_and_skips() {
    let Some(path) = disc("player_catches_up_and_skips") else { return };
    // Audio off, and a clock that jumps: late frames are dropped and B pictures skipped while behind.
    let mut p = logo_player(&path);
    p.set_audio_enabled(false);
    assert_eq!(p.update(0).expect("update"), PlayerStatus::Playing);
    assert_eq!(p.frame().map(|f| f.info.display_index), Some(0));
    assert_eq!(p.frame().map(|f| f.info.picture_type), Some(PictureType::I));
    p.update(5_000_000_000).expect("update");
    // Frame 149 is due at 4.97 s; with B pictures skipped the newest due frame may be an I or P a little before.
    let at = p.frame().expect("frame").info.display_index;
    assert!((140..=149).contains(&at), "{at}");
    assert!(p.frames_dropped() + p.decoder_stats().skipped_b > 100);
    assert!(p.decoder_stats().skipped_b > 0);
    // Back on time: the following updates show consecutive frames again.
    let next = p.frame_time_ns(at + 1);
    p.update(next.max(5_000_000_000) + 40_000_000).expect("update");
    assert!(p.frame().expect("frame").info.display_index > at);
    assert_eq!(p.audio_buffered(), 0);
    // Skip: ended at once, and no more audio.
    p.skip();
    assert!(p.is_finished());
    assert_eq!(p.update(5_100_000_000).expect("update"), PlayerStatus::Ended);
    let mut buf = [0i16; 64];
    assert_eq!(p.read_audio(&mut buf).expect("audio"), 0);
}
