//! Making a world's objects from a Sly 2 world file (the rules are in the [module documentation](super)).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use formats::geometry::Affine;
use formats::stream::ObjectStream;
use formats::world::{Header, ScriptTable, WorldLayout};
use formats_sly2::exe::OptionTable;
use formats_sly2::options::OptionList;
use formats_sly2::stream::{BLOCK_OPTIONS, Body, Record};
use formats_sly2::transforms::{TransformList, TransformRef};
use formats_sly2::world::PackageBody;

use super::options::{REMOVE_AT_START, args_of, decode_list};
use super::{RecordKey, Sly2Level, Sly2Object, Sly2Options};
use crate::math::affine_inverse;
use crate::object::{MAX_OBJECTS, flags};
use crate::world::{LevelData, LoadError, NONE, World};

/// What the loader did, for diagnostics and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadStats {
    /// Objects created.
    pub objects: u32,
    /// Copies (package instances): kept packages and placements.
    pub copies: u32,
    /// Records left out by their level conditions (each with its subtree).
    pub excluded_by_row: u32,
    /// Class 132 and 133 records created inactive because their conditions exclude the row.
    pub inactive_missions: u32,
    /// Records of kind −1 or −2 left out because their ID already existed in the copy or package.
    pub clone_skips: u32,
    /// Objects out of the world at start (option 26 in their list, a root patch or block).
    pub removed_at_start: u32,
    /// Placements naming a package the world does not hold.
    pub unresolved_placements: u32,
    /// Placements not followed because of the depth limit.
    pub depth_cut: u32,
    /// Records whose body did not parse (created without children).
    pub unparsed: u32,
    /// Objects whose parent's loaded world matrix could not be inverted (their local transform is their world
    /// matrix).
    pub singular_parents: u32,
    /// Objects whose `s32` ID does not fit the 16-bit OID column (their OID is −1; `World::sly2_id` has the ID).
    pub wide_ids: u32,
    /// Option values that did not decode.
    pub option_errors: u32,
    /// Copy patches whose target ID is in no object of the copy.
    pub unmatched_patches: u32,
    /// Copy roots bound to a module of their placement's package by a patch's script index.
    pub patch_modules: u32,
    /// Pool copies of fixed placements (option 1178) kept out of the world at load.
    pub fixed_pool_copies: u32,
}

/// The loading context of one object list.
#[derive(Clone, Copy)]
struct Cx<'e> {
    /// Package the records belong to.
    pkg: usize,
    /// Set ID of the package being instantiated (kind −2 scope).
    set: i32,
    /// Copy serial.
    copy: u32,
    /// Parent slot.
    parent: u32,
    /// Reference −1.
    root: Affine,
    /// Reference 0x2000.
    owner: Affine,
    /// The enclosing solid's transforms (world, as loaded) and slot.
    enclosing: &'e [Affine],
    enclosing_slot: u32,
    /// Placements followed so far.
    depth: u32,
}

struct Loader<'b> {
    data: &'b [u8],
    bodies: &'b [PackageBody],
    table: &'b OptionTable,
    set_index: BTreeMap<i32, usize>,
    first_module: Vec<u32>,
    row: Option<i16>,
    max_depth: u32,
    level: Sly2Level,
    record_ids: BTreeMap<u32, u32>,
    load_world: Vec<Affine>,
    copy_ids: Vec<BTreeSet<i32>>,
    set_ids: BTreeMap<i32, BTreeSet<i32>>,
    path: Vec<u32>,
}

/// An empty Sly 1 layout for the Sly 1 part of `LevelData`, which a Sly 2 world does not use.
fn empty_layout() -> WorldLayout {
    WorldLayout {
        header: Header { texture_size: 0, timestamp: 0 },
        debug: 0,
        sound_bank: -1,
        name_count: 0,
        scripts: ScriptTable { range: 0..0, count: 0, symbols: 0, extra: 0, scripts: Vec::new() },
        world_options: Default::default(),
        descriptors: 0..0,
        audio: None,
        speakers: 0..0,
        music: None,
        materials_range: 0..0,
        materials: Default::default(),
        object_stream: 0,
    }
}

