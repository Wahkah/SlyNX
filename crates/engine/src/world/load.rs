//! Making the world's objects from a world file (object-stream.md 5 to 7, classes/proxy.md, transforms.md).
//!
//! Objects are created in stream order, depth first, so a parent exists before its children and siblings keep their
//! order. The rules:
//!
//! - top-level records hang under the world (the top level), child-list records under their placed object;
//! - stock templates (top-level OIDs 12 to 42 and everything below them) are not created; their records are kept
//!   for [`World::spawn_template`];
//! - records owned by state machines, explosion groups, warps and exits are data of their owner, not objects;
//! - a proxy's sources (full or back-referenced) are created under the proxy's parent, with the proxy's local
//!   transform composed with the source's, which is the re-parenting of classes/proxy.md 2 with world transforms
//!   kept; a back-reference creates a fresh copy of the referenced source's subtree (its nested back-references
//!   resolved the same way), without the source's own proxy's child list (classes/proxy.md 3);
//! - the children listed after a single-source proxy are created under that proxy's (only) placed object, with
//!   their transform made relative to it;
//! - proxy overrides are applied to the first placed object with the target OID, or the first object with it below
//!   a placed object.
//!
//! Each object gets its class defaults, then its option list in order (options-by-class.md 1), then the proxy
//! overrides that target it.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use formats::geometry::Affine;
use formats::options::{OptionList, OptionTable};
use formats::reader::Reader;
use formats::stream::{Context, Extension, Layout, ObjectStream, ProxyEntry, STOCK_TEMPLATE_OIDS, layout_of};
use formats::world::WorldLayout;

use super::{LevelData, NONE, World, WorldInfo, record_local};
use crate::behavior::Behavior;
use crate::object::options::decode_list;
use crate::object::{OptionValue, flags};

/// Why a world could not be loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// The world file did not parse.
    Format(String),
    /// More objects than a script object ID can name.
    TooManyObjects,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Format(e) => write!(f, "world file: {e}"),
            LoadError::TooManyObjects => f.write_str("too many objects"),
        }
    }
}

impl From<formats::Error> for LoadError {
    fn from(e: formats::Error) -> Self {
        LoadError::Format(alloc::format!("{e}"))
    }
}

/// A proxy override (classes/proxy.md 1.2).
struct Override {
    target: i16,
    script: i16,
    options: OptionList,
}

impl World {
    /// Loads a decompressed world file and creates its objects. Behaviours are created and the world's transforms
    /// computed; scripts are attached by the game ([`crate::game::Game::new`]).
    pub fn load(bytes: Vec<u8>, info: WorldInfo) -> Result<World, LoadError> {
        let table = OptionTable::sly1();
        let layout = WorldLayout::parse(&bytes, &table)?;
        let stream = ObjectStream::walk(&bytes, layout.object_stream, &table)?;
        World::from_parts(LevelData { bytes, layout, stream, table }, info)
    }

    /// Creates the objects of an already parsed level.
    pub fn from_parts(level: LevelData, info: WorldInfo) -> Result<World, LoadError> {
        let mut w = World::empty(level, info);
        let wopts = decode_list(&w.level.bytes, &w.level.layout.world_options, &w.level.table);
        for (id, v) in wopts {
            w.world_options.push(id, v);
        }
        let top: Vec<u32> = w.level.stream.top_level.clone();
        for r in top {
            let rec = w.level.record(r);
            if rec.template || (rec.context == Context::TopLevel && STOCK_TEMPLATE_OIDS.contains(&rec.oid)) {
                if STOCK_TEMPLATE_OIDS.contains(&rec.oid) && rec.context == Context::TopLevel {
                    w.templates[(rec.oid - 12) as usize] = r;
                }
                continue;
            }
            let local = record_local(rec);
            w.instantiate(r, NONE, local, 0)?;
        }
        w.finish_load();
        Ok(w)
    }

    /// Creates a stock template's copy (for [`World::spawn_template`]): out of the world until placed.
    pub(crate) fn instantiate_spawn(&mut self, rec: u32, at: Affine, serial: u32) -> u32 {
        let slot = match self.instantiate(rec, NONE, at, serial) {
            Ok(s) => s,
            Err(_) => return NONE,
        };
        self.flags[slot as usize] |= flags::SPAWNED;
        self.flags[slot as usize] &= !flags::IN_WORLD;
        self.mark_dirty(slot);
        self.attach_behaviours_from(slot);
        slot
    }

