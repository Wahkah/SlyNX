//! Sly 2 dialogue on the player's own Sly 2 disc (`SLYNX_DISC_SLY2`; skipped without it): the vectors of
//! `specs/sly2/dialogue-and-voice.md`. Every play-line command of every sequence object finds its voice entry and
//! subtitle; the counts per world and per command; the nightclub-cellar sequence; the talk-target table; and every
//! world's sound loads, installs and plays a line with its subtitle.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use audio::sly2::talk::{self, Command, Talk};
use audio::sly2::{Sly2Disc, WorldAudio};
use formats_sly2::stream::{Record, visit};
use harness::golden::{Game, require_disc};

/// Option IDs of the dialogue commands.
const DIALOGUE: [i16; 9] = [695, 708, 692, 696, 685, 686, 679, 697, 698];

struct Sequence {
    id: i32,
    top: bool,
    body_offset: u32,
    /// (option id, decoded command if a dialogue command, raw bytes)
    options: Vec<(i16, Option<Command>, Vec<u8>)>,
}

struct World {
    row: usize,
    audio: WorldAudio,
    sequences: Vec<Sequence>,
}

struct Data {
    path: std::path::PathBuf,
    worlds: Vec<World>,
}

fn data(test: &str) -> Option<&'static Data> {
    let path = require_disc(Game::Sly2, test)?;
    static CELL: OnceLock<Data> = OnceLock::new();
    Some(CELL.get_or_init(|| {
        let mut fd = formats_sly2::Disc::open_path(&path).expect("open (formats)");
        let mut ad = Sly2Disc::open_path(&path).expect("open (audio)");
        let rows: Vec<usize> = fd.world_rows().collect();
        let worlds = rows
            .into_iter()
            .map(|row| {
                let bytes = fd.read_world(row).unwrap();
                let w = formats_sly2::World::parse(&bytes, &fd.options).unwrap();
                let mut sequences = Vec::new();
                for b in &w.bodies {
                    let top: BTreeSet<u32> = b.objects.iter().map(|r| r.header_offset).collect();
                    visit(&b.objects, &mut |r: &Record| {
                        if r.class != talk::SEQUENCE_CLASS {
                            return;
                        }
                        let list = r.options().expect("sequence option list");
                        let options = list
                            .options
                            .iter()
                            .map(|o| {
                                let v = bytes[o.value.0 as usize..(o.value.0 + o.value.1) as usize].to_vec();
                                (o.id, Command::decode(o.id, &v), v)
                            })
                            .collect();
                        sequences.push(Sequence {
                            id: r.id,
                            top: top.contains(&r.header_offset),
                            body_offset: r.body_offset,
                            options,
                        });
                    });
                }
                World { row, audio: ad.load_world_audio(row).unwrap(), sequences }
            })
            .collect();
        Data { path, worlds }
    }))
}

