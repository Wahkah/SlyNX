//! Sly 2 worlds in the object model, on every world of the player's own Sly 2 (USA) disc (`SLYNX_DISC_SLY2`); skipped
//! when it is not set. Every world loads at its level row with per-class object counts and loaded world matrices equal
//! to an independent walk of `formats-sly2`'s records and placements; record, module and identity columns are checked;
//! and every world runs 90 frames deterministically with no behaviours, without and with its scripts attached.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use engine::game::progress::Progress;
use engine::game::systems::{FrameInput, Systems};
use engine::game::{Game, GameConfig};
use engine::world::sly2::Sly2Options;
use engine::world::{World, WorldInfo};
use formats::geometry::Affine;
use formats_sly2::Disc;
use formats_sly2::exe::{Executable, OptionTable};
use formats_sly2::stream::{Body, Record};
use formats_sly2::transforms::TransformRef;
use harness::golden::{Game as Title, require_disc};
use script::{BuiltinTable, OptionMap, Program, ScriptError};

struct DiscData {
    exe: Executable,
    options: OptionTable,
    /// (row, world id, bytes)
    worlds: Vec<(usize, u32, Vec<u8>)>,
}

fn disc(test: &str) -> Option<&'static DiscData> {
    static DATA: OnceLock<DiscData> = OnceLock::new();
    let path = require_disc(Title::Sly2, test)?;
    Some(DATA.get_or_init(|| {
        let mut d = Disc::open_path(&path).expect("open disc");
        let rows: Vec<usize> = d.world_rows().collect();
        let worlds = rows
            .into_iter()
            .map(|row| {
                let id = d.rows[row].world_id();
                (row, id, d.read_world(row).expect("read world"))
            })
            .collect();
        DiscData { exe: d.executable.clone(), options: d.options.clone(), worlds }
    }))
}

fn info(row: usize, id: u32) -> WorldInfo {
    WorldInfo { index: row as u32, id, task_mask: 0, identity: id }
}

fn load(d: &DiscData, i: usize, builtins: Option<BuiltinTable>) -> World {
    let (row, id, bytes) = &d.worlds[i];
    let opts = Sly2Options { row: Some(*row as i16), info: info(*row, *id), builtins, ..Sly2Options::default() };
    World::load_sly2(bytes.clone(), &d.options, opts).expect("world loads")
}

/// Runs `f` on every world index, on a few threads.
fn each_world(d: &DiscData, f: impl Fn(usize) + Sync) {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8);
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= d.worlds.len() {
                        break;
                    }
                    f(i);
                }
            });
        }
    });
}

/// An independent walk of the records the loader should create, in creation order, with each one's class and loaded
/// world matrix (the rules of `engine::world::sly2`, written from object-stream.md 2.3, classes/placement.md and
/// transforms.md).
struct Expect<'w> {
    world: &'w formats_sly2::World<'w>,
    row: Option<i16>,
    objects: Vec<(i16, i32, Affine)>,
    copies: Vec<BTreeSet<i32>>,
    sets: BTreeMap<i32, BTreeSet<i32>>,
}

#[derive(Clone, Copy)]
struct Frame<'e> {
    set: i32,
    copy: usize,
    parent: Affine,
    root: Affine,
    owner: Affine,
    enclosing: &'e [Affine],
    depth: u32,
}

impl Expect<'_> {
    fn copy(&mut self) -> usize {
        self.copies.push(BTreeSet::new());
        self.copies.len() - 1
    }

    fn package(&mut self, pkg: usize, root: Affine, depth: u32) -> usize {
        let copy = self.copy();
        let set = self.world.header.packages[pkg].set_id;
        let f = Frame { set, copy, parent: root, root, owner: root, enclosing: &[], depth };
        let w = self.world;
        for r in &w.bodies[pkg].objects {
            self.record(r, f);
        }
        copy
    }

    fn at(f: &Frame<'_>, reference: i16) -> Affine {
        match TransformRef::decode(reference) {
            TransformRef::Enclosing(n) => f.enclosing.get(n as usize).copied().unwrap_or(f.root),
            TransformRef::Owner => f.owner,
            _ => f.root,
        }
    }

    fn record(&mut self, r: &Record, f: Frame<'_>) {
        let excluded = !r.conditions.is_empty() && self.row.is_some_and(|row| !r.conditions.contains(&row));
        if excluded && !matches!(r.class, 132 | 133) {
            return;
        }
        let skip = match r.kind {
            -1 => self.copies[f.copy].contains(&r.id),
            -2 => self.sets.get(&f.set).is_some_and(|s| s.contains(&r.id)),
            _ => false,
        };
        if skip {
            return;
        }
        self.copies[f.copy].insert(r.id);
        self.sets.entry(f.set).or_default().insert(r.id);
        match &r.body {
            Body::Solid(s) => {
                let own = s.transforms.world_matrices(&f.root, f.enclosing);
                let m = own.first().copied().unwrap_or(f.parent);
                self.objects.push((r.class, r.id, m));
                for c in &s.children {
                    self.record(c, Frame { parent: m, enclosing: &own, ..f });
                }
            }
            Body::Placement(p) => {
                let m = Self::at(&f, p.parent).mul(&p.matrix);
                self.objects.push((r.class, r.id, m));
                let copy = match self.world.package_by_set(p.set_id) {
                    Some(t) if f.depth < 8 => self.package(t, m, f.depth + 1),
                    _ => self.copy(),
                };
                let extra = Frame { copy, parent: m, root: m, owner: m, enclosing: &[], depth: f.depth + 1, ..f };
                for c in &p.extra {
                    self.record(c, extra);
                }
            }
            Body::Other { header, children, .. } => {
                let m = header.map_or(f.parent, |h| Self::at(&f, h.parent).mul(&h.matrix));
                self.objects.push((r.class, r.id, m));
                let one = [m];
                let child = match header {
                    Some(_) => Frame { parent: m, owner: m, enclosing: &one, ..f },
                    None => Frame { parent: m, ..f },
                };
                for c in children {
                    self.record(c, child);
                }
            }
            Body::Unparsed => self.objects.push((r.class, r.id, f.parent)),
        }
    }
}

