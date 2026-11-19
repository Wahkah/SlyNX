//! Sly 3 level flow, safe house and shop on the player's own Sly 3 disc (`SLYNX_DISC_SLY3`; skipped without it):
//! specs/sly3/level-flow.md and safehouse-and-shop.md.

use std::collections::{BTreeMap, BTreeSet};

use engine::game::systems::FrameInput;
use game::sly3::flow::{self, EpisodeProgress};
use game::sly3::safehouse::{self, Action, Pressed};
use game::sly3::{Sly3Session, checkpoint_classes};
use harness::golden::{Game, require_disc};
use render::NullBackend;

type Session = Sly3Session<NullBackend, std::fs::File>;

fn session(path: &std::path::Path) -> Session {
    let disc = formats_sly3::Disc::open_path(path).map_err(|e| e.to_string());
    let mut s = Sly3Session::new(NullBackend::new(320, 180), disc, 7);
    s.follow = false;
    s
}

fn frames(s: &mut Session, n: usize) {
    for _ in 0..n {
        s.update(&FrameInput::default(), 1.0 / 60.0);
    }
}

/// Where the current player should stand after arriving at `point`: the point, or the marker option 848 gives it.
fn expected_place(s: &Session, point: i32) -> Option<[f32; 3]> {
    let w = &s.game.as_ref()?.world;
    let classes = checkpoint_classes(&s.disc().unwrap().classes);
    let a = flow::find_arrival(w, point, &classes)?;
    let me = w.player.and_then(|h| w.sly3_id(h));
    let marker = a.place.iter().find(|p| Some(p.0) == me).and_then(|p| w.find_sly3_id(p.1, None));
    let h = marker.unwrap_or(a.handle);
    w.world_transform(h).map(|t| t.t)
}

fn player_at(s: &Session, p: [f32; 3]) -> bool {
    let w = &s.game.as_ref().unwrap().world;
    let Some(q) = w.player.and_then(|h| w.position(h)) else { return false };
    (q[0] - p[0]).abs() < 1.0 && (q[1] - p[1]).abs() < 1.0 && (q[2] - p[2]).abs() < 60.0
}

