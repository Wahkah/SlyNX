//! The Sly 2 session on the player's own Sly 2 disc (`SLYNX_DISC_SLY2`), on the renderer's null backend; skipped
//! when the disc is not set.

use engine::game::systems::FrameInput;
use game::sly2::{HUB_WORLD_ID, Sly2Session};
use harness::golden::{Game, require_disc};
use render::NullBackend;

fn session(path: &std::path::Path) -> Sly2Session<NullBackend, std::fs::File> {
    let disc = formats_sly2::Disc::open_path(path).map_err(|e| e.to_string());
    Sly2Session::new(NullBackend::new(1280, 720), disc, 7)
}

#[test]
fn the_hub_loads_draws_and_runs_deterministically() {
    let Some(path) = require_disc(Game::Sly2, "the_hub_loads_draws_and_runs_deterministically") else { return };
    let run = || {
        let mut s = session(&path);
        assert!(s.gang_constants.is_some(), "the gang's constants read from the executable");
        let row = s.find_world(HUB_WORLD_ID).expect("hub row");
        s.load_row(row).unwrap();
        assert!(s.current().unwrap().objects > 0);
        let input = FrameInput { left: [0.0, 1.0], ..Default::default() };
        let mut hashes = Vec::new();
        for _ in 0..120 {
            s.update(&input, 1.0 / 60.0);
            s.render().unwrap();
            hashes.push(s.state_hash());
        }
        eprintln!("{:?}", s.status_lines());
        hashes
    };
    assert_eq!(run(), run());
}

#[test]
fn an_unloaded_session_renders_and_loads_again() {
    let Some(path) = require_disc(Game::Sly2, "an_unloaded_session_renders_and_loads_again") else { return };
    let mut s = session(&path);
    let row = s.find_world(HUB_WORLD_ID).expect("hub row");
    s.load_row(row).unwrap();
    s.update(&FrameInput::default(), 1.0 / 60.0);
    s.render().unwrap();
    // The reset a failed upload makes: no world, and frames still update and render.
    s.unload();
    assert!(s.current().is_none());
    s.update(&FrameInput::default(), 1.0 / 60.0);
    s.render().unwrap();
    s.load_row(row).unwrap();
    s.update(&FrameInput::default(), 1.0 / 60.0);
    s.render().unwrap();
    assert!(s.current().unwrap().objects > 0);
}

#[test]
fn the_safe_house_chooses_and_acts() {
    let Some(path) = require_disc(Game::Sly2, "the_safe_house_chooses_and_acts") else { return };
    let mut s = session(&path);
    let row = s.find_world(HUB_WORLD_ID).expect("hub row");
    s.load_row(row).unwrap();
    // safehouse-and-shop.md 1: selectable states (8, 10, 7, 9) on the disc.
    assert_eq!(s.safe_house.as_ref().expect("the hub's safe house").states, vec![8, 10, 7, 9]);
    assert!(s.enter_safe_house());
    let press = |b: platform_api::Buttons| FrameInput { pressed: b.0, buttons: b.0, ..Default::default() };
    // To the computer: Cross opens the shop (object 888).
    while s.safe_house.as_ref().unwrap().state() != Some(10) {
        s.update(&press(platform_api::Buttons::RIGHT), 1.0 / 60.0);
    }
    s.update(&press(platform_api::Buttons::B), 1.0 / 60.0);
    assert!(s.menus_open().1.is_some(), "the shop opened");
    // A character's state takes that character out.
    s.update(&press(platform_api::Buttons::A), 1.0 / 60.0);
    let mut s = session(&path);
    s.load_row(row).unwrap();
    assert!(s.enter_safe_house());
    while s.safe_house.as_ref().unwrap().state() != Some(7) {
        s.update(&press(platform_api::Buttons::RIGHT), 1.0 / 60.0);
    }
    s.update(&press(platform_api::Buttons::B), 1.0 / 60.0);
    assert!(!s.in_safe_house, "Sly was taken out");
}