fn close(a: &Affine, b: &Affine) -> bool {
    let scale = 1.0 + b.t.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let mut d = 0.0f32;
    for k in 0..3 {
        for j in 0..3 {
            d = d.max((a.cols[k][j] - b.cols[k][j]).abs());
        }
        d = d.max((a.t[k] - b.t[k]).abs() / scale);
    }
    d < 2e-3
}

#[test]
fn every_world_loads_with_the_formats_counts() {
    let Some(d) = disc("every_world_loads_with_the_formats_counts") else { return };
    assert_eq!(d.worlds.len(), 43);
    let totals = std::sync::Mutex::new((0usize, 0u32, 0u32));
    each_world(d, |i| {
        let (row, _, bytes) = &d.worlds[i];
        let w = load(d, i, None);
        let fw = formats_sly2::World::parse(bytes, &d.options).unwrap();
        let mut e = Expect {
            world: &fw,
            row: Some(*row as i16),
            objects: Vec::new(),
            copies: Vec::new(),
            sets: BTreeMap::new(),
        };
        for (p, entry) in fw.header.packages.iter().enumerate() {
            if entry.keep != 0 {
                e.package(p, Affine::IDENTITY, 0);
            }
        }
        // Per-class counts.
        let mut want: BTreeMap<i16, usize> = BTreeMap::new();
        for (c, _, _) in &e.objects {
            *want.entry(*c).or_default() += 1;
        }
        let mut got: BTreeMap<i16, usize> = BTreeMap::new();
        for h in w.handles() {
            *got.entry(w.class(h).unwrap()).or_default() += 1;
        }
        assert_eq!(got, want, "row {row}: objects per class");
        for (&c, &n) in &want {
            assert_eq!(w.slots_of_class(c).len(), n, "row {row}: class index {c}");
        }
        // Creation order, IDs and world matrices (after the world's own transform update).
        assert_eq!(w.len(), e.objects.len());
        let l = w.sly2.as_deref().unwrap();
        let mut bad = 0;
        for (s, (class, id, m)) in e.objects.iter().enumerate() {
            let h = w.handle(s as u32).unwrap();
            assert_eq!(w.class(h), Some(*class));
            assert_eq!(w.sly2_id(h), Some(*id));
            if l.stats.singular_parents == 0 && !close(&w.world_transform(h).unwrap(), m) {
                bad += 1;
            }
            // The record column names the record the object was made from.
            let r = w.sly2_record(h).expect("record");
            assert_eq!((r.class, r.id), (*class, *id));
        }
        assert_eq!(bad, 0, "row {row}: world matrices differ from the loaded ones");
        // Identity across reloads: persist keys are unique.
        let keys: BTreeSet<u64> = w.handles().map(|h| w.persist_key(h).unwrap()).collect();
        assert_eq!(keys.len(), w.len(), "row {row}: persist keys");
        // Modules are the scripts' package modules, except the copy roots a placement's patch binds to one of its
        // own package's modules (missions.md 4.3).
        let program = Program::from_sly2_world(&fw).unwrap();
        let mut bound = 0;
        for h in w.handles() {
            let r = w.sly2_record(h).unwrap();
            let pkg = l.record_package(w.record(h).unwrap()).unwrap();
            let want = program.package_module(pkg, r.script).map_or(-1, |m| m as i16);
            if w.module(h) != want {
                // Only a copy root (its parent a placement) is bound; one with a module of its own keeps that as
                // its outer module.
                let root_of_placement = w.parent(h).and_then(|p| w.class(p)).is_some_and(|c| c == 6 || c == 7);
                assert!(root_of_placement, "row {row}: module of a non-root");
                assert!(want < 0 || w.outer_module(h) == Some(want), "row {row}: outer module");
                bound += 1;
            }
        }
        assert_eq!(bound, l.stats.patch_modules, "row {row}: modules bound by patches");
        let mut t = totals.lock().unwrap();
        t.0 += w.len();
        t.1 += l.stats.unresolved_placements;
        t.2 += l.stats.option_errors;
        eprintln!("row {row:2}: {:6} objects, {:?}", w.len(), l.stats);
    });
    let (objects, unresolved, option_errors) = *totals.lock().unwrap();
    eprintln!("all worlds: {objects} objects, {unresolved} unresolved placements");
    assert_eq!(option_errors, 0);
}