#[test]
fn worlds_hold_the_spec_exits_and_points() {
    let Some(path) = require_disc(Game::Sly3, "worlds_hold_the_spec_exits_and_points") else { return };
    let mut s = session(&path);
    // level-flow.md 8: slot → (exits, points, warps, 424, points with 843).
    let table: BTreeMap<usize, (usize, usize, usize, bool, usize)> = [
        (2, (2, 5, 0, true, 1)),
        (3, (12, 35, 16, true, 1)),
        (4, (12, 2, 0, false, 1)),
        (5, (12, 6, 6, false, 1)),
        (6, (12, 7, 15, false, 0)),
        (7, (12, 3, 2, false, 0)),
        (8, (17, 30, 12, true, 1)),
        (9, (17, 6, 2, true, 1)),
        (10, (17, 3, 0, false, 0)),
        (11, (17, 3, 5, false, 1)),
        (12, (17, 3, 3, false, 1)),
        (13, (17, 2, 0, false, 0)),
        (14, (17, 4, 2, false, 0)),
        (15, (13, 32, 14, true, 1)),
        (16, (13, 6, 2, false, 0)),
        (17, (13, 1, 0, false, 0)),
        (18, (13, 2, 0, false, 0)),
        (19, (13, 2, 0, false, 0)),
        (20, (13, 4, 6, false, 1)),
        (21, (13, 3, 0, true, 1)),
        (22, (13, 1, 0, false, 0)),
        (23, (15, 33, 34, true, 1)),
        (24, (15, 13, 4, false, 0)),
        (25, (15, 1, 1, false, 0)),
        (26, (15, 2, 3, false, 1)),
        (27, (15, 1, 2, false, 0)),
        (28, (15, 2, 2, false, 0)),
        (29, (15, 4, 0, false, 0)),
        (30, (15, 5, 3, false, 0)),
        (31, (12, 27, 23, true, 1)),
        (32, (12, 15, 4, true, 1)),
        (33, (12, 4, 3, false, 0)),
        (34, (12, 2, 1, false, 0)),
        (35, (10, 20, 14, true, 1)),
        (36, (10, 2, 0, false, 0)),
        (37, (10, 1, 10, false, 0)),
        (38, (10, 11, 10, false, 0)),
        (39, (10, 2, 4, false, 1)),
    ]
    .into_iter()
    .collect();
    let mut sources = flow::ExitSources::default();
    let mut points_by_slot: BTreeMap<usize, BTreeSet<i32>> = BTreeMap::new();
    let mut destinations: BTreeMap<i32, Option<usize>> = BTreeMap::new();
    let mut disc = formats_sly3::Disc::open_path(&path).unwrap();
    let mut got = BTreeMap::new();
    let mut wipes: BTreeMap<i32, i32> = BTreeMap::new();
    let mut named = BTreeSet::new();
    for (slot, name) in s.worlds() {
        s.load_slot(slot).unwrap_or_else(|e| panic!("{name}: {e}"));
        let g = s.game.as_ref().unwrap();
        let w = &g.world;
        let exits = &s.flow.as_ref().unwrap().exits;
        // The points and warps the world file holds (conditional records included: the load leaves out those whose
        // conditions fail in the fresh context, such as p_at_sea's point 424).
        let bytes = disc.read_world(slot).unwrap();
        let fw = formats_sly3::World::parse_with(&bytes, &disc.options, disc.tangent_multipliers).unwrap();
        let (mut points, mut warps, mut alias) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        let mut stack: Vec<&formats_sly3::stream::Record> = fw.bodies.iter().flat_map(|b| b.objects.iter()).collect();
        while let Some(r) = stack.pop() {
            stack.extend(r.nested());
            match r.class {
                flow::ARRIVAL_CLASS => {
                    points.insert(r.id);
                    if r.options().is_some_and(|o| o.options.iter().any(|x| x.id == flow::opt::ALIAS_424 as i16)) {
                        alias.insert(r.id);
                    }
                }
                flow::WARP_CLASS => {
                    warps.insert(r.id);
                }
                _ => {}
            }
        }
        let row = (exits.len(), points.len(), warps.len(), points.contains(&424), alias.len());
        got.insert(slot, row);
        for (e, _) in exits {
            destinations.insert(e.oid, e.slot);
            assert!(e.slot.is_some(), "{name}: exit {} leads to world {:#x}", e.oid, e.destination);
            // level-flow.md 4.1 says no exit has options; on the disc many carry 834 (a point) and 835 (a wipe).
            if e.point != 424 {
                named.insert(e.oid);
            }
            wipes.insert(e.oid, e.wipe);
        }
        points_by_slot.insert(slot, points);
        let src = flow::exit_sources(w);
        sources.jobs.extend(src.jobs);
        sources.tasks.extend(src.tasks);
        sources.commands.extend(src.commands);
        // Exit 2417 in v_ext (8): to v_ext (world 0xEF) with movie 0x133.
        if name == "v_ext" {
            let e = exits.iter().find(|e| e.0.oid == 2417).unwrap();
            assert_eq!((e.0.destination, e.0.slot, e.0.movies.as_slice()), (0xEF, Some(3), &[0x133u32][..]));
        }
    }
    // Exits, object 424 and option 843 match the spec's table. The file holds a few more points and warps than
    // the table in 9 worlds (by distinct object ID over every record, fresh placements' data included); those are
    // printed and reported, and must not be fewer.
    for (slot, want) in &table {
        let g = got[slot];
        assert_eq!((g.0, g.3, g.4), (want.0, want.3, want.4), "slot {slot}");
        assert!(g.1 >= want.1 && g.2 >= want.2, "slot {slot}: {g:?} {want:?}");
        if (g.1, g.2) != (want.1, want.2) {
            eprintln!("slot {slot}: points and warps {:?}, the spec {:?}", (g.1, g.2), (want.1, want.2));
        }
    }
    assert_eq!(got.get(&0).map(|r| (r.0, r.1, r.2)), Some((0, 0, 0)), "dvd_menu has no exits or points");
    // 81 distinct exits; jobs (1381) and tasks (1406) that fire them; command 0x68's pairs, each point a class-40
    // object of the destination.
    assert_eq!(destinations.len(), 81, "level-flow.md 4.1 (corrected)");
    let mut by_wipe: BTreeMap<i32, usize> = BTreeMap::new();
    for w in wipes.values() {
        *by_wipe.entry(*w).or_default() += 1;
    }
    eprintln!("exits by wipe object (option 835): {by_wipe:?}; exits naming a point (834): {named:?}");
    assert_eq!((by_wipe.get(&-1), named.len()), (Some(&7), 12), "level-flow.md 4.2 (corrected)");
    let jobs: BTreeSet<i32> = sources.jobs.iter().map(|j| j.0).collect();
    let tasks: BTreeSet<i32> = sources.tasks.iter().map(|t| t.0).collect();
    assert_eq!(jobs.len(), 25, "{jobs:?}");
    assert_eq!(tasks, (1825..=1840).collect());
    let pairs: BTreeSet<(i32, i32)> = sources.commands.iter().map(|c| (c.1, c.2)).collect();
    eprintln!(
        "{} jobs, {} tasks, {} command uses, {} distinct pairs",
        jobs.len(),
        tasks.len(),
        sources.commands.len(),
        pairs.len()
    );
    assert_eq!(pairs.len(), 63);
    for (exit, point) in &pairs {
        let dest = destinations[exit].unwrap();
        assert!(*point == 424 || points_by_slot[&dest].contains(point), "exit {exit} point {point} in slot {dest}");
    }
}

