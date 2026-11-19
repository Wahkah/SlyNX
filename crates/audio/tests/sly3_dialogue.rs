//! Sly 3 dialogue on the player's own Sly 3 disc (`SLYNX_DISC_SLY3`; skipped without it): the dialogue commands of
//! every command script (specs/sly3/scripts.md 7.2 and 8), each listed line against its world's voice entries and
//! strings, and a line played through the talk player.

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use audio::sly2::Talk;
use audio::sly3::talk::{self, DialogueCommand};
use audio::sly3::{Sly3Disc, WorldAudio};
use formats_sly3::stream::{Record, command_script, visit};
use harness::golden::{Game, require_disc};

struct World {
    name: String,
    audio: WorldAudio,
    /// (script OID, command id, decoded dialogue command)
    commands: Vec<(i32, u8, Option<DialogueCommand>)>,
    /// Commands of top-level scripts: (command id) counts.
    top: BTreeMap<u8, usize>,
}

struct Data {
    path: std::path::PathBuf,
    worlds: Vec<World>,
}

fn data(test: &str) -> Option<&'static Data> {
    let path = require_disc(Game::Sly3, test)?;
    static CELL: OnceLock<Data> = OnceLock::new();
    Some(CELL.get_or_init(|| {
        let mut fd = formats_sly3::Disc::open_path(&path).expect("open (formats)");
        let mut ad = Sly3Disc::open_path(&path).expect("open (audio)");
        let slots: Vec<usize> = fd.world_slots().collect();
        let worlds = slots
            .into_iter()
            .map(|slot| {
                let bytes = fd.read_world(slot).unwrap();
                let w = formats_sly3::World::parse(&bytes, &fd.options).unwrap();
                let mut commands = Vec::new();
                let mut top = BTreeMap::new();
                for b in &w.bodies {
                    let tops: std::collections::BTreeSet<u32> = b.objects.iter().map(|r| r.header_offset).collect();
                    visit(&b.objects, &mut |r: &Record| {
                        if r.class != talk::COMMAND_SCRIPT_CLASS {
                            return;
                        }
                        let (_, cmds) = command_script(&bytes, r.body_offset, &fd.options).unwrap();
                        for c in cmds {
                            if tops.contains(&r.header_offset) {
                                *top.entry(c.id).or_insert(0usize) += 1;
                            }
                            let a = &bytes[c.args.0 as usize..(c.args.0 + c.args.1) as usize];
                            commands.push((r.id, c.id, DialogueCommand::decode(c.id, a)));
                        }
                    });
                }
                World { name: fd.worlds[slot].name.clone(), audio: ad.load_world_audio(slot).unwrap(), commands, top }
            })
            .collect();
        Data { path, worlds }
    }))
}

