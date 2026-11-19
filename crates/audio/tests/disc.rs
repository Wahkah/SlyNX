//! Tests against the player's own Sly 1 disc: every PCM test vector and structure check in `specs/sly1/audio.md`,
//! plus walks of every bank and music file. They skip (and pass) when `SLYNX_DISC_SLY1` is not set.
//!
//! Only numbers and hashes live here; no game data.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};

use audio::adpcm::{decode_to_end, sample_len};
use audio::bank::{EffectBank, StepKind};
use audio::disc::{
    ambient_descriptors, bank_descriptors, bank_sound_counts, commentary_table, prompt_descriptor, read_bank, read_vag,
    world_audio,
};
use audio::music::{Event, EventCursor, MusicFile, commands};
use audio::{Config, Emitter, create};
use formats::disc::{Descriptor, Disc};
use formats::hash::{hex, sha256};
use harness::golden::{Game, require_disc};

type IsoDisc = Disc<std::fs::File>;

fn open(test: &str) -> Option<Mutex<IsoDisc>> {
    let path = require_disc(Game::Sly1, test)?;
    Some(Mutex::new(Disc::open_path(path).expect("open disc")))
}

fn pcm_sha(pcm: &[i16]) -> String {
    let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
    hex(&sha256(&bytes))
}

fn raw(disc: &mut IsoDisc, sector: u32, size: u32) -> Vec<u8> {
    let d = Descriptor { sector, size, identity: 0, unknown: [0; 2] };
    disc.read_descriptor(&d).unwrap()
}

/// The 7 banks, parsed once.
fn banks(disc: &mut IsoDisc) -> &'static [EffectBank] {
    static CELL: OnceLock<Vec<EffectBank>> = OnceLock::new();
    CELL.get_or_init(|| (0..7).map(|i| read_bank(disc, i).unwrap()).collect())
}

/// A world's index, name, music descriptor and sound bank.
type WorldMusic = (usize, String, Option<Descriptor>, i16);

/// Every world's music descriptor and sound bank, decompressing worlds in parallel.
fn world_music(disc: &mut IsoDisc) -> &'static [WorldMusic] {
    static CELL: OnceLock<Vec<WorldMusic>> = OnceLock::new();
    CELL.get_or_init(|| {
        let compressed: Vec<Vec<u8>> = (0..disc.worlds.len()).map(|i| disc.read_world_compressed(i).unwrap()).collect();
        let names: Vec<String> = disc.worlds.iter().map(|w| w.name.clone()).collect();
        std::thread::scope(|s| {
            let hs: Vec<_> = compressed
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    s.spawn(move || {
                        let w = formats::compress::decompress(c).unwrap();
                        let (m, b) = world_audio(&w).unwrap();
                        (i, m, b)
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).map(|(i, m, b)| (i, names[i].clone(), m, b)).collect()
        })
    })
}

