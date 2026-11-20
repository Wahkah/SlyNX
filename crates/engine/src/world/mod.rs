//! The world: every runtime object of one loaded level, stored by column.
//!
//! # Storage
//!
//! Objects live in slots. Each property is a column (`Vec`) indexed by slot: class, OID, record, parent and sibling
//! links, local and world transforms, flags, options, behaviour. A slot is reused after its object is destroyed; a
//! [`Handle`] carries the slot's generation so stale handles are refused. Loaded objects are created in stream order,
//! so slot order is load order.
//!
//! # Hierarchy and order
//!
//! Children are kept in the order they were added (object-stream.md 7.3). The world keeps a preorder of the whole
//! tree (parents before children, siblings in order), rebuilt lazily when the tree changes. It is the update order
//! (clock-and-loop.md 4), and each object's subtree is a contiguous range of it, which makes "below this object"
//! lookups and transform updates of a moved subtree cheap.
//!
//! # Transforms
//!
//! Local transforms are what objects and animations write. World transforms are recomputed for dirty subtrees only,
//! once per frame ([`World::update_transforms`]), following transforms.md: the parent chain, the ancestor rotation
//! and position override (extension bits 3 and 2), degenerate matrices replaced at load, and the "not placed"
//! sentinel position. The same pass computes which objects are active and visible, and lists the drawables whose
//! transform or visibility changed for the renderer.

mod load;
mod lookup;
pub mod queue;
pub mod shared;
pub mod sly2; // Sly 2 hook: Sly 2 worlds (world::sly2).
pub mod sly3; // Sly 3 hook: Sly 3 worlds (world::sly3).
pub mod volume;

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec;
use alloc::vec::Vec;

use formats::geometry::Affine;
use formats::options::OptionTable;
use formats::stream::{ObjectStream, Record};
use formats::world::WorldLayout;
use script::RaisedEvent;

use crate::behavior::Behavior;
use crate::object::class::CLASS_COUNT;
use crate::object::{Handle, OptionStore, OptionValue, flags};

pub use load::LoadError;
pub use queue::{AnimCommand, Notice, PhysicsCommand, Request, SoundRequest};
pub use shared::{AlertGroup, AlertLevel, Damage, Hiding, PlayerState, ScreenEffects, Shared};

/// "No slot".
pub const NONE: u32 = u32::MAX;

/// The parsed world file the objects were made from. Records stay available for data that is read on demand
/// (meshes, volumes, animation segments).
pub struct LevelData {
    /// The decompressed world file.
    pub bytes: Vec<u8>,
    /// Its outer layout.
    pub layout: WorldLayout,
    /// The walked object stream.
    pub stream: ObjectStream,
    /// The option encoding table used to read it.
    pub table: OptionTable,
}

impl LevelData {
    /// A record of the stream.
    pub fn record(&self, i: u32) -> &Record {
        &self.stream.records[i as usize]
    }
}

/// Which world of the disc this is (from the executable's world table, disc.md 4.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorldInfo {
    /// Index in the world table (0 to 45).
    pub index: u32,
    /// World id: episode in the high byte, slot in the low byte.
    pub id: u32,
    /// Task mask (bit 1 key, bit 2 vault, bit 3 sprint).
    pub task_mask: u32,
    /// The level descriptor's identity (search key), as exits name it.
    pub identity: u32,
}

/// An object that is drawn: it has a model. The renderer keeps one or more instances per drawable and updates them
/// from [`World::render_changes`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawable {
    /// The object.
    pub object: Handle,
    /// The record whose mesh set it draws.
    pub record: u32,
    /// Visible after the last transform update.
    pub visible: bool,
}

/// An object moving at a constant velocity and angular velocity (placed-object options 21 and 22), integrated
/// kinematically each game frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mover {
    /// The object's slot.
    pub slot: u32,
    /// Velocity, in the parent's space, units per second.
    pub velocity: [f32; 3],
    /// Angular velocity as a rotation vector (axis × radians per second) in the parent's space.
    pub angular: [f32; 3],
}

/// The world.
pub struct World {
    /// The world file.
    pub level: LevelData,
    /// Which world this is.
    pub info: WorldInfo,
    /// The world's own option list (world-file.md 4.4).
    pub world_options: OptionStore,

    generation: Vec<u32>,
    live: Vec<bool>,
    free: Vec<u32>,

    class: Vec<i16>,
    oid: Vec<i16>,
    record: Vec<u32>,
    module: Vec<i16>,
    persist: Vec<u64>,
    parent: Vec<u32>,
    first_child: Vec<u32>,
    last_child: Vec<u32>,
    next_sibling: Vec<u32>,
    prev_sibling: Vec<u32>,
    local: Vec<Affine>,
    world: Vec<Affine>,
    flags: Vec<u16>,
    options: Vec<OptionStore>,
    behavior: Vec<Behavior>,
    drawable: Vec<u32>,
    inbox: Vec<Vec<Notice>>,
    inbox_used: Vec<u32>,
    inbox_stamp: Vec<u32>,
    inbox_frame: u32,

    root_first: u32,
    root_last: u32,
    by_oid: BTreeMap<i16, Vec<u32>>,
    by_class: Vec<Vec<u32>>,

    order: Vec<u32>,
    order_pos: Vec<u32>,
    subtree_len: Vec<u32>,
    order_valid: bool,
    ticking: Vec<u32>,
    ticking_valid: bool,
    event1_modules: Vec<bool>,

    dirty: Vec<u32>,
    basis: BTreeMap<u32, (u32, u32)>,
    movers: Vec<Mover>,
    placed_by: BTreeMap<u32, Vec<u32>>,
    outer_module: BTreeMap<u32, i16>,
    sleep_until: BTreeMap<u32, f32>,

    drawables: Vec<Drawable>,
    render_changed: Vec<u32>,
    render_flag: Vec<bool>,