#[test]
fn the_vault_waits_for_every_bottle() {
    let Some(path) = require_disc(Game::Sly2, "the_vault_waits_for_every_bottle") else { return };
    let mut s = session(&path);
    s.load_row(13).unwrap();
    let w = &s.game.as_ref().unwrap().world;
    let vaults =
        w.handles().filter(|&h| matches!(w.behavior(h), Some(engine::behavior::Behavior::Sly2Vault(_)))).count();
    assert_eq!(vaults, 1, "row 13 has the episode's vault");
    for _ in 0..30 {
        s.update(&FrameInput::default(), 1.0 / 60.0);
    }
    let g = s.game.as_mut().unwrap();
    assert_eq!(engine::behavior::sly2_vault::clue_bits(&mut g.world), 0, "no bottles in a new game");
}

#[test]
fn bison_fights_on_the_guards_engine() {
    let Some(path) = require_disc(Game::Sly2, "bison_fights_on_the_guards_engine") else { return };
    let mut s = session(&path);
    // vehicles-and-minigames.md 7.5: the fight is row 37; Bison has 900,000 hit points and a block of 500.
    s.load_row(37).unwrap();
    let w = &s.game.as_ref().unwrap().world;
    let bison = w
        .handles()
        .filter(|&h| w.class(h) == Some(62))
        .find_map(|h| match w.behavior(h) {
            Some(engine::behavior::Behavior::Sly2Guard(g)) => Some(&**g),
            _ => None,
        })
        .expect("Bison on the guards' engine");
    eprintln!("bison: hp {} block {} parts {:?}", bison.hit_points, bison.block, bison.parts);
    for a in &bison.attacks {
        eprintln!("  action {} length {} windows {:?}", a.action, a.length, &a.strike[..a.windows as usize]);
    }
    assert_eq!((bison.hit_points, bison.block, bison.boss), (900_000.0, 500.0, true));
    assert!(bison.parts.iter().any(|p| p.kind == 0x400 && p.amount == 20.0), "the crush part");
    let first = bison.attacks.iter().find(|a| a.action == 19643).expect("attack 19643");
    assert!((first.length - 2.0).abs() < 1e-3 && first.windows == 2);
}

#[test]
fn every_world_loads_on_the_renderer() {
    let Some(path) = require_disc(Game::Sly2, "every_world_loads_on_the_renderer") else { return };
    let mut s = session(&path);
    let worlds = s.worlds();
    assert!(!worlds.is_empty());
    for (row, id) in worlds {
        s.load_row(row).unwrap_or_else(|e| panic!("row {row} ({id:#x}): {e}"));
        s.update(&FrameInput::default(), 1.0 / 60.0);
        s.render().unwrap();
        eprintln!("{:?}", s.status_lines());
    }
}

