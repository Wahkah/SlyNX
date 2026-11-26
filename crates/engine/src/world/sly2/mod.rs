//! Sly 2 worlds in the engine's object model (specs/sly2: object-stream.md, world-file.md, transforms.md,
//! classes/placement.md).
//!
//! [`World::load_sly2`] parses a decompressed Sly 2 world with `formats-sly2` and creates one object per loaded
//! record in the same columns as a Sly 1 world: class, ID, record, script module, parent and sibling links, local and
//! world transforms, flags and options. What is Sly 2 only is kept in [`Sly2Level`] (`World::sly2`): the parsed
//! packages, the `s32` object IDs, each object's package and copy, its transform list as loaded, its bone
//! attachment, and the actions of its option lists.
//!
//! # What becomes an object (rules in [`load`])
//!
//! - **Packages.** Packages whose keep flag is set are created where they are stored, in package order. Every other
//!   package is a template: it is created once per placement (classes 6 and 7) that names it, under the placement
//!   object, recursively up to [`Sly2Options::max_depth`] placements deep. A placement naming a package the world
//!   does not hold places nothing (classes/placement.md 1.3).
//! - **Records.** Every record of a created list becomes an object, nested records included (solid children,
//!   copy-placement extras, explosion-group parts, track and state-machine lists), under the object whose body holds
//!   them.
//! - **Level conditions** (object-stream.md 2.3). A record with conditions that exclude the level row is not created,
//!   with its subtree; classes 132 and 133 are created anyway, out of the world (inactive).
//! - **Clone kinds.** A record of kind −1 is not created when an object with its ID already exists in the same copy
//!   (one placement's instance of a package); kind −2 when one exists in any copy of the same package.
//! - **Options** (object-stream.md 1) are decoded with the executable's option table ([`options`]). Values are
//!   stored in the object's `OptionStore`; typed argument lists (actions) are kept in [`Sly2Level::actions`].
//!   Option 26 (remove at start) takes the object out of the world, and so does a placement's root patch or block
//!   carrying it. A fresh placement's option block (kind 5) applies to the placement object; a copy placement's
//!   patches apply to the copy's root (targets 5 and 6: the placement object) or to the first object of the copy
//!   with the target ID.
//! - **Transforms** (transforms.md 1). Every object's world matrix as loaded is computed with the reference rules:
//!   −1 the root of its copy (the placement matrix, or the identity for kept packages), `n` its own transform,
//!   `0x1000 | n` the enclosing solid's transform `n`, `0x2000` the owner of the list being read (the explosion group
//!   for its parts, the copy placement for its extras). A solid's object transform is its transform 0; the whole
//!   list is kept relative to it ([`Sly2Level::pose`]). The local transform is the world matrix made relative to the
//!   parent object's, so `World::update_transforms` reproduces the loaded world matrices.
//! - **Bones.** An object whose parent reference is `0x1000 | n` hangs from the enclosing solid's transform `n`;
//!   [`Sly2Object::bone`] records which, with the object's matrix in that bone's frame, so an animation system can
//!   move it with the bone ([`World::sly2_follow_bone`]).
//!
//! No behaviours are attached (every object is data until Sly 2 behaviours exist), no drawables are registered (the
//! renderer draws Sly 2 worlds from `formats_sly2::scene`), and Sly 1 option meanings (generic option effects, live
//! script options, option 2) are not applied. Scripts: [`program`] builds the Sly 2 program and
//! [`configure_interpreter`] gives the interpreter the executable's tables; each object's module is its record's
//! script index in its own package (`Program::package_module`).

pub mod load;
pub mod options;

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use alloc::vec::Vec;

use formats::geometry::Affine;
use formats_sly2::exe::OptionTable;
use formats_sly2::options::OptionList;
use formats_sly2::stream::Record;
use formats_sly2::world::{HeaderStart, PackageBody, TreeNode};
use script::{BuiltinTable, Interpreter, ParseError, Program};

