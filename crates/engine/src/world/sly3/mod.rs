//! Sly 3 worlds in the engine's object model (specs/sly3: world-file.md, object-stream.md, transforms.md,
//! classes/placement.md), built the way [`crate::world::sly2`] builds Sly 2 worlds and sharing its hooks: the
//! `World::sly3` slot holds what is Sly 3 only, and every place where the engine applies Sly 1 option meanings skips
//! Sly 2 and Sly 3 worlds alike.
//!
//! [`World::load_sly3`] parses a decompressed world with `formats-sly3` and creates one object per loaded record in
//! the same columns as the other games: class, ID, record, parent and sibling links, local and world transforms,
//! flags and options. [`Sly3Level`] (`World::sly3`) keeps the parsed groups, the `s32` object IDs, each object's group
//! and copy, its transform list as loaded, its bone attachment and the actions of its option lists.
//!
//! # What becomes an object (rules in [`load`])
//!
//! - **Groups.** Shared groups (world-file.md 3.2) are created where they are stored, in group order. Every other
//!   group is a template, created once per placement (class 8) that names it by group id, under the placement
//!   object, up to [`Sly3Options::max_depth`] placements deep. Mode-2 placements are spawners (gameplay creates them
//!   later): only the placement object is made. A placement naming a group the world does not hold places nothing.
//! - **Copy and fresh placements** (classes/placement.md 1). A copy placement's extra objects are created with the
//!   copy, under the placement; a fresh placement's data is skipped, as the game skips it. Copy counts after option
//!   set 0 are kept ([`Sly3Level::copy_counts`]); placements past a group's count are counted, not refused.
//! - **Selection** (world-file.md 6). A mode-1 record whose world filter or conditions fail in the load context is
//!   not created, with its subtree. The context is the world slot and option set 0's condition unless given.
//! - **Modes 2 and 3** (object-stream.md 2.3, Sly 2's kinds −2 and −1): a record is not created when an object with
//!   its ID already exists in any copy of the same group (2) or in the same copy (3).
//! - **Options** decode with the executable's option table ([`options`]); values go to the object's `OptionStore`,
//!   typed argument lists to [`Sly3Level::actions`]. Option 27 takes the object out of the world. A placement's
//!   binds apply to the placement (target 7), the copy's first object (8) or the object of the copy with the target
//!   ID. Grouper (class 29) objects are created out of the world (gameplay adds them).
//! - **Transforms** follow transforms.md 1 as for Sly 2: −1 the root of the copy, `n` own, `0x1000 | n` the
//!   enclosing solid's transform `n`, `0x2000` the owner of the list (a copy placement for its extras, a zone or
//!   grouper for its objects). Local transforms are the world matrices relative to the parent object's.
//! - **Not objects:** effect components (they belong to their effect) and the records inside fresh placements' data.
//!
//! No behaviours, drawables or scripts are attached (the Sly 3 scripts spec is not written; each object's script
//! index is kept in [`Sly3Object::script`]).

pub mod load;
pub mod options;

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use formats::geometry::Affine;
use formats_sly2::exe::OptionTable;
use formats_sly3::filter::Context;
use formats_sly3::options::OptionList;
use formats_sly3::stream::Record;
use formats_sly3::world::{GroupBody, GroupEntry, KeyEntry, OptionSets, PartitionSet, Sections, WideString};

use super::{World, WorldInfo};
use crate::object::{Arg, Handle};

pub use load::LoadStats;

/// How a Sly 3 world is loaded.
#[derive(Clone, Debug)]
pub struct Sly3Options {
    /// The world slot (selects world filters); `None` matches every filter.
    pub slot: Option<i16>,
    /// The selection context; `None`: the slot with option set 0's condition (a fresh load).
    pub context: Option<Context>,
    /// Sly 3 hook: the option set chosen (its copy counts apply; conditions.md); 0 for a fresh load.
    pub set: usize,
    /// Which world this is.
    pub info: WorldInfo,
    /// Deepest chain of placements followed.
    pub max_depth: u32,
    /// The packed-tangent multipliers of the executable (animation.md 2.1).
    pub multipliers: [f32; 3],
}

