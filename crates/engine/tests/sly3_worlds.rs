//! Sly 3 worlds in the object model, on every world of the player's own Sly 3 (USA) disc (`SLYNX_DISC_SLY3`);
//! skipped when it is not set. Every world loads at its slot with per-class object counts, IDs and loaded world
//! matrices equal to an independent walk of `formats-sly3`'s records and placements; every world runs 90 frames
//! deterministically, without and with its scripts (specs/sly3/scripts.md), counting the scripts' error values.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use engine::game::progress::Progress;
use engine::game::systems::{FrameInput, Systems};
use engine::game::{Game, GameConfig};
use engine::world::sly3::Sly3Options;
use engine::world::{World, WorldInfo};
use formats::geometry::Affine;
use formats_sly2::exe::OptionTable;
use formats_sly2::transforms::TransformRef;
use formats_sly3::Disc;
use formats_sly3::filter::Context;
use formats_sly3::stream::{Body, Layout, Record};
use harness::golden::{Game as Title, require_disc};

struct DiscData {
    exe: formats_sly3::exe::Executable,
    options: OptionTable,
    multipliers: [f32; 3],
    /// (slot, world id, bytes)
    worlds: Vec<(usize, u32, Vec<u8>)>,
}

fn disc(test: &str) -> Option<&'static DiscData> {
    static DATA: OnceLock<DiscData> = OnceLock::new();
    let path = require_disc(Title::Sly3, test)?;
    Some(DATA.get_or_init(|| {
        let mut d = Disc::open_path(&path).expect("open disc");
        let slots: Vec<usize> = d.world_slots().collect();
        let worlds = slots
            .into_iter()
            .map(|s| {
                let id = d.worlds[s].world_id();
                (s, id, d.read_world(s).expect("read world"))
            })
            .collect();
        DiscData { exe: d.executable.clone(), options: d.options.clone(), multipliers: d.tangent_multipliers, worlds }
    }))
}

fn load(d: &DiscData, i: usize) -> World {
    let (slot, id, bytes) = &d.worlds[i];
    let info = WorldInfo { index: *slot as u32, id: *id, task_mask: 0, identity: *id };
    let opts = Sly3Options { slot: Some(*slot as i16), info, multipliers: d.multipliers, ..Sly3Options::default() };
    World::load_sly3(bytes.clone(), &d.options, opts).expect("world loads")
}

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

/// An independent walk of the records the loader should create, in creation order, with each one's class, ID and
/// loaded world matrix (the rules of `engine::world::sly3`, from world-file.md 6, object-stream.md 2.3,
/// classes/placement.md and transforms.md).
struct Expect<'w> {
    world: &'w formats_sly3::World<'w>,
    ctx: Context,
    objects: Vec<(i16, i32, Affine)>,
    copies: Vec<BTreeSet<i32>>,
    groups: BTreeMap<usize, BTreeSet<i32>>,
}