use super::{World, WorldInfo};
use crate::object::{Arg, Handle};

pub use load::LoadStats;

/// How a Sly 2 world is loaded.
#[derive(Clone, Debug)]
pub struct Sly2Options {
    /// The level row the world is loaded as (selects conditional records); `None` loads every record.
    pub row: Option<i16>,
    /// Which world this is.
    pub info: WorldInfo,
    /// Deepest chain of placements followed.
    pub max_depth: u32,
    /// The executable's built-in table, for the scripts (`BuiltinTable::from_executable`).
    pub builtins: Option<BuiltinTable>,
}

impl Default for Sly2Options {
    fn default() -> Self {
        Sly2Options { row: None, info: WorldInfo::default(), max_depth: 8, builtins: None }
    }
}

/// The Sly 2 data of one object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sly2Object {
    /// The record's `s32` object ID.
    pub id: i32,
    /// The package the record belongs to.
    pub package: u16,
    /// The copy (package instance) the object belongs to: one per kept package and one per placement.
    pub copy: u32,
    /// The object's transform list, relative to its object transform: a range of [`Sly2Level::pose`].
    pub pose: (u32, u32),
    /// The enclosing solid's slot and transform this object hangs from (parent reference `0x1000 | n`).
    pub bone: Option<(u32, u16)>,
    /// The object's matrix in the frame of that bone, as loaded.
    pub bone_local: Affine,
}

impl Default for Sly2Object {
    fn default() -> Self {
        Sly2Object { id: -1, package: 0, copy: 0, pose: (0, 0), bone: None, bone_local: Affine::IDENTITY }
    }
}

/// Where a record is: its package and the path of indices from the package's object list through `Record::nested`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RecordKey {
    package: u16,
    start: u32,
    len: u16,
}

/// The Sly 2 part of a loaded world.
pub struct Sly2Level {
    /// The world header (packages, level rows, sections).
    pub header: HeaderStart,
    /// String count of the string table.
    pub strings: usize,
    /// The partition tree.
    pub tree: Vec<TreeNode>,
    /// The two world-level option lists (decoded into `World::world_options`).
    pub world_lists: [OptionList; 2],
    /// The package bodies with their parsed object lists.
    pub bodies: Vec<PackageBody>,
    /// Bytes after the last package body.
    pub tail: usize,
    /// The executable's option table the world was read with.
    pub table: OptionTable,
    /// The level row the world was loaded as.
    pub row: Option<i16>,
    /// The executable's built-in table, for the scripts.
    pub builtins: Option<BuiltinTable>,
    /// What the loader did.
    pub stats: LoadStats,
    /// Actions of the option lists, in load order: (slot, option ID, arguments). Their meanings are per class and
    /// not specified yet; option 26 is applied at load.
    pub actions: Vec<(u32, u16, Box<[Arg]>)>,
    /// Transform lists relative to their object's transform (see [`Sly2Object::pose`]).
    pub pose: Vec<Affine>,
    /// Objects the game itself provides to scripts (operations 118 and 137), made on first use: (operation, slot).
    pub game_objects: alloc::collections::BTreeMap<u32, u32>,
    /// The current job, current task and checkpoint task (operations 100 to 102; missions.md), as slots: set by
    /// the job system, `None` (the null object to scripts) until it does.
    pub job: [Option<u32>; 3],
    /// The current difficulty help, 0 to 1 (operation 103; missions.md 6).
    pub difficulty_help: f32,
    /// The job and progression system with the live progress block (`game::sly2_jobs`), once attached.
    pub jobs: Option<Box<crate::game::sly2_jobs::Jobs>>,
    /// State-machine and clip playbacks (`game::sly2_anim`).
    pub anim: crate::game::sly2_anim::Sly2Anim,
    /// Sly 2 hook: the clips applied to the objects' poses (`anim::sly2_pose`).
    pub poser: crate::anim::sly2_pose::Sly2Poser,
    /// What the HUD shows (`game::sly2_hud`).
    pub hud: crate::game::sly2_hud::Sly2Hud,
    /// Task events raised for the job system, oldest first (operation 139 raises 59), with the object each concerns
    /// (missions.md 4.3: a volume's 26 and 27 carry the volume).
    pub task_events: Vec<(u32, Option<Handle>)>,
    /// Slots whose module a placement's patch bound (scripts.md 1: those share the module's frame, not a copy's).
    pub bound: alloc::collections::BTreeSet<u32>,
    /// Noise objects made by option 1095: (slot, game time it expires at; negative never).
    pub noise_objects: Vec<(u32, f32)>,
    objects: Vec<Sly2Object>,
    /// Slots by object ID (built at load; a destroyed object leaves it), for the ID searches.
    ids: alloc::collections::BTreeMap<i32, Vec<u32>>,
    records: Vec<RecordKey>,
    paths: Vec<u32>,
}

