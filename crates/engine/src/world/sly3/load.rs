//! Making a world's objects from a Sly 3 world file (the rules are in the [module documentation](super)).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use formats::geometry::Affine;
use formats::stream::ObjectStream;
use formats::world::{Header, ScriptTable, WorldLayout};
use formats_sly2::exe::OptionTable;
use formats_sly2::transforms::{TransformList, TransformRef};
use formats_sly3::filter::Context;
use formats_sly3::options::OptionList;
use formats_sly3::stream::{BIND_FIRST, BIND_SELF, Body, Layout, Record};
use formats_sly3::world::GroupBody;

use super::options::{REMOVE_AT_START, args_of, decode_list};
use super::{RecordKey, Sly3Level, Sly3Object, Sly3Options};
use crate::math::affine_inverse;
use crate::object::{MAX_OBJECTS, flags};
use crate::world::{LevelData, LoadError, NONE, World};

/// Jobs and tasks: loaded whatever the world filter (missions.md 4.1a).
const JOB_CLASS: i16 = 168;
const TASK_CLASS: i16 = 169;
/// The core crew's classes (Sly 52, Murray 53, Bentley 54), whose spawners are pooled.
const CREW_FIRST: i16 = 52;
const CREW_LAST: i16 = 54;

/// Class IDs a Sly 3 world can hold (0 to 255); the per-class index is widened to this at load.
pub const SLY3_CLASS_SLOTS: usize = 256;

/// What the loader did, for diagnostics and tests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadStats {
    /// Objects created.
    pub objects: u32,
    /// Copies (group instances): shared groups and placements.
    pub copies: u32,
    /// Mode-1 records left out by their world filter or conditions (each with its subtree).
    pub excluded: u32,
    /// Sly 3 hook (job system): jobs and tasks of other worlds, loaded out of the world (missions.md 4.1a).
    pub off_world: u32,
    /// Mode-2 and mode-3 records left out because their ID already existed in the group or copy.
    pub clone_skips: u32,
    /// Objects out of the world at start (option 27, a bind carrying it, or a grouper's objects).
    pub removed_at_start: u32,
    /// Placements naming a group the world does not hold.
    pub unresolved_placements: u32,
    /// Placements not followed because of the depth limit.
    pub depth_cut: u32,
    /// Mode-2 placements (spawners): only the placement object is created.
    pub spawners: u32,
    /// Fresh placements (their data is skipped).
    pub fresh: u32,
    /// Placements beyond their group's copy count.
    pub over_copy_count: u32,
    /// Records whose body did not parse (created without children).
    pub unparsed: u32,
    /// Objects whose parent's loaded world matrix could not be inverted.
    pub singular_parents: u32,
    /// Objects whose `s32` ID does not fit the 16-bit OID column.
    pub wide_ids: u32,
    /// Option values that did not decode.
    pub option_errors: u32,
    /// Binds whose target is in no object of the copy.
    pub unmatched_binds: u32,
}

/// The loading context of one object list.
#[derive(Clone, Copy)]
struct Cx<'e> {
    group: usize,
    copy: u32,
    parent: u32,
    root: Affine,
    owner: Affine,
    enclosing: &'e [Affine],
    enclosing_slot: u32,
    depth: u32,
    /// Objects of this list start out of the world (a grouper's).
    held: bool,
}

/// The guard and projectile classes (pooled for spawn points).
const GUARD_CLASS: i16 = 58;
const PROJECTILE_CLASS: i16 = 89;