impl World {
    /// Loads a decompressed Sly 2 world: parses it with the executable's option table (`formats_sly2`) and creates
    /// its objects. Transforms are computed; no behaviours are attached; scripts are attached by
    /// [`crate::game::Game::new`].
    pub fn load_sly2(bytes: Vec<u8>, table: &OptionTable, opts: Sly2Options) -> Result<World, LoadError> {
        let parsed = formats_sly2::World::parse(&bytes, table)?;
        let formats_sly2::World { header, strings, tree, options, bodies, tail, .. } = parsed;

        let level = LevelData {
            bytes: Vec::new(),
            layout: empty_layout(),
            stream: ObjectStream::default(),
            table: formats::options::OptionTable::empty(),
        };
        let mut w = World::empty(level, opts.info);

        let mut first_module = Vec::with_capacity(bodies.len());
        let mut m = 0u32;
        for b in &bodies {
            first_module.push(m);
            m += b.scripts;
        }
        let mut ld = Loader {
            data: &bytes,
            bodies: &bodies,
            table,
            set_index: header.packages.iter().enumerate().map(|(i, p)| (p.set_id, i)).rev().collect(),
            first_module,
            row: opts.row,
            max_depth: opts.max_depth,
            level: Sly2Level {
                header: Default::default(),
                strings,
                tree,
                world_lists: Default::default(),
                bodies: Vec::new(),
                tail,
                table: table.clone(),
                row: opts.row,
                builtins: opts.builtins,
                stats: LoadStats::default(),
                actions: Vec::new(),
                pose: Vec::new(),
                game_objects: BTreeMap::new(),
                job: [None; 3],
                difficulty_help: 0.0,
                jobs: None,
                anim: Default::default(),
                poser: Default::default(), // Sly 2 hook: clip poses
                hud: Default::default(),
                task_events: Vec::new(),
                bound: Default::default(),
                placed: Default::default(),
                noise_objects: Vec::new(),
                objects: Vec::new(),
                ids: Default::default(),
                records: Vec::new(),
                paths: Vec::new(),
            },
            record_ids: BTreeMap::new(),
            load_world: Vec::new(),
            copy_ids: Vec::new(),
            set_ids: BTreeMap::new(),
            path: Vec::new(),
        };

        for list in &options {
            let (decoded, failed) = decode_list(&bytes, list, table);
            ld.level.stats.option_errors += failed;
            for d in decoded {
                w.world_options.push(d.id, d.value);
            }
        }
        for (i, p) in header.packages.iter().enumerate() {
            if p.keep != 0 {
                ld.package(&mut w, i, p.set_id, NONE, Affine::IDENTITY, 0)?;
            }
        }

        let mut level = ld.level;
        level.header = header;
        level.world_lists = options;
        level.bodies = bodies;
        level.stats.objects = w.len() as u32;
        w.level.bytes = bytes;
        level.build_ids();
        w.sly2 = Some(alloc::boxed::Box::new(level));
        // A fixed placement (its root block sets 1178 = 1, script ID 1022) does not lease its copy when it enters the
        // world, so the pool's stored copy stays out of it until leased (placement.md 2.1: Dimitri's bullets).
        let fixed: Vec<u32> = w
            .handles()
            .filter(|&h| {
                w.class(h).is_some_and(|c| super::is_a(c, 6)) && w.options(h).and_then(|o| o.i32(1178)) == Some(1)
            })
            .filter_map(|h| w.children(h).next().and_then(|c| w.slot(c)))
            .collect();
        for s in fixed {
            w.set_flag(s, crate::object::flags::IN_WORLD, false);
            if let Some(l) = w.sly2.as_deref_mut() {
                l.stats.fixed_pool_copies += 1;
            }
        }
        w.ensure_order();
        w.update_transforms();
        Ok(w)
    }
}

impl<'b> Loader<'b> {
    fn new_copy(&mut self) -> u32 {
        self.copy_ids.push(BTreeSet::new());
        self.level.stats.copies += 1;
        self.copy_ids.len() as u32 - 1
    }