#[test]
fn effect_bank_vectors() {
    let Some(disc) = open("effect_bank_vectors") else { return };
    let mut disc = disc.lock().unwrap();
    let expect = [
        (0xBD9A3, 768_656, 169, 431),
        (0xD4BE2, 598_240, 65, 233),
        (0xBEFFE, 748_060, 136, 194),
        (0xBFAFF, 704_904, 121, 300),
        (0xC3DA5, 743_188, 102, 460),
        (0xC4C26, 744_324, 98, 410),
        (0xBC77E, 378_980, 80, 233),
    ];
    let descs = bank_descriptors(&disc.executable).unwrap();
    let counts = bank_sound_counts(&mut disc).unwrap();
    let banks = banks(&mut disc);
    for (i, &(lba, size, sounds, steps)) in expect.iter().enumerate() {
        assert_eq!((descs[i].sector, descs[i].size), (lba, size), "bank {i} descriptor");
        assert_eq!(banks[i].sounds.len(), sounds, "bank {i} sounds");
        assert_eq!(banks[i].steps.len(), steps, "bank {i} steps");
        assert_eq!(counts[i] as usize, sounds);
        assert_eq!(banks[i].description.start, 0x18);
        assert_eq!(banks[i].samples.end, banks[i].bytes.len());
    }
    // Sound ID bases.
    let mut base = 0;
    let mut bases = vec![];
    for c in counts {
        bases.push(base);
        base += c as u32;
    }
    assert_eq!(bases, [0, 169, 234, 370, 491, 593, 691]);
    assert_eq!(base, 771);

    // Bank 0, sounds 0 and 1.
    let b0 = &banks[0];
    assert_eq!(b0.samples.start, 0x4B90);
    let s0 = b0.sounds[0];
    assert_eq!((s0.volume, s0.group, s0.step_count), (100, 3, 1));
    let StepKind::Voice(v) = b0.sound_steps(0)[0].kind else { panic!("sound 0 step is not a voice") };
    assert_eq!((v.priority, v.volume, v.centre, v.fine), (0, 110, -86, 66));
    assert_eq!((v.adsr.w1, v.adsr.w2, v.sample_offset), (0x80FF, 0x9FC0, 156_880));
    let d = decode_to_end(b0.sample_data(), v.sample_offset as usize);
    assert!(d.terminated);
    assert_eq!(d.pcm.len(), 1876);
    assert_eq!(&d.pcm[..6], &[0; 6]);
    assert_eq!(pcm_sha(&d.pcm), "4a52bbf6fb26807754386a303bfefce62c1e5923052ec1d88f37b90e9b6c7b61");

    let s1 = b0.sounds[1];
    assert_eq!((s1.volume, s1.group), (100, 3));
    let StepKind::Voice(v1) = b0.sound_steps(1)[0].kind else { panic!("sound 1 step is not a voice") };
    assert_eq!((v1.volume, v1.centre, v1.fine, v1.sample_offset), (110, -86, 66, 170_816));
    let d = decode_to_end(b0.sample_data(), v1.sample_offset as usize);
    assert_eq!(d.pcm.len(), 16_548);
    assert_eq!(&d.pcm[..6], &[0; 6]);
    assert_eq!(pcm_sha(&d.pcm), "cad14339248374645d8c1b8a9710ffc4b02edc70ab77adfcc8f8a534fa2a9f23");
}

#[test]
fn every_bank_step_and_voice_sample() {
    let Some(disc) = open("every_bank_step_and_voice_sample") else { return };
    let mut disc = disc.lock().unwrap();
    let banks = banks(&mut disc);
    let mut kinds: BTreeMap<u32, usize> = BTreeMap::new();
    let mut voices = 0;
    let mut overruns = vec![];
    let mut voice_flags: BTreeMap<u16, usize> = BTreeMap::new();
    for (bi, b) in banks.iter().enumerate() {
        for (si, s) in b.sounds.iter().enumerate() {
            assert!((0..=127).contains(&s.volume) && (0..=3).contains(&s.group), "bank {bi} sound {si}");
            for (k, step) in b.sound_steps(si).iter().enumerate() {
                *kinds.entry(step.kind_number()).or_default() += 1;
                match step.kind {
                    StepKind::Voice(v) => {
                        voices += 1;
                        *voice_flags.entry(v.flags).or_default() += 1;
                        assert!(v.centre < 0, "bank {bi} sound {si}: centre note stored negated");
                        let len = sample_len(b.sample_data(), v.sample_offset as usize);
                        assert!(len.is_some(), "bank {bi} sound {si} step {k}: sample has no end flag");
                    }
                    StepKind::Child { sound, .. } => {
                        assert!((0..b.sounds.len() as i16).contains(&sound), "bank {bi} sound {si}: child index")
                    }
                    StepKind::RandomChoice { choices, per_choice, .. } => {
                        assert!(choices > 0 && per_choice > 0);
                        let after = k + 1 + choices as usize * per_choice as usize;
                        if after > s.step_count as usize {
                            overruns.push((bi, si, after - s.step_count as usize));
                        }
                    }
                    StepKind::Unknown(n) => panic!("unknown step kind {n}"),
                    _ => {}
                }
            }
        }
    }
    let expect: BTreeMap<u32, usize> = [
        (1, 1429),
        (4, 74),
        (5, 15),
        (20, 28),
        (21, 19),
        (22, 19),
        (24, 1),
        (25, 218),
        (26, 122),
        (27, 308),
        (29, 2),
        (31, 23),
        (38, 3),
    ]
    .into_iter()
    .collect();
    assert_eq!(kinds, expect);
    assert_eq!(voices, 1429);
    // Two sounds have a random choice whose last group runs one step past the sound (the player stops at the
    // sound's end).
    assert_eq!(overruns, [(5, 28, 1), (5, 29, 1)]);
    // audio.md says voice flags are 0 in the effects; 469 voice steps have bit 0 set (meaning unknown).
    assert_eq!(voice_flags, [(0, 960), (1, 469)].into_iter().collect());
}