impl Sly2Level {
    /// Builds the ID index from the loaded objects.
    pub(crate) fn build_ids(&mut self) {
        self.ids.clear();
        for (s, o) in self.objects.iter().enumerate() {
            if o.id != -1 {
                self.ids.entry(o.id).or_default().push(s as u32);
            }
        }
    }

    /// A destroyed object leaves the index (and its slot's ID is cleared, so a run-time object that reuses the
    /// slot has none).
    pub(crate) fn forget(&mut self, slot: u32) {
        let Some(o) = self.objects.get_mut(slot as usize) else { return };
        if let Some(v) = self.ids.get_mut(&o.id) {
            v.retain(|&s| s != slot);
        }
        o.id = -1;
    }

    /// The Sly 2 data of a slot.
    pub fn object(&self, slot: u32) -> Option<&Sly2Object> {
        self.objects.get(slot as usize)
    }

    /// Number of distinct records objects were made from (the values of `World::record`).
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Distinct record `i` (the value `World::record` gives for a Sly 2 object).
    pub fn record(&self, i: u32) -> Option<&Record> {
        let k = self.records.get(i as usize)?;
        let path = &self.paths[k.start as usize..k.start as usize + k.len as usize];
        let (&first, rest) = path.split_first()?;
        let mut r = self.bodies.get(k.package as usize)?.objects.get(first as usize)?;
        for &p in rest {
            r = r.nested().get(p as usize)?;
        }
        Some(r)
    }

    /// The package of distinct record `i`.
    pub fn record_package(&self, i: u32) -> Option<usize> {
        self.records.get(i as usize).map(|k| k.package as usize)
    }

    /// The transform list of a slot as loaded, relative to its object transform.
    pub fn pose_of(&self, slot: u32) -> &[Affine] {
        match self.objects.get(slot as usize) {
            Some(o) => &self.pose[o.pose.0 as usize..o.pose.1 as usize],
            None => &[],
        }
    }

    /// Runs `f` on a `formats_sly2::World` view of the level over `data` (the world bytes, `World::level.bytes`).
    pub fn with_view<R>(&mut self, data: &[u8], f: impl FnOnce(&formats_sly2::World<'_>) -> R) -> R {
        let view = formats_sly2::World {
            data,
            header: core::mem::take(&mut self.header),
            strings: self.strings,
            tree: core::mem::take(&mut self.tree),
            options: core::mem::take(&mut self.world_lists),
            bodies: core::mem::take(&mut self.bodies),
            tail: self.tail,
            errors: Vec::new(),
            object_bodies: 0,
            undecided: Vec::new(),
        };
        let out = f(&view);
        self.header = view.header;
        self.tree = view.tree;
        self.world_lists = view.options;
        self.bodies = view.bodies;
        out
    }
}

impl World {
    /// The Sly 2 data of an object (`None` for a Sly 1 world or a stale handle).
    pub fn sly2_object(&self, h: Handle) -> Option<&Sly2Object> {
        let s = self.slot(h)?;
        self.sly2.as_deref()?.object(s)
    }