struct Loader<'b> {
    data: &'b [u8],
    bodies: &'b [GroupBody],
    table: &'b OptionTable,
    by_id: BTreeMap<i32, usize>,
    context: Context,
    max_depth: u32,
    level: Sly3Level,
    record_ids: BTreeMap<u32, u32>,
    load_world: Vec<Affine>,
    copy_ids: Vec<BTreeSet<i32>>,
    group_ids: BTreeMap<usize, BTreeSet<i32>>,
    instances: BTreeMap<i32, i32>,
    /// Sly 3 hook (missions.md 4.1a, script ID 825): the groups mode-2 placements (spawn points) name, outside the
    /// crew.
    spawn_groups: BTreeSet<i32>,
    path: Vec<u32>,
    /// Sly 3 script hook: the program-wide index of each group's first script module (as
    /// `script::Program::from_sly3_bodies` numbers them).
    first_module: Vec<u32>,
}

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
    /// Loads a decompressed Sly 3 world: parses it with the executable's option table (`formats_sly3`) and creates
    /// its objects. Transforms are computed; each object records the script module its record names (attached by
    /// the game when it runs scripts); no behaviours are attached.
    pub fn load_sly3(bytes: Vec<u8>, table: &OptionTable, opts: Sly3Options) -> Result<World, LoadError> {
        let parsed = formats_sly3::World::parse_with(&bytes, table, opts.multipliers)?;
        let context = opts.context.clone().unwrap_or_else(|| parsed.fresh_context(opts.slot));
        let copy_counts = parsed.copy_counts(opts.set);
        let formats_sly3::World {
            keys, groups, sections, strings, options, option_sets, partitions, bodies, tail, ..
        } = parsed;

        let level = LevelData {
            bytes: Vec::new(),
            layout: empty_layout(),
            stream: ObjectStream::default(),
            table: formats::options::OptionTable::empty(),
        };
        let mut w = World::empty(level, opts.info);
        w.by_class.resize(SLY3_CLASS_SLOTS, Vec::new());

        let mut ld = Loader {
            data: &bytes,
            bodies: &bodies,
            table,
            by_id: groups.iter().enumerate().map(|(i, g)| (g.id, i)).rev().collect(),
            context: context.clone(),
            max_depth: opts.max_depth,
            level: Sly3Level {
                keys,
                groups: Vec::new(),
                sections,
                strings,
                world_list: OptionList::default(),
                option_sets,
                partitions,
                bodies: Vec::new(),
                tail,
                table: table.clone(),
                context,
                copy_counts,
                stats: LoadStats::default(),
                actions: Vec::new(),
                pose: Vec::new(),
                game_objects: BTreeMap::new(),                   // Sly 3 host hook
                pickpocket: alloc::collections::BTreeSet::new(), // Sly 3 host hook
                pools: BTreeMap::new(),                          // Sly 3 spawn hook
                pool_origins: BTreeMap::new(),                   // Sly 3 spawn hook
                bound: BTreeSet::new(),                          // Sly 3 script hook
                poser: Default::default(),                       // Sly 3 pose hook
                objects: Vec::new(),
                ids: Default::default(),
                records: Vec::new(),
                paths: Vec::new(),
            },
            record_ids: BTreeMap::new(),
            load_world: Vec::new(),
            copy_ids: Vec::new(),
            group_ids: BTreeMap::new(),
            instances: BTreeMap::new(),
            spawn_groups: BTreeSet::new(),
            path: Vec::new(),
            first_module: bodies
                .iter()
                .scan(0u32, |n, b| {
                    let first = *n;
                    *n += b.scripts;
                    Some(first)
                })
                .collect(), // Sly 3 script hook
        };

        let (decoded, failed) = decode_list(&bytes, &options, table);
        ld.level.stats.option_errors += failed;
        for d in decoded {
            w.world_options.push(d.id, d.value);
        }
        for (i, g) in groups.iter().enumerate() {
            if g.shared != 0 {
                ld.group(&mut w, i, NONE, Affine::IDENTITY, 0)?;
            }
        }
        // Sly 3 spawn hook (missions.md 4.1a: script ID 825 takes a new instance of a placement). Stand-in, a working
        // choice [I] for the pool's size: a group a spawn point names gets the copies its copy count (the world's
        // header) leaves after the placed ones, made now at the top level and out of the world; 825 takes a free one.
        // v_gauntlet: the brawl's guards (144809) have 7, 3 placed, so 4 in the pool. Only groups of guards and of
        // projectiles (a class-58 or class-89 record at their top level: v_canal's missiles, 150052) are pooled; the
        // other spawn points' groups are left as they were.
        for gid in core::mem::take(&mut ld.spawn_groups) {
            let Some(g) = ld.by_id.get(&gid).copied() else { continue };
            if !bodies
                .get(g)
                .is_some_and(|b| b.objects.iter().any(|r| matches!(r.class, GUARD_CLASS | PROJECTILE_CLASS)))
            {
                continue;
            }
            let placed = ld.instances.get(&gid).copied().unwrap_or(0);
            let want = ld.level.copy_counts.get(&gid).copied().unwrap_or(0) - placed;
            for _ in 0..want.max(0) {
                let first = w.slot_count() as u32;
                let copy = ld.group(&mut w, g, NONE, Affine::IDENTITY, 1)?;
                for s in first..w.slot_count() as u32 {
                    if w.parent_of_slot(s) == NONE {
                        w.flags[s as usize] &= !flags::IN_WORLD;
                        ld.level.pool_origins.insert(s, w.local[s as usize]);
                    }
                }
                ld.level.pools.entry(gid).or_default().push(copy);
            }
        }

        let mut level = ld.level;
        level.groups = groups;
        level.world_list = options;
        level.bodies = bodies;
        level.stats.objects = w.len() as u32;
        w.level.bytes = bytes;
        level.build_ids();
        w.sly3 = Some(alloc::boxed::Box::new(level));
        w.ensure_order();
        w.update_transforms();
        Ok(w)
    }
}