#[test]
fn vag_vectors() {
    let Some(disc) = open("vag_vectors") else { return };
    let mut disc = disc.lock().unwrap();

    let ambient = ambient_descriptors(&disc.executable).unwrap();
    assert_eq!(ambient.len(), 6);
    assert_eq!((ambient[0].sector, ambient[0].size, ambient[0].identity), (0x3AA4B, 9120, 0x23F));
    let a = read_vag(&mut disc, &ambient[0]).unwrap();
    let head: Vec<u8> = a.bytes[..20].to_vec();
    assert_eq!(hex(&head), "5641477000000020000000000000237000005622");
    assert_eq!(a.header.sample_rate, 22_050);
    assert_eq!(a.header.name(), b"dap");
    let d = decode_to_end(a.adpcm(), 0);
    assert_eq!(d.pcm.len(), 15_848);
    assert_eq!(&d.pcm[..6], &[0; 6]);
    assert_eq!(pcm_sha(&d.pcm), "dd730987e69cb1b117595c7d98799f91cb990bdd3740a7e754c271fa088ce940");
    for (i, d) in ambient.iter().enumerate() {
        let v = read_vag(&mut disc, d).unwrap();
        assert_eq!(v.header.sample_rate, 22_050, "ambient {i}");
        assert_eq!(v.header.file_size(), d.size as usize, "ambient {i}");
    }

    let p = prompt_descriptor(&disc.executable).unwrap();
    assert_eq!((p.sector, p.size, p.identity), (0x6ED70, 130_944, 0x28D));
    let v = read_vag(&mut disc, &p).unwrap();
    assert_eq!(v.header.sample_rate, 22_050);
    let d = decode_to_end(v.adpcm(), 0);
    assert_eq!(d.pcm.len(), 229_040);
    assert_eq!(&d.pcm[..6], &[0; 6]);
    assert_eq!(pcm_sha(&d.pcm), "563b38b9b2b8f36a9f67508c507451eaf38a47c2fb9d08fcfc04df7ca4f45dc5");

    // The shortest Paris dialogue line, by its location.
    let line = Descriptor { sector: 0x14DEA0, size: 19_008, identity: 0x261, unknown: [0; 2] };
    let v = read_vag(&mut disc, &line).unwrap();
    assert_eq!(v.header.sample_rate, 22_050);
    assert_eq!(v.header.file_size(), 19_008);
    let d = decode_to_end(v.adpcm(), 0);
    assert_eq!(d.pcm.len(), 33_152);
    assert_eq!(&d.pcm[..6], &[0; 6]);
    assert_eq!(pcm_sha(&d.pcm), "e9c1d5afe99ef6e3ee5de55dc9b3c3fa9505257040a95c6a951cb5977d92f9e0");

    let c = commentary_table(&disc.executable).unwrap();
    assert_eq!(c.len(), 33);
    for e in &c {
        let d = e.file.expect("commentary file");
        let v = read_vag(&mut disc, &d).unwrap();
        assert!(v.header.sample_rate == 22_050 || v.header.sample_rate == 44_100);
        assert!(e.duration >= 0.0);
    }
}