impl Default for Sly3Options {
    fn default() -> Self {
        Sly3Options {
            slot: None,
            context: None,
            set: 0,
            info: WorldInfo::default(),
            max_depth: 8,
            multipliers: formats_sly3::anim::DEFAULT_MULTIPLIERS,
        }
    }
}

/// The Sly 3 data of one object.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sly3Object {
    /// The record's `s32` object ID.
    pub id: i32,
    /// The group the record belongs to.
    pub group: u16,
    /// The copy (group instance) the object belongs to: one per shared group and one per placement.
    pub copy: u32,
    /// The record's script index in its group's script block (−1 none).
    pub script: i16,
    /// The object's transform list, relative to its object transform: a range of [`Sly3Level::pose`].
    pub pose: (u32, u32),
    /// The enclosing solid's slot and transform this object hangs from (parent reference `0x1000 | n`).
    pub bone: Option<(u32, u16)>,
    /// The object's matrix in the frame of that bone, as loaded.
    pub bone_local: Affine,
}

impl Default for Sly3Object {
    fn default() -> Self {
        Sly3Object { id: -1, group: 0, copy: 0, script: -1, pose: (0, 0), bone: None, bone_local: Affine::IDENTITY }
    }
}

/// Where a record is: its group and the path of indices from the group's object list through `Record::nested`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RecordKey {
    group: u16,
    start: u32,
    len: u16,
}

/// The Sly 3 part of a loaded world.
pub struct Sly3Level {
    pub keys: [Vec<KeyEntry>; 2],
    pub groups: Vec<GroupEntry>,
    pub sections: Sections,
    pub strings: Vec<WideString>,
    pub world_list: OptionList,
    pub option_sets: OptionSets,
    pub partitions: Vec<PartitionSet>,
    /// The group bodies with their parsed object lists.
    pub bodies: Vec<GroupBody>,
    /// Bytes after the last group body.
    pub tail: usize,
    /// The executable's option table the world was read with.
    pub table: OptionTable,
    /// The selection the world was loaded with.
    pub context: Context,
    /// Copy counts by group id after option set 0.
    pub copy_counts: BTreeMap<i32, i32>,
    /// What the loader did.
    pub stats: LoadStats,
    /// Actions of the option lists, in load order: (slot, option ID, arguments).
    pub actions: Vec<(u32, u16, Box<[Arg]>)>,
    /// Transform lists relative to their object's transform (see [`Sly3Object::pose`]).
    pub pose: Vec<Affine>,
    /// Sly 3 host hook: objects the game provides to scripts (operations 124, 125, 126 and 145), made on first use:
    /// (operation, slot). Used by `game::script_host::sly3`.
    pub game_objects: BTreeMap<u32, u32>,
    /// Sly 3 host hook: pickpocket items marked collected (operations 146 and 147).
    pub pickpocket: alloc::collections::BTreeSet<i32>,
    /// Sly 3 pose hook: the crew's clips applied to their poses (`anim::sly3_pose`).
    pub poser: crate::anim::sly3_pose::Sly3Poser,
    /// Sly 3 spawn hook: the pooled copies of the groups spawn points name, by group ID (copy numbers), for
    /// [`World::sly3_acquire`].
    pub pools: BTreeMap<i32, Vec<u32>>,
    /// Sly 3 spawn hook: each pooled copy's top-level objects' transforms as loaded (relative to the group's origin).
    pub pool_origins: BTreeMap<u32, Affine>,
    /// Sly 3 script hook: the objects a placement's bind gave a module (classes/placement.md 1), which keep that
    /// module's frame.
    pub bound: alloc::collections::BTreeSet<u32>,
    objects: Vec<Sly3Object>,
    /// Slots by object ID (built at load; a destroyed object leaves it), for the ID searches.
    ids: alloc::collections::BTreeMap<i32, Vec<u32>>,
    records: Vec<RecordKey>,
    paths: Vec<u32>,
}

impl Sly3Level {
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

