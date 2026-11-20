//! The object model and game loop on every world of the player's own Sly 1 disc (`SLYNX_DISC_SLY1`); skipped
//! when it is not set. Checks the spec vectors the object model can reproduce: transforms.md 8, the pickup and
//! interactable counts of pickups-and-progress.md 8 and interactables.md 10 (proxy clones counted), and runs every
//! world's scripts and loop.

use std::sync::OnceLock;

use engine::game::progress::Progress;
use engine::game::systems::{FrameInput, Systems};
use engine::game::{Game, GameConfig};
use engine::world::{World, WorldInfo};
use formats::disc::Disc;
use formats::stream::PlacementOptions;
use harness::golden::{Game as Title, require_disc};
use script::OptionMap;

struct DiscData {
    worlds: Vec<(WorldInfo, String, Vec<u8>)>,
    map: OptionMap,
}

fn disc(test: &str) -> Option<&'static DiscData> {
    static DATA: OnceLock<DiscData> = OnceLock::new();
    let path = require_disc(Title::Sly1, test)?;
    Some(DATA.get_or_init(|| {
        let mut d = Disc::open_path(&path).expect("open disc");
        let map = OptionMap::from_executable(&d.executable).expect("option map");
        let entries = d.worlds.clone();
        let worlds = entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let info = WorldInfo { index: i as u32, id: e.id, task_mask: e.task_mask, identity: e.file.identity };
                (info, e.name.clone(), d.read_world(i).expect("read world"))
            })
            .collect();
        DiscData { worlds, map }
    }))
}

fn load(d: &DiscData, i: usize) -> World {
    let (info, _, bytes) = &d.worlds[i];
    World::load(bytes.clone(), *info).expect("world loads")
}

fn count_class(w: &World, class: i16) -> usize {
    w.slots_of_class(class).len()
}

#[test]
fn transform_vectors() {
    let Some(d) = disc("transform_vectors") else { return };
    let near = |w: &World, oid: i16, want: [f32; 3]| {
        w.slots_with_oid(oid).iter().any(|&s| {
            let p = w.position(w.handle(s).unwrap()).unwrap();
            p.iter().zip(want).all(|(a, b)| (a - b).abs() < 0.01)
        })
    };
    let paris = load(d, 1);
    assert!(near(&paris, 1392, [1380.658, 176.828, -2388.437]));
    assert!(near(&paris, 1637, [1380.658, 176.828, -2408.577]));
    assert!(near(&paris, 1640, [1380.658, 176.828, -2423.166]));
    let mugg = load(d, 13);
    assert!(near(&mugg, 1399, [-1886.785, -2937.677, 1050.544]));
    assert!(near(&mugg, 1401, [-1973.572, -2915.919, 445.694]));
    assert!(near(&mugg, 1402, [-1968.164, -2901.048, 489.992]));
}

#[test]
fn every_world_loads_with_the_spec_counts() {
    let Some(d) = disc("every_world_loads_with_the_spec_counts") else { return };
    // Disc totals, proxy clones counted (pickups-and-progress.md 8, interactables.md 10, level-flow.md 4). Clue
    // bottles and checkpoints differ from the spec tables (595 against 545, 51 against 30) while matching the
    // stream's placements; reported as a spec question.
    let classes: [(i16, usize); 14] = [
        (46, 595),
        (86, 43),
        (87, 55),
        (88, 950),
        (89, 28),
        (90, 22),
        (40, 232),
        (41, 461),
        (42, 87),
        (123, 98),
        (33, 1728),
        (39, 1609),
        (125, 114),
        (56, 51),
    ];
    let mut totals = [0usize; 14];
    for i in 0..d.worlds.len() {
        let w = load(d, i);
        let name = &d.worlds[i].1;
        for (k, &(c, _)) in classes.iter().enumerate() {
            totals[k] += count_class(&w, c);
            let placed = w
                .slots_of_class(c)
                .iter()
                .filter(|&&s| w.flags(w.handle(s).unwrap()) & engine::object::flags::UNPLACED == 0)
                .count();
            let scene = w.level.stream.placements_where(PlacementOptions::default(), &|r| r.class == c).len();
            assert_eq!(placed, scene, "{name}: class {c}: objects against the stream's placements");
        }
        // One static world mesh, every stock template kept, and one drawable per placement of the scene.
        assert_eq!(count_class(&w, 4), 1, "{name}");
        assert_eq!((12..=42).filter(|&o| w.template_record(o).is_some()).count(), 31, "{name}");
        // Drawables whose position is the "not placed" sentinel are kept (invisible); the scene leaves them out.
        let placements = w.level.stream.placements(PlacementOptions::default()).len();
        let placed = w.drawables().iter().filter(|x| w.flags(x.object) & engine::object::flags::UNPLACED == 0).count();
        assert_eq!(placed, placements, "{name}: drawables");
    }
    let got: Vec<(i16, usize)> = classes.iter().zip(totals).map(|(&(c, _), t)| (c, t)).collect();
    assert_eq!(got, classes);
}