#[test]
fn every_line_command_finds_its_voice_and_subtitle() {
    let Some(d) = data("every_line_command_finds_its_voice_and_subtitle") else { return };
    let (mut sequences, mut top, mut lines, mut quiet) = (0, 0, 0, 0);
    let mut uses: BTreeMap<i16, usize> = BTreeMap::new();
    let mut distinct = BTreeSet::new();
    let mut binoc: BTreeMap<(i32, i32, i32), usize> = BTreeMap::new();
    let (mut voice_entries, mut voice_distinct) = (0, BTreeSet::new());
    for w in &d.worlds {
        sequences += w.sequences.len();
        top += w.sequences.iter().filter(|s| s.top).count();
        for e in &w.audio.sound().voice_lines {
            voice_entries += 1;
            voice_distinct.insert(e.number);
            assert!(w.audio.subtitle(e.number).is_some(), "row {}: line {} has no string", w.row, e.number);
        }
        for s in &w.sequences {
            for (id, cmd, raw) in &s.options {
                let Some(cmd) = cmd else {
                    assert!(!DIALOGUE.contains(id), "row {}: option {id} {raw:02x?} does not decode", w.row);
                    continue;
                };
                *uses.entry(*id).or_default() += 1;
                match *cmd {
                    Command::PlayLine { line, quiet: q, .. } => {
                        lines += 1;
                        quiet += usize::from(q);
                        distinct.insert(line);
                        assert!(w.audio.line(line).is_some(), "row {}: line {line} has no voice entry", w.row);
                        assert!(w.audio.subtitle(line).is_some(), "row {}: line {line} has no subtitle", w.row);
                    }
                    Command::WaitLine { time } => assert_eq!(time, -1.0),
                    Command::OpenBinocucom { viewer, other, window, .. } => {
                        *binoc.entry((viewer, other, window)).or_default() += 1
                    }
                    Command::CloseEyes { amount, .. } => assert_eq!(amount, 0.0),
                    _ => {}
                }
            }
        }
    }
    assert_eq!((sequences, top, sequences - top), (3_871, 3_545, 326));
    assert_eq!((lines, quiet, lines - quiet), (5_262, 662, 4_600));
    assert_eq!(distinct.len(), 1_054);
    assert_eq!((voice_entries, voice_distinct.len()), (7_970, 1_691));
    let u = |id: i16| uses.get(&id).copied().unwrap_or(0);
    assert_eq!((u(708), u(692), u(696), u(685), u(686)), (1_054, 26, 429, 405, 405));
    assert_eq!((u(679), u(697), u(698)), (195, 1_702, 1_443));
    assert_eq!(
        binoc.into_iter().collect::<Vec<_>>(),
        [((7, 8, -1), 271), ((8, -1, 2385), 9), ((8, 7, -1), 43), ((8, 9, -1), 15), ((9, 7, -1), 14), ((9, 8, -1), 53)]
    );
}

#[test]
fn per_world_counts() {
    let Some(d) = data("per_world_counts") else { return };
    // dialogue-and-voice.md 8: (row, sequences, with lines, line commands, voice entries).
    let want = [
        (0, 57, 28, 59, 61),
        (2, 98, 44, 119, 193),
        (3, 82, 44, 119, 156),
        (12, 110, 43, 129, 169),
        (17, 102, 49, 156, 275),
        (32, 169, 58, 152, 262),
        (38, 84, 28, 89, 149),
    ];
    let count = |s: &Sequence| s.options.iter().filter(|o| matches!(o.1, Some(Command::PlayLine { .. }))).count();
    for (row, seqs, with, lines, voices) in want {
        let w = d.worlds.iter().find(|w| w.row == row).unwrap();
        let got = (
            w.sequences.len(),
            w.sequences.iter().filter(|s| count(s) > 0).count(),
            w.sequences.iter().map(count).sum::<usize>(),
            w.audio.sound().voice_lines.len(),
        );
        assert_eq!(got, (seqs, with, lines, voices), "row {row}");
    }
    // 50 to 169 per world, except row 1, which has none (the spec's 42 rows with sequences).
    for w in &d.worlds {
        let n = w.sequences.len();
        assert!((50..=169).contains(&n) || (w.row == 1 && n == 0), "row {}: {n}", w.row);
    }
}