#[test]
fn the_gang_lands_runs_and_switches_in_the_hub() {
    let Some(path) = require_disc(Game::Sly2, "the_gang_lands_runs_and_switches_in_the_hub") else { return };
    let mut s = session(&path);
    s.load_row(s.find_world(HUB_WORLD_ID).unwrap()).unwrap();
    let class_of_player = |s: &Sly2Session<NullBackend, std::fs::File>| {
        let w = &s.game.as_ref().unwrap().world;
        w.player.and_then(|h| w.class(h))
    };
    let pos = |s: &Sly2Session<NullBackend, std::fs::File>| {
        let w = &s.game.as_ref().unwrap().world;
        w.player.and_then(|h| w.position(h)).unwrap()
    };
    assert_eq!(class_of_player(&s), Some(53), "Sly is played first");
    assert!(s.character_instances() > 0, "the characters' skinned meshes are drawn");
    s.render().unwrap();
    // Drop to the floor, then run forward for a second.
    for _ in 0..120 {
        s.update(&FrameInput::default(), 1.0 / 60.0);
    }
    let landed = pos(&s);
    let run = FrameInput { left: [0.0, 1.0], buttons: platform_api::Buttons::ZR.0, ..Default::default() };
    for _ in 0..60 {
        s.update(&run, 1.0 / 60.0);
    }
    let moved = pos(&s);
    let d = ((moved[0] - landed[0]).powi(2) + (moved[1] - landed[1]).powi(2)).sqrt();
    assert!(d > 100.0, "Sly runs: {d}");
    // The d-pad switches: up Murray, right Bentley, left Sly.
    for (b, class) in
        [(platform_api::Buttons::UP, 54), (platform_api::Buttons::RIGHT, 55), (platform_api::Buttons::LEFT, 53)]
    {
        s.update(&FrameInput { pressed: b.0, buttons: b.0, ..Default::default() }, 1.0 / 60.0);
        assert_eq!(class_of_player(&s), Some(class));
    }
    // The follow camera looks at the played character (once the episode's opening cutscene, whose clips add and
    // remove camera objects, has given the view back; camera.md 7.2.1).
    let mut d = f32::MAX;
    for _ in 0..1800 {
        let g = s.game.as_ref().unwrap();
        let eye = g.render.camera.eye;
        let p = pos(&s);
        d = ((eye[0] - p[0]).powi(2) + (eye[1] - p[1]).powi(2)).sqrt();
        if d < 3000.0 {
            break;
        }
        s.update(&FrameInput::default(), 1.0 / 60.0);
    }
    assert!(d < 3000.0, "the camera follows the gang: {d}");
}

#[test]
fn guards_are_placed_with_pockets_in_their_worlds() {
    let Some(path) = require_disc(Game::Sly2, "guards_are_placed_with_pockets_in_their_worlds") else { return };
    let mut s = session(&path);
    // f_nightclub_cellar (row 3): guards.md 1 lists 4 boar guards and 8 rats placed at start.
    s.load_row(3).unwrap();
    let w = &s.game.as_ref().unwrap().world;
    let guards: Vec<&engine::behavior::sly2::Sly2Guard> = w
        .handles()
        .filter(|&h| w.in_world(h))
        .filter_map(|h| match w.behavior(h) {
            Some(engine::behavior::Behavior::Sly2Guard(g)) => Some(&**g),
            _ => None,
        })
        .collect();
    let (swarmers, boars) = (guards.iter().filter(|g| g.swarmer).count(), guards.iter().filter(|g| !g.swarmer).count());
    eprintln!("row 3: {boars} guards, {swarmers} swarmers in the world");
    assert!(boars >= 4 && swarmers >= 8, "{boars} guards, {swarmers} swarmers");
    assert!(guards.iter().filter(|g| !g.swarmer).all(|g| g.hit_points == 40.0));
    assert!(guards.iter().any(|g| !g.pocket.entries.is_empty()), "pockets filled from the executable's tables");
    // guards.md 6.3: every guard strikes with part 13056, knock-back, value -10, in its attack clips' windows.
    for g in guards.iter().filter(|g| !g.swarmer) {
        let p = g.parts.iter().find(|p| p.tag == 13056).expect("the guard's striking part");
        assert_eq!((p.kind, p.amount), (0x200, 10.0));
    }
    let windows: Vec<engine::behavior::sly2::Window> = guards
        .iter()
        .filter(|g| !g.swarmer)
        .flat_map(|g| g.attacks.iter().flat_map(|a| a.strike[..a.windows as usize].to_vec()))
        .collect();
    eprintln!("guard strike windows: {windows:?}");
    assert!(windows.iter().any(|w| w.2 == 13056), "attack clips switch the striking part");
    let rats: Vec<_> = guards.iter().filter(|g| g.swarmer).flat_map(|g| g.parts.iter().copied()).collect();
    eprintln!("swarmer parts: {rats:?}");
    assert!(rats.iter().any(|p| p.amount == 3.0), "rats bite for 3");
    for _ in 0..120 {
        s.update(&FrameInput::default(), 1.0 / 60.0);
    }
}