#[test]
fn each_episode_starts_and_an_exit_arrives_at_its_point() {
    let Some(path) = require_disc(Game::Sly3, "each_episode_starts_and_an_exit_arrives_at_its_point") else { return };
    let mut s = session(&path);
    let p = EpisodeProgress { china_recon_complete: true, opening_played: true, ..Default::default() };
    // level-flow.md 2 and 3: the first worlds by episode, with the exceptions.
    for (episode, slot) in [(1, 0), (2, 2), (3, 3), (4, 8), (5, 15), (6, 23), (7, 31), (8, 35)] {
        s.start_episode(episode, &p).unwrap();
        assert_eq!(s.current().unwrap().slot, slot, "episode {episode}");
        // Point 424, or the started task's checkpoint (episode 8: task 1841).
        let point = s.arrival_id;
        assert!(point == 424 || s.started_task.is_some(), "episode {episode}");
        if let Some(want) = expected_place(&s, point) {
            assert!(player_at(&s, want), "episode {episode}: the player at point {point}");
        }
        // Take an exit: the first command-0x68 pair of this world whose point is in another world.
        let w = &s.game.as_ref().unwrap().world;
        let src = flow::exit_sources(w);
        let exits = &s.flow.as_ref().unwrap().exits;
        let Some((exit, point, dest)) = src.commands.iter().find_map(|&(_, e, pt)| {
            let d = exits.iter().find(|x| x.0.oid == e)?.0.slot?;
            (d != slot && d != 0 && pt != 424).then_some((e, pt, d))
        }) else {
            assert!(episode <= 2, "episode {episode} has an exit with a point");
            continue;
        };
        frames(&mut s, 2);
        assert!(s.fire_exit(exit, point), "episode {episode}: exit {exit} armed");
        frames(&mut s, 1);
        assert_eq!(s.current().unwrap().slot, dest, "episode {episode}: exit {exit}");
        assert_eq!(s.last_load.as_ref().map(|l| (l.point, l.exit)), Some((point, Some(exit))));
        let want = expected_place(&s, point).expect("the destination holds the point");
        assert!(player_at(&s, want), "episode {episode}: exit {exit} arrives at point {point}");
        eprintln!("episode {episode}: slot {slot}, exit {exit} to slot {dest} at point {point}");
    }
    // The exceptions: China before its recon, the pirates between the treasure and the heist, the prologue.
    s.start_episode(6, &EpisodeProgress::default()).unwrap();
    assert_eq!(s.current().unwrap().slot, flow::SLOT_C_INTRO);
    s.start_episode(7, &EpisodeProgress { treasure_complete: true, ..Default::default() }).unwrap();
    assert_eq!(s.current().unwrap().slot, flow::SLOT_P_AT_SEA);
    s.start_episode(1, &EpisodeProgress::default()).unwrap();
    assert_eq!((s.current().unwrap().slot, s.started_task), (flow::SLOT_M_EXT, Some(flow::PROLOGUE_TASK)));
}

#[test]
fn restart_reloads_at_the_checkpoint_with_wipe_875() {
    let Some(path) = require_disc(Game::Sly3, "restart_reloads_at_the_checkpoint_with_wipe_875") else { return };
    let mut s = session(&path);
    s.start_episode(3, &EpisodeProgress::default()).unwrap();
    let want = expected_place(&s, 424).unwrap();
    let input = FrameInput { left: [0.0, 1.0], ..Default::default() };
    for _ in 0..60 {
        s.update(&input, 1.0 / 60.0);
    }
    assert!(!player_at(&s, want), "the player walked off");
    let mut pairs = Vec::new();
    for _ in 0..3 {
        s.restart();
        frames(&mut s, 1);
        let l = s.last_load.clone().unwrap();
        assert_eq!((l.slot, l.point, l.wipe), (3, 424, flow::RESTART_WIPE));
        pairs.push(l.wipe_pair.unwrap());
        assert!(player_at(&s, want), "back at the checkpoint");
    }
    pairs.sort_unstable();
    assert_eq!(pairs, flow::WIPE_PAIRS, "three restarts use the three pairs");
}