#[derive(Clone, Copy)]
struct Frame<'e> {
    group: usize,
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

    fn group(&mut self, group: usize, root: Affine, depth: u32) -> usize {
        let copy = self.copy();
        let f = Frame { group, copy, parent: root, root, owner: root, enclosing: &[], depth };
        let w = self.world;
        for r in &w.bodies[group].objects {
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
        if r.mode == 1 && !r.filter.matches(&self.ctx) {
            return;
        }
        let skip = match r.mode {
            3 => self.copies[f.copy].contains(&r.id),
            2 => self.groups.get(&f.group).is_some_and(|s| s.contains(&r.id)),
            _ => false,
        };
        if skip {
            return;
        }
        self.copies[f.copy].insert(r.id);
        self.groups.entry(f.group).or_default().insert(r.id);
        let header = match &r.body {
            Body::Emitter(e) => Some(e.header),
            Body::Effect(e) => Some(e.header),
            Body::Other { header, .. } => *header,
            _ => None,
        };
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
                if p.mode == 2 {
                    return;
                }
                let copy = match self.world.group_by_id(p.group_id) {
                    Some(g) if f.depth < 8 => self.group(g, m, f.depth + 1),
                    _ => self.copy(),
                };
                if !p.fresh {
                    let extra = Frame { copy, parent: m, root: m, owner: m, enclosing: &[], depth: f.depth + 1, ..f };
                    for c in &p.objects {
                        self.record(c, extra);
                    }
                }
            }
            _ => {
                let m = header.map_or(f.parent, |h| Self::at(&f, h.parent).mul(&h.matrix));
                self.objects.push((r.class, r.id, m));
                let one = [m];
                let child = match header {
                    Some(_) => Frame { parent: m, owner: m, enclosing: &one, ..f },
                    None => Frame { parent: m, ..f },
                };
                for c in r.nested() {
                    self.record(c, child);
                }
            }
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
    assert_eq!(d.worlds.len(), 39);
    let totals = std::sync::Mutex::new((0usize, 0u32));
    each_world(d, |i| {
        let (slot, _, bytes) = &d.worlds[i];
        let w = load(d, i);
        let fw = formats_sly3::World::parse_with(bytes, &d.options, d.multipliers).unwrap();
        let ctx = fw.fresh_context(Some(*slot as i16));
        let mut e = Expect { world: &fw, ctx, objects: Vec::new(), copies: Vec::new(), groups: BTreeMap::new() };
        for (g, entry) in fw.groups.iter().enumerate() {
            if entry.shared != 0 {
                e.group(g, Affine::IDENTITY, 0);
            }
        }
        let mut want: BTreeMap<i16, usize> = BTreeMap::new();
        for (c, _, _) in &e.objects {
            *want.entry(*c).or_default() += 1;
        }
        let mut got: BTreeMap<i16, usize> = BTreeMap::new();
        for h in w.handles() {
            *got.entry(w.class(h).unwrap()).or_default() += 1;
        }
        assert_eq!(got, want, "slot {slot}: objects per class");
        for (&c, &n) in &want {
            assert_eq!(w.slots_of_class(c).len(), n, "slot {slot}: class index {c}");
        }
        assert_eq!(w.len(), e.objects.len());
        let l = w.sly3.as_deref().unwrap();
        let mut bad = 0;
        for (s, (class, id, m)) in e.objects.iter().enumerate() {
            let h = w.handle(s as u32).unwrap();
            assert_eq!(w.class(h), Some(*class));
            assert_eq!(w.sly3_id(h), Some(*id));
            if l.stats.singular_parents == 0 && !close(&w.world_transform(h).unwrap(), m) {
                bad += 1;
            }
            let r = w.sly3_record(h).expect("record");
            assert_eq!((r.class, r.id), (*class, *id));
            assert_eq!(w.sly3_object(h).unwrap().script, r.script);
        }
        assert_eq!(bad, 0, "slot {slot}: world matrices differ from the loaded ones");
        let keys: BTreeSet<u64> = w.handles().map(|h| w.persist_key(h).unwrap()).collect();
        assert_eq!(keys.len(), w.len(), "slot {slot}: persist keys");
        // Groupers' objects start out of the world.
        for h in w.handles() {
            if let Some(p) = w.parent(h)
                && matches!(w.sly3_record(p).map(|r| &r.body), Some(Body::Other { layout: Layout::Grouper, .. }))
            {
                assert!(!w.in_world(h), "slot {slot}: grouper object in the world");
            }
        }
        let mut t = totals.lock().unwrap();
        t.0 += w.len();
        t.1 += l.stats.option_errors;
        eprintln!("slot {slot:2}: {:6} objects, {:?}", w.len(), l.stats);
    });
    let (objects, option_errors) = *totals.lock().unwrap();
    eprintln!("all worlds: {objects} objects");
    assert_eq!(option_errors, 0);
}

#[test]
fn every_world_runs_and_is_deterministic() {
    use script::{OptionMap, ScriptError};
    let Some(d) = disc("every_world_runs_and_is_deterministic") else { return };
    let map = OptionMap::from_sly3_executable(&d.exe).expect("option map");
    let kinds = std::sync::Mutex::new(BTreeMap::<String, u64>::new());
    let totals = std::sync::Mutex::new((0u64, 0u64, 0usize));
    each_world(d, |i| {
        let run = |scripts: bool| {
            let w = load(d, i);
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
            let errors = g.scripts().map_or(0, |s| s.error_count());
            let host: u64 = g
                .script_error_kinds
                .iter()
                .filter(|k| matches!(k.0, ScriptError::Host(_)))
                .map(|k| u64::from(k.1))
                .sum();
            let by_kind: Vec<(String, u64)> =
                g.script_error_kinds.iter().map(|k| (format!("{:?}", k.0), u64::from(k.1))).collect();
            (hashes, attached, errors, host, by_kind)
        };
        let slot = d.worlds[i].0;
        assert_eq!(run(false).0, run(false).0, "slot {slot}: not deterministic without scripts");
        let (a, attached, errors, host, by_kind) = run(true);
        let (b, ..) = run(true);
        assert_eq!(a, b, "slot {slot}: not deterministic with scripts");
        // Every module of the world runs: objects are attached through their records' script indices.
        assert!(attached > 0 || d.worlds[i].2.is_empty(), "slot {slot}: no object attached");
        let mut all = kinds.lock().unwrap();
        for (k, n) in by_kind {
            *all.entry(k).or_default() += n;
        }
        let mut t = totals.lock().unwrap();
        t.0 += errors;
        t.1 += host;
        t.2 += attached;
        eprintln!(
            "slot {slot:2}: {attached} objects attached to modules, {errors} script errors ({host} host) after 90 frames"
        );
    });
    let mut k: Vec<(String, u64)> = kinds.into_inner().unwrap().into_iter().collect();
    k.sort_by_key(|x| std::cmp::Reverse(x.1));
    let (errors, host, attached) = totals.into_inner().unwrap();
    eprintln!(
        "all worlds: {attached} objects attached, {errors} script errors, {host} of them the host's; by kind: {k:?}"
    );
    // The scripts are understood: no keyword, form, cell or operation is unknown to the interpreter.
    for (kind, n) in &k {
        assert!(
            !["UnknownKeyword", "BadForm", "BadCell", "UnknownOperation", "StaleEnvironment"]
                .iter()
                .any(|p| kind.starts_with(p)),
            "{n} x {kind}"
        );
    }
}