#[test]
fn a_sequence_plays_its_lines_with_subtitles() {
    let Some(path) = require_disc(Game::Sly2, "a_sequence_plays_its_lines_with_subtitles") else { return };
    let mut s = session(&path);
    let (engine, mut mixer) = audio::create(audio::Config::default());
    s.set_sound(engine, Box::new(std::fs::File::open(&path).unwrap()));
    assert!(s.sound.is_some());
    // f_nightclub_cellar (row 3): the first loaded sequence that plays a line. (Sequence 1714 of the spec's test
    // vector is not among the world's loaded objects.)
    s.load_row(3).unwrap();
    let h = {
        let w = &s.game.as_ref().unwrap().world;
        w.handles()
            .filter(|&h| w.class(h) == Some(114))
            .find(|&h| {
                game::sly2::talk::sequence_commands(w, h).iter().any(|c| {
                    matches!(
                        c,
                        game::sly2::talk::SeqCommand::Talk(audio::sly2::TalkCommand::PlayLine { quiet: false, .. })
                    )
                })
            })
            .expect("a loaded sequence with a subtitled line")
    };
    // A script's call 614 starts it with the current character as starter (dialogue-and-voice.md 2.1).
    assert!(s.game.as_mut().unwrap().world.run_action(h, 614, &[]));
    let mut out = vec![0.0f32; 800 * 2];
    let mut shown = false;
    for _ in 0..120 {
        mixer.render(&mut out);
        s.update(&FrameInput::default(), 1.0 / 60.0);
        shown |= s.subtitle().is_some_and(|t| !t.text.is_empty());
    }
    assert!(s.sequences_started >= 1, "started (the world's own scripts start others too)");
    let sound = s.sound.as_ref().unwrap();
    assert!(sound.lines_played >= 1, "the first line plays");
    assert!(shown, "its subtitle is revealed");
}

#[test]
fn sly2_behaviours_attach_with_the_spec_counts() {
    use engine::behavior::Behavior;
    let Some(path) = require_disc(Game::Sly2, "sly2_behaviours_attach_with_the_spec_counts") else { return };
    let mut s = session(&path);
    for row in [0usize, 2, 3, 13] {
        s.load_row(row).unwrap();
        let w = &s.game.as_ref().unwrap().world;
        let (mut lasers, mut beams, mut pads, mut detectors, mut pickups, mut bottles, mut props, mut volumes) =
            (0, 0, 0, 0, 0, 0, 0, 0);
        for h in w.handles().filter(|&h| w.in_world(h)) {
            match w.behavior(h) {
                Some(Behavior::Sly2Laser(l)) => {
                    lasers += 1;
                    beams += l.beam_count();
                }
                Some(Behavior::Sly2Pad(_)) => pads += 1,
                Some(Behavior::Sly2Detector(_)) => detectors += 1,
                Some(Behavior::Sly2Pickup(p)) => {
                    pickups += 1;
                    bottles += matches!(p.kind, engine::behavior::sly2_pickups::Kind::Clue(_)) as usize;
                }
                Some(Behavior::Sly2Prop(_)) => props += 1,
                Some(Behavior::Sly2Volume(_)) => volumes += 1,
                _ => {}
            }
        }
        eprintln!(
            "row {row}: lasers {lasers} ({beams} beams), pads {pads}, detectors {detectors}, pickups {pickups} \
             ({bottles} bottles), props {props}, volumes {volumes}"
        );
        if row == 13 {
            assert!(lasers > 0 && beams >= lasers, "row 13 has lasers with beams");
        }
        if row == 2 {
            assert_eq!(bottles, 30, "the nightclub hub holds the episode's 30 clue bottles");
        }
        for _ in 0..60 {
            s.update(&FrameInput::default(), 1.0 / 60.0);
        }
    }
}