    /// Creates package `pkg`'s object list as a new copy under `parent`, with `root` for reference −1.
    fn package(
        &mut self,
        w: &mut World,
        pkg: usize,
        set: i32,
        parent: u32,
        root: Affine,
        depth: u32,
    ) -> Result<u32, LoadError> {
        let copy = self.new_copy();
        let cx = Cx { pkg, set, copy, parent, root, owner: root, enclosing: &[], enclosing_slot: NONE, depth };
        let saved = core::mem::take(&mut self.path);
        let bodies = self.bodies;
        let r = self.list(w, &bodies[pkg].objects, cx);
        self.path = saved;
        r.map(|_| copy)
    }

    fn list(&mut self, w: &mut World, records: &'b [Record], cx: Cx<'_>) -> Result<(), LoadError> {
        for (i, r) in records.iter().enumerate() {
            self.path.push(i as u32);
            let res = self.record(w, r, cx);
            self.path.pop();
            res?;
        }
        Ok(())
    }

    fn resolve(cx: &Cx<'_>, reference: i16) -> Affine {
        match TransformRef::decode(reference) {
            TransformRef::Enclosing(n) => cx.enclosing.get(n as usize).copied().unwrap_or(cx.root),
            TransformRef::Owner => cx.owner,
            _ => cx.root,
        }
    }

    /// World matrices of a transform list as loaded (transforms.md 1).
    fn transforms(list: &TransformList, cx: &Cx<'_>) -> Vec<Affine> {
        let mut out: Vec<Affine> = Vec::with_capacity(list.transforms.len());
        for t in &list.transforms {
            let parent = match TransformRef::decode(t.parent) {
                TransformRef::Own(n) => out.get(n as usize).copied().unwrap_or(cx.root),
                _ => Self::resolve(cx, t.parent),
            };
            out.push(parent.mul(&t.local));
        }
        out
    }

    fn record_index(&mut self, pkg: usize, r: &Record) -> u32 {
        if let Some(&i) = self.record_ids.get(&r.header_offset) {
            return i;
        }
        let i = self.level.records.len() as u32;
        let start = self.level.paths.len() as u32;
        self.level.paths.extend_from_slice(&self.path);
        self.level.records.push(RecordKey { package: pkg as u16, start, len: self.path.len() as u16 });
        self.record_ids.insert(r.header_offset, i);
        i
    }

    fn module_of(&self, pkg: usize, script: i16) -> i16 {
        let count = self.bodies[pkg].scripts;
        match u32::try_from(script) {
            Ok(m) if m < count => i16::try_from(self.first_module[pkg] + m).unwrap_or(-1),
            _ => -1,
        }
    }

    /// Applies an option list to a slot: values stored, actions kept, option 26 applied.
    fn apply(&mut self, w: &mut World, slot: u32, list: &OptionList) {
        let (decoded, failed) = decode_list(self.data, list, self.table);
        self.level.stats.option_errors += failed;
        for d in decoded {
            if d.action {
                if d.id == REMOVE_AT_START {
                    w.flags[slot as usize] &= !flags::IN_WORLD;
                }
                self.level.actions.push((slot, d.id, args_of(&d.value)));
            } else {
                w.options[slot as usize].push(d.id, d.value);
            }
        }
    }