#[test]
fn paris_and_hideout_music() {
    let Some(disc) = open("paris_and_hideout_music") else { return };
    let mut disc = disc.lock().unwrap();
    let bytes = raw(&mut disc, 0xC8E0E, 501_408);
    assert_eq!(hex(&bytes[0x20 + 12..0x20 + 16]), hex(b"PARI"));
    let m = MusicFile::parse(bytes).unwrap();
    assert_eq!(&m.name, b"PARI");
    assert_eq!(m.samples.start, 0x7A0);
    assert_eq!(m.instruments.len(), 17);
    assert_eq!(m.regions.len(), 71);
    assert_eq!(m.segments.len(), 8);
    let tempos: Vec<u32> = m.segments.iter().take(3).map(|s| s.tempo).collect();
    assert_eq!(tempos, [300_000, 545_454, 500_000]);
    assert!(m.segments.iter().all(|s| s.ticks_per_quarter == 480));
    for (i, s) in m.segments.iter().enumerate() {
        assert_eq!(s.handle, 0x0300_0000 | (i as u32) << 16);
    }
    let t = m.track;
    assert_eq!((t.volume, t.repeat, t.group, t.pan), (127, 0, 1, 0));
    let i0 = &m.instruments[0];
    assert_eq!((i0.regions.len(), i0.volume, i0.pan), (8, 90, 0));
    let r = m.instrument_regions(0)[0];
    assert_eq!((r.volume, r.centre, r.low_note, r.high_note), (127, -48, 36, 36));
    assert_eq!((r.adsr.w1, r.adsr.w2, r.sample_offset), (0x80FF, 0x9FCB, 0));
    let d = decode_to_end(m.sample_data(), 0);
    assert_eq!(d.pcm.len(), 3164);
    assert_eq!(&d.pcm[..6], &[0; 6]);
    assert_eq!(pcm_sha(&d.pcm), "96c6ec1762cc114d336efa18ee8e926a746832de4757a16dbb1fd30025877ed6");
    let looping = m
        .regions
        .iter()
        .filter(|r| r.flags & 0x80 == 0)
        .filter(|r| decode_to_end(m.sample_data(), r.sample_offset as usize).repeats)
        .count();
    assert_eq!(looping, 26);
    // Segment 0 starts the real first segment (the documented Paris pattern).
    let mut c = EventCursor::new();
    let (_, ev) = c.next(m.segment_events(0)).unwrap().unwrap();
    assert_eq!(ev, Event::Adaptive(&[0x13, 0x03, 0x00, 0x11, 0x07, 0x13, 0x01, 0x00]));

    let worlds = world_music(&mut disc);
    let hideout = worlds.iter().find(|w| w.1 == "The Hideout").expect("hideout");
    let h = MusicFile::parse(disc.read_descriptor(&hideout.2.unwrap()).unwrap()).unwrap();
    assert_eq!(&h.name, b"SLHQ");
    assert_eq!((h.instruments.len(), h.regions.len(), h.segments.len()), (17, 71, 2));
    let paris = worlds.iter().find(|w| w.1 == "Paris, France").unwrap();
    let pd = paris.2.unwrap();
    assert_eq!((pd.sector, pd.size, pd.identity), (0xC8E0E, 501_408, 0x55));
}

/// Counts adaptive commands in a list, including inside macro bodies when `nested`.
fn count_commands(list: &[u8], nested: bool, counts: &mut BTreeMap<u8, usize>) {
    for c in commands(list) {
        let c = c.expect("adaptive command");
        *counts.entry(c.code()).or_default() += 1;

        if nested && let audio::music::Command::DefineMacro(_, body) = c {
            count_commands(body, nested, counts);
        }
    }
}

