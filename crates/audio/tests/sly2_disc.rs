//! Tests against the player's own Sly 2 disc: every test vector and count in `specs/sly2/audio.md` and the
//! movie-audio vector of `specs/sly2/movies.md`. They skip (and pass) when `SLYNX_DISC_SLY2` is not set. Only
//! numbers and hashes live here.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};

use audio::adpcm::{decode_to_end, sample_len};
use audio::bank::{EffectBank, StepKind};
use audio::sly2::vpk::{VpkHeader, VpkStream};
use audio::sly2::{CueValue, MovieAudio, Sly2Disc, WorldHeader};
use formats::hash::{hex, sha256};
use harness::golden::{Game, require_disc};

type Disc = Sly2Disc<std::fs::File>;

struct Data {
    disc: Mutex<Disc>,
    /// (row, world id, header) for every non-empty row.
    worlds: Vec<(usize, u32, WorldHeader)>,
}

fn data(test: &str) -> Option<&'static Data> {
    let path = require_disc(Game::Sly2, test)?;
    static CELL: OnceLock<Data> = OnceLock::new();
    Some(CELL.get_or_init(|| {
        let mut disc = Sly2Disc::open_path(path).expect("open Sly 2 disc");
        let rows: Vec<(usize, audio::sly2::FileRecord)> =
            disc.levels.iter().enumerate().filter_map(|(i, l)| l.file.map(|f| (i, f))).collect();
        let compressed: Vec<(usize, u32, Vec<u8>)> =
            rows.iter().map(|(i, f)| (*i, f.identity, disc.read(f).unwrap())).collect();
        let worlds = std::thread::scope(|s| {
            let hs: Vec<_> = compressed
                .iter()
                .map(|(i, id, c)| {
                    s.spawn(move || {
                        let w = formats::compress::decompress(c).unwrap();
                        (*i, *id, WorldHeader::parse(&w).unwrap_or_else(|e| panic!("row {i}: {e}")))
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        Data { disc: Mutex::new(disc), worlds }
    }))
}

fn pcm_sha(pcm: &[i16]) -> String {
    let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
    hex(&sha256(&bytes))
}

fn header(d: &Data, row: usize) -> &WorldHeader {
    &d.worlds.iter().find(|w| w.0 == row).unwrap().2
}

#[test]
fn disc_layout() {
    let Some(d) = data("disc_layout") else { return };
    let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(disc.files.len(), 29);
    assert_eq!(disc.levels.len(), 44);
    assert!(disc.levels[18].file.is_none());
    assert_eq!(d.worlds.len(), 43);
    let boot = disc.files[24].unwrap();
    assert_eq!((boot.sector, boot.size, boot.identity), (335_488, 9_969_830, 0x5D));
    let bytes = disc.read(&boot).unwrap();
    assert_eq!(hex(&sha256(&bytes)), "af84b84e273d38f2ff56736e22fb8396fcd2eca2d31e672546c132c0e015f2d2");
    let r3 = disc.levels[3].file.unwrap();
    assert_eq!((r3.sector, r3.size, r3.identity), (377_568, 6_651_154, 0x3B));
    assert_eq!(header(d, 3).level_name(), "f_nightclub_cellar");
    // Sound-section boundaries (world-file.md 6).
    for (row, range) in [(0, 0x4E6E..0xAA94), (1, 0xFE..0x660), (3, 0x3872..0x15EAB), (43, 0x564E..0x15823)] {
        assert_eq!(header(d, row).sound_range, range, "row {row}");
    }
}

#[test]
fn world_sound_sections() {
    let Some(d) = data("world_sound_sections") else { return };
    let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
    let mut flags: BTreeMap<u8, usize> = BTreeMap::new();
    let (mut with_music, mut without) = (0, 0);
    let mut refs = 0;
    let mut numbers: BTreeMap<i16, audio::sly2::FileRecord> = BTreeMap::new();
    let mut speakers: BTreeMap<i16, usize> = BTreeMap::new();
    let mut other_files = BTreeSet::new();
    let mut empty_streams = 0;
    let mut headset: BTreeMap<i8, usize> = BTreeMap::new();
    let mut targets: BTreeMap<i16, usize> = BTreeMap::new();
    let (mut mouth_min, mut mouth_max) = (i32::MAX, i32::MIN);
    let mut counted = BTreeSet::new();
    for (row, id, h) in &d.worlds {
        let s = &h.sound;
        assert_eq!(s.bank.unwrap().identity, id - 1, "row {row}: bank identity is the world's - 1");
        for &f in &s.flags {
            *flags.entry(f).or_default() += 1;
        }
        assert!((290..=690).contains(&s.id_space), "row {row}: id space {}", s.id_space);
        let ids: BTreeSet<i16> = s.sound_ids.iter().copied().collect();
        assert_eq!(ids.len(), s.sound_ids.len(), "row {row}: sound IDs distinct");
        assert!(s.sound_ids.iter().all(|&i| (0..s.id_space).contains(&i)));
        match s.resident {
            Some(m) => {
                with_music += 1;
                assert_eq!(m.identity, id + 1, "row {row}: music identity");
                let head = disc.source_read(&m, 2048);
                let vh = VpkHeader::parse(&head).unwrap();
                assert_eq!(vh.channels.len(), s.resident_channels.len(), "row {row}: one channel number per channel");
                assert!((2..=4).contains(&vh.channels.len()));
                assert_eq!(vh.sample_rate, 32_000);
                assert!(vh.channels.iter().enumerate().all(|(k, c)| c.pan == 0 && c.group as usize == k));
            }
            None => without += 1,
        }
        assert_eq!(s.tracks, [0]);
        for l in &s.voice_lines {
            refs += 1;
            *headset.entry(l.tracks[0].headset).or_default() += 1;
            // Timed records are counted once per line number.
            if counted.insert(l.number) {
                for r in &l.records {
                    *targets.entry(r.target).or_default() += 1;
                    if r.target == audio::sly2::sound::TARGET_MOUTH
                        && let CueValue::Goal(v) = r.value
                    {
                        mouth_min = mouth_min.min(v);
                        mouth_max = mouth_max.max(v);
                    }
                }
            }
            let f = l.tracks[0].file.unwrap();
            if let Some(prev) = numbers.insert(l.number, f) {
                assert_eq!(prev, f, "a line number always names the same file");
            }
            *speakers.entry(l.speaker).or_default() += 1;
        }
        for (_, r) in &s.music {
            match r {
                Some(r) => {
                    other_files.insert(*r);
                }
                None => empty_streams += 1,
            }
        }
    }
    assert_eq!(flags, [(0, 12_229), (2, 742)].into_iter().collect());
    assert_eq!((with_music, without), (36, 7));
    assert_eq!(refs, 7_970);
    assert_eq!(headset, [(-1, 4_867), (0, 2_879), (1, 112), (2, 76), (3, 36)].into_iter().collect());
    let expect: BTreeMap<i16, usize> =
        [(5, 57_042), (1, 3_294), (4, 3_036), (0, 1_624), (2, 1_398), (3, 308)].into_iter().collect();
    assert_eq!(targets, expect);
    assert_eq!((mouth_min, mouth_max), (5_372, 5_378));
    assert_eq!(numbers.len(), 1_691);
    let expect: BTreeMap<i16, usize> = [
        (55, 3651),
        (53, 1608),
        (54, 836),
        (63, 463),
        (62, 424),
        (60, 281),
        (64, 260),
        (65, 99),
        (59, 97),
        (61, 75),
        (-1, 176),
    ]
    .into_iter()
    .collect();
    assert_eq!(speakers, expect);
    assert_eq!((other_files.len(), empty_streams), (64, 122));
    // Other streams: 2 channels (a stereo pair, group 5) or 5 (mono group 0 plus pairs of groups 1 and 2).
    let mut rates: BTreeMap<u32, usize> = BTreeMap::new();
    let mut layouts: BTreeMap<Vec<(u8, u16)>, usize> = BTreeMap::new();
    for f in &other_files {
        let vh = VpkHeader::parse(&disc.source_read(f, 2048)).unwrap();
        *rates.entry(vh.sample_rate).or_default() += 1;
        let layout: Vec<(u8, u16)> = vh.channels.iter().map(|c| (c.group, c.pan)).collect();
        *layouts.entry(layout).or_default() += 1;
    }
    // The four music-stream layouts of audio.md 3.2.
    let expect: BTreeMap<Vec<(u8, u16)>, usize> = [
        (vec![(0, 0), (1, 270), (1, 90), (2, 270), (2, 90)], 40),
        (vec![(1, 270), (1, 90), (2, 0), (3, 0), (4, 0)], 1),
        (vec![(3, 270), (3, 90), (4, 270), (4, 90)], 1),
        (vec![(5, 270), (5, 90)], 22),
    ]
    .into_iter()
    .collect();
    assert_eq!(layouts, expect);
    assert_eq!(rates, [(32_000, 14), (44_100, 50)].into_iter().collect());
    // Every voice line is a 22,050 Hz VAG whose duration matches.
    for f in numbers.values() {
        let head = disc.source_read(f, 48);
        let vh = audio::vag::VagHeader::parse(&head).unwrap();
        assert_eq!((vh.sample_rate, vh.version), (22_050, 0));
    }
}

/// Reads the first `n` bytes of a record.
trait ReadHead {
    fn source_read(&mut self, r: &audio::sly2::FileRecord, n: usize) -> Vec<u8>;
}

impl ReadHead for Disc {
    fn source_read(&mut self, r: &audio::sly2::FileRecord, n: usize) -> Vec<u8> {
        use formats::iso::ReadAt;
        self.source().read_vec(r.offset(), n.min(r.size as usize)).unwrap()
    }
}

fn raw_kinds(b: &EffectBank) -> Vec<u32> {
    let d = &b.bytes[b.description.clone()];
    let w = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    let n = u16::from_le_bytes([d[24], d[25]]) as usize;
    let st = w(32) as usize;
    (0..n).map(|i| w(st + i * 8) >> 24).collect()
}

#[test]
fn effect_banks() {
    let Some(d) = data("effect_banks") else { return };
    let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
    let mut seen = BTreeSet::new();
    let mut kinds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut extras: BTreeMap<usize, usize> = BTreeMap::new();
    // (flag 2 with the continuous bit, flag 2 without it, the bit with flag 0)
    let mut flagged = (0, 0, 0);
    let mut bends: BTreeMap<i16, usize> = BTreeMap::new();
    let mut add_bends = BTreeSet::new();
    let (mut delays_ok, mut set_regs, mut reg_ok, mut reg_all, mut legacy, mut cycle_fit) = (0, 0, 0, 0, 0, 0);
    let mut markers = Vec::new();
    let mut kind1_nonneg = Vec::new();
    for (row, _, h) in &d.worlds {
        let r = h.sound.bank.unwrap();
        let bytes = disc.read(&r).unwrap();
        if !seen.insert(sha256(&bytes)) {
            continue;
        }
        let b = EffectBank::parse(bytes).unwrap();
        assert_eq!(b.version, 3);
        for k in raw_kinds(&b) {
            *kinds.entry(k).or_default() += 1;
        }
        let d = &b.bytes[b.description.clone()];
        let distinct_samples = u16::from_le_bytes([d[26], d[27]]) as usize;
        let mut offsets = BTreeSet::new();
        for (si, s) in b.sounds.iter().enumerate() {
            match (h.sound.flags.get(si), s.continuous()) {
                (Some(2), true) => flagged.0 += 1,
                (Some(2), false) => flagged.1 += 1,
                (Some(0), true) => flagged.2 += 1,
                _ => {}
            }
            let n = s.step_count as usize;
            for (k, st) in b.sound_steps(si).iter().enumerate() {
                match st.kind {
                    StepKind::RandomDelay(m) => delays_ok += (m < 10_000) as usize,
                    StepKind::RandomBend(a) => *bends.entry(a).or_default() += 1,
                    StepKind::AddBendScaled(a) => {
                        add_bends.insert(a);
                    }
                    StepKind::SetRegister { register, value } => set_regs += (register == 0 && value == 100) as usize,
                    StepKind::RandomRegister { low, high, .. } => {
                        reg_all += 1;
                        reg_ok += (low <= high) as usize;
                    }
                    StepKind::VoiceLegacy(v) => {
                        assert!(v.centre >= 0, "kind 9 uses a non-negative centre note");
                        assert!(sample_len(b.sample_data(), v.sample_offset as usize).is_some());
                        legacy += 1;
                    }
                    StepKind::Voice(v) => {
                        if v.centre >= 0 {
                            kind1_nonneg.push((row, si, k, v.centre));
                        }
                    }
                    StepKind::GroupCycle { groups, per_group, pass } => {
                        assert_eq!(pass, 0);
                        cycle_fit += (k + 1 + groups as usize * per_group as usize <= n) as usize;
                    }
                    StepKind::StopMarker => markers.push(n - k),
                    _ => {}
                }
            }
            for st in b.sound_steps(si) {
                if let StepKind::Voice(v) = st.kind {
                    offsets.insert(v.sample_offset);
                    assert!(sample_len(b.sample_data(), v.sample_offset as usize).is_some(), "row {row} sound {si}");
                }
            }
            assert!(s.group <= 3);
        }
        assert_eq!(offsets.len(), distinct_samples, "row {row}: distinct samples");
        // b flags, up to two more sounds in the bank.
        let extra = b.sounds.len() - h.sound.flags.len();
        *extras.entry(extra).or_default() += 1;
        let counts = (b.sounds.len(), b.steps.len(), distinct_samples);
        match row {
            0 => assert_eq!(counts, (296, 2490, 351)),
            3 => assert_eq!(counts, (309, 2510, 387)),
            43 => assert_eq!(counts, (290, 2445, 343)),
            _ => {}
        }
    }
    eprintln!("bank sounds beyond the flag list (extra: banks): {extras:?}");
    // audio.md says 42 distinct banks; the 43 worlds' banks are 43 distinct files by content.
    assert_eq!(seen.len(), 43, "distinct effect banks");
    // audio.md: "up to two more sounds than b"; one bank has three.
    assert_eq!(extras, [(0, 25), (1, 13), (2, 4), (3, 1)].into_iter().collect());
    let expect: BTreeMap<u32, usize> = [
        (1, 65_493),
        (27, 10_968),
        (25, 10_734),
        (26, 5_525),
        (4, 5_022),
        (20, 882),
        (39, 596),
        (21, 466),
        (22, 466),
        (31, 249),
        (5, 77),
        (24, 75),
        (9, 64),
        (29, 45),
        (30, 13),
        (23, 1),
        (43, 1),
    ]
    .into_iter()
    .collect();
    assert_eq!(kinds, expect);
    // Per-sound flags and the continuous bit (audio.md 1).
    // audio.md 2 says every kind-1 centre note is negative; 101 kind-1 steps hold centre 120.
    assert_eq!(kind1_nonneg.len(), 101);
    assert!(kind1_nonneg.iter().all(|x| x.3 == 120));
    assert_eq!(flagged, (654, 88, 388));
    // Inline arguments (audio.md 2).
    assert_eq!(delays_ok, 5_525);
    assert_eq!((bends[&100], bends[&127]), (8_277, 2_130));
    assert_eq!(*bends.keys().next().unwrap(), -106);
    assert_eq!(add_bends, [50i8, 70, 127, -128, -100, -70].into_iter().collect());
    assert_eq!(set_regs, 13);
    assert_eq!((reg_ok, reg_all), (248, 249));
    assert_eq!(legacy, 64);
    assert_eq!(cycle_fit, 572);
    assert_eq!(markers, [3], "the stop marker sits 3 steps before the end of its sound");
}

#[test]
fn row3_bank_vector() {
    let Some(d) = data("row3_bank_vector") else { return };
    let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
    let r = header(d, 3).sound.bank.unwrap();
    assert_eq!((r.sector, r.size, r.identity), (380_864, 1_330_700, 58));
    let bytes = disc.read(&r).unwrap();
    assert_eq!(hex(&bytes[..24]), "03000000020000001800000074fc00008cfc000080511300");
    let b = EffectBank::parse(bytes).unwrap();
    assert_eq!(b.samples.start, 0xFC8C);
    let s0 = b.sounds[0];
    assert_eq!((s0.volume, s0.group, s0.step_count), (75, 3, 8));
    assert!(b.sound_steps(0).iter().all(|s| matches!(s.kind, StepKind::Voice(_))));
    // The first voice step's 24 data bytes.
    let d = &b.bytes[b.description.clone()];
    let area = u32::from_le_bytes(d[52..56].try_into().unwrap()) as usize;
    let st = u32::from_le_bytes(d[32..36].try_into().unwrap()) as usize;
    let w0 = u32::from_le_bytes(d[st..st + 4].try_into().unwrap());
    let at = area + (w0 & 0xFF_FFFF) as usize;
    assert_eq!(hex(&d[at..at + 24]), "006eaa42000000000000ff80c09f0000c0f8040000000000");
    let StepKind::Voice(v) = b.sound_steps(0)[0].kind else { unreachable!() };
    assert_eq!(v.sample_offset, 325_824);
    let pcm = decode_to_end(b.sample_data(), v.sample_offset as usize);
    assert!(pcm.terminated);
    assert_eq!(pcm.pcm.len(), 15_316);
    assert_eq!(pcm_sha(&pcm.pcm), "7aecc0b63cebc53d3dbdf68891e8ff96e5e006ea467ab77b4c3721d889dd12c5");
}

#[test]
fn row3_resident_stream() {
    let Some(d) = data("row3_resident_stream") else { return };
    let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
    let m = header(d, 3).sound.resident.unwrap();
    assert_eq!((m.sector, m.size, m.identity), (381_600, 8_783_872, 60));
    let s: VpkStream = disc.read_vpk(&m).unwrap();
    assert_eq!(s.header.channels.len(), 4);
    assert_eq!((s.header.bytes_per_channel, s.header.sample_rate), (2_194_304, 32_000));
    assert_eq!((s.header.data_offset, s.header.interleave), (0x800, 0x2000));
    assert!((s.header.duration() - 120.0).abs() < 0.01);
    let expect = [
        "6726ba3a83a1144a66f0f60dd751a6b909f7e6843d3e0a04b42fe9a91780ce83",
        "26d66eda76398c343e4c4094ff4d22661c4818987585ae3eb2ce8dab0aeedea0",
        "89e0779c61928dd895b3c7bdeb0d1dbc8af38acf4e5023c6d58448b1f3d3f4b5",
        "a9f5b67d4f91d6aa3656b45ab07ec6de10ba8d7ea9500756a790f5427a7448eb",
    ];
    for (c, data) in s.channels.iter().enumerate() {
        let pcm = decode_to_end(data, 0);
        assert!(!pcm.terminated, "no end flags");
        assert_eq!(pcm.pcm.len(), 3_840_032);
        if c == 0 {
            assert_eq!(&pcm.pcm[..4], &[4096, 4864, 3536, 4339]);
        }
        assert_eq!(pcm_sha(&pcm.pcm), expect[c], "channel {c}");
    }
}

#[test]
fn row3_voice_line() {
    let Some(d) = data("row3_voice_line") else { return };
    let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
    let l = header(d, 3).sound.voice_line(1285).unwrap().clone();
    assert_eq!(l.speaker, 55);
    let t = l.tracks[0];
    let f = t.file.unwrap();
    assert_eq!((f.sector, f.size, f.identity), (1_253_673, 123_968, 439));
    assert!((t.duration - 9.835).abs() < 0.001);
    let v = disc.read_vag(&f).unwrap();
    assert_eq!(v.header.sample_rate, 22_050);
    let pcm = decode_to_end(v.adpcm(), 0);
    assert_eq!(pcm.pcm.len(), 216_860);
    assert!((pcm.pcm.len() as f32 / 22_050.0 - t.duration).abs() < 0.01);
    assert_eq!(pcm_sha(&pcm.pcm), "e04a3943d7b08e82a774afe8fe9792d1f706d4dea320a01a26403acc17b7505c");
}

#[test]
fn movie_soundtrack_0x808() {
    let Some(d) = data("movie_soundtrack_0x808") else { return };
    let mut disc = d.disc.lock().unwrap_or_else(|e| e.into_inner());
    let f = disc.files.iter().flatten().find(|f| f.identity == 0x808).copied().unwrap();
    assert_eq!((f.sector, f.size), (959_782, 2_998_276));
    let bytes = disc.read(&f).unwrap();
    let mut demux = video::demux::Demuxer::new();
    let mut audio = MovieAudio::new();
    let mut out = Vec::new();
    let mut first = None;
    for chunk in bytes.chunks(64 * 1024) {
        demux.push(chunk);
        while let Some(p) = demux.next_packet().unwrap() {
            if let video::demux::Packet::Audio(a) = p {
                if first.is_none() {
                    first = Some(a[..24].to_vec());
                }
                audio.push(a, &mut out).unwrap();
            }
        }
    }
    // The first payload after the 4-byte sub-stream header `FF A1 00 00`.
    assert_eq!(hex(&first.unwrap()), "53536864180000001000000044ac00000200000020000000");
    let h = *audio.header().unwrap();
    assert_eq!((h.format, h.sample_rate, h.channels, h.interleave, h.data_size), (16, 44_100, 2, 32, 222_144));
    assert!(audio.done());
    let left: Vec<i16> = out.chunks(2).map(|f| f[0]).collect();
    let right: Vec<i16> = out.chunks(2).map(|f| f[1]).collect();
    assert_eq!(left.len(), 194_376);
    assert_eq!(left, right, "left and right are identical in this movie");
    assert_eq!(pcm_sha(&left), "9b4584d0fc117ec50e1761adca5cfdbd7b29246648590fa7c1a7145a9434c908");
}
