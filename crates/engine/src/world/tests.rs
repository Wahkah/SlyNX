//! World tests on synthetic levels built by [`fixture::Fixture`].

use alloc::vec::Vec;

use formats::geometry::Affine;
use formats::stream::Context;

use super::*;
use crate::object::class::id;
use fixture::{Fixture, at};

/// Builds small levels record by record: the parsed structures the loader reads, plus the bytes that option lists,
/// proxy overrides, exit descriptors and volumes are read from.
pub(crate) mod fixture {
    use alloc::vec;
    use alloc::vec::Vec;

    use formats::geometry::{Affine, Instance, Mesh, MeshSet};
    use formats::options::{OptionEncoding, OptionList, OptionTable};
    use formats::stream::{Context, Extension, ObjectStream, OwnedAnimation, Proxy, ProxyEntry, Record, VolumeRef};
    use formats::textures::MaterialTables;
    use formats::world::{Header, ScriptTable, WorldLayout};

    use crate::world::{LevelData, World, WorldInfo};

    /// A proxy override: target OID, script index, options.
    pub type OverrideSpec<'a> = (i16, i16, &'a [(i16, &'a [f32])]);

    /// A transform at a position.
    pub fn at(x: f32, y: f32, z: f32) -> Affine {
        Affine { cols: crate::math::MAT3_IDENTITY, t: [x, y, z] }
    }

    /// A level under construction.
    pub struct Fixture {
        pub bytes: Vec<u8>,
        pub records: Vec<Record>,
        pub proxies: Vec<Proxy>,
        pub sources: Vec<u32>,
        pub volumes: Vec<VolumeRef>,
        pub owned: Vec<OwnedAnimation>,
        pub world_options: OptionList,
        table: OptionTable,
    }

    impl Default for Fixture {
        fn default() -> Self {
            Fixture::new()
        }
    }

    impl Fixture {
        /// An empty level (16 zero bytes so offset 0 is never a real structure).
        pub fn new() -> Fixture {
            Fixture {
                bytes: vec![0; 16],
                records: Vec::new(),
                proxies: Vec::new(),
                sources: Vec::new(),
                volumes: Vec::new(),
                owned: Vec::new(),
                world_options: OptionList::default(),
                table: OptionTable::sly1(),
            }
        }

        /// Appends an option list; each value is given as numbers and encoded by the option's encoding.
        pub fn options(&mut self, opts: &[(i16, &[f32])]) -> OptionList {
            let start = self.bytes.len();
            for &(id, v) in opts {
                self.bytes.extend_from_slice(&id.to_le_bytes());
                let enc = self.table.get(id).unwrap_or_else(|| panic!("option {id} not in the table"));
                match enc {
                    OptionEncoding::Byte => self.bytes.push(v[0] as u8),
                    OptionEncoding::Short => self.bytes.extend_from_slice(&(v[0] as i16).to_le_bytes()),
                    OptionEncoding::Int => self.bytes.extend_from_slice(&(v[0] as i32).to_le_bytes()),
                    OptionEncoding::Color => self.bytes.extend(v.iter().map(|&x| x as u8)),
                    OptionEncoding::Args(n) => assert_eq!(n, 0, "only argument-less actions in fixtures"),
                    _ => {
                        for &x in v {
                            self.bytes.extend_from_slice(&x.to_le_bytes());
                        }
                    }
                }
            }
            self.bytes.extend_from_slice(&(-1i16).to_le_bytes());
            OptionList { start, end: self.bytes.len(), count: opts.len() as u32, origin_kind: None }
        }

        /// Adds a record; returns its index.
        pub fn add(&mut self, class: i16, oid: i16, parent: Option<u32>, context: Context, local: Affine) -> u32 {
            let depth = parent.map_or(0, |p| self.records[p as usize].depth + 1);
            let in_world = parent.is_none_or(|p| self.records[p as usize].in_world)
                && !matches!(context, Context::StateMachine | Context::GroupMember);
            let template = parent.is_some_and(|p| self.records[p as usize].template)
                || (context == Context::TopLevel && (12..=42).contains(&oid));
            let transform_parent = parent;
            let offset = self.bytes.len();
            self.bytes.extend_from_slice(&[0; 6]);
            self.records.push(Record {
                offset,
                end: offset + 6,
                class,
                oid,
                script: -1,
                context,
                parent,
                transform_parent,
                depth,
                in_world,
                template,
                subtree_end: 0,
                local: Some(local),
                options: None,
                mesh_set: None,
                extension: None,
            });
            self.records.len() as u32 - 1
        }

        /// A top-level record.
        pub fn top(&mut self, class: i16, oid: i16, local: Affine) -> u32 {
            self.add(class, oid, None, Context::TopLevel, local)
        }

        /// A child-list record.
        pub fn child(&mut self, parent: u32, class: i16, oid: i16, local: Affine) -> u32 {
            self.add(class, oid, Some(parent), Context::Child, local)
        }

        /// Gives a record options.
        pub fn set_options(&mut self, r: u32, opts: &[(i16, &[f32])]) {
            let l = self.options(opts);
            self.records[r as usize].options = Some(l);
        }

        /// Gives a record a drawable mesh set (one instance mesh of mesh 0).
        pub fn drawable(&mut self, r: u32) {
            let mesh = |instance| Mesh {
                offset: 0,
                flags: 0,
                instance,
                zone_mask: None,
                cull_radius: None,
                sort_key: None,
                fog: None,
                max_distance: None,
                outline_distance: None,
                animation: None,
                center: [0.0; 3],
                radius: 1.0,
                oid: 0,
                billboard: 0,
                pass: 3,
                mesh_flags: 0,
                submeshes: Vec::new(),
                outlines: Vec::new(),
                end: 0,
            };
            let set = MeshSet {
                offset: 0,
                relight: 0,
                unknown: 0,
                bones: Vec::new(),
                pose_weights: Vec::new(),
                meshes: vec![mesh(None), mesh(Some(Instance { source: 0, matrix: Affine::IDENTITY }))],
                end: 0,
            };
            self.records[r as usize].mesh_set = Some(set);
        }

        /// Sets a record's extension block (basis override).
        pub fn basis(&mut self, r: u32, flags: u32, oid: i16) {
            self.records[r as usize].extension = Some(Extension { flags, basis: Some(oid), offset: 0 });
        }

        /// A proxy with the given entries (`None`: a full source added by `make_source`; `Some(i)`: a back-reference
        /// to source index `i`) and overrides (target OID, script, options). Sources must be added right after
        /// with [`Fixture::source`], then the proxy's children with [`Fixture::proxy_child`], then [`Fixture::end_proxy`].
        pub fn proxy(&mut self, parent: Option<u32>, oid: i16, local: Affine) -> u32 {
            let ctx = if parent.is_some() { Context::Child } else { Context::TopLevel };
            let p = self.add(83, oid, parent, ctx, local);
            self.proxies.push(Proxy { record: p, entries: Vec::new(), overrides: 0 });
            p
        }

        /// A full source of proxy `p`.
        pub fn source(&mut self, p: u32, class: i16, oid: i16, local: Affine) -> u32 {
            let r = self.add(class, oid, Some(p), Context::ProxySource, local);
            let source_index = self.sources.len() as u32;
            self.sources.push(r);
            let px = self.proxies.iter_mut().find(|x| x.record == p).unwrap();
            px.entries.push(ProxyEntry::Source { record: r, source_index, use_count: 1 });
            r
        }

        /// A back-reference of proxy `p` to source index `si`.
        pub fn backref(&mut self, p: u32, si: u32) {
            let px = self.proxies.iter_mut().find(|x| x.record == p).unwrap();
            px.entries.push(ProxyEntry::BackRef { source_index: si });
        }

        /// A child of single-source proxy `p`.
        pub fn proxy_child(&mut self, p: u32, class: i16, oid: i16, local: Affine) -> u32 {
            self.add(class, oid, Some(p), Context::ProxyChild, local)
        }

        /// Writes proxy `p`'s body bytes for its overrides (target OID, script, options) so the loader can read
        /// them: the header and transform, the entries (use counts after each full source), the overrides.
        pub fn end_proxy(&mut self, p: u32, overrides: &[OverrideSpec<'_>]) {
            let entries = self.proxies.iter().find(|x| x.record == p).unwrap().entries.clone();
            // Body: header (6) + transform (48) + count (1), then per entry either 4 bytes or the source record.
            let start = self.bytes.len();
            self.bytes.extend_from_slice(&[0; 55]);
            self.records[p as usize].offset = start;
            for e in &entries {
                match *e {
                    ProxyEntry::BackRef { .. } => self.bytes.extend_from_slice(&[0xFF, 0xFF, 0, 0]),
                    ProxyEntry::Source { record, .. } => {
                        let s = self.bytes.len();
                        self.bytes.extend_from_slice(&[0; 6]);
                        self.records[record as usize].end = self.bytes.len();
                        let _ = s;
                        self.bytes.extend_from_slice(&1i16.to_le_bytes());
                    }
                }
            }
            self.bytes.push(overrides.len() as u8);
            for &(target, script, opts) in overrides {
                self.bytes.extend_from_slice(&target.to_le_bytes());
                self.bytes.extend_from_slice(&script.to_le_bytes());
                self.bytes.extend_from_slice(&0u32.to_le_bytes());
                self.bytes.push(0);
                let l = self.options(opts);
                // `options` appended the list right here; nothing else to do.
                let _ = l;
            }
            self.proxies.iter_mut().find(|x| x.record == p).unwrap().overrides = overrides.len() as u32;
        }

        /// Gives a record a level descriptor (exits) whose identity is `identity`, at body offset 0.
        pub fn exit_body(&mut self, r: u32, identity: u32, volume_half: f32) {
            let start = self.bytes.len();
            self.bytes.extend_from_slice(&[0; 6]);
            // Descriptor words A0 A1 A2 ? ? B0 B2 B1: identity = A2 ^ B2.
            for w in [1u32, 2, identity, 0, 0, 0, 0, 0] {
                self.bytes.extend_from_slice(&w.to_le_bytes());
            }
            self.records[r as usize].offset = start;
            let v = self.convex_box(volume_half);
            self.volumes.push(VolumeRef { record: r, offset: v });
        }

        /// A record's convex volume (volumes, checkpoints' children).
        pub fn volume(&mut self, r: u32, half: f32) {
            let v = self.convex_box(half);
            self.volumes.push(VolumeRef { record: r, offset: v });
        }

        /// Writes the box |x|, |y|, |z| < half as a convex volume; returns its offset.
        fn convex_box(&mut self, half: f32) -> usize {
            let at = self.bytes.len();
            let normals = [
                [1.0f32, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.0, 0.0, 1.0],
                [0.0, 0.0, -1.0],
            ];
            self.bytes.extend_from_slice(&6u16.to_le_bytes());
            for n in normals {
                self.bytes.extend_from_slice(&half.to_le_bytes());
                for x in n {
                    self.bytes.extend_from_slice(&x.to_le_bytes());
                }
            }
            self.bytes.extend_from_slice(&6u16.to_le_bytes());
            for i in 0..6i16 {
                self.bytes.extend_from_slice(&(i as u16).to_le_bytes());
                self.bytes.extend_from_slice(&(if i < 5 { i + 1 } else { -1 }).to_le_bytes());
                self.bytes.extend_from_slice(&(-1i16).to_le_bytes());
            }
            at
        }

        /// Sets the world option list.
        pub fn world_options(&mut self, opts: &[(i16, &[f32])]) {
            self.world_options = self.options(opts);
        }

        /// The parsed level.
        pub fn level(mut self) -> LevelData {
            let n = self.records.len();
            for (i, r) in self.records.iter_mut().enumerate() {
                r.subtree_end = i as u32 + 1;
            }
            for i in (0..n).rev() {
                if let Some(p) = self.records[i].parent {
                    let e = self.records[i].subtree_end;
                    let pe = &mut self.records[p as usize].subtree_end;
                    *pe = (*pe).max(e);
                }
            }
            let top_level = (0..n as u32).filter(|&i| self.records[i as usize].parent.is_none()).collect();
            let layout = WorldLayout {
                header: Header { texture_size: 0, timestamp: 0 },
                debug: 0,
                sound_bank: 1,
                name_count: 0,
                scripts: ScriptTable { range: 0..0, count: 0, symbols: 0, extra: 0, scripts: Vec::new() },
                world_options: self.world_options,
                descriptors: 0..0,
                audio: None,
                speakers: 0..0,
                music: None,
                materials_range: 0..0,
                materials: MaterialTables::default(),
                object_stream: 0,
            };
            let stream = ObjectStream {
                start: 0,
                end: 0,
                records: self.records,
                top_level,
                proxies: self.proxies,
                sources: self.sources,
                counts: Default::default(),
                owned_animations: self.owned,
                volumes: self.volumes,
            };
            LevelData { bytes: self.bytes, layout, stream, table: self.table }
        }

        /// The world.
        pub fn world(self, info: WorldInfo) -> World {
            World::from_parts(self.level(), info).expect("fixture loads")
        }
    }
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
}

#[test]
fn hierarchy_order_and_transforms() {
    let mut f = Fixture::new();
    let a = f.top(2, 100, at(10.0, 0.0, 0.0));
    let b = f.child(a, 2, 101, at(0.0, 5.0, 0.0));
    let _c = f.child(b, 2, 102, at(0.0, 0.0, 1.0));
    let _d = f.top(2, 103, at(0.0, 0.0, 0.0));
    let mut w = f.world(WorldInfo::default());
    assert_eq!(w.len(), 4);
    let ha = w.find_oid(100, None).unwrap();
    let hc = w.find_oid(102, None).unwrap();
    assert!(close(w.position(hc).unwrap(), [10.0, 5.0, 1.0]));
    let order: Vec<i16> = w.order().to_vec().iter().map(|&s| w.oid_of_slot(s)).collect();
    assert_eq!(order, [100, 101, 102, 103]);
    // Moving the parent moves the subtree.
    w.set_position(ha, [0.0, 0.0, 0.0]);
    w.update_transforms();
    assert!(close(w.position(hc).unwrap(), [0.0, 5.0, 1.0]));
    // Reparent keeping the world transform.
    let hd = w.find_oid(103, None).unwrap();
    w.reparent(hc, Some(hd), true);
    w.update_transforms();
    assert!(close(w.position(hc).unwrap(), [0.0, 5.0, 1.0]));
    assert_eq!(w.parent(hc), Some(hd));
    let order: Vec<i16> = w.order().to_vec().iter().map(|&s| w.oid_of_slot(s)).collect();
    assert_eq!(order, [100, 101, 103, 102]);
}

#[test]
fn proxies_place_sources_clones_and_children() {
    let mut f = Fixture::new();
    // Proxy 1 at x = 100, rotated 90 degrees about Z, with a source (class 33) and one child.
    let rot = Affine { cols: [[0.0, 1.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]], t: [100.0, 0.0, 0.0] };
    let p1 = f.proxy(None, 500, rot);
    let s = f.source(p1, 33, 501, at(10.0, 0.0, 0.0));
    let _sc = f.child(s, 39, 502, at(0.0, 0.0, 5.0));
    let _pc = f.proxy_child(p1, 96, 503, at(0.0, 0.0, 20.0));
    f.end_proxy(p1, &[(502, -1, &[(125, &[1.0])])]);
    // Proxy 2 back-references the source, with its own child.
    let p2 = f.proxy(None, 510, at(0.0, 300.0, 0.0));
    f.backref(p2, 0);
    let _pc2 = f.proxy_child(p2, 96, 503, at(0.0, 0.0, 30.0));
    f.end_proxy(p2, &[]);
    let mut w = f.world(WorldInfo::default());

    // Original: source at proxy · source = (100, 10, 0); its child 5 above; the proxy child 20 above the proxy.
    let mut breakables = Vec::new();
    w.find_class(33, None, &mut breakables);
    assert_eq!(breakables.len(), 2, "source and its clone");
    assert!(close(w.position(breakables[0]).unwrap(), [100.0, 10.0, 0.0]));
    assert!(close(w.position(breakables[1]).unwrap(), [10.0, 300.0, 0.0]));
    // Sources are re-parented under the proxy's parent (the top level).
    assert_eq!(w.parent(breakables[0]), None);
    // Each proxy child hangs under its own placed source, once.
    let kids0 = w.children_of_class(breakables[0], 96);
    let kids1 = w.children_of_class(breakables[1], 96);
    assert_eq!((kids0.len(), kids1.len()), (1, 1));
    assert!(close(w.position(kids0[0]).unwrap(), [100.0, 0.0, 20.0]));
    assert!(close(w.position(kids1[0]).unwrap(), [0.0, 300.0, 30.0]));
    // The override (option 125, hidden) reached the piece below the first source only.
    let pieces: Vec<Handle> = w.slots_of_class(39).iter().map(|&s| w.handle(s).unwrap()).collect();
    assert_eq!(pieces.len(), 2);
    assert!(!w.is_visible(pieces[0]));
    assert!(w.is_visible(pieces[1]));
    // Clones carry their own persist keys.
    assert_ne!(w.persist_key(breakables[0]), w.persist_key(breakables[1]));
    assert!(w.flags(breakables[1]) & flags::CLONE != 0);
    // A proxy is an anchor for what it placed.
    let p1h = w.handle(w.slots_of_class(83)[0]).unwrap();
    assert_eq!(w.find_oid(502, Some(p1h)), Some(pieces[0]));
    // Looking a proxy's OID up finds what it placed.
    assert_eq!(w.find_oid(500, None), Some(breakables[0]));
    assert_eq!(w.placed_by(p1h), &[breakables[0].index()]);
}

#[test]
fn lookups_by_scope() {
    let mut f = Fixture::new();
    let a = f.top(2, 1, at(0.0, 0.0, 0.0));
    let a1 = f.child(a, 2, 7, at(0.0, 0.0, 0.0));
    let _a11 = f.child(a1, 2, 9, at(0.0, 0.0, 0.0));
    let b = f.top(2, 2, at(0.0, 0.0, 0.0));
    let _b1 = f.child(b, 2, 9, at(0.0, 0.0, 0.0));
    let mut w = f.world(WorldInfo::default());
    let hb = w.find_oid(2, None).unwrap();
    let ha1 = w.find_oid(7, None).unwrap();
    let nines: Vec<Handle> = w.slots_with_oid(9).iter().map(|&s| w.handle(s).unwrap()).collect();
    assert_eq!(w.find_oid(9, None), Some(nines[0]));
    assert_eq!(w.find_oid(9, Some(hb)), Some(nines[1]));
    assert_eq!(w.find_oid(7, Some(hb)), None);
    assert_eq!(w.find_nearest(9, Some(hb)), Some(nines[1]));
    assert_eq!(w.find_nearest(9, Some(ha1)), Some(nines[0]));
    let mut v = Vec::new();
    w.find_all_nearest(9, Some(hb), &mut v);
    assert_eq!(v, [nines[1]]);
    v.clear();
    w.find_class(2, Some(hb), &mut v);
    assert_eq!(v, [nines[1]]);
    assert_eq!(w.find_ancestor(nines[0], 1), w.find_oid(1, None));
}

#[test]
fn options_flags_and_sentinels() {
    let mut f = Fixture::new();
    let a = f.top(2, 1, at(0.0, 0.0, 0.0));
    f.set_options(a, &[(125, &[1.0])]);
    let b = f.top(2, 2, at(0.0, 0.0, 0.0));
    f.set_options(b, &[(2, &[])]);
    let c = f.top(2, 3, at(0.0, 0.0, 3.0e38));
    let _c1 = f.child(c, 2, 4, at(0.0, 0.0, 0.0));
    for r in [a, b, c] {
        f.drawable(r);
    }
    let mut w = f.world(WorldInfo::default());
    let h = |w: &mut World, oid| w.find_oid(oid, None).unwrap();
    let (ha, hb, hc, hc1) = (h(&mut w, 1), h(&mut w, 2), h(&mut w, 3), h(&mut w, 4));
    assert!(w.is_active(ha) && !w.is_visible(ha), "hidden");
    assert!(!w.in_world(hb) && !w.is_active(hb), "option 2 removes");
    assert!(!w.is_visible(hc) && !w.is_visible(hc1), "not placed, nor its children");
    assert!(w.flags(hc1) & flags::UNPLACED != 0);
    assert_eq!(w.drawables().len(), 3);
    let mut changed = Vec::new();
    w.render_changes(&mut changed);
    assert_eq!(changed.len(), 3);
    w.render_changes(&mut changed);
    assert!(changed.is_empty());
    // Showing it again marks it changed.
    w.set_option(ha, 125, crate::object::OptionValue::Byte(0));
    w.update_transforms();
    w.render_changes(&mut changed);
    assert_eq!(changed, [0]);
    assert!(w.drawables()[0].visible);
}

#[test]
fn movers_and_basis_override() {
    let mut f = Fixture::new();
    let a = f.top(2, 1, at(0.0, 0.0, 0.0));
    // Spins about Z at pi/2 rad/s, moves along +X at 10 units/s.
    f.set_options(a, &[(22, &[0.0, 0.0, core::f32::consts::FRAC_PI_2]), (21, &[10.0, 0.0, 0.0])]);
    let child = f.child(a, 2, 2, at(1.0, 0.0, 0.0));
    let grand = f.child(child, 79, 3, at(0.0, 0.0, 0.0));
    // The grandchild takes its rotation from ancestor OID 2's parent... from OID 1 (bit 3).
    f.basis(grand, formats::stream::Extension::ROTATION_FROM_BASIS, 1);
    let mut w = f.world(WorldInfo::default());
    assert_eq!(w.movers().len(), 1);
    w.integrate_movers(1.0, 1.0, 0.0);
    w.update_transforms();
    let ha = w.find_oid(1, None).unwrap();
    let hc = w.find_oid(2, None).unwrap();
    assert!(close(w.position(ha).unwrap(), [10.0, 0.0, 0.0]));
    // The child at local x = 1 is now at +Y of the spun parent.
    assert!(close(w.position(hc).unwrap(), [10.0, 1.0, 0.0]));
    let hg = w.find_oid(3, None).unwrap();
    let g = w.world_transform(hg).unwrap();
    assert!(close(g.cols[0], [0.0, 1.0, 0.0]));
}

#[test]
fn templates_and_destroy() {
    let mut f = Fixture::new();
    let _t = f.top(88, 20, at(0.0, 0.0, 0.0));
    let _a = f.top(2, 1, at(0.0, 0.0, 0.0));
    let mut w = f.world(WorldInfo::default());
    assert_eq!(w.len(), 1, "templates are not objects");
    assert!(w.template_record(20).is_some());
    let h = w.spawn_template(20, at(5.0, 0.0, 0.0)).unwrap();
    assert!(!w.in_world(h));
    assert_eq!(w.class(h), Some(id::COIN));
    assert!(matches!(w.behavior(h), Some(crate::behavior::Behavior::Pickup(_))));
    w.destroy(h);
    assert!(!w.contains(h));
    let h2 = w.spawn_template(20, at(0.0, 0.0, 0.0)).unwrap();
    assert_eq!(h2.index(), h.index());
    assert_ne!(h2.generation(), h.generation());
    assert!(w.class(h).is_none(), "stale handles are refused");
}

#[test]
fn owned_records_are_not_objects() {
    let mut f = Fixture::new();
    let m = f.top(136, 1, at(0.0, 0.0, 0.0));
    let _seg = f.add(131, 2, Some(m), Context::StateMachine, at(0.0, 0.0, 0.0));
    let w = f.world(WorldInfo::default());
    assert_eq!(w.len(), 1);
}