#[test]
fn every_music_file() {
    let Some(disc) = open("every_music_file") else { return };
    let mut disc = disc.lock().unwrap();
    let worlds = world_music(&mut disc).to_vec();

    let with_music: Vec<_> = worlds.iter().filter(|w| w.2.is_some()).collect();
    assert_eq!(with_music.len(), 45);
    assert!(worlds.iter().all(|w| (-1..=6).contains(&w.3)));
    assert_eq!(worlds.iter().filter(|w| w.2.is_none()).map(|w| w.1.as_str()).collect::<Vec<_>>(), ["Splash"]);
    // 38 identities, 37 distinct files: two identities share byte-identical contents.
    let identities: BTreeSet<u32> = with_music.iter().map(|w| w.2.unwrap().identity).collect();
    assert_eq!(identities.len(), 38);
    let mut contents = BTreeSet::new();
    let mut by_identity: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut counts_all = BTreeMap::new();
    let mut counts_all_nested = BTreeMap::new();
    let mut counts_distinct = BTreeMap::new();
    let mut counts_distinct_nested = BTreeMap::new();
    for w in &with_music {
        let d = w.2.unwrap();

        let bytes = disc.read_descriptor(&d).unwrap();
        let first = contents.insert(sha256(&bytes));
        if let Some(prev) = by_identity.get(&d.identity) {
            assert_eq!(prev, &bytes, "copies with the same identity are identical");
        }
        let m = MusicFile::parse(bytes.clone()).unwrap();
        by_identity.insert(d.identity, bytes);
        // Every sample decodes to an end flag.
        for (ri, r) in m.regions.iter().enumerate() {
            if r.flags & 0x80 == 0 {
                assert!(sample_len(m.sample_data(), r.sample_offset as usize).is_some(), "{}: region {ri}", w.1);
            }
        }
        // Every segment walks; 0x9n and 0xDn messages balance.
        let (mut on, mut pressure) = (0usize, 0usize);
        for si in 0..m.segments.len() {
            let data = m.segment_events(si);
            let mut c = EventCursor::new();
            let mut running = 0u8;
            let mut eos = 0;
            loop {
                // Segment 0 ends in zero padding without an end-of-segment event.
                if data[c.pos..].iter().all(|&b| b == 0) {
                    break;
                }
                if c.read_delta(data).unwrap().is_none() {
                    break;
                }
                let b = data[c.pos];
                let ev = c.read_event(data).unwrap_or_else(|e| panic!("{} segment {si}: {e}", w.1));
                let status = match b {
                    0x80..=0xEF => {
                        running = b;
                        b
                    }
                    0xF0..=0xFF => {
                        running = 0;
                        b
                    }
                    _ => running,
                };
                match status & 0xF0 {
                    0x90 => on += 1,
                    0xD0 => pressure += 1,
                    _ => {}
                }
                if ev == Event::EndOfSegment {
                    eos += 1;
                    break;
                }
                if let Event::Adaptive(list) = ev {
                    count_commands(list, false, &mut counts_all);
                    count_commands(list, true, &mut counts_all_nested);
                    if first {
                        count_commands(list, false, &mut counts_distinct);
                        count_commands(list, true, &mut counts_distinct_nested);
                    }
                }
            }
            // Only segment 0 (a start-up stub followed by padding) has no end-of-segment event.
            assert!(eos == 1 || si == 0, "{} segment {si}: no end of segment", w.1);
        }
        assert_eq!(on, pressure, "{}: note-ons vs pressure note-offs", w.1);
    }
    let spec: BTreeMap<u8, usize> = [
        (0x00, 6707),
        (0x11, 4487),
        (0x16, 3395),
        (0x12, 3071),
        (0x0C, 2542),
        (0x04, 2368),
        (0x05, 2368),
        (0x10, 1795),
        (0x13, 1298),
        (0x02, 1276),
        (0x0E, 1152),
        (0x07, 609),
        (0x0D, 607),
        (0x06, 467),
        (0x03, 30),
        (0x0B, 27),
        (0x14, 27),
        (0x0F, 23),
    ]
    .into_iter()
    .collect();
    // Top-level commands over all 45 files (copies counted again) match the spec's counts, except `00`: the disc
    // has 6,687 where the spec says 6,707 (open question in docs/audio.md).
    let mut expect = spec.clone();
    expect.insert(0x00, 6687);
    assert_eq!(counts_all, expect, "distinct files: {counts_distinct:?}; with macro bodies: {counts_all_nested:?}");
    let _ = counts_distinct_nested;
}

#[test]
fn renders_paris_music() {
    let Some(disc) = open("renders_paris_music") else { return };
    let mut disc = disc.lock().unwrap();
    let m = std::sync::Arc::new(MusicFile::parse(raw(&mut disc, 0xC8E0E, 501_408)).unwrap());
    let (mut e, mut r) = create(Config::default());
    e.play_music(m);
    let mut out = vec![0.0f32; 384 * 2];
    let mut energy = 0.0f64;
    for _ in 0..(10 * 48_000 / 384) {
        r.render(&mut out);
        energy += out.iter().map(|&x| (x * x) as f64).sum::<f64>();
    }
    assert!(energy > 1.0, "the Paris music should be audible (energy {energy})");
    // Segment 0 handed over to segment 7.
    assert!(r.sequencer().active_segments().any(|s| s == 7) || r.sequencer().playing());
    assert_eq!(r.sequencer().registers[3], 0);
}

#[test]
fn renders_bank0_sound0() {
    let Some(disc) = open("renders_bank0_sound0") else { return };
    let mut disc = disc.lock().unwrap();
    let b0 = std::sync::Arc::new(read_bank(&mut disc, 0).unwrap());
    let (mut e, mut r) = create(Config::default());
    e.load_bank(0, Some(b0));
    let h = e.play(0, Emitter::global());
    assert!(h.is_some());
    let mut pcm = vec![0.0f32; 48_000 * 2];
    r.render(&mut pcm);
    let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
    let sha = hex(&sha256(&bytes));
    eprintln!("bank 0 sound 0, 1 s mixed: {sha}");
    // 1,876 samples at 11,025 Hz: about 8,170 output frames, then silence.
    let last = pcm.chunks(2).rposition(|f| f[0] != 0.0 || f[1] != 0.0).unwrap();
    assert!((8000..8400).contains(&last), "last audible frame {last}");
    assert_eq!(sha, "006d322bf6990d073e8a65c8cc99ad6077c2aae6352cf0d4e117f18ac2cae057");
}