    templates: [u32; 31],
    clone_serial: u32,
    pending_actions: Vec<(u32, u16, alloc::boxed::Box<[crate::object::Arg]>)>,

    /// Script events raised by objects, waiting to be dispatched (scripts.md 6). The game raises them right after
    /// the object that raised them finishes its step; the script host also hands them to the interpreter while it
    /// runs, so events raised from inside scripts stay synchronous.
    pub events: VecDeque<RaisedEvent>,
    /// Requests for the game application.
    pub requests: Vec<Request>,
    /// Requests for the animation system.
    pub anim: Vec<AnimCommand>,
    /// One-shot sounds for the audio frame.
    pub sounds: Vec<SoundRequest>,
    /// Requests for the physics system.
    pub physics: Vec<PhysicsCommand>,
    /// State behaviours share: the player's state, damage, guard groups, music intensity, screen effects.
    pub shared: Shared,
    /// The player-controlled object, once the player exists (operation 107).
    pub player: Option<Handle>,
    /// Where player-only triggers look for the player when there is no player object (the free camera in the
    /// development build), or `None`.
    pub probe: Option<[f32; 3]>,
    /// Pickups may be collected by the probe (development aid; the player collects them in the game).
    pub probe_collects: bool,
    /// Script action calls (scripts.md 11.5) that no behaviour implements yet, by option ID: they have no effect and
    /// are counted here instead of failing (their meanings are open questions in docs/engine.md).
    pub unhandled_actions: alloc::collections::BTreeMap<u16, u32>,
    /// The object an action gives back to the script that called it (a method's result), set by the behaviour.
    pub action_result: Option<Handle>,
    /// Sly 2 hook: the Sly 2 world data when this world was loaded by [`World::load_sly2`], else `None`. Sly 1
    /// option meanings (generic option effects, option 2) are not applied to a Sly 2 world.
    pub sly2: Option<alloc::boxed::Box<sly2::Sly2Level>>,
    /// Sly 3 hook: the Sly 3 world data when this world was loaded by [`World::load_sly3`], else `None`. Like a
    /// Sly 2 world, a Sly 3 world gets no Sly 1 option meanings ([`World::sly1_meanings`]).
    pub sly3: Option<alloc::boxed::Box<sly3::Sly3Level>>,
}

/// The local transform of a record, with a degenerate matrix replaced (classes/placed-object.md 1.1).
pub(crate) fn record_local(r: &Record) -> Affine {
    r.local_transform()
}

impl World {
    /// An empty world around a parsed level (objects are added by the loader).
    pub(crate) fn empty(level: LevelData, info: WorldInfo) -> World {
        World {
            level,
            info,
            world_options: OptionStore::new(),
            generation: Vec::new(),
            live: Vec::new(),
            free: Vec::new(),
            class: Vec::new(),
            oid: Vec::new(),
            record: Vec::new(),
            module: Vec::new(),
            persist: Vec::new(),
            parent: Vec::new(),
            first_child: Vec::new(),
            last_child: Vec::new(),
            next_sibling: Vec::new(),
            prev_sibling: Vec::new(),
            local: Vec::new(),
            world: Vec::new(),
            flags: Vec::new(),
            options: Vec::new(),
            behavior: Vec::new(),
            drawable: Vec::new(),
            inbox: Vec::new(),
            inbox_used: Vec::new(),
            inbox_stamp: Vec::new(),
            inbox_frame: 0,
            root_first: NONE,
            root_last: NONE,
            by_oid: BTreeMap::new(),
            by_class: vec![Vec::new(); CLASS_COUNT],
            order: Vec::new(),
            order_pos: Vec::new(),
            subtree_len: Vec::new(),
            order_valid: false,
            ticking: Vec::new(),
            ticking_valid: false,
            event1_modules: Vec::new(),
            dirty: Vec::new(),
            basis: BTreeMap::new(),
            movers: Vec::new(),
            placed_by: BTreeMap::new(),
            outer_module: BTreeMap::new(),
            sleep_until: BTreeMap::new(),
            drawables: Vec::new(),
            render_changed: Vec::new(),
            render_flag: Vec::new(),
            templates: [NONE; 31],
            clone_serial: 0,
            pending_actions: Vec::new(),
            events: VecDeque::new(),
            requests: Vec::new(),
            anim: Vec::new(),
            sounds: Vec::new(),
            physics: Vec::new(),
            shared: Shared::default(),
            player: None,
            probe: None,
            probe_collects: false,
            unhandled_actions: alloc::collections::BTreeMap::new(),
            action_result: None,
            sly2: None, // Sly 2 hook
            sly3: None, // Sly 3 hook
        }
    }

    // ---------------------------------------------------------------- slots and handles

    /// Sly 2 and Sly 3 hook: whether Sly 1 option meanings (generic option effects, option 2, live options,
    /// option 23 animations) apply, i.e. this is a Sly 1 world.
    pub fn sly1_meanings(&self) -> bool {
        self.sly2.is_none() && self.sly3.is_none()
    }

    /// Number of slots (live or free).
    pub fn slot_count(&self) -> usize {
        self.class.len()
    }

    /// Number of live objects.
    pub fn len(&self) -> usize {
        self.class.len() - self.free.len()
    }

    /// Whether the world has no objects.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The handle of a live slot.
    pub fn handle(&self, slot: u32) -> Option<Handle> {
        let s = slot as usize;
        (s < self.live.len() && self.live[s]).then(|| Handle::new(slot, self.generation[s]))
    }

    /// The slot of a handle, if the object still exists.
    #[inline]
    pub fn slot(&self, h: Handle) -> Option<u32> {
        let s = h.index() as usize;
        (s < self.live.len() && self.live[s] && self.generation[s] == h.generation()).then_some(h.index())
    }

    /// Whether the handle names a live object.
    pub fn contains(&self, h: Handle) -> bool {
        self.slot(h).is_some()
    }

