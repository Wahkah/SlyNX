//! Sly 3 world behaviours on the player's own Sly 3 (USA) disc (`SLYNX_DISC_SLY3`; skipped when it is not set):
//! the per-class record counts of the specs, the behaviours attached by `behavior::sly3`, the guards' options
//! carried over, and a deterministic 90-frame run with behaviours on all 39 worlds.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use engine::behavior::Behavior;
use engine::behavior::sly3::class;
use engine::game::progress::Progress;
use engine::game::systems::{FrameInput, Systems};
use engine::game::{Game, GameConfig};
use engine::world::sly3::Sly3Options;
use engine::world::{World, WorldInfo};
use formats_sly3::Disc;
use formats_sly3::stream::visit;

fn disc_path() -> Option<String> {
    std::env::var("SLYNX_DISC_SLY3").ok().filter(|p| !p.is_empty())
}

macro_rules! need_disc {
    () => {
        if disc_path().is_none() {
            eprintln!("skipped: SLYNX_DISC_SLY3 is not set");
            return;
        }
    };
}

struct Data {
    options: formats_sly2::exe::OptionTable,
    multipliers: [f32; 3],
    /// (slot, world id, bytes)
    worlds: Vec<(usize, u32, Vec<u8>)>,
}

fn data() -> &'static Data {
    static D: OnceLock<Data> = OnceLock::new();
    D.get_or_init(|| {
        let mut d = Disc::open_path(disc_path().unwrap()).unwrap();
        let slots: Vec<usize> = d.world_slots().collect();
        let worlds = slots
            .into_iter()
            .map(|s| {
                let id = d.worlds[s].world_id();
                (s, id, d.read_world(s).unwrap())
            })
            .collect();
        Data { options: d.options.clone(), multipliers: d.tangent_multipliers, worlds }
    })
}

fn load(i: usize) -> World {
    let d = data();
    let (slot, id, bytes) = &d.worlds[i];
    let info = WorldInfo { index: *slot as u32, id: *id, task_mask: 0, identity: *id };
    let opts = Sly3Options { slot: Some(*slot as i16), info, multipliers: d.multipliers, ..Sly3Options::default() };
    World::load_sly3(bytes.clone(), &d.options, opts).unwrap()
}

#[test]
fn class_counts_match_the_specs() {
    need_disc!();
    let d = data();
    // Records of every mode at every depth, over all 39 worlds (guards.md 1 and 5, alarms-sensors.md 1,
    // interactables.md 1, pickpocket-and-pickups.md 2).
    let mut count: BTreeMap<i16, usize> = BTreeMap::new();
    let mut worlds_with: BTreeMap<i16, usize> = BTreeMap::new();
    for (_, _, bytes) in &d.worlds {
        let w = formats_sly3::World::parse_with(bytes, &d.options, d.multipliers).unwrap();
        let mut here = std::collections::BTreeSet::new();
        for b in &w.bodies {
            visit(&b.objects, &mut |r| {
                *count.entry(r.class).or_default() += 1;
                here.insert(r.class);
            });
        }
        for c in here {
            *worlds_with.entry(c).or_default() += 1;
        }
    }
    let c = |k: i16| count.get(&k).copied().unwrap_or(0);
    let w = |k: i16| worlds_with.get(&k).copied().unwrap_or(0);
    // (class, records, worlds)
    for (k, n, worlds) in [
        (class::GUARD, 65, 28),
        (class::LASER, 84, 10),
        (class::DETECTOR, 5, 0),
        // alarms-sensors.md says 14 worlds; the records occur in 15 (spec question).
        (class::CONTACT, 24, 15),
        (class::BOUNCER, 19, 11),
        (class::PROP, 375 + 40, 36),
        (97, 153 + 295, 38),
        (101, 36 + 3, 17),
        (class::TRIGGER, 348 + 184, 34),
        (33, 48 + 38, 26),
        (34, 142, 34),
        (35, 3 + 6, 5),
        (25, 16 + 87, 18),
        (37, 42 + 104, 22),
        (38, 3 + 39, 20),
        (class::COIN, 39, 39),
        (class::TASK_ITEM, 4, 0),
        (class::MEDICINE, 39, 39),
        (class::LOOT, 27, 27),
        (112, 27, 27),
        (116, 39, 39),
        (121, 4, 4),
        (125, 36, 0),
        (127, 12, 6),
        (153, 6, 6),
    ] {
        assert_eq!(c(k), n, "class {k} records");
        if worlds > 0 {
            assert_eq!(w(k), worlds, "class {k} worlds");
        }
    }
    assert_eq!(c(22), 0, "class 22 does not occur");
}

#[test]
fn behaviours_attach_with_their_options() {
    need_disc!();
    let d = data();
    let mut attached: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut hit_points: BTreeMap<i32, usize> = BTreeMap::new();
    for i in 0..d.worlds.len() {
        let w = load(i);
        let g = Game::new(w, Progress::new_game(), Systems::null(), GameConfig { seed: 7, ..Default::default() });
        for h in g.world.handles().collect::<Vec<_>>() {
            let name = match g.world.behavior(h) {
                Some(Behavior::Sly2Guard(_)) => "guard",
                Some(Behavior::Sly2Laser(_)) => "laser",
                Some(Behavior::Sly2Detector(_)) => "detector",
                Some(Behavior::Sly2Pad(_)) => "pad",
                Some(Behavior::Sly2Prop(_)) => "prop",
                Some(Behavior::Sly2Volume(_)) => "volume",
                Some(Behavior::Sly2Pickup(_)) => "pickup",
                _ => continue,
            };
            *attached.entry(name).or_default() += 1;
            if name == "guard"
                && let Some(hp) = g.world.options(h).and_then(|o| o.f32(254))
            {
                *hit_points.entry(hp as i32).or_default() += 1;
            }
        }
    }
    eprintln!("attached: {attached:?}; guard hit points (option 937 as 254): {hit_points:?}");
    for k in ["guard", "laser", "pad", "prop", "volume", "pickup"] {
        assert!(attached.get(k).copied().unwrap_or(0) > 0, "{k}");
    }
    // Option 937 (hit points) reached the guards: 40, 70 and 100 occur.
    assert!(hit_points.contains_key(&40) && hit_points.contains_key(&100), "{hit_points:?}");
}