    /// Creates record `rec` (and its subtree) under `parent` with local transform `local`.
    fn instantiate(&mut self, rec: u32, parent: u32, local: Affine, serial: u32) -> Result<u32, LoadError> {
        if self.class.len() as u32 >= crate::object::MAX_OBJECTS {
            return Err(LoadError::TooManyObjects);
        }
        let r = self.level.record(rec).clone();
        let persist = ((serial as u64) << 32) | rec as u64;
        let mut extra = if serial != 0 { flags::CLONE } else { 0 };
        if r.has_degenerate_matrix() {
            extra |= flags::DEGENERATE;
        }
        let slot = self.create(r.class, r.oid, rec, r.script, local, parent, persist, extra);
        self.apply_record_options(slot, r.options.as_ref());
        if let Some(ext) = r.extension
            && ext.flags & (Extension::ROTATION_FROM_BASIS | Extension::POSITION_FROM_BASIS) != 0
            && let Some(oid) = ext.basis
            && let Some(b) = self.ancestor_slot_with_oid(slot, oid)
        {
            self.set_basis(slot, b, ext.flags);
        }
        if r.is_drawable() {
            self.add_drawable(slot);
        }
        match layout_of(r.class) {
            Some(Layout::Proxy) => self.instantiate_proxy(rec, slot, parent, serial)?,
            _ => {
                for c in self.direct_children(rec) {
                    let cr = self.level.record(c);
                    if cr.context == Context::Child && cr.in_world && !cr.template {
                        let l = record_local(cr);
                        self.instantiate(c, slot, l, serial)?;
                    }
                }
            }
        }
        Ok(slot)
    }

    /// Direct children records of `rec` (records read inside its body whose parent is `rec`), in stream order.
    fn direct_children(&self, rec: u32) -> Vec<u32> {
        let s = &self.level.stream;
        let end = s.records[rec as usize].subtree_end;
        let mut out = Vec::new();
        let mut k = rec + 1;
        while k < end {
            if s.records[k as usize].parent == Some(rec) {
                out.push(k);
            }
            k = s.records[k as usize].subtree_end.max(k + 1);
        }
        out
    }

    fn instantiate_proxy(&mut self, rec: u32, proxy: u32, parent: u32, serial: u32) -> Result<(), LoadError> {
        let entries =
            self.level.stream.proxies.iter().find(|p| p.record == rec).map(|p| p.entries.clone()).unwrap_or_default();
        let proxy_local = self.local[proxy as usize];
        let mut placed: Vec<u32> = Vec::with_capacity(entries.len());
        let mut source_locals: Vec<Affine> = Vec::with_capacity(entries.len());
        for e in &entries {
            let (src, clone_serial) = match *e {
                ProxyEntry::Source { record, .. } => (record, serial),
                ProxyEntry::BackRef { source_index } => {
                    self.clone_serial += 1;
                    (self.level.stream.sources[source_index as usize], self.clone_serial)
                }
            };
            let sl = record_local(self.level.record(src));
            let slot = self.instantiate(src, parent, proxy_local.mul(&sl), clone_serial)?;
            placed.push(slot);
            source_locals.push(sl);
        }
        for o in self.read_overrides(rec, &entries) {
            if let Some(t) = self.override_target(&placed, o.target) {
                if o.script >= 0 {
                    // scripts.md 11.3: the override's module goes to the target; an earlier module stays as its
                    // outer environment.
                    let old = self.module[t as usize];
                    if old >= 0 && old != o.script {
                        self.outer_module.insert(t, old);
                    }
                    self.module[t as usize] = o.script;
                }
                self.apply_record_options(t, Some(&o.options));
            }
        }
        if entries.len() == 1 {
            let host = placed[0];
            let inv = crate::math::affine_inverse(&source_locals[0]).unwrap_or(Affine::IDENTITY);
            for c in self.direct_children(rec) {
                let cr = self.level.record(c);
                if cr.context == Context::ProxyChild && cr.in_world && !cr.template {
                    let l = inv.mul(&record_local(cr));
                    self.instantiate(c, host, l, serial)?;
                }
            }
        }
        self.placed_by.insert(proxy, placed);
        Ok(())
    }