    /// Every live object, in slot order (load order for loaded objects).
    pub fn handles(&self) -> impl Iterator<Item = Handle> + '_ {
        (0..self.live.len() as u32).filter_map(|s| self.handle(s))
    }

    /// Creates an object as the last child of `parent` (`NONE`: top level).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn create(
        &mut self,
        class: i16,
        oid: i16,
        record: u32,
        module: i16,
        local: Affine,
        parent: u32,
        persist: u64,
        extra_flags: u16,
    ) -> u32 {
        let slot = if let Some(s) = self.free.pop() {
            let i = s as usize;
            self.generation[i] = self.generation[i].wrapping_add(1);
            self.live[i] = true;
            self.class[i] = class;
            self.oid[i] = oid;
            self.record[i] = record;
            self.module[i] = module;
            self.persist[i] = persist;
            self.parent[i] = NONE;
            self.first_child[i] = NONE;
            self.last_child[i] = NONE;
            self.next_sibling[i] = NONE;
            self.prev_sibling[i] = NONE;
            self.local[i] = local;
            self.world[i] = local;
            self.flags[i] = flags::IN_WORLD | flags::DIRTY | extra_flags;
            self.options[i] = OptionStore::new();
            self.behavior[i] = Behavior::None;
            self.drawable[i] = NONE;
            self.inbox[i].clear();
            s
        } else {
            let s = self.class.len() as u32;
            self.generation.push(0);
            self.live.push(true);
            self.class.push(class);
            self.oid.push(oid);
            self.record.push(record);
            self.module.push(module);
            self.persist.push(persist);
            self.parent.push(NONE);
            self.first_child.push(NONE);
            self.last_child.push(NONE);
            self.next_sibling.push(NONE);
            self.prev_sibling.push(NONE);
            self.local.push(local);
            self.world.push(local);
            self.flags.push(flags::IN_WORLD | flags::DIRTY | extra_flags);
            self.options.push(OptionStore::new());
            self.behavior.push(Behavior::None);
            self.drawable.push(NONE);
            self.inbox.push(Vec::new());
            s
        };
        self.link(slot, parent);
        self.by_oid.entry(oid).or_default().push(slot);
        if let Some(list) = self.by_class.get_mut(class as usize) {
            list.push(slot);
        }
        self.dirty.push(slot);
        self.order_valid = false;
        self.ticking_valid = false;
        slot
    }

    /// Destroys an object and its descendants (spawned objects; loaded objects are removed from the world with
    /// option 2 instead, so scripts can find them again).
    pub fn destroy(&mut self, h: Handle) {
        let Some(slot) = self.slot(h) else { return };
        let mut stack = vec![slot];
        let mut doomed = Vec::new();
        while let Some(s) = stack.pop() {
            doomed.push(s);
            let mut c = self.first_child[s as usize];
            while c != NONE {
                stack.push(c);
                c = self.next_sibling[c as usize];
            }
        }
        self.unlink(slot);
        for s in doomed {
            let i = s as usize;
            self.live[i] = false;
            // Sly 2 and Sly 3 hook: the object leaves the ID index.
            if let Some(l) = self.sly2.as_deref_mut() {
                l.forget(s);
            }
            if let Some(l) = self.sly3.as_deref_mut() {
                l.forget(s);
            }
            if let Some(v) = self.by_oid.get_mut(&self.oid[i]) {
                v.retain(|&x| x != s);
            }
            if let Some(v) = self.by_class.get_mut(self.class[i] as usize) {
                v.retain(|&x| x != s);
            }
            if self.drawable[i] != NONE {
                let d = self.drawable[i] as usize;
                self.drawables[d].visible = false;
                self.mark_render(d as u32);
            }
            self.behavior[i] = Behavior::None;
            self.basis.remove(&s);
            self.sleep_until.remove(&s);
            self.movers.retain(|m| m.slot != s);
            self.free.push(s);
        }
        self.order_valid = false;
        self.ticking_valid = false;
    }

    fn link(&mut self, slot: u32, parent: u32) {
        let i = slot as usize;
        self.parent[i] = parent;
        self.next_sibling[i] = NONE;
        let last = if parent == NONE { self.root_last } else { self.last_child[parent as usize] };
        self.prev_sibling[i] = last;
        if last == NONE {
            if parent == NONE {
                self.root_first = slot;
            } else {
                self.first_child[parent as usize] = slot;
            }
        } else {
            self.next_sibling[last as usize] = slot;
        }
        if parent == NONE {
            self.root_last = slot;
        } else {
            self.last_child[parent as usize] = slot;
        }
    }

    fn unlink(&mut self, slot: u32) {
        let i = slot as usize;
        let (p, prev, next) = (self.parent[i], self.prev_sibling[i], self.next_sibling[i]);
        if prev == NONE {
            if p == NONE {
                self.root_first = next;
            } else {
                self.first_child[p as usize] = next;
            }
        } else {
            self.next_sibling[prev as usize] = next;
        }
        if next == NONE {
            if p == NONE {
                self.root_last = prev;
            } else {
                self.last_child[p as usize] = prev;
            }
        } else {
            self.prev_sibling[next as usize] = prev;
        }
        self.parent[i] = NONE;
        self.prev_sibling[i] = NONE;
        self.next_sibling[i] = NONE;
    }

    /// Moves an object under a new parent (`None`: top level), as the last child. With `keep_world`, its local
    /// transform is changed so its world transform stays (proxies, classes/proxy.md 2).
    pub fn reparent(&mut self, h: Handle, parent: Option<Handle>, keep_world: bool) {
        let Some(slot) = self.slot(h) else { return };
        let p = match parent {
            Some(ph) => match self.slot(ph) {
                Some(p) => p,
                None => return,
            },
            None => NONE,
        };
        if p != NONE && self.is_descendant_or_self(p, slot) {
            return;
        }
        if keep_world {
            self.update_transforms();
            let w = self.world[slot as usize];
            let pw = if p == NONE { Affine::IDENTITY } else { self.world[p as usize] };
            if let Some(inv) = crate::math::affine_inverse(&pw) {
                self.local[slot as usize] = inv.mul(&w);
            }
        }
        self.unlink(slot);
        self.link(slot, p);
        self.mark_dirty(slot);
        self.order_valid = false;
        self.ticking_valid = false;
    }

    fn is_descendant_or_self(&self, mut s: u32, ancestor: u32) -> bool {
        while s != NONE {
            if s == ancestor {
                return true;
            }
            s = self.parent[s as usize];
        }
        false
    }

    // ---------------------------------------------------------------- columns

    /// Class ID.
    pub fn class(&self, h: Handle) -> Option<i16> {
        self.slot(h).map(|s| self.class[s as usize])
    }

    /// Object ID.
    pub fn oid(&self, h: Handle) -> Option<i16> {
        self.slot(h).map(|s| self.oid[s as usize])
    }

    /// The stream record the object was made from.
    pub fn record(&self, h: Handle) -> Option<u32> {
        self.slot(h).map(|s| self.record[s as usize])
    }

    /// Script module index (the record's script field, or a proxy override's), or −1.
    pub fn module(&self, h: Handle) -> i16 {
        self.slot(h).map_or(-1, |s| self.module[s as usize])
    }

    /// The module an override attached over an earlier one keeps that earlier module as its outer environment
    /// (scripts.md 11.3): the earlier module, if any.
    pub fn outer_module(&self, h: Handle) -> Option<i16> {
        self.slot(h).and_then(|s| self.outer_module.get(&s).copied())
    }

    /// A key naming the object across reloads of the same world: the record index, and for clones and spawned
    /// objects a load-order serial in the high half.
    pub fn persist_key(&self, h: Handle) -> Option<u64> {
        self.slot(h).map(|s| self.persist[s as usize])
    }

    /// The parent, or `None` at the top level.
    pub fn parent(&self, h: Handle) -> Option<Handle> {
        let p = self.parent[self.slot(h)? as usize];
        if p == NONE { None } else { self.handle(p) }
    }

    /// The children in order.
    pub fn children(&self, h: Handle) -> Children<'_> {
        let first = self.slot(h).map_or(NONE, |s| self.first_child[s as usize]);
        Children { world: self, next: first }
    }

    /// The top-level objects in order.
    pub fn roots(&self) -> Children<'_> {
        Children { world: self, next: self.root_first }
    }

    /// Flags (see [`crate::object::flags`]).
    pub fn flags(&self, h: Handle) -> u16 {
        self.slot(h).map_or(0, |s| self.flags[s as usize])
    }

    /// Whether the object is active (in the world, with every ancestor) after the last transform update.
    pub fn is_active(&self, h: Handle) -> bool {
        self.flags(h) & flags::ACTIVE != 0
    }

    /// Whether the object is visible after the last transform update.
    pub fn is_visible(&self, h: Handle) -> bool {
        self.flags(h) & flags::VISIBLE != 0
    }

    /// The local transform.
    pub fn local(&self, h: Handle) -> Option<Affine> {
        self.slot(h).map(|s| self.local[s as usize])
    }

    /// Sets the local transform (marks the subtree for a transform update).
    pub fn set_local(&mut self, h: Handle, local: Affine) {
        if let Some(s) = self.slot(h) {
            self.local[s as usize] = local;
            self.mark_dirty(s);
        }
    }

    /// Sets the local position only.
    pub fn set_position(&mut self, h: Handle, p: [f32; 3]) {
        if let Some(s) = self.slot(h) {
            self.local[s as usize].t = p;
            self.mark_dirty(s);
        }
    }

    /// Sets the local rotation only.
    pub fn set_rotation(&mut self, h: Handle, cols: [[f32; 3]; 3]) {
        if let Some(s) = self.slot(h) {
            self.local[s as usize].cols = cols;
            self.mark_dirty(s);
        }
    }

    /// The world transform after the last transform update.
    pub fn world_transform(&self, h: Handle) -> Option<Affine> {
        self.slot(h).map(|s| self.world[s as usize])
    }

    /// The world position after the last transform update.
    pub fn position(&self, h: Handle) -> Option<[f32; 3]> {
        self.slot(h).map(|s| self.world[s as usize].t)
    }

    /// Sets the world transform by changing the local one (the parent's world transform must be current).
    pub fn set_world_transform(&mut self, h: Handle, w: Affine) {
        let Some(s) = self.slot(h) else { return };
        let p = self.parent[s as usize];
        let local = if p == NONE {
            w
        } else {
            match crate::math::affine_inverse(&self.world[p as usize]) {
                Some(inv) => inv.mul(&w),
                None => return,
            }
        };
        self.local[s as usize] = local;
        self.mark_dirty(s);
    }

    /// The object's options.
    pub fn options(&self, h: Handle) -> Option<&OptionStore> {
        self.slot(h).map(|s| &self.options[s as usize])
    }

    /// The object's options, mutably (prefer [`World::set_option`], which also tells the behaviour).
    pub fn options_mut(&mut self, h: Handle) -> Option<&mut OptionStore> {
        self.slot(h).map(|s| &mut self.options[s as usize])
    }

    /// The object's behaviour.
    pub fn behavior(&self, h: Handle) -> Option<&Behavior> {
        self.slot(h).map(|s| &self.behavior[s as usize])
    }

    /// The object's behaviour, mutably.
    pub fn behavior_mut(&mut self, h: Handle) -> Option<&mut Behavior> {
        self.slot(h).map(|s| &mut self.behavior[s as usize])
    }

    /// Replaces the object's behaviour.
    pub fn set_behavior(&mut self, h: Handle, b: Behavior) {
        if let Some(s) = self.slot(h) {
            self.behavior[s as usize] = b;
            self.ticking_valid = false;
        }
    }

    pub(crate) fn take_behavior(&mut self, slot: u32) -> Behavior {
        core::mem::take(&mut self.behavior[slot as usize])
    }

    pub(crate) fn put_behavior(&mut self, slot: u32, b: Behavior) {
        if self.live[slot as usize] {
            self.behavior[slot as usize] = b;
        }
    }

    /// Delivers a notice to an object's behaviour (read on its next update). Notices nobody read during an update
    /// pass are dropped after it ([`World::clear_inboxes`]).
    pub fn notify(&mut self, h: Handle, n: Notice) {
        if let Some(s) = self.slot(h) {
            if self.inbox[s as usize].is_empty() {
                self.inbox_used.push(s);
                if self.inbox_stamp.len() <= s as usize {
                    self.inbox_stamp.resize(s as usize + 1, 0);
                }
                self.inbox_stamp[s as usize] = self.inbox_frame;
            }
            self.inbox[s as usize].push(n);
        }
    }

    /// The notices waiting for an object (without taking them).
    pub fn notices(&self, h: Handle) -> &[Notice] {
        self.slot(h).map_or(&[], |s| self.inbox[s as usize].as_slice())
    }

    /// Starts a frame for the notices: those delivered from now on are kept through the next update pass.
    pub fn begin_notice_frame(&mut self) {
        self.inbox_frame = self.inbox_frame.wrapping_add(1);
    }

    /// Drops the notices delivered before this frame's [`World::begin_notice_frame`] that nobody read; notices
    /// delivered since (by objects that updated later in the pass, by physics) wait for the next pass.
    pub fn clear_inboxes(&mut self) {
        let frame = self.inbox_frame;
        let used = core::mem::take(&mut self.inbox_used);
        for s in used {
            let i = s as usize;
            if self.inbox.get(i).is_none_or(|b| b.is_empty()) {
                continue;
            }
            if self.inbox_stamp.get(i).copied() == Some(frame) {
                self.inbox_used.push(s);
            } else {
                self.inbox[i].clear();
            }
        }
    }

    pub(crate) fn take_inbox(&mut self, slot: u32) -> Vec<Notice> {
        core::mem::take(&mut self.inbox[slot as usize])
    }

    // ---------------------------------------------------------------- flags and options with side effects

    fn set_flag(&mut self, slot: u32, flag: u16, on: bool) {
        let f = &mut self.flags[slot as usize];
        let old = *f;
        if on {
            *f |= flag;
        } else {
            *f &= !flag;
        }
        if *f != old {
            self.mark_dirty(slot);
        }
    }

    /// Takes the object out of the world, or puts it back (option 2; scripts.md 10). Raises event 5 or 4.
    pub fn set_in_world(&mut self, h: Handle, on: bool) {
        let Some(s) = self.slot(h) else { return };
        let was = self.flags[s as usize] & flags::IN_WORLD != 0;
        if was == on {
            return;
        }
        self.set_flag(s, flags::IN_WORLD, on);
        self.raise(h, if on { script::event::ADDED } else { script::event::REMOVED }, Vec::new());
    }

    pub(crate) fn clear_in_world_flag(&mut self, slot: u32) {
        self.set_flag(slot, flags::IN_WORLD, false);
    }

    /// Whether the object itself is in the world (its ancestors may not be; see [`World::is_active`]).
    pub fn in_world(&self, h: Handle) -> bool {
        self.flags(h) & flags::IN_WORLD != 0
    }

    /// Hides or shows the object (option 125, warp hidden-object lists).
    pub fn set_hidden(&mut self, h: Handle, on: bool) {
        if let Some(s) = self.slot(h) {
            self.set_flag(s, flags::HIDDEN, on);
        }
    }

    /// Postpones the object's updates until game time `until` (operation 98).
    pub fn postpone(&mut self, h: Handle, until: f32) {
        if let Some(s) = self.slot(h) {
            self.sleep_until.insert(s, until);
        }
    }

    /// Whether the object's updates are postponed at game time `t`.
    pub fn is_postponed(&self, slot: u32, t: f32) -> bool {
        self.sleep_until.get(&slot).is_some_and(|&u| t < u)
    }

    /// Sets an option the way a script or an option list does (options-by-class.md 1): stores it, applies the
    /// generic effects of the placed-object options, then tells the behaviour. Actions run instead of being stored.
    pub fn set_option(&mut self, h: Handle, id: u16, value: OptionValue) {
        let Some(s) = self.slot(h) else { return };
        if let OptionValue::Args(args) = &value {
            let args = args.clone();
            self.run_action(h, id, &args);
            return;
        }
        self.options[s as usize].set(id, value.clone());
        self.apply_generic_option(s, id, &value);
        let mut b = self.take_behavior(s);
        b.option_changed(self, h, id, &value);
        self.put_behavior(s, b);
    }

    /// Runs an action option (code `0` to `6`) on an object. Option 2 takes the object out of the world; other
    /// actions go to the behaviour. Returns whether anything handled it.
    pub fn run_action(&mut self, h: Handle, id: u16, args: &[crate::object::Arg]) -> bool {
        let Some(s) = self.slot(h) else { return false };
        if id == opt::REMOVE && self.sly1_meanings() {
            self.set_in_world(h, false);
            return true;
        }
        // Sly 2: option 26 is "removed when the level starts" (specs/sly2/object-stream.md 1.4); called by a script it
        // takes the object out of the world now (our reading: the same effect at the time of the call).
        if id == crate::world::sly2::options::REMOVE_AT_START && self.sly2.is_some() {
            self.set_in_world(h, false);
            return true;
        }
        // Sly 2 generic object actions (set parent, move to, add, show and hide meshes).
        if self.sly2.is_some() && crate::behavior::sly2_commands::generic_action(self, h, id, args) {
            return true;
        }
        let mut b = self.take_behavior(s);
        let done = b.action(self, h, id, args);
        self.put_behavior(s, b);
        done
    }

    /// Effects every placed object shares (options-by-class.md 2.1).
    pub(crate) fn apply_generic_option(&mut self, slot: u32, id: u16, value: &OptionValue) {
        if !self.sly1_meanings() {
            return; // Sly 2 and Sly 3 hook: these are Sly 1 option IDs.
        }
        match id {
            opt::HIDDEN => self.set_flag(slot, flags::HIDDEN, value.as_bool().unwrap_or(false)),
            opt::REAL_CLOCK => {
                let on = value.as_bool().unwrap_or(false);
                let f = &mut self.flags[slot as usize];
                if on {
                    *f |= flags::REAL_CLOCK;
                } else {
                    *f &= !flags::REAL_CLOCK;
                }
            }
            opt::VELOCITY | opt::ANGULAR_VELOCITY => {
                let v = value.as_vec3().unwrap_or([0.0; 3]);
                let i = match self.movers.iter().position(|m| m.slot == slot) {
                    Some(i) => i,
                    None => {
                        self.movers.push(Mover { slot, velocity: [0.0; 3], angular: [0.0; 3] });
                        self.movers.len() - 1
                    }
                };
                if id == opt::VELOCITY {
                    self.movers[i].velocity = v;
                } else {
                    self.movers[i].angular = v;
                }
                let m = self.movers[i];
                if m.velocity == [0.0; 3] && m.angular == [0.0; 3] {
                    self.movers.remove(i);
                }
            }
            _ => {}
        }
    }

    /// Objects moving kinematically (options 21 and 22).
    pub fn movers(&self) -> &[Mover] {
        &self.movers
    }

    /// Advances every kinematic mover by `dt` seconds (positions by velocity, rotations by the rotation vector of
    /// angular velocity × dt, both in the parent's space). Movers on the real clock use `real_dt`.
    pub fn integrate_movers(&mut self, dt: f32, real_dt: f32, t: f32) {
        for k in 0..self.movers.len() {
            let m = self.movers[k];
            let i = m.slot as usize;
            if self.flags[i] & flags::ACTIVE == 0 || self.is_postponed(m.slot, t) {
                continue;
            }
            let step = if self.flags[i] & flags::REAL_CLOCK != 0 { real_dt } else { dt };
            if step <= 0.0 {
                continue;
            }
            let l = &mut self.local[i];
            l.t = crate::math::madd(l.t, m.velocity, step);
            if m.angular != [0.0; 3] {
                let r = rotation_from_vector(crate::math::scale(m.angular, step));
                l.cols = crate::math::mat_mul(&r, &l.cols);
            }
            self.mark_dirty(m.slot);
        }
    }

    // ---------------------------------------------------------------- events and requests

    /// Queues script event `event` for object `h` with extra arguments (scripts.md 6).
    pub fn raise(&mut self, h: Handle, event: u32, args: Vec<script::Value>) {
        if self.module(h) < 0 {
            return;
        }
        self.events.push_back(RaisedEvent { object: h.to_script(), event, args });
    }

    /// Hurts an object (the player): queued for the player system, and noticed by the target's behaviour.
    pub fn hurt(&mut self, target: Handle, source: Handle, kind: u8, recover_to: Option<[f32; 3]>) {
        self.shared.damage.push(Damage { target: Some(target), source, kind, recover_to, throw: None });
        self.notify(target, Notice::Hit(source));
    }

    /// Hurts the player (or records it for the player's stand-in when there is no player object).
    pub fn hurt_player(&mut self, source: Handle, kind: u8, recover_to: Option<[f32; 3]>) {
        match self.player_target() {
            Some(p) => self.hurt(p, source, kind, recover_to),
            None => self.shared.damage.push(Damage { target: None, source, kind, recover_to, throw: None }),
        }
    }

    /// Hurts the player and throws it at `velocity` (guards' attack effects, guards.md 6.2).
    pub fn hurt_player_thrown(&mut self, source: Handle, kind: u8, velocity: [f32; 3]) {
        self.hurt_player(source, kind, None);
        if let Some(d) = self.shared.damage.last_mut() {
            d.throw = Some(velocity);
        }
    }

    /// The damage dealt since the last call (the player system takes it every frame).
    pub fn take_damage_events(&mut self) -> Vec<Damage> {
        core::mem::take(&mut self.shared.damage)
    }

    /// The maximum draw distance of an object's drawable block (geometry.md 1.1; `None`: unlimited, or no block).
    pub fn max_draw_distance(&self, h: Handle) -> Option<f32> {
        let r = self.level.record(self.record(h)?);
        r.local?;
        let at = r.options.as_ref()?.start.checked_sub(16)?;
        let b = self.level.bytes.get(at..at + 4)?;
        let d = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        (d.is_finite() && d < 3.0e38).then_some(d)
    }

    /// The object standing for the player in hit tests: the player object, else `None` (the probe has no object).
    pub fn player_target(&self) -> Option<Handle> {
        self.player.filter(|&p| self.contains(p))
    }

    /// Queues a request for the game application.
    pub fn request(&mut self, r: Request) {
        self.requests.push(r);
    }

    /// Queues a one-shot sound.
    pub fn play_sound(&mut self, sound: i32, emitter: Option<Handle>) {
        self.sounds.push(SoundRequest { sound, emitter });
    }

    /// The animation system reports that a playback started by `owner` with `tag` reached its end (or was removed).
    pub fn animation_finished(&mut self, owner: Handle, tag: u32) {
        let Some(s) = self.slot(owner) else { return };
        let mut b = self.take_behavior(s);
        b.animation_finished(self, owner, tag);
        self.put_behavior(s, b);
    }

    // ---------------------------------------------------------------- update order

    /// Asks for the ticking list to be rebuilt (a behaviour started or stopped ticking).
    pub fn mark_ticking_dirty(&mut self) {
        self.ticking_valid = false;
    }

    /// Marks which script modules bind event 1 (objects attached to them update every frame).
    pub fn set_event1_modules(&mut self, modules: Vec<bool>) {
        self.event1_modules = modules;
        self.ticking_valid = false;
    }

    /// Whether object `slot` raises event 1 each frame.
    pub fn raises_event1(&self, slot: u32) -> bool {
        let m = self.module[slot as usize];
        m >= 0
            && self.event1_modules.get(m as usize).copied().unwrap_or(false)
            && crate::object::class_info(self.class[slot as usize]).is_some_and(|c| c.is_placed())
    }

    fn rebuild_order(&mut self) {
        let n = self.class.len();
        self.order.clear();
        self.order_pos.clear();
        self.order_pos.resize(n, NONE);
        self.subtree_len.clear();
        self.subtree_len.resize(n, 0);
        let mut stack: Vec<u32> = Vec::new();
        let mut r = self.root_last;
        while r != NONE {
            stack.push(r);
            r = self.prev_sibling[r as usize];
        }
        let mut open: Vec<u32> = Vec::new();
        while let Some(s) = stack.pop() {
            // Close subtrees that ended: an open entry whose subtree does not contain `s`.
            while let Some(&o) = open.last() {
                if self.is_descendant_or_self(s, o) {
                    break;
                }
                open.pop();
                self.subtree_len[o as usize] = self.order.len() as u32 - self.order_pos[o as usize];
            }
            self.order_pos[s as usize] = self.order.len() as u32;
            self.order.push(s);
            open.push(s);
            let mut c = self.last_child[s as usize];
            while c != NONE {
                stack.push(c);
                c = self.prev_sibling[c as usize];
            }
        }
        while let Some(o) = open.pop() {
            self.subtree_len[o as usize] = self.order.len() as u32 - self.order_pos[o as usize];
        }
        self.order_valid = true;
        self.ticking_valid = false;
    }

    /// The preorder of the tree (update order), rebuilt if the tree changed.
    pub fn order(&mut self) -> &[u32] {
        if !self.order_valid {
            self.rebuild_order();
        }
        &self.order
    }

    /// The slots in `slot`'s subtree, as a range of the preorder (the order must be current).
    pub(crate) fn subtree_range(&self, slot: u32) -> core::ops::Range<usize> {
        let p = self.order_pos[slot as usize] as usize;
        p..p + self.subtree_len[slot as usize] as usize
    }

    pub(crate) fn ensure_order(&mut self) {
        if !self.order_valid {
            self.rebuild_order();
        }
    }

    /// The objects that do something every frame (a behaviour that ticks, or an event-1 handler), in update order.
    pub fn ticking(&mut self) -> &[u32] {
        self.ensure_order();
        if !self.ticking_valid {
            let mut t = Vec::new();
            for &s in &self.order {
                if self.behavior[s as usize].ticks() || self.raises_event1(s) {
                    t.push(s);
                }
            }
            self.ticking = t;
            self.ticking_valid = true;
        }
        &self.ticking
    }

    /// The ticking list, copied (for loops that change the world).
    pub(crate) fn ticking_snapshot(&mut self, out: &mut Vec<u32>) {
        out.clear();
        let t = self.ticking();
        out.extend_from_slice(t);
    }

    // ---------------------------------------------------------------- transforms

    /// Marks an object's subtree for a transform update.
    pub fn mark_dirty(&mut self, slot: u32) {
        let f = &mut self.flags[slot as usize];
        if *f & flags::DIRTY == 0 {
            *f |= flags::DIRTY;
            self.dirty.push(slot);
        }
    }

    pub(crate) fn set_basis(&mut self, slot: u32, basis: u32, ext_flags: u32) {
        self.basis.insert(slot, (basis, ext_flags));
    }

    /// Recomputes world transforms, activity and visibility of every dirty subtree, and lists the drawables that
    /// changed.
    pub fn update_transforms(&mut self) {
        if self.dirty.is_empty() {
            return;
        }
        self.ensure_order();
        let mut dirty = core::mem::take(&mut self.dirty);
        dirty.retain(|&s| self.live[s as usize]);
        dirty.sort_unstable_by_key(|&s| self.order_pos[s as usize]);
        let mut covered_end = 0usize;
        for &s in &dirty {
            let r = self.subtree_range(s);
            if r.start < covered_end {
                continue;
            }
            covered_end = r.end;
            for k in r {
                let i = self.order[k];
                self.compute_one(i);
            }
        }
        dirty.clear();
        self.dirty = dirty;
    }

    fn compute_one(&mut self, s: u32) {
        let i = s as usize;
        let p = self.parent[i];
        let (pw, pflags) = if p == NONE {
            (Affine::IDENTITY, flags::ACTIVE | flags::VISIBLE)
        } else {
            (self.world[p as usize], self.flags[p as usize])
        };
        let local = self.local[i];
        let mut w = if p == NONE { local } else { pw.mul(&local) };
        if let Some(&(b, ext)) = self.basis.get(&s)
            && self.live[b as usize]
        {
            let bw = self.world[b as usize];
            if ext & formats::stream::Extension::ROTATION_FROM_BASIS != 0 {
                w.cols = crate::math::mat_mul(&bw.cols, &local.cols);
            }
            if ext & formats::stream::Extension::POSITION_FROM_BASIS != 0 {
                w.t = bw.transform_point(local.t);
            }
        }
        self.world[i] = w;
        let own = self.flags[i];
        let unplaced = (pflags & flags::UNPLACED != 0) || formats::stream::is_sentinel_position(w.t);
        let active = pflags & flags::ACTIVE != 0 && own & flags::IN_WORLD != 0;
        let visible = active && pflags & flags::VISIBLE != 0 && own & flags::HIDDEN == 0 && !unplaced;
        let mut f = own & !(flags::ACTIVE | flags::VISIBLE | flags::UNPLACED | flags::DIRTY);
        if active {
            f |= flags::ACTIVE;
        }
        if visible {
            f |= flags::VISIBLE;
        }
        if unplaced {
            f |= flags::UNPLACED;
        }
        self.flags[i] = f;
        let d = self.drawable[i];
        if d != NONE {
            self.drawables[d as usize].visible = visible;
            self.mark_render(d);
        }
    }

    fn mark_render(&mut self, d: u32) {
        let f = &mut self.render_flag[d as usize];
        if !*f {
            *f = true;
            self.render_changed.push(d);
        }
    }

    /// Every drawable, in load order.
    pub fn drawables(&self) -> &[Drawable] {
        &self.drawables
    }

    /// The drawables whose transform or visibility changed since the last call (indices into
    /// [`World::drawables`]), in the order they changed. Call after [`World::update_transforms`].
    pub fn render_changes(&mut self, out: &mut Vec<u32>) {
        out.clear();
        for &d in &self.render_changed {
            self.render_flag[d as usize] = false;
        }
        out.append(&mut self.render_changed);
    }

    pub(crate) fn add_drawable(&mut self, slot: u32) {
        let d = self.drawables.len() as u32;
        self.drawables.push(Drawable {
            object: Handle::new(slot, self.generation[slot as usize]),
            record: self.record[slot as usize],
            visible: false,
        });
        self.render_flag.push(false);
        self.drawable[slot as usize] = d;
    }

    // ---------------------------------------------------------------- proxies and templates

    /// The objects a proxy placed (its sources or clones), in entry order.
    pub fn placed_by(&self, proxy: Handle) -> &[u32] {
        self.slot(proxy).and_then(|s| self.placed_by.get(&s)).map_or(&[], |v| v.as_slice())
    }

    /// The record of stock template `oid` (12 to 42), if the world has it (object-stream.md 7.1).
    pub fn template_record(&self, oid: i16) -> Option<u32> {
        let i = (oid as i32 - 12) as usize;
        self.templates.get(i).copied().filter(|&r| r != NONE)
    }

    /// Spawns a copy of stock template `oid` at the top level, out of the world (hidden until placed), and returns
    /// it. Pickups dropped by breakables and effects come from here.
    pub fn spawn_template(&mut self, oid: i16, at: Affine) -> Option<Handle> {
        let rec = self.template_record(oid)?;
        self.clone_serial += 1;
        let serial = self.clone_serial;
        let slot = self.instantiate_spawn(rec, at, serial);
        self.handle(slot)
    }

    // ---------------------------------------------------------------- lookups by slot (used by systems)

    /// The class column.
    pub fn class_of_slot(&self, slot: u32) -> i16 {
        self.class[slot as usize]
    }

    /// The OID column.
    pub fn oid_of_slot(&self, slot: u32) -> i16 {
        self.oid[slot as usize]
    }

    /// The parent column (`NONE` at the top level).
    pub fn parent_of_slot(&self, slot: u32) -> u32 {
        self.parent[slot as usize]
    }

    /// The world transforms column (current after [`World::update_transforms`]).
    pub fn world_transforms(&self) -> &[Affine] {
        &self.world
    }

    /// The flags column.
    pub fn flags_column(&self) -> &[u16] {
        &self.flags
    }
}