#[test]
fn cellar_sequence_and_lines() {
    let Some(d) = data("cellar_sequence_and_lines") else { return };
    let w = d.worlds.iter().find(|w| w.row == 3).unwrap();
    assert_eq!(w.audio.header.level_name(), "f_nightclub_cellar");
    let s = w.sequences.iter().find(|s| s.id == 1714).unwrap();
    assert!(s.top);
    assert_eq!(s.body_offset, 0x722FC5);
    let ids: Vec<i16> = s.options.iter().map(|o| o.0).collect();
    assert_eq!(ids, [689, 685, 695, 689, 695, 695, 689, 695, 689, 695, 660, 686]);
    assert!(matches!(s.options[1].1, Some(Command::OpenBinocucom { viewer: 7, other: 8, window: -1, .. })));
    let played: Vec<(i16, bool)> = s
        .options
        .iter()
        .filter_map(|o| match o.1 {
            Some(Command::PlayLine { line, quiet, .. }) => Some((line, quiet)),
            _ => None,
        })
        .collect();
    assert_eq!(played, [(1299, false), (1300, false), (1301, false), (1302, false), (1303, false)]);
    assert_eq!(s.options[4].2, [0x10, 0, 0x14, 0x05, 0x5d, 0x10, 0, 0, 0, 0, 0, 0, 0]);
    for (n, speaker, len, duration, records) in
        [(1299, 55, 101, 7.425, 65), (1300, 53, 29, 1.280, 19), (1301, 55, 124, 8.726, 70)]
    {
        let l = w.audio.line(n).unwrap();
        assert_eq!(l.speaker, speaker, "line {n}");
        assert_eq!(w.audio.subtitle(n).unwrap().len(), len, "line {n}");
        assert!((l.tracks[0].duration - duration).abs() < 0.001, "line {n}");
        assert_eq!(l.records.len(), records, "line {n}");
    }
    assert_eq!(talk::subtitle_text(w.audio.subtitle(1300).unwrap()), "How am I supposed to do that?");

    // The line through the talk player: voice, subtitle reveal (22 of 29 after 0.6 s), colour, cues.
    let mut disc = Sly2Disc::open_path(&d.path).unwrap();
    assert_eq!(disc.talk_targets().unwrap(), [1548, 1549, 1550, -1, 1551, 1552]);
    let vag = disc.read_line(&w.audio, 1300).unwrap().unwrap();
    assert_eq!(vag.header.sample_rate, 22_050);
    let (mut engine, mut renderer) = audio::create(audio::Config::default());
    w.audio.install(&mut engine);
    let mut talk = Talk::default();
    let mut cues = Vec::new();
    let line = w.audio.line(1300).unwrap();
    talk.play(&mut engine, line, Some(Arc::new(vag)), w.audio.subtitle(1300), false, &mut cues);
    let mut out = vec![0.0f32; 1600];
    let mut peak = 0.0f32;
    for _ in 0..36 {
        talk.update(1.0 / 60.0, &mut cues);
        engine.update(1.0 / 60.0);
        renderer.render(&mut out);
        peak = out.iter().fold(peak, |m, s| m.max(s.abs()));
    }
    let sub = talk.subtitle(0.6).unwrap();
    assert_eq!(sub.text.chars().count(), 23, "22 characters and the cursor: {:?}", sub.text);
    assert!(sub.text.ends_with('|'));
    assert_eq!(sub.colour, [0x33, 0x66, 0x99, 0x7F]);
    assert!(peak > 0.01, "the line is audible");
    for _ in 0..60 {
        talk.update(1.0 / 60.0, &mut cues);
    }
    assert_eq!(cues.len(), 19, "every timed record fired");
    assert!(talk.wait_done(-1.0));
    // Whole text: the cursor blinks, shown when floor(4 t) is odd.
    assert_eq!(talk.subtitle(0.25).unwrap().text, "How am I supposed to do that?|");
    assert_eq!(talk.subtitle(0.5).unwrap().text, "How am I supposed to do that?");
}

#[test]
fn every_world_sound_installs_and_speaks() {
    let Some(d) = data("every_world_sound_installs_and_speaks") else { return };
    let mut disc = Sly2Disc::open_path(&d.path).unwrap();
    for w in &d.worlds {
        let (mut engine, mut renderer) = audio::create(audio::Config::default());
        let h = w.audio.install(&mut engine);
        assert_eq!(h.is_some(), w.audio.stream.is_some(), "row {}", w.row);
        let mut talk = Talk::default();
        let mut cues = Vec::new();
        if let Some(&n) = audio::sly2::world::voiced_lines(w.audio.sound()).first() {
            let vag = disc.read_line(&w.audio, n).unwrap().map(Arc::new);
            talk.play(&mut engine, w.audio.line(n).unwrap(), vag, w.audio.subtitle(n), false, &mut cues);
        }
        let mut out = vec![0.0f32; 1600];
        let mut peak = 0.0f32;
        for _ in 0..30 {
            talk.update(1.0 / 60.0, &mut cues);
            engine.update(1.0 / 60.0);
            renderer.render(&mut out);
            peak = out.iter().fold(peak, |m, s| m.max(s.abs()));
        }
        assert!(peak > 0.001, "row {}: silent", w.row);
    }
}