#[test]
fn dialogue_commands_and_their_lines() {
    let Some(d) = data("dialogue_commands_and_their_lines") else { return };
    let mut uses: BTreeMap<u8, usize> = BTreeMap::new();
    let (mut lists, mut listed, mut voiced, mut subtitled, mut rotated) = (0, 0, 0, 0, 0);
    let mut rng = audio::rng::Rng::new(1);
    let mut list_lengths: BTreeMap<usize, usize> = BTreeMap::new();
    let mut quiet = 0;
    for w in &d.worlds {
        for (_, id, cmd) in &w.commands {
            let dialogue = [0x52u8, 0x53, 0x54, 0x55, 0x56, 0x5F].contains(id);
            assert_eq!(cmd.is_some(), dialogue, "{}: command {id:#x}", w.name);
            if !dialogue {
                continue;
            }
            *uses.entry(*id).or_default() += 1;
            if let Some(DialogueCommand::PlayLine { lines, quiet: q, .. }) = cmd {
                lists += 1;
                quiet += usize::from(*q);
                *list_lengths.entry(lines.len()).or_default() += 1;
                for &l in lines {
                    listed += 1;
                    let l = l as i16;
                    voiced += usize::from(w.audio.line(l).is_some());
                    subtitled += usize::from(w.audio.subtitle(l).is_some());
                }
                // dialogue-and-voice.md 1.1 on the disc's lists: three rounds play only listed lines, a list of
                // several never the same line twice in a row, and a fresh list its second entry first.
                if lines.len() > 1 {
                    rotated += 1;
                    let mut order = talk::LineOrder::new(lines);
                    let played: Vec<i32> = (0..lines.len() * 3).map(|_| order.next(&mut rng).unwrap()).collect();
                    assert_eq!(played[0], lines[1]);
                    assert!(played.iter().all(|l| lines.contains(l)));
                    let distinct = lines.iter().collect::<std::collections::BTreeSet<_>>().len() == lines.len();
                    assert!(!distinct || played.windows(2).all(|p| p[0] != p[1]), "{}: {lines:?}", w.name);
                }
            }
        }
    }
    eprintln!(
        "uses {uses:?}; {lists} play commands ({quiet} quiet), list lengths {list_lengths:?}; {listed} listed lines, \
         {voiced} with a voice entry, {subtitled} with a string; {rotated} lists of several lines"
    );
    // scripts.md 7.2 counts the commands of the 5,863 top-level scripts; the 548 nested scripts add a few more.
    let mut top: BTreeMap<u8, usize> = BTreeMap::new();
    for w in &d.worlds {
        for (k, v) in &w.top {
            *top.entry(*k).or_default() += v;
        }
    }
    let t = |id: u8| top.get(&id).copied().unwrap_or(0);
    eprintln!("top-level: {:?}", [0x52u8, 0x53, 0x54, 0x55, 0x56, 0x5F].map(t));
    assert_eq!((t(0x52), t(0x53), t(0x54), t(0x5F)), (12_729, 49, 2_011, 1_169));
    assert_eq!((t(0x55), t(0x56)), (4_222, 3_998));
    let u = |id: u8| uses.get(&id).copied().unwrap_or(0);
    assert_eq!((u(0x52), u(0x53), u(0x54), u(0x55), u(0x56), u(0x5F)), (12_752, 49, 2_011, 4_232, 4_002, 1_169));
    assert_eq!(lists, 12_752);
    // Every listed line names a voice entry of its world, and a string with its number (Sly 2's subtitle rule).
    assert_eq!((voiced, subtitled), (listed, listed));
    assert_eq!(rotated, 712);
}

#[test]
fn v_apt_script_2088_plays_its_line() {
    let Some(d) = data("v_apt_script_2088_plays_its_line") else { return };
    let w = d.worlds.iter().find(|w| w.name == "v_apt").unwrap();
    // scripts.md 8: script 2088's play command: line list (1587), quiet, source −1, fall-off 300 to 3,000, volume 1.
    let cmds: Vec<&(i32, u8, Option<DialogueCommand>)> = w.commands.iter().filter(|c| c.0 == 2088).collect();
    let ids: Vec<u8> = cmds.iter().map(|c| c.1).collect();
    assert_eq!(ids, [0x00, 0x2F, 0x52, 0x01]);
    let play = cmds[2].2.clone().unwrap();
    assert_eq!(
        play,
        DialogueCommand::PlayLine { lines: vec![1587], quiet: true, source: -1, falloff: (300, 3000), volume: 1.0 }
    );
    let DialogueCommand::PlayLine { lines, .. } = &play else { unreachable!() };
    let n = talk::LineOrder::new(lines).next(&mut audio::rng::Rng::new(1)).unwrap() as i16;
    let line = w.audio.line(n).expect("voice entry");
    let mut disc = Sly3Disc::open_path(&d.path).unwrap();
    let file = line.tracks[0].file.expect("voice file");
    let vag = disc.read_vag(&file).unwrap();
    assert_eq!(vag.header.sample_rate, 22_050);
    let (mut engine, mut renderer) = audio::create(audio::Config::default());
    w.audio.install(&mut engine);
    let mut t = Talk::default();
    let mut cues = Vec::new();
    t.play_text(&mut engine, line, Some(Arc::new(vag)), w.audio.subtitle(n), true, &mut cues);
    assert_eq!(t.subtitle(0.0), None, "a quiet line has no subtitle");
    let mut out = vec![0.0f32; 1600];
    let mut peak = 0.0f32;
    for _ in 0..30 {
        t.update(1.0 / 60.0, &mut cues);
        engine.update(1.0 / 60.0);
        renderer.render(&mut out);
        peak = out.iter().fold(peak, |m, s| m.max(s.abs()));
    }
    assert!(peak > 0.001, "the line is audible");
}