/// Iterator over a list of siblings.
pub struct Children<'a> {
    world: &'a World,
    next: u32,
}

impl Iterator for Children<'_> {
    type Item = Handle;

    fn next(&mut self) -> Option<Handle> {
        while self.next != NONE {
            let s = self.next;
            self.next = self.world.next_sibling[s as usize];
            if let Some(h) = self.world.handle(s) {
                return Some(h);
            }
        }
        None
    }
}

/// Stream option IDs the engine itself acts on (options-by-class.md 2.1).
pub mod opt {
    /// Action: take the object out of the world.
    pub const REMOVE: u16 = 2;
    /// Initial velocity.
    pub const VELOCITY: u16 = 21;
    /// Initial angular velocity (a rotation vector per second).
    pub const ANGULAR_VELOCITY: u16 = 22;
    /// An animation segment (OID) to play on the object.
    pub const ANIMATION: u16 = 23;
    /// The time to start that animation at.
    pub const ANIMATION_TIME: u16 = 24;
    /// Ambient sound: sound ID.
    pub const SOUND: u16 = 101;
    /// Ambient sound: distance where it starts to be heard.
    pub const SOUND_START: u16 = 102;
    /// Ambient sound: full-volume distance.
    pub const SOUND_FULL: u16 = 103;
    /// Ambient sound: volume.
    pub const SOUND_VOLUME: u16 = 104;
    /// Ambient sound: pitch.
    pub const SOUND_PITCH: u16 = 105;
    /// Ambient sound: Doppler factor.
    pub const SOUND_DOPPLER: u16 = 109;
    /// Runs on the real clock.
    pub const REAL_CLOCK: u16 = 119;
    /// Hidden.
    pub const HIDDEN: u16 = 125;
    /// Collide as a sphere of this radius.
    pub const SPHERE_RADIUS: u16 = 133;
}

/// The rotation matrix of a rotation vector (transforms.md 6): axis r / |r|, angle |r|; identity below 0.0001.
pub fn rotation_from_vector(r: [f32; 3]) -> [[f32; 3]; 3] {
    let a = crate::math::length(r);
    if a.is_nan() || a <= 0.0001 {
        return crate::math::MAT3_IDENTITY;
    }
    let [x, y, z] = crate::math::scale(r, 1.0 / a);
    let (s, k) = crate::math::sin_cos(a);
    let t = 1.0 - k;
    [
        [t * x * x + k, t * x * y + s * z, t * x * z - s * y],
        [t * x * y - s * z, t * y * y + k, t * y * z + s * x],
        [t * x * z + s * y, t * y * z - s * x, t * z * z + k],
    ]
}

#[cfg(test)]
pub(crate) mod tests;