    /// The `s32` object ID of a Sly 2 object.
    pub fn sly2_id(&self, h: Handle) -> Option<i32> {
        self.sly2_object(h).map(|o| o.id)
    }

    /// A Sly 2 object's zone mask (its transform header's, `0xFF` without one; camera.md 7.1). Object-model hook:
    /// activation by visibility volume.
    pub fn sly2_zone_mask(&self, h: Handle) -> u32 {
        match self.sly2_record(h).map(|r| &r.body) {
            Some(formats_sly2::stream::Body::Solid(s)) => s.zone_mask,
            Some(formats_sly2::stream::Body::Placement(p)) => p.zone_mask,
            Some(formats_sly2::stream::Body::Other { header: Some(h), .. }) => h.zone_mask,
            _ => 0xFF,
        }
    }

    /// The record a Sly 2 object was made from.
    pub fn sly2_record(&self, h: Handle) -> Option<&Record> {
        let r = self.record(h)?;
        self.sly2.as_deref()?.record(r)
    }

    /// Every live object with Sly 2 object ID `id`, in load order.
    pub fn slots_with_sly2_id(&self, id: i32) -> Vec<u32> {
        self.sly2_id_slots(id).collect()
    }

    /// The live slots with Sly 2 object ID `id`, in load order, from the ID index (no allocation).
    pub fn sly2_id_slots(&self, id: i32) -> impl Iterator<Item = u32> + '_ {
        let slots = self.sly2.as_deref().and_then(|l| l.ids.get(&id)).map_or(&[][..], |v| v.as_slice());
        slots.iter().copied().filter(|&s| self.live[s as usize])
    }

    /// The first object with Sly 2 ID `id` in the same copy as `context` (references between objects are by ID
    /// within the owning package or copy, object-stream.md 4), else the first in the world.
    pub fn find_sly2_id(&self, id: i32, context: Option<Handle>) -> Option<Handle> {
        if self.sly2.is_none() {
            return self.find_sly3_id(id, context); // Sly 3 hook: the Sly 2 behaviours run on Sly 3 worlds
        }
        let l = self.sly2.as_deref()?;
        let copy = context.and_then(|h| self.sly2_object(h)).map(|o| o.copy);
        let pick = copy
            .and_then(|c| self.sly2_id_slots(id).find(|&s| l.objects[s as usize].copy == c))
            .or_else(|| self.sly2_id_slots(id).next())?;
        self.handle(pick)
    }

    /// Moves a bone-attached Sly 2 object with its bone: sets its local transform from the bone's matrix `bone`
    /// (relative to the enclosing object's transform, as an animated pose gives it) and the object's loaded matrix in
    /// the bone's frame. Does nothing for an object without a bone.
    pub fn sly2_follow_bone(&mut self, h: Handle, bone: &Affine) {
        let Some(o) = self.sly2_object(h).copied() else { return };
        if o.bone.is_some() {
            self.set_local(h, bone.mul(&o.bone_local));
        }
    }
}

/// The Sly 2 classes with descendants and the end of each one's range (object-stream.md 3.1): class C covers
/// C ≤ c < end. Every other class is a leaf and covers only itself.
const CLASS_RANGES: [(i16, i16); 32] = [
    (0, 199),
    (2, 134),
    (3, 48),
    (6, 8),
    (8, 10),
    (10, 15),
    (18, 20),
    (22, 28),
    (24, 26),
    (30, 34),
    (32, 34),
    (35, 38),
    (48, 109),
    (50, 79),
    (51, 69),
    (52, 56),
    (56, 68),
    (58, 60),
    (71, 77),
    (79, 83),
    (80, 83),
    (84, 86),
    (90, 96),
    (100, 103),
    (109, 111),
    (126, 132),
    (138, 145),
    (148, 151),
    (155, 199),
    (157, 164),
    (160, 164),
    (166, 169),
];