    fn record(&mut self, w: &mut World, r: &'b Record, cx: Cx<'_>) -> Result<(), LoadError> {
        let excluded = !r.conditions.is_empty() && self.row.is_some_and(|row| !r.conditions.contains(&row));
        let mission = matches!(r.class, 132 | 133);
        if excluded && !mission {
            self.level.stats.excluded_by_row += 1;
            return Ok(());
        }
        let seen = match r.kind {
            -1 => self.copy_ids[cx.copy as usize].contains(&r.id),
            -2 => self.set_ids.get(&cx.set).is_some_and(|s| s.contains(&r.id)),
            _ => false,
        };
        if seen {
            self.level.stats.clone_skips += 1;
            return Ok(());
        }
        self.copy_ids[cx.copy as usize].insert(r.id);
        self.set_ids.entry(cx.set).or_default().insert(r.id);

        if w.slot_count() as u32 >= MAX_OBJECTS {
            return Err(LoadError::TooManyObjects);
        }
        let parent_world = if cx.parent == NONE { Affine::IDENTITY } else { self.load_world[cx.parent as usize] };
        let mut own: Vec<Affine> = Vec::new();
        let (world, parent_ref) = match &r.body {
            Body::Solid(s) => {
                own = Self::transforms(&s.transforms, &cx);
                let t0 = s.transforms.transforms.first().map_or(-1, |t| t.parent);
                (own.first().copied().unwrap_or(parent_world), t0)
            }
            Body::Placement(p) => (Self::resolve(&cx, p.parent).mul(&p.matrix), p.parent),
            Body::Other { header: Some(h), .. } => (Self::resolve(&cx, h.parent).mul(&h.matrix), h.parent),
            Body::Other { .. } | Body::Unparsed => (parent_world, -1),
        };
        if matches!(r.body, Body::Unparsed) {
            self.level.stats.unparsed += 1;
        }
        let local = if cx.parent == NONE {
            world
        } else {
            match affine_inverse(&parent_world) {
                Some(inv) => inv.mul(&world),
                None => {
                    self.level.stats.singular_parents += 1;
                    world
                }
            }
        };
        let oid = match i16::try_from(r.id) {
            Ok(v) => v,
            Err(_) => {
                self.level.stats.wide_ids += 1;
                -1
            }
        };
        let rec = self.record_index(cx.pkg, r);
        let module = self.module_of(cx.pkg, r.script);
        let persist = ((cx.copy as u64) << 32) | r.header_offset as u64;
        let degenerate = if world.determinant().abs() < 1e-12 { flags::DEGENERATE } else { 0 };
        let slot = w.create(r.class, oid, rec, module, local, cx.parent, persist, degenerate);
        let i = slot as usize;
        if self.load_world.len() <= i {
            self.load_world.resize(i + 1, Affine::IDENTITY);
            self.level.objects.resize(i + 1, Sly2Object::default());
        }
        self.load_world[i] = world;

        let inv_world = affine_inverse(&world);
        let pose_start = self.level.pose.len() as u32;
        for m in &own {
            self.level.pose.push(inv_world.map_or(Affine::IDENTITY, |inv| inv.mul(m)));
        }
        let bone = match TransformRef::decode(parent_ref) {
            TransformRef::Enclosing(n) if cx.enclosing_slot != NONE && (n as usize) < cx.enclosing.len() => {
                Some((cx.enclosing_slot, n))
            }
            _ => None,
        };
        let bone_local = bone
            .and_then(|(_, n)| affine_inverse(&cx.enclosing[n as usize]))
            .map_or(Affine::IDENTITY, |inv| inv.mul(&world));
        self.level.objects[i] = Sly2Object {
            id: r.id,
            package: cx.pkg as u16,
            copy: cx.copy,
            pose: (pose_start, self.level.pose.len() as u32),
            bone,
            bone_local,
        };

        if let Some(list) = r.options() {
            self.apply(w, slot, list);
        }
        if excluded {
            self.level.stats.inactive_missions += 1;
            w.flags[i] &= !flags::IN_WORLD;
        }

        match &r.body {
            Body::Solid(s) => {
                let child = Cx { parent: slot, enclosing: &own, enclosing_slot: slot, ..cx };
                self.list(w, &s.children, child)?;
            }
            Body::Placement(p) => {
                for b in p.blocks.iter().filter(|b| b.kind == BLOCK_OPTIONS) {
                    if let Some(o) = &b.options {
                        self.apply(w, slot, o);
                    }
                }
                let first = w.slot_count() as u32;
                // Whether the package was copied: a depth-cut or unresolved placement has no copy root (`first` would
                // then name its first extra object, or nothing).
                let mut copied = false;
                let copy = match self.set_index.get(&p.set_id).copied() {
                    Some(_) if cx.depth >= self.max_depth => {
                        self.level.stats.depth_cut += 1;
                        self.new_copy()
                    }
                    Some(t) => {
                        copied = true;
                        self.package(w, t, p.set_id, slot, world, cx.depth + 1)?
                    }
                    None => {
                        self.level.stats.unresolved_placements += 1;
                        self.new_copy()
                    }
                };
                let root = (copied && first < w.slot_count() as u32).then_some(first);
                // The copy's root and its number of top-level objects, kept for leases: a script can detach the root
                // from the placement later (classes/placement.md 2.1).
                if let Some(r) = root {
                    let ph = w.handle(slot);
                    let tops = (first..w.slot_count() as u32)
                        .filter(|&s| {
                            w.handle(s).and_then(|h| w.parent(h)) == ph
                                && self.level.objects.get(s as usize).is_some_and(|o| o.copy == copy)
                        })
                        .count() as u32;
                    let local = w.handle(r).and_then(|h| w.local(h)).unwrap_or(Affine::IDENTITY);
                    self.level.placed.insert(slot, (r, tops, local));
                }
                let extra = Cx {
                    copy,
                    parent: slot,
                    root: world,
                    owner: world,
                    enclosing: &[],
                    enclosing_slot: NONE,
                    depth: cx.depth + 1,
                    ..cx
                };
                self.list(w, &p.extra, extra)?;
                // A fresh placement's blocks for other targets (classes/placement.md 1.1): 6 is the copy's root (the
                // Dimitri placement 122332's route, missions.md 4.3), other IDs an object of the copy.
                for b in p.blocks.iter().filter(|b| b.kind != BLOCK_OPTIONS) {
                    let target = if b.kind == 6 {
                        root
                    } else {
                        (first..w.slot_count() as u32).find(|&s| {
                            let o = &self.level.objects[s as usize];
                            o.copy == copy && o.id == b.kind
                        })
                    };
                    match (target, &b.options) {
                        (Some(t), Some(o)) => self.apply(w, t, o),
                        _ => self.level.stats.unmatched_patches += 1,
                    }
                }
                for patch in &p.patches {
                    let target = if matches!(patch.target, 5 | 6) {
                        Some(slot)
                    } else {
                        (first..w.slot_count() as u32).find(|&s| {
                            let o = &self.level.objects[s as usize];
                            o.copy == copy && o.id == patch.target
                        })
                    };
                    match target {
                        Some(t) => self.apply(w, t, &patch.options),
                        None => self.level.stats.unmatched_patches += 1,
                    }
                    // A patch's script index binds the copy's root to that module of the placement's package
                    // (missions.md 4.3: 123863's patch (5, class 48, index 5) gives the vent prop 120985 module 5 of
                    // `f_nightclub_cellar_rig`); an earlier module stays as its outer environment (scripts.md 11.3).
                    // Several 5 / 6 patches with indices each bind in turn: the last wins, and each overwrites the
                    // outer module recorded by the one before.
                    if matches!(patch.target, 5 | 6)
                        && let Some(r) = root
                    {
                        let m = self.module_of(cx.pkg, patch.index);
                        let old = w.module[r as usize];
                        if m >= 0 && old != m {
                            if old >= 0 {
                                w.outer_module.insert(r, old);
                            }
                            w.module[r as usize] = m;
                            self.level.bound.insert(r);
                            self.level.stats.patch_modules += 1;
                        }
                    }
                }
            }
            Body::Other { header, children, .. } if !children.is_empty() => {
                let frame = [world];
                let child = match header {
                    Some(_) => Cx { parent: slot, owner: world, enclosing: &frame, enclosing_slot: slot, ..cx },
                    None => Cx { parent: slot, ..cx },
                };
                self.list(w, children, child)?;
            }
            _ => {}
        }
        if w.flags[i] & flags::IN_WORLD == 0 && !excluded {
            self.level.stats.removed_at_start += 1;
        }
        Ok(())
    }
}