    /// Reads a proxy's override entries (classes/proxy.md 1.2), which the walk steps over.
    fn read_overrides(&self, rec: u32, entries: &[ProxyEntry]) -> Vec<Override> {
        let data = &self.level.bytes;
        let records = &self.level.stream.records;
        // After the 6-byte header: mat3 + vec3 (48 bytes), then the u8 source count.
        let mut pos = records[rec as usize].offset + 6 + 48 + 1;
        for e in entries {
            pos = match *e {
                ProxyEntry::BackRef { .. } => pos + 4,
                ProxyEntry::Source { record, .. } => records[record as usize].end + 2,
            };
        }
        let mut out = Vec::new();
        let mut r = Reader::at(data, pos);
        let Ok(m) = r.u8() else { return out };
        for _ in 0..m {
            let parsed = (|| -> formats::Result<Override> {
                let target = r.i16()?;
                let script = r.i16()?;
                let _zone_mask = r.u32()?;
                if r.i8()? == 2 {
                    let _distance = r.f32()?;
                }
                let options = OptionList::read(&mut r, &self.level.table)?;
                Ok(Override { target, script, options })
            })();
            match parsed {
                Ok(o) => out.push(o),
                Err(_) => break,
            }
        }
        out
    }

    /// The first placed object with `oid`, else the first object with it below a placed object (in preorder of
    /// each placed object's subtree, placed objects in order).
    fn override_target(&self, placed: &[u32], oid: i16) -> Option<u32> {
        if let Some(&p) = placed.iter().find(|&&p| self.oid[p as usize] == oid) {
            return Some(p);
        }
        for &p in placed {
            let mut stack: Vec<u32> = Vec::new();
            let mut c = self.last_child[p as usize];
            while c != NONE {
                stack.push(c);
                c = self.prev_sibling[c as usize];
            }
            while let Some(s) = stack.pop() {
                if self.oid[s as usize] == oid {
                    return Some(s);
                }
                let mut c = self.last_child[s as usize];
                while c != NONE {
                    stack.push(c);
                    c = self.prev_sibling[c as usize];
                }
            }
        }
        None
    }

    /// Applies a record option list to an object: each option in order, actions run, the rest stored.
    fn apply_record_options(&mut self, slot: u32, list: Option<&OptionList>) {
        let Some(list) = list else { return };
        let entries = decode_list(&self.level.bytes, list, &self.level.table);
        for (id, v) in entries {
            if let OptionValue::Args(args) = &v {
                // Actions run at load (options-by-class.md 1). Behaviours do not exist yet, so only the generic
                // ones apply here; option 2 is the only action the loader needs.
                if id == super::opt::REMOVE {
                    self.flags[slot as usize] &= !flags::IN_WORLD;
                } else {
                    self.pending_actions.push((slot, id, args.clone()));
                }
                continue;
            }
            self.apply_generic_option(slot, id, &v);
            self.options[slot as usize].push(id, v);
        }
    }

    /// The nearest ancestor of `slot` with `oid`.
    pub(crate) fn ancestor_slot_with_oid(&self, slot: u32, oid: i16) -> Option<u32> {
        let mut p = self.parent[slot as usize];
        while p != NONE {
            if self.oid[p as usize] == oid {
                return Some(p);
            }
            p = self.parent[p as usize];
        }
        None
    }

    /// After every object exists: behaviours, deferred actions, transforms.
    fn finish_load(&mut self) {
        let n = self.class.len() as u32;
        for s in 0..n {
            self.attach_behaviour(s);
        }
        self.run_pending_actions();
        self.ensure_order();
        self.update_transforms();
    }

    fn attach_behaviour(&mut self, s: u32) {
        let Some(h) = self.handle(s) else { return };
        let b = Behavior::for_class(self, h);
        self.behavior[s as usize] = b;
    }

    fn attach_behaviours_from(&mut self, root: u32) {
        let mut stack = alloc::vec![root];
        while let Some(s) = stack.pop() {
            self.attach_behaviour(s);
            let mut c = self.first_child[s as usize];
            while c != NONE {
                stack.push(c);
                c = self.next_sibling[c as usize];
            }
        }
        self.run_pending_actions();
        self.ticking_valid = false;
    }

    fn run_pending_actions(&mut self) {
        let pending = core::mem::take(&mut self.pending_actions);
        for (slot, id, args) in pending {
            if let Some(h) = self.handle(slot) {
                self.run_action(h, id, &args);
            }
        }
    }
}