#[test]
fn the_safe_house_and_the_shop() {
    let Some(path) = require_disc(Game::Sly3, "the_safe_house_and_the_shop") else { return };
    let mut s = session(&path);
    for name in ["v_ext", "o_ext", "h_ext", "c_ext", "p_ext", "p_at_sea", "m_ext", "v_apt"] {
        s.load_slot(s.find_world(name).unwrap()).unwrap();
        // p_at_sea's safe house is placed inside `p_boat_hero` by a mode-1 placement with world filter 32 and
        // condition 17, which the fresh load context does not meet.
        let has = ["v_ext", "o_ext", "h_ext", "c_ext", "p_ext"].contains(&name);
        assert_eq!(s.safe_house.is_some(), has, "{name}");
        let Some(h) = &s.safe_house else { continue };
        // safehouse-and-shop.md 5 gives (25, 27, 24); the stored list has a fourth entry, Murray (26).
        assert_eq!(h.states, [25, 27, 24, 26], "{name}");
        let shop = Action { state: 27, buttons: 0x40, target: safehouse::SHOP_SCREEN };
        assert!(h.actions.contains(&shop), "{name}");
        let take =
            [(24, 44600), (25, 44602), (26, 44598)].map(|(state, target)| Action { state, buttons: 0x40, target });
        let all = take.iter().all(|a| h.actions.contains(a));
        assert!(all, "{name}: the placement's actions");
    }
    // In v_ext: the first state is Bentley (Cross would take him out); right reaches the computer and the shop.
    s.start_episode(3, &EpisodeProgress::default()).unwrap();
    assert!(s.enter_safe_house());
    let h = s.safe_house.as_ref().unwrap();
    assert_eq!(h.press(0x40), Some(Pressed::TakeOut { member: engine::gang::sly3::Member::Bentley, script: 44602 }));
    let press = |b: platform_api::Buttons| FrameInput { pressed: b.0, buttons: b.0, ..Default::default() };
    s.update(&press(platform_api::Buttons::RIGHT), 1.0 / 60.0);
    assert_eq!(s.safe_house.as_ref().unwrap().state(), Some(27));
    s.update(&press(platform_api::Buttons::B), 1.0 / 60.0);
    let shop = s.shop.clone().expect("the shop opens");
    let items: Vec<usize> = shop.items.iter().map(|i| i.0).collect();
    assert_eq!(items, [8, 9, 25, 26, 31, 47], "Venice's items");
    assert_eq!(shop.items.iter().map(|i| i.1.price).sum::<i32>(), 1_700);
    // Every episode's shop: 34 items, 23,700 coins in all.
    let table = engine::gang::sly3::Sly3ItemTable::from_executable(&s.disc().unwrap().executable).unwrap();
    let all = safehouse::Shop::new(&table, 8, &[]);
    assert_eq!((all.items.len(), all.items.iter().map(|i| i.1.price).sum::<i32>()), (34, 23_700));
    // Right to Sly; Cross takes him out.
    s.update(&press(platform_api::Buttons::RIGHT), 1.0 / 60.0);
    s.update(&press(platform_api::Buttons::B), 1.0 / 60.0);
    assert!(!s.in_safe_house);
    frames(&mut s, 1);
    let w = &s.game.as_ref().unwrap().world;
    assert_eq!(w.player.and_then(|h| w.class(h)), Some(52), "Sly is played");
}

#[test]
fn every_world_carries_the_same_save_tree() {
    let Some(path) = require_disc(Game::Sly3, "every_world_carries_the_same_save_tree") else { return };
    let mut s = session(&path);
    let mut first: Option<save::sly2::VarTree> = None;
    for (slot, name) in s.worlds() {
        s.load_slot(slot).unwrap();
        let t = game::sly3::save_tree(&s.game.as_ref().unwrap().world).unwrap_or_else(|| panic!("{name}"));
        // saving.md 1: 6,559 records, used size 21,764, checksum 2,978,085; the same section in all 39 worlds.
        assert_eq!(
            (t.records().len(), t.layout_word(), t.checksum()),
            (6_559, save::sly3::USED_SIZE, save::sly3::CHECKSUM)
        );
        match &first {
            Some(f) => assert_eq!(f, &t, "{name}"),
            None => first = Some(t),
        }
    }
    // saving.md 2: where values live.
    let t = std::sync::Arc::new(first.unwrap());
    let off = |p: &[&str]| t.var_named(p, None).map(|v| v.offset);
    assert_eq!(off(&["loot", "ccoin"]), Some(172));
    assert_eq!(off(&["oidCheckpointForce"]), Some(12));
    assert_eq!(off(&["widCheckpointForce"]), Some(16));
    let mut p = save::sly3::progress(t.clone());
    assert_eq!(p.block.len(), save::sly3::BLOCK_SIZE);
    p.set_named(&["loot", "ccoin"], None, save::sly2::SaveValue::Int(1_700));
    assert_eq!(save::sly3::coins(&p), 1_700);
}