#[test]
fn every_world_runs_and_is_deterministic() {
    let Some(d) = disc("every_world_runs_and_is_deterministic") else { return };
    let map = OptionMap::from_sly2_executable(&d.exe).expect("option map");
    let builtins = BuiltinTable::from_executable(&d.exe).expect("built-in table");
    let actions = std::sync::Mutex::new(BTreeMap::<u16, u32>::new());
    let totals = std::sync::Mutex::new((0u64, 0u32));
    each_world(d, |i| {
        let run = |scripts: bool| {
            let w = load(d, i, Some(builtins.clone()));
            let attached = w.handles().filter(|&h| w.module(h) >= 0).count();
            let cfg = GameConfig { seed: 7, option_map: Some(map.clone()), no_scripts: !scripts };
            let mut g = Game::new(w, Progress::new_game(), Systems::null(), cfg);
            assert_eq!(g.scripts().is_some(), scripts);
            let input = FrameInput { left: [0.0, 1.0], ..Default::default() };
            let mut hashes = Vec::with_capacity(90);
            for _ in 0..90 {
                g.step_fixed(&input);
                hashes.push(g.state_hash());
            }
            assert!((g.clock.game_time - 1.5).abs() < 1e-3);
            // The host refuses nothing: no error value is the host's own (`ScriptError::Host`). The errors left come
            // from values the scripts get from actions no behaviour runs yet (they give no value), custom properties
            // nothing has added, and options the class map does not list (see docs/engine.md).
            let host: Vec<_> = g.script_error_kinds.iter().filter(|k| matches!(k.0, ScriptError::Host(_))).collect();
            assert!(host.is_empty(), "row {}: host errors {host:?}", d.worlds[i].0);
            let errors = g.scripts().map_or(0, |s| s.error_count());
            (hashes, attached, errors, g.world.unhandled_actions.clone())
        };
        let row = d.worlds[i].0;
        let (a, ..) = run(false);
        let (b, ..) = run(false);
        assert_eq!(a, b, "row {row}: not deterministic without scripts");
        let (a, attached, errors, unhandled) = run(true);
        let (b, ..) = run(true);
        assert_eq!(a, b, "row {row}: not deterministic with scripts");
        let mut all = actions.lock().unwrap();
        for (k, n) in &unhandled {
            *all.entry(*k).or_default() += n;
        }
        let mut t = totals.lock().unwrap();
        t.0 += errors;
        t.1 += unhandled.values().sum::<u32>();
        eprintln!("row {row:2}: {attached} objects attached to modules, {errors} script errors after 90 frames");
    });
    let mut a: Vec<(u16, u32)> = actions.into_inner().unwrap().into_iter().collect();
    a.sort_by_key(|x| std::cmp::Reverse(x.1));
    let (errors, calls) = totals.into_inner().unwrap();
    eprintln!("all worlds: {errors} script errors; {calls} calls of {} actions with no behaviour: {a:?}", a.len());
}

#[test]
fn level_conditions_use_the_level_table_row() {
    let Some(d) = disc("level_conditions_use_the_level_table_row") else { return };
    // Row 14 (the prison hub, world 0x82): `p_prison_macro`'s hack sequences 2410 and 2411 (class 114) and 2408 list
    // row 14 in their level conditions, so they load there (missions.md 4.1, vehicles-and-minigames.md 10.3).
    let i = d.worlds.iter().position(|w| w.0 == 14).expect("row 14");
    assert_eq!(d.worlds[i].1, 0x82);
    let w = load(d, i, None);
    for id in [2408, 2410, 2411] {
        let h = w.find_sly2_id(id, None);
        eprintln!(
            "object {id}: {h:?}, class {:?}, in world {:?}, module {:?}",
            h.and_then(|h| w.class(h)),
            h.map(|h| w.in_world(h)),
            h.map(|h| w.module(h))
        );
        assert_eq!(h.and_then(|h| w.class(h)), Some(114), "sequence {id} loads in row 14");
    }
    assert!(w.find_sly2_id(2409, None).is_some(), "screen 2409 (rows 14 and 15)");
    let excluded = w.sly2.as_deref().map_or(0, |l| l.stats.excluded_by_row);
    eprintln!("row 14: {excluded} records left out by their conditions");
}