/// The Sly 2 class test (object-stream.md 3.1): whether class `class` is `of` or derived from it.
pub fn is_a(class: i16, of: i32) -> bool {
    let Ok(of) = i16::try_from(of) else { return false };
    let end = CLASS_RANGES.iter().find(|r| r.0 == of).map_or(of.saturating_add(1), |r| r.1);
    (of..end).contains(&class)
}

/// The script program of a world: the Sly 2 packages' script blocks for a Sly 2 world, the Sly 3 groups' script
/// blocks for a Sly 3 world, the Sly 1 script table otherwise.
pub fn program(world: &mut World) -> Result<Program, ParseError> {
    if let Some(l) = world.sly3.as_deref() {
        return Program::from_sly3_bodies(&world.level.bytes, &l.bodies); // Sly 3 hook
    }
    match world.sly2.as_deref_mut() {
        Some(l) => l.with_view(&world.level.bytes, Program::from_sly2_world),
        None => Program::from_world(&world.level.bytes, &world.level.layout),
    }
}

/// Stream options whose typed arguments (the property ID as tag 21, then the value) give an object a custom script
/// property at load (specs/sly2/vehicles-and-minigames.md 2, the RC chopper's table): 3 a boolean, 4 an integer,
/// 5 a float, 8 an object ID (−1 none), which scripts pass to the searches by ID (112 to 115).
pub const CUSTOM_PROPERTY_OPTIONS: [u16; 4] = [3, 4, 5, 8];

/// The placement class (classes/placement.md).
const PLACEMENT_CLASS: i16 = 6;

/// One custom script property an option list declares.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CustomProperty {
    /// The object whose list declares it.
    pub object: Handle,
    /// The placement's own ID when the object is a placement (its properties go to its copy's root).
    pub placement: Option<i32>,
    pub id: u32,
    pub value: CustomValue,
}

/// A declared property's value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CustomValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    /// An object ID (−1: none), kept as the integer the scripts search for.
    Object(i32),
}

/// Each scripted object's copy ordinal within its package (scripts.md 1: copy 0 keeps the package's module frames,
/// copies 1 and up get clones), by slot: copies numbered in load order per package. Objects whose module a
/// placement bound are left out, so they keep the shared frame. Empty for other worlds.
pub fn copy_ordinals(world: &World) -> alloc::collections::BTreeMap<u32, u32> {
    use alloc::collections::BTreeMap;
    if world.sly3.is_some() {
        return world.sly3_copy_ordinals(); // Sly 3 hook (scripts.md 7.4)
    }
    let mut out = BTreeMap::new();
    let Some(l) = world.sly2.as_deref() else { return out };
    let mut seen: BTreeMap<u16, Vec<u32>> = BTreeMap::new();
    for (slot, o) in l.objects.iter().enumerate() {
        let slot = slot as u32;
        let copies = seen.entry(o.package).or_default();
        let n = match copies.iter().position(|&c| c == o.copy) {
            Some(n) => n,
            None => {
                copies.push(o.copy);
                copies.len() - 1
            }
        };
        if !l.bound.contains(&slot) {
            out.insert(slot, n as u32);
        }
    }
    out
}

/// The custom script properties a Sly 2 world's option lists declare (options 3, 4, 5 and 8), in load order; none
/// for other worlds. The aim managers' properties 1314 to 1317 are among them: without them their searches find
/// nothing and call the undefined procedure 1533.
pub fn custom_properties(world: &World) -> Vec<CustomProperty> {
    if world.sly3.is_some() {
        return sly3_custom_properties(world); // Sly 3 hook
    }
    let Some(l) = world.sly2.as_deref() else { return Vec::new() };
    let mut out = Vec::new();
    for (slot, option, args) in &l.actions {
        if !CUSTOM_PROPERTY_OPTIONS.contains(option) {
            continue;
        }
        let (Some(h), [Arg::Short(21, id), value]) = (world.handle(*slot), &args[..]) else { continue };
        let Ok(id) = u32::try_from(*id) else { continue };
        let value = match (*option, value) {
            (3, Arg::Bool(b)) => CustomValue::Bool(*b),
            (4, Arg::Int(i)) => CustomValue::Int(*i),
            (5, Arg::Float(f)) => CustomValue::Float(*f),
            (8, Arg::Int(oid)) => CustomValue::Object(*oid),
            _ => continue,
        };
        let placement = if world.class(h) == Some(PLACEMENT_CLASS) { world.sly2_id(h) } else { None };
        out.push(CustomProperty { object: h, placement, id, value });
    }
    out
}