impl<'b> Loader<'b> {
    /// Sly 3 script hook: the program-wide module a record's script index names in its own group, or −1.
    fn module_of(&self, group: usize, script: i16) -> i16 {
        let count = self.bodies.get(group).map_or(0, |b| b.scripts);
        match u32::try_from(script) {
            Ok(m) if m < count => i16::try_from(self.first_module[group] + m).unwrap_or(-1),
            _ => -1,
        }
    }

    fn new_copy(&mut self) -> u32 {
        self.copy_ids.push(BTreeSet::new());
        self.level.stats.copies += 1;
        self.copy_ids.len() as u32 - 1
    }

    /// Creates group `group`'s object list as a new copy under `parent`, with `root` for reference −1.
    fn group(&mut self, w: &mut World, group: usize, parent: u32, root: Affine, depth: u32) -> Result<u32, LoadError> {
        let copy = self.new_copy();
        let cx =
            Cx { group, copy, parent, root, owner: root, enclosing: &[], enclosing_slot: NONE, depth, held: false };
        let saved = core::mem::take(&mut self.path);
        let bodies = self.bodies;
        let r = self.list(w, &bodies[group].objects, cx);
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

    fn record_index(&mut self, group: usize, r: &Record) -> u32 {
        if let Some(&i) = self.record_ids.get(&r.header_offset) {
            return i;
        }
        let i = self.level.records.len() as u32;
        let start = self.level.paths.len() as u32;
        self.level.paths.extend_from_slice(&self.path);
        self.level.records.push(RecordKey { group: group as u16, start, len: self.path.len() as u16 });
        self.record_ids.insert(r.header_offset, i);
        i
    }

    /// Applies an option list to a slot: values stored, actions kept, option 27 applied.
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

    /// The world matrix of a record as loaded, and its parent reference.
    fn placed(r: &Record, cx: &Cx<'_>, parent_world: Affine, own: &mut Vec<Affine>) -> (Affine, i16) {
        let at = |h: &formats_sly3::stream::Header| (Self::resolve(cx, h.parent).mul(&h.matrix), h.parent);
        match &r.body {
            Body::Solid(s) => {
                *own = Self::transforms(&s.transforms, cx);
                let t0 = s.transforms.transforms.first().map_or(-1, |t| t.parent);
                (own.first().copied().unwrap_or(parent_world), t0)
            }
            Body::Placement(p) => (Self::resolve(cx, p.parent).mul(&p.matrix), p.parent),
            Body::Emitter(e) => at(&e.header),
            Body::Effect(e) => at(&e.header),
            Body::Other { header: Some(h), .. } => at(h),
            _ => (parent_world, -1),
        }
    }

    fn record(&mut self, w: &mut World, r: &'b Record, cx: Cx<'_>) -> Result<(), LoadError> {
        // Sly 3 hook (job system): jobs and tasks (classes 168, 169) of other worlds load anyway, marked off-world
        // (missions.md 4.1a): out of the world, so they get no task events, but present for the job graph.
        let off_world = !r.loaded(&self.context);
        if off_world && !matches!(r.class, JOB_CLASS | TASK_CLASS) {
            self.level.stats.excluded += 1;
            return Ok(());
        }
        let seen = match r.mode {
            3 => self.copy_ids[cx.copy as usize].contains(&r.id),
            2 => self.group_ids.get(&cx.group).is_some_and(|s| s.contains(&r.id)),
            _ => false,
        };
        if seen {
            self.level.stats.clone_skips += 1;
            return Ok(());
        }
        self.copy_ids[cx.copy as usize].insert(r.id);
        self.group_ids.entry(cx.group).or_default().insert(r.id);

        if w.slot_count() as u32 >= MAX_OBJECTS {
            return Err(LoadError::TooManyObjects);
        }
        let parent_world = if cx.parent == NONE { Affine::IDENTITY } else { self.load_world[cx.parent as usize] };
        let mut own: Vec<Affine> = Vec::new();
        let (world, parent_ref) = Self::placed(r, &cx, parent_world, &mut own);
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
        let rec = self.record_index(cx.group, r);
        let persist = ((cx.copy as u64) << 32) | r.header_offset as u64;
        let degenerate = if world.determinant().abs() < 1e-12 { flags::DEGENERATE } else { 0 };
        let module = self.module_of(cx.group, r.script); // Sly 3 script hook
        let slot = w.create(r.class, oid, rec, module, local, cx.parent, persist, degenerate);
        let i = slot as usize;
        if self.load_world.len() <= i {
            self.load_world.resize(i + 1, Affine::IDENTITY);
            self.level.objects.resize(i + 1, Sly3Object::default());
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
        self.level.objects[i] = Sly3Object {
            id: r.id,
            group: cx.group as u16,
            copy: cx.copy,
            script: r.script,
            pose: (pose_start, self.level.pose.len() as u32),
            bone,
            bone_local,
        };

        if let Some(list) = r.options() {
            self.apply(w, slot, list);
        }
        if cx.held || off_world {
            w.flags[i] &= !flags::IN_WORLD;
        }

        match &r.body {
            Body::Solid(s) => {
                let child = Cx { parent: slot, enclosing: &own, enclosing_slot: slot, ..cx };
                self.list(w, &s.children, child)?;
            }
            Body::Placement(p) => self.placement(w, slot, world, p, cx)?,
            Body::Other { layout, header, children, .. } if !children.is_empty() => {
                let frame = [world];
                let held = cx.held || *layout == Layout::Grouper;
                let child = match header {
                    Some(_) => Cx { parent: slot, owner: world, enclosing: &frame, enclosing_slot: slot, held, ..cx },
                    None => Cx { parent: slot, held, ..cx },
                };
                self.list(w, children, child)?;
            }
            _ => {}
        }
        if off_world {
            self.level.stats.off_world += 1;
        } else if w.flags[i] & flags::IN_WORLD == 0 {
            self.level.stats.removed_at_start += 1;
        }
        Ok(())
    }

    fn placement(
        &mut self,
        w: &mut World,
        slot: u32,
        world: Affine,
        p: &'b formats_sly3::stream::Placement,
        cx: Cx<'_>,
    ) -> Result<(), LoadError> {
        if p.mode == 2 {
            self.level.stats.spawners += 1;
            for b in p.binds.iter().filter(|b| b.target == BIND_SELF) {
                self.apply(w, slot, &b.options);
            }
            if cx.parent == NONE || !matches!(w.class_of_slot(cx.parent), CREW_FIRST..=CREW_LAST) {
                self.spawn_groups.insert(p.group_id);
            }
            // Sly 3 hook, stand-in (a working choice [I]; the specs do not say how Sly 3 makes a gadget's objects):
            // a core crew member's mode-2 spawner gets one copy of its group, out of the world, which the session uses
            // as that member's gadget object. Basis: v_gauntlet's Bentley spawner 623 is the only source of
            // `bentley_mine` (class 114, his bomb) in the world, and the bomb door needs one (missions.md 4.1a).
            if cx.parent != NONE
                && matches!(w.class_of_slot(cx.parent), CREW_FIRST..=CREW_LAST)
                && cx.depth < self.max_depth
                && let Some(g) = self.by_id.get(&p.group_id).copied()
            {
                let first = w.slot_count() as u32;
                self.group(w, g, slot, world, cx.depth + 1)?;
                for s in first..w.slot_count() as u32 {
                    w.flags[s as usize] &= !flags::IN_WORLD;
                }
            }
            return Ok(());
        }
        let first = w.slot_count() as u32;
        let copy = match self.by_id.get(&p.group_id).copied() {
            Some(_) if cx.depth >= self.max_depth => {
                self.level.stats.depth_cut += 1;
                self.new_copy()
            }
            Some(g) => {
                let n = self.instances.entry(p.group_id).or_insert(0);
                *n += 1;
                if *n > self.level.copy_counts.get(&p.group_id).copied().unwrap_or(0) {
                    self.level.stats.over_copy_count += 1;
                }
                self.group(w, g, slot, world, cx.depth + 1)?
            }
            None => {
                self.level.stats.unresolved_placements += 1;
                self.new_copy()
            }
        };
        if p.fresh {
            self.level.stats.fresh += 1;
        } else {
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
            self.list(w, &p.objects, extra)?;
        }
        let end = w.slot_count() as u32;
        for b in &p.binds {
            let target = match b.target {
                BIND_SELF => Some(slot),
                BIND_FIRST => (first < end).then_some(first),
                id => (first..end).find(|&s| {
                    let o = &self.level.objects[s as usize];
                    o.copy == copy && o.id == id
                }),
            };
            match target {
                Some(t) => {
                    self.apply(w, t, &b.options);
                    // Sly 3 script hook (classes/placement.md 1, a proxy override): a bind with a module index
                    // attaches its target to that module of the placement's own group; the target's earlier module
                    // becomes the new one's outer module. Such objects keep the module's frame (no clone).
                    let m = if b.index >= 0 { self.module_of(cx.group, b.index) } else { -1 };
                    let old = w.module[t as usize];
                    if m >= 0 && old != m {
                        if old >= 0 {
                            w.outer_module.insert(t, old);
                        }
                        w.module[t as usize] = m;
                        self.level.bound.insert(t);
                    }
                }
                None => self.level.stats.unmatched_binds += 1,
            }
        }
        Ok(())
    }
}