    /// The Sly 3 data of a slot.
    pub fn object(&self, slot: u32) -> Option<&Sly3Object> {
        self.objects.get(slot as usize)
    }

    /// Number of distinct records objects were made from (the values of `World::record`).
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Distinct record `i` (the value `World::record` gives for a Sly 3 object).
    pub fn record(&self, i: u32) -> Option<&Record> {
        let k = self.records.get(i as usize)?;
        let path = &self.paths[k.start as usize..k.start as usize + k.len as usize];
        let (&first, rest) = path.split_first()?;
        let mut r = self.bodies.get(k.group as usize)?.objects.get(first as usize)?;
        for &p in rest {
            r = r.nested().get(p as usize)?;
        }
        Some(r)
    }

    /// The group of distinct record `i`.
    pub fn record_group(&self, i: u32) -> Option<usize> {
        self.records.get(i as usize).map(|k| k.group as usize)
    }

    /// The transform list of a slot as loaded, relative to its object transform.
    pub fn pose_of(&self, slot: u32) -> &[Affine] {
        match self.objects.get(slot as usize) {
            Some(o) => &self.pose[o.pose.0 as usize..o.pose.1 as usize],
            None => &[],
        }
    }
}

impl World {
    /// The Sly 3 data of an object (`None` for another game's world or a stale handle).
    pub fn sly3_object(&self, h: Handle) -> Option<&Sly3Object> {
        let s = self.slot(h)?;
        self.sly3.as_deref()?.object(s)
    }

    /// The `s32` object ID of a Sly 3 object.
    pub fn sly3_id(&self, h: Handle) -> Option<i32> {
        self.sly3_object(h).map(|o| o.id)
    }

    /// Sly 3 hook (missions.md 4.1a): a hit record (class 196) of `hitter` hitting `target`, for event 12's
    /// handlers: scripts read the object hit as script ID 556 and the hitter as 276. It has no record and stays out of
    /// the world.
    pub fn sly3_hit_record(&mut self, target: Handle, hitter: Handle) -> Option<Handle> {
        const HIT_CLASS: i16 = 196;
        let slot =
            self.create(HIT_CLASS, -1, u32::MAX, -1, formats::geometry::Affine::IDENTITY, crate::world::NONE, 0, 0);
        let h = self.handle(slot)?;
        self.set_in_world(h, false);
        self.shared.sly3_hits.insert(slot, (target, hitter));
        Some(h)
    }

    /// Sly 3 script hook (scripts.md 7.4): each object's copy ordinal within its group, for
    /// `script::Interpreter::attach_copy`: the group's first copy in load order is 0 (it keeps the module frame of
    /// the block load), every further copy has its own number and so its own clone of the frame. The guard pool's
    /// copies are cloned here at load, standing in for the clone the spec makes at each lease (825, `0x6F`); a
    /// copy taken again keeps the frame it had.
    pub fn sly3_copy_ordinals(&self) -> BTreeMap<u32, u32> {
        self.sly3_copy_ordinals_with(false)
    }

    /// [`World::sly3_copy_ordinals`], with the objects placement binds gave a module too (`bound`): their copy
    /// still clones the earlier module's frame, which the override module's frame continues in.
    pub fn sly3_copy_ordinals_with(&self, bound: bool) -> BTreeMap<u32, u32> {
        let mut out = BTreeMap::new();
        let Some(l) = self.sly3.as_deref() else { return out };
        let mut seen: BTreeMap<u16, Vec<u32>> = BTreeMap::new();
        for (slot, o) in l.objects.iter().enumerate() {
            let copies = seen.entry(o.group).or_default();
            let n = copies.iter().position(|&c| c == o.copy).unwrap_or_else(|| {
                copies.push(o.copy);
                copies.len() - 1
            });
            if bound || !l.bound.contains(&(slot as u32)) {
                out.insert(slot as u32, n as u32);
            }
        }
        out
    }