/// Sly 3 hook (missions.md 4.2): stream option 8 is the custom-property setter for every class, a load-time method
/// (property ID as tag 21, value); the value keeps its own kind. A placement's (class 8) setters apply to its placed
/// copy's root (its first child), after the objects' own, so they override them: v_ext's placement 121540 sets
/// waypoint 35785's property 1990 (its task) to 2086 over the waypoint's own −1. Stream options 3, 4 and 5 with the
/// same shape (a tag-21 property ID, then a bool, an integer or a float) set them too: a working reading [I] by
/// shape, as Sly 2's typed setters (v_gauntlet's spawner 139785: 1979 off, 1981 at most 3 alive, 1983 every 6 s).
fn sly3_custom_properties(world: &World) -> Vec<CustomProperty> {
    let Some(l) = world.sly3.as_deref() else { return Vec::new() };
    let (mut own, mut placed) = (Vec::new(), Vec::new());
    for (slot, option, args) in &l.actions {
        if !matches!(*option, 3 | 4 | 5 | 8) {
            continue;
        }
        let (Some(h), [Arg::Short(21, id), value]) = (world.handle(*slot), &args[..]) else { continue };
        let Ok(id) = u32::try_from(*id) else { continue };
        let value = match value {
            Arg::Bool(b) => CustomValue::Bool(*b),
            Arg::Float(f) => CustomValue::Float(*f),
            Arg::Int(i) => CustomValue::Int(*i),
            Arg::Short(_, i) => CustomValue::Int(*i as i32),
            _ => continue,
        };
        if world.class(h) == Some(formats_sly3::stream::PLACEMENT) {
            if let Some(root) = world.children(h).next() {
                placed.push(CustomProperty { object: root, placement: None, id, value });
            }
        } else {
            own.push(CustomProperty { object: h, placement: None, id, value });
        }
    }
    own.extend(placed);
    own
}

/// Stores `props` in the interpreter before any script runs. A placement's properties go to its copy's root, the
/// object scripts reach through the placement's ID (scripts.md 5; inferred for these options), found with `host`'s
/// wide-ID search. Object IDs stay integers: the scripts search for them (`(op115 (op111 self 1314) self)`).
pub fn seed_custom_properties(props: &[CustomProperty], interp: &mut Interpreter, host: &mut dyn script::ScriptHost) {
    for p in props {
        let me = p.object.to_script();
        let owner = p.placement.and_then(|oid| host.find_object_id(oid, Some(me), false)).unwrap_or(me);
        let v = match p.value {
            CustomValue::Bool(b) => script::Value::Bool(b),
            CustomValue::Float(f) => script::Value::Float(f),
            CustomValue::Int(i) | CustomValue::Object(i) => script::Value::Int(i),
        };
        interp.set_custom_property(owner, p.id, v);
    }
}

/// Gives a Sly 2 or Sly 3 world's interpreter the executable's built-in and option tables (nothing for Sly 1).
/// Sly 3 passes no built-in table: its calls are checked against the spec's rules, which equal the table's.
pub fn configure_interpreter(world: &World, interp: &mut Interpreter) {
    if let Some(l) = world.sly2.as_deref() {
        interp.set_sly2_tables(l.builtins.clone(), &l.table);
    }
    if let Some(l) = world.sly3.as_deref() {
        interp.set_sly3_tables(None, &l.table); // Sly 3 hook
    }
}