#[test]
fn paris_counts_and_scripts() {
    let Some(d) = disc("paris_counts_and_scripts") else { return };
    let w = load(d, 1);
    assert_eq!((count_class(&w, 88), count_class(&w, 86), count_class(&w, 87), count_class(&w, 23)), (16, 1, 1, 1));
    // pickups-and-progress.md 4.2: Paris's vault opens with 937 (the placement's option 1046).
    let vault = w.handle(w.slots_of_class(23)[0]).expect("vault");
    let combo = w.options(vault).and_then(|o| o.i32(engine::behavior::vault::opt::COMBINATION));
    assert_eq!(combo.map(engine::behavior::vault::Vault::combination), Some([9, 3, 7]));
    let mut g = Game::new(
        w,
        Progress::new_game(),
        Systems::default(),
        GameConfig { seed: 1, option_map: Some(d.map.clone()), ..Default::default() },
    );
    let s = g.scripts().expect("Paris has scripts");
    assert!(s.program().module_count() > 0);
    for _ in 0..120 {
        g.step_fixed(&FrameInput::default());
    }
    assert!((g.clock.game_time - 2.0).abs() < 1e-3);
}

#[test]
fn every_world_runs_and_is_deterministic() {
    let Some(d) = disc("every_world_runs_and_is_deterministic") else { return };
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let unhandled = std::sync::Mutex::new(std::collections::BTreeMap::<u16, u32>::new());
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= d.worlds.len() {
                        break;
                    }
                    let run = || {
                        let w = load(d, i);
                        let cfg = GameConfig { seed: 7, option_map: Some(d.map.clone()), ..Default::default() };
                        let mut g = Game::new(w, Progress::new_game(), Systems::default(), cfg);
                        let input = FrameInput { left: [0.0, 1.0], ..Default::default() };
                        let mut hashes = Vec::new();
                        for _ in 0..90 {
                            g.step_fixed(&input);
                            hashes.push(g.state_hash());
                        }
                        // No host errors: options read and write as their kinds say (object references read as
                        // objects or the null object, mass takes integers), and actions no behaviour implements
                        // are counted instead (`World::unhandled_actions`); every override module gets its outer
                        // module.
                        for (k, n) in &g.script_error_kinds {
                            assert!(!matches!(k, script::ScriptError::Host(_)), "world {i}: {n} x {k:?}");
                        }
                        for (a, n) in &g.world.unhandled_actions {
                            unhandled.lock().unwrap().entry(*a).and_modify(|x: &mut u32| *x += n).or_insert(*n);
                        }
                        assert_eq!(g.outer_refused, 0, "world {i}: outer modules refused");
                        let mut kinds = g.script_error_kinds.clone();
                        kinds.sort_by_key(|k| std::cmp::Reverse(k.1));
                        kinds.truncate(4);
                        let errors = (g.scripts().map_or(0, |s| s.error_count()), kinds);
                        (hashes, errors)
                    };
                    let (a, errors) = run();
                    let (b, _) = run();
                    assert_eq!(a, b, "world {i} ({}) is not deterministic", d.worlds[i].1);
                    eprintln!("world {i:2} {:<28} script errors after 90 frames: {errors:?}", d.worlds[i].1);
                }
            });
        }
    });
    // Actions the scripts call that no behaviour implements yet (two runs per world).
    eprintln!("unhandled actions (calls): {:?}", unhandled.lock().unwrap());
}

#[test]
fn collision_surfaces_are_polygons() {
    // specs/sly1/collision.md 1.2: world 1's mesh, surface 1275 (corners 1191, 1190, 1187) is a pentagon whose edges
    // end at 1183 and 1184; about a quarter of the surfaces in worlds 1, 2 and 12 are larger than their triangle.
    let Some(d) = disc("collision_surfaces_are_polygons") else { return };
    let w = load(d, 1);
    let level = &w.level;
    let (mut found, mut surfaces, mut larger) = (false, 0usize, 0usize);
    for r in &level.stream.records {
        if !matches!(
            formats::stream::layout_of(r.class),
            Some(formats::stream::Layout::Solid | formats::stream::Layout::Conveyor)
        ) {
            continue;
        }
        let Ok(solid) = formats::collision::Solid::parse(&level.bytes, r.offset + 6) else { continue };
        let m = &solid.mesh;
        let ends = m.edge_ends();
        for t in 0..m.triangles.len() {
            let poly = m.surface_polygon(t, &ends);
            surfaces += 1;
            larger += usize::from(poly.len() > 3);
            if t == 1275 && m.triangles[t].corners == [1191, 1190, 1187] {
                // collision.md 1.2: the pentagon of 1183, 1184, 1187, 1190 and 1191 (edges stored under 1275, 1277
                // and 1278).
                let mut p = poly.clone();
                p.sort_unstable();
                assert_eq!(p, [1183, 1184, 1187, 1190, 1191], "the pentagon");
                found = true;
            }
        }
    }
    eprintln!("world 1: {larger} of {surfaces} surfaces are larger than their triangle");
    assert!(found, "surface 1275");
    assert!(larger * 5 > surfaces, "{larger} of {surfaces}");
}