#[test]
fn every_world_runs_90_frames_deterministically_with_behaviours() {
    need_disc!();
    let d = data();
    let input = FrameInput { left: [0.0, 1.0], ..Default::default() };
    for i in 0..d.worlds.len() {
        let run = || {
            let w = load(i);
            let mut g =
                Game::new(w, Progress::new_game(), Systems::null(), GameConfig { seed: 7, ..Default::default() });
            let mut hashes = Vec::new();
            for _ in 0..90 {
                g.step_fixed(&input);
                hashes.push(g.state_hash());
            }
            hashes
        };
        let a = run();
        let b = run();
        assert_eq!(a, b, "world {i} is deterministic");
    }
}

#[test]
fn combo_definitions_and_interactables() {
    use engine::behavior::sly3::interactables::{Barrier, TreasureMap, UsePoint};
    use engine::gang::button;
    use engine::gang::combo::ComboSet;
    need_disc!();
    let d = data();
    let (mut defs, mut sly_worlds, mut custom) = (0, 0, BTreeMap::<i16, usize>::new());
    let (mut barrier_lists, mut guard_barriers, mut radii, mut maps) =
        (0, 0, BTreeMap::<i32, usize>::new(), Vec::new());
    let mut volume_lists = [0usize; 3];
    let mut safes = Vec::new();
    for i in 0..d.worlds.len() {
        let w = load(i);
        // characters.md 5: Sly's combo definitions; every set has a Triangle definition with a clip.
        let set = ComboSet::for_class(&w, 52);
        if !set.is_empty() {
            sly_worlds += 1;
            defs += set.defs.len();
            let t = set.choose(button::TRIANGLE, |_| true).expect("a Triangle definition");
            assert!(set.defs[t].clip(false).is_some());
        }
        for h in w.handles().collect::<Vec<_>>() {
            match w.class(h) {
                Some(97) => {
                    let b = Barrier::new(&w, h);
                    barrier_lists += usize::from(!b.classes.is_empty() || !b.objects.is_empty());
                    guard_barriers += usize::from(b.classes.contains(&58));
                }
                Some(33) => *radii.entry(UsePoint::new(&w, h).radius as i32).or_default() += 1,
                Some(153) => maps.push(TreasureMap::new(&w, h)),
                Some(80) => safes.push(engine::behavior::sly3::devices::Safe::new(&w, h)),
                _ => {}
            }
        }
        let g = Game::new(w, Progress::new_game(), Systems::null(), GameConfig { seed: 7, ..Default::default() });
        for h in g.world.handles().collect::<Vec<_>>() {
            let c = g.world.class(h).unwrap_or(-1);
            if matches!(g.world.behavior(h), Some(Behavior::Custom(_))) {
                *custom.entry(c).or_default() += 1;
            }
            if c == class::TRIGGER
                && let Some(o) = g.world.options(h)
            {
                for (k, id) in [432u16, 433, 440].into_iter().enumerate() {
                    volume_lists[k] += usize::from(o.has(id));
                }
            }
        }
    }
    eprintln!(
        "combo defs {defs} in {sly_worlds} worlds; custom {custom:?}; barriers with lists {barrier_lists}, against guards \
         {guard_barriers}; use radii {radii:?}; volumes with 432/433/440 {volume_lists:?}"
    );
    assert_eq!(defs, 938, "Sly's combo definitions");
    assert!(sly_worlds >= 30);
    assert!(custom.get(&97).copied().unwrap_or(0) > 400 && custom.get(&33).copied().unwrap_or(0) > 80, "{custom:?}");
    assert!(barrier_lists > 400 && guard_barriers > 100);
    assert!(radii.contains_key(&50) && radii.contains_key(&100) && radii.contains_key(&150));
    assert!(volume_lists.iter().all(|&n| n > 50), "{volume_lists:?}");
    // pickpocket-and-pickups.md 6.1: six safe placements, three tumblers each (found through their placements), the
    // combinations' digits from the last tumbler backwards, all opening as soon as the dials match.
    assert_eq!(safes.len(), 6);
    for s in &safes {
        assert_eq!(s.tumblers.len(), 3);
        assert!(s.tumblers.iter().all(|t| t.0.is_some()), "tumblers found");
        let digits: i32 = s.tumblers.iter().fold(0, |n, t| n * 10 + i32::from(t.1.wanted));
        assert_eq!(digits, s.combination);
        assert!(s.auto);
    }
    assert!(custom.get(&80).copied().unwrap_or(0) == 6 && custom.get(&94).copied().unwrap_or(0) > 10);
    // The answers: six maps, pace 400, radius 5,000 (one 4,000), a marker per trail point, clue and hint strings.
    assert_eq!(maps.len(), 6);
    assert!(maps.iter().all(|m| m.pace == 400.0 && (m.radius == 5000.0 || m.radius == 4000.0)));
    assert!(maps.iter().all(|m| !m.markers.is_empty() && !m.strings.is_empty()));
}