    /// Sly 3 spawn hook (missions.md 4.1a, script ID 825): takes a new instance of placement `placement` (class 8):
    /// a free pooled copy of its group (none of its top-level objects in the world), placed at the placement, its
    /// behaviours made afresh and its top-level objects brought into the world. Returns the copy's root, or None when
    /// the pool has none free.
    pub fn sly3_acquire(&mut self, placement: Handle) -> Option<Handle> {
        let at = self.world_transform(placement)?;
        let l = self.sly3.as_deref()?;
        let formats_sly3::stream::Body::Placement(p) = &l.record(self.record(placement)?)?.body else { return None };
        let pool = l.pools.get(&p.group_id)?;
        let slots_of = |copy: u32| -> Vec<u32> {
            (0..self.slot_count() as u32)
                .filter(|&s| self.live[s as usize] && l.object(s).is_some_and(|o| o.copy == copy))
                .collect()
        };
        let (slots, roots) = pool.iter().find_map(|&copy| {
            let slots = slots_of(copy);
            let roots: Vec<u32> = slots.iter().copied().filter(|&s| self.parent_of_slot(s) == super::NONE).collect();
            let free = !roots.is_empty() && roots.iter().all(|&s| self.flags[s as usize] & super::flags::IN_WORLD == 0);
            free.then_some((slots, roots))
        })?;
        for &s in &roots {
            let Some(h) = self.handle(s) else { continue };
            let origin = self.sly3.as_deref().and_then(|l| l.pool_origins.get(&s).copied()).unwrap_or(Affine::IDENTITY);
            self.set_world_transform(h, at.mul(&origin));
            self.set_in_world(h, true);
        }
        // The behaviours are made once the copy stands at the placement (a guard keeps its start as its home).
        self.update_transforms();
        for &s in &slots {
            let Some(h) = self.handle(s) else { continue };
            let b = crate::behavior::sly3::for_class(self, h);
            self.set_behavior(h, b);
        }
        self.handle(*roots.first()?)
    }

    /// Frees every hit record made so far ([`World::sly3_hit_record`]): called before a new blast makes its own, once
    /// the last blast's event-12 handlers have long run.
    pub fn sly3_free_hit_records(&mut self) {
        for slot in core::mem::take(&mut self.shared.sly3_hits).into_keys() {
            if let Some(h) = self.handle(slot) {
                self.destroy(h);
            }
        }
    }

    /// The record a Sly 3 object was made from.
    pub fn sly3_record(&self, h: Handle) -> Option<&Record> {
        let r = self.record(h)?;
        self.sly3.as_deref()?.record(r)
    }

    /// Every live object with Sly 3 object ID `id`, in load order.
    pub fn slots_with_sly3_id(&self, id: i32) -> Vec<u32> {
        self.sly3_id_slots(id).collect()
    }

    /// The live slots with Sly 3 object ID `id`, in load order, from the ID index (no allocation).
    pub fn sly3_id_slots(&self, id: i32) -> impl Iterator<Item = u32> + '_ {
        let slots = self.sly3.as_deref().and_then(|l| l.ids.get(&id)).map_or(&[][..], |v| v.as_slice());
        slots.iter().copied().filter(|&s| self.live[s as usize])
    }

    /// The first object with Sly 3 ID `id` in the same copy as `context` (references are by ID within the owning
    /// group or copy, object-stream.md 4), else the first in the world.
    pub fn find_sly3_id(&self, id: i32, context: Option<Handle>) -> Option<Handle> {
        let l = self.sly3.as_deref()?;
        let copy = context.and_then(|h| self.sly3_object(h)).map(|o| o.copy);
        let pick = copy
            .and_then(|c| self.sly3_id_slots(id).find(|&s| l.objects[s as usize].copy == c))
            .or_else(|| self.sly3_id_slots(id).next())?;
        self.handle(pick)
    }

    /// Moves a bone-attached Sly 3 object with its bone (as [`World::sly2_follow_bone`]).
    pub fn sly3_follow_bone(&mut self, h: Handle, bone: &Affine) {
        let Some(o) = self.sly3_object(h).copied() else { return };
        if o.bone.is_some() {
            self.set_local(h, bone.mul(&o.bone_local));
        }
    }
}
