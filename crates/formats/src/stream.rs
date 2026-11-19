//! The object-stream walker (object-stream.md sections 5 to 7, classes/*.md, transforms.md).
//!
//! [`ObjectStream::walk`] reads every object record from the top-level list to the end of the stream and keeps a
//! flat list of [`Record`]s in stream order (depth first, parents before children). Each record notes how it was
//! reached ([`Context`]), its structural parent, the parent its transform is relative to, its stored transform, its
//! option list, its mesh set and extension block. Proxies keep their source and back-reference lists.
//!
//! [`ObjectStream::world_transforms`] composes world transforms; [`ObjectStream::placements`] lists every drawable
//! object to draw (including the clones that proxy back-references create) with its world transform.

use alloc::vec;
use alloc::vec::Vec;

use crate::error::{Error, Result};
use crate::geometry::{Affine, MeshSet, read_transform};
use crate::options::{ORIGIN_KIND_MESH, OptionList, OptionTable};
use crate::reader::{Reader, Vec3};

/// How a record was reached (object-stream.md 5.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    /// The world's top-level object list.
    TopLevel,
    /// The child list at the end of a placed-object body.
    Child,
    /// A proxy source written in full.
    ProxySource,
    /// The child list after a single-source proxy.
    ProxyChild,
    /// An animation of a state-machine transition (owned, not in the world).
    StateMachine,
    /// A member of an explosion group (owned, not in the world).
    GroupMember,
}

impl Context {
    /// Whether the record was read through an object list (`u16` count framing).
    pub fn is_list_loaded(self) -> bool {
        matches!(self, Context::TopLevel | Context::Child | Context::ProxyChild)
    }
}

/// Body layouts (object-stream.md section 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Base object: options only.
    Base,
    /// Placed object.
    Placed,
    /// Solid object (collision, then the placed-object layout).
    Solid,
    /// Conveyor: solid object and two floats.
    Conveyor,
    /// Particle emitter.
    Emitter,
    /// Flash volume.
    Flash,
    /// Dialogue.
    Dialogue,
    /// Camera zone.
    CameraZone,
    /// Proxy.
    Proxy,
    /// Curve shape.
    Shape,
    /// Transform marker.
    Marker,
    /// Warp point.
    Warp,
    /// Explosion group.
    ExplosionGroup,
    /// Explosion emitter.
    ExplosionEmitter,
    /// Volume.
    Volume,
    /// Level exit.
    Exit,
    /// Point.
    Point,
    /// Scan point.
    ScanPoint,
    /// Animation segment.
    AnimationSegment,
    /// Visibility map.
    VisibilityMap,
    /// State machine.
    StateMachine,
    /// Path zone.
    PathZone,
    /// Reach map.
    ReachMap,
    /// Screen mask.
    Keyhole,
}

/// Class ids that exist but never occur on the USA disc; the class table gives them no layout, so a record with one
/// of them is corrupt input (object-stream.md section 6).
pub const CLASSES_NOT_ON_DISC: [i16; 17] = [5, 6, 8, 16, 30, 32, 44, 45, 48, 53, 77, 85, 109, 114, 119, 132, 133];

/// The body layout of a class id, or `None` for ids with no layout: those that never occur as records (0, 108,
/// 146), those not on the disc ([`CLASSES_NOT_ON_DISC`]) and anything out of range.
pub fn layout_of(class: i16) -> Option<Layout> {
    use Layout::*;
    if CLASSES_NOT_ON_DISC.contains(&class) {
        return None;
    }
    Some(match class {
        1 | 135 | 137 | 140 | 141 | 143 | 144 | 145 => Base,
        2 | 78 | 79 | 81 | 82 | 84 | 86..=95 | 97 | 98 | 100 | 102..=107 => Placed,
        3..=56 | 58..=76 => Solid,
        57 => Conveyor,
        80 => Emitter,
        96 => Flash,
        101 => Dialogue,
        99 => CameraZone,
        83 => Proxy,
        110..=113 => Shape,
        115 | 117 | 118 => Marker,
        116 => Warp,
        120 => ExplosionGroup,
        121 | 122 => ExplosionEmitter,
        123 | 124 => Volume,
        125 => Exit,
        126..=129 => Point,
        130 => ScanPoint,
        131 => AnimationSegment,
        134 => VisibilityMap,
        136 => StateMachine,
        138 => PathZone,
        139 => ReachMap,
        142 => Keyhole,
        _ => return None,
    })
}

/// Class id of the static world mesh.
pub const CLASS_WORLD_MESH: i16 = 4;
/// Class id of the proxy.
pub const CLASS_PROXY: i16 = 83;
/// OIDs of the stock templates (object-stream.md 7.1).
pub const STOCK_TEMPLATE_OIDS: core::ops::RangeInclusive<i16> = 12..=42;

/// The extension block of a placed object (classes/placed-object.md section 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Extension {
    /// Flags.
    pub flags: u32,
    /// Basis object OID (bits 2 or 3).
    pub basis: Option<i16>,
    /// File offset of the block (its flags word).
    pub offset: usize,
}

impl Extension {
    /// Bit 2: position taken from the basis ancestor.
    pub const POSITION_FROM_BASIS: u32 = 0x04;
    /// Bit 3: rotation taken from the basis ancestor.
    pub const ROTATION_FROM_BASIS: u32 = 0x08;
    /// Bit 0: a pre-rotation follows the flags.
    pub const PRE_ROTATION: u32 = 0x01;
    /// Bit 1: a post-rotation follows.
    pub const POST_ROTATION: u32 = 0x02;
    /// Bit 7: the object is a joint.
    pub const JOINT: u32 = 0x80;

    /// The pre-rotation (bit 0), applied on the object side of an animated rotation (animation.md 4.1).
    pub fn pre_rotation(&self, data: &[u8]) -> Option<[Vec3; 3]> {
        if self.flags & Self::PRE_ROTATION == 0 {
            return None;
        }
        Reader::at(data, self.offset + 4).mat3().ok()
    }

    /// The post-rotation (bit 1), applied on the parent side of an animated rotation (animation.md 4.1).
    pub fn post_rotation(&self, data: &[u8]) -> Option<[Vec3; 3]> {
        if self.flags & Self::POST_ROTATION == 0 {
            return None;
        }
        let skip = if self.flags & Self::PRE_ROTATION != 0 { 36 } else { 0 };
        Reader::at(data, self.offset + 4 + skip).mat3().ok()
    }
}

/// One object record.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    /// File offset of the 6-byte header (for a proxy source, of its class id).
    pub offset: usize,
    /// File offset just past the body.
    pub end: usize,
    /// Class id.
    pub class: i16,
    /// Object id.
    pub oid: i16,
    /// Script index or -1.
    pub script: i16,
    /// How the record was reached.
    pub context: Context,
    /// The record whose body holds this one (`None` at the top level).
    pub parent: Option<u32>,
    /// The record this one's transform is relative to (`None`: the world). For proxy sources and proxy children,
    /// the proxy (transforms.md 3).
    pub transform_parent: Option<u32>,
    /// Nesting depth (top level 0).
    pub depth: u8,
    /// Whether the record is part of the world hierarchy (false for owned state-machine animations and
    /// explosion-group members, and everything inside them).
    pub in_world: bool,
    /// Whether the record is a stock template or inside one.
    pub template: bool,
    /// Records with index in `self_index + 1 .. subtree_end` were read inside this record's body.
    pub subtree_end: u32,
    /// Stored local transform, as read (see [`Record::local_transform`]). `None` for layouts without one.
    pub local: Option<Affine>,
    /// The record's own option list.
    pub options: Option<OptionList>,
    /// The mesh set of placed-object layouts.
    pub mesh_set: Option<MeshSet>,
    /// The extension block of placed-object layouts.
    pub extension: Option<Extension>,
}

impl Record {
    /// The local transform used for composition: identity for layouts without one, and identity in place of a
    /// degenerate stored matrix (the position is kept).
    pub fn local_transform(&self) -> Affine {
        match self.local {
            Some(l) => Affine { cols: Affine::sanitize_matrix(l.cols), t: l.t },
            None => Affine::IDENTITY,
        }
    }

    /// Whether the record has a mesh set that draws something.
    pub fn is_drawable(&self) -> bool {
        self.mesh_set.as_ref().is_some_and(MeshSet::is_drawable)
    }

    /// Whether the stored rotation is degenerate (classes/placed-object.md 1.1).
    pub fn has_degenerate_matrix(&self) -> bool {
        self.local.is_some_and(|l| Affine::sanitize_matrix(l.cols) != l.cols)
    }
}

/// One source entry of a proxy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProxyEntry {
    /// A source written in full.
    Source {
        /// Its record.
        record: u32,
        /// Its index in the world-wide source list.
        source_index: u32,
        /// How many proxies place it, this one included.
        use_count: i16,
    },
    /// A back-reference to an earlier source: this proxy places a copy (clone) of it.
    BackRef {
        /// Index in the world-wide source list.
        source_index: u32,
    },
}

/// A proxy's sources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proxy {
    /// The proxy's record.
    pub record: u32,
    /// Its sources, in order.
    pub entries: Vec<ProxyEntry>,
    /// Number of override entries.
    pub overrides: u32,
}

/// Counts gathered while walking.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WalkCounts {
    /// Option entries in every option list of the stream: objects, proxy overrides, and the animations owned by
    /// warps and exits (worlds.md "Options", minus the world option list, which is not part of the stream).
    pub options: u64,
    /// The part of `options` that comes from warp and exit animations.
    pub animation_options: u64,
    /// Warp and exit animation items read.
    pub warp_exit_animations: u32,
}

/// The walked object stream.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectStream {
    /// File offset of the top-level `u16` count.
    pub start: usize,
    /// File offset just past the last record.
    pub end: usize,
    /// Every record, in stream order.
    pub records: Vec<Record>,
    /// Indices of the top-level records.
    pub top_level: Vec<u32>,
    /// Every proxy.
    pub proxies: Vec<Proxy>,
    /// World-wide proxy source list: record index of each full source, in stream order.
    pub sources: Vec<u32>,
    /// Counters.
    pub counts: WalkCounts,
    /// Animation segments owned by warps and exits (not records), in stream order.
    pub owned_animations: Vec<OwnedAnimation>,
    /// Convex volumes of volumes, exits, camera zones and flash volumes, in stream order.
    pub volumes: Vec<VolumeRef>,
}

/// One pose-weight set: a name (an OID used only as the set's name) and one weight per pose.
#[derive(Clone, Debug, PartialEq)]
pub struct PoseWeightSet {
    /// The set's name.
    pub name: i16,
    /// Weights.
    pub weights: Vec<f32>,
}

impl Record {
    /// The pose-weight sets the record carries (placed layout row 14; animation.md 8.3), read from `data`. Empty for
    /// records without the placed layout.
    pub fn pose_weight_sets(&self, data: &[u8]) -> Result<Vec<PoseWeightSet>> {
        let (Some(set), Some(ext)) = (&self.mesh_set, &self.extension) else { return Ok(Vec::new()) };
        let f = ext.flags;
        let mut size = 4;
        size += if f & 0x01 != 0 { 36 } else { 0 };
        size += if f & 0x02 != 0 { 36 } else { 0 };
        size += if f & 0x0C != 0 { 2 } else { 0 };
        size += if f & 0x10 != 0 { 2 } else { 0 };
        size += if f & 0x20 != 0 { 30 } else { 0 };
        size += if f & 0x40 != 0 { 4 } else { 0 };
        size += if f & 0x80 != 0 { 1 } else { 0 };
        let mut r = Reader::at(data, ext.offset + size);
        let count = r.u8()?;
        let poses = set.pose_count();
        (0..count)
            .map(|_| {
                let name = r.i16()?;
                let weights = (0..poses).map(|_| r.f32()).collect::<Result<Vec<f32>>>()?;
                Ok(PoseWeightSet { name, weights })
            })
            .collect()
    }
}

/// An animation segment owned by a warp or an exit (classes/markers-and-volumes.md 5 and 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnedAnimation {
    /// The warp or exit record.
    pub owner: u32,
    /// Class id from the item header.
    pub class: i16,
    /// OID from the item header.
    pub oid: i16,
    /// File offset of the segment body.
    pub body: usize,
}

/// Where a record's convex volume is (collision.md 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeRef {
    /// The record (volume, exit, camera zone or flash volume).
    pub record: u32,
    /// File offset of the convex volume.
    pub offset: usize,
}

/// Maximum nesting accepted (the disc's deepest is 12).
pub const MAX_DEPTH: u8 = 64;

/// The most records that proxy back-references may copy in one world, nested clones included (the disc's largest
/// world copies a few thousand). A stream past this is corrupt input; the limit keeps clone placement bounded.
pub const MAX_CLONED_RECORDS: u64 = 1 << 20;

struct Walker<'a, 't> {
    table: &'t OptionTable,
    out: ObjectStream,
    _data: &'a [u8],
}

#[derive(Clone, Copy)]
struct Frame {
    context: Context,
    parent: Option<u32>,
    transform_parent: Option<u32>,
    depth: u8,
    in_world: bool,
    template: bool,
}

fn corrupt(r: &Reader<'_>, what: &'static str) -> Error {
    r.corrupt(what)
}

fn count_i8(r: &mut Reader<'_>) -> Result<usize> {
    let v = r.i8()?;
    if v < 0 {
        return Err(Error::Corrupt { offset: r.pos() - 1, what: "negative count" });
    }
    Ok(v as usize)
}

fn count_i16(r: &mut Reader<'_>) -> Result<usize> {
    let v = r.i16()?;
    if v < 0 {
        return Err(Error::Corrupt { offset: r.pos() - 2, what: "negative count" });
    }
    Ok(v as usize)
}

/// Collision mesh; returns (position count, triangle count).
fn skip_collision_mesh(r: &mut Reader<'_>) -> Result<(usize, usize)> {
    r.skip(4)?;
    let p = r.u16()? as usize;
    r.skip_n(p, 12)?;
    let t = r.u16()? as usize;
    let e_total = r.u16()? as usize;
    let mut e_sum = 0;
    for _ in 0..t {
        let corners = r.array::<6>()?;
        for c in corners.as_chunks::<2>().0.iter() {
            if u16::from_le_bytes([c[0], c[1]]) as usize >= p {
                return Err(corrupt(r, "collision triangle corner out of range"));
            }
        }
        r.skip(16)?;
        let n = count_i8(r)?;
        r.skip_n(n, 6)?;
        e_sum += n;
    }
    if e_sum != e_total {
        return Err(corrupt(r, "collision edge total does not match"));
    }
    Ok((p, t))
}

/// Collision tree.
fn skip_collision_tree(r: &mut Reader<'_>) -> Result<()> {
    let n = r.u16()? as usize;
    r.skip(2)?;
    r.skip_n(n, 6)
}

/// Convex volume.
fn skip_convex_volume(r: &mut Reader<'_>) -> Result<()> {
    let p = r.u16()? as usize;
    r.skip_n(p, 16)?;
    let n = r.u16()? as usize;
    r.skip_n(n, 6)
}

/// Curve of a given kind (0 polyline, 1 cubic).
fn skip_curve(r: &mut Reader<'_>, kind: i32) -> Result<()> {
    let size = match kind {
        0 => 16,
        1 => 40,
        _ => return Err(Error::Corrupt { offset: r.pos().saturating_sub(1), what: "unknown curve kind" }),
    };
    r.skip(1)?;
    let c = r.u8()? as usize;
    r.skip_n(c, size)
}

fn skip_scalar_curve(r: &mut Reader<'_>, kind: i8) -> Result<()> {
    let size = match kind {
        0 => 16,
        1 => 24,
        2 => 6,
        3 | 4 => return Ok(()),
        _ => return Err(Error::Corrupt { offset: r.pos().saturating_sub(1), what: "unknown scalar curve kind" }),
    };
    let k = r.u16()? as usize;
    r.skip_n(k, size)
}

fn skip_vector_curve(r: &mut Reader<'_>, kind: i8) -> Result<()> {
    match kind {
        0 => {
            let mask = r.u8()?;
            r.skip(12)?;
            for bit in 0..3 {
                if mask >> bit & 1 != 0 {
                    let k = r.i8()?;
                    skip_scalar_curve(r, k)?;
                }
            }
            Ok(())
        }
        1 => {
            let k = r.u16()? as usize;
            r.skip_n(k, 38)
        }
        2 => Ok(()),
        _ => Err(Error::Corrupt { offset: r.pos().saturating_sub(1), what: "unknown vector curve kind" }),
    }
}

fn event_payload(r: &mut Reader<'_>, ty: i8) -> Result<()> {
    let n = match ty {
        -1 | 1 | 2 | 3 | 8 | 11 | 14 | 15 | 34 | 42 | 43 | 49 | 50 | 53 | 55 | 56 => 0,
        0 => 24,
        4 | 7 | 9 | 10 => 6,
        5 | 33 | 57 | 58 => 4,
        6 | 12 | 13 | 17 | 18 | 22 | 24 | 29 | 30 | 31 | 32 | 35 | 36 | 39 | 41 | 59 => 2,
        16 => 4,
        19 | 20 => 3,
        21 | 23 => 10,
        25 | 26 => 36,
        27 => 3,
        28 => 12,
        37 | 38 | 45 | 47 => 4,
        40 => 2,
        44 | 51 => 32,
        46 | 52 => 6,
        48 => 10,
        54 => {
            r.skip(4)?;
            r.string()?;
            return Ok(());
        }
        _ => return Err(Error::Corrupt { offset: r.pos() - 1, what: "unknown animation event type" }),
    };
    r.skip(n)
}

fn skip_events(r: &mut Reader<'_>, framed: bool) -> Result<()> {
    let n = count_i8(r)?;
    for _ in 0..n {
        let ty = r.i8()?;
        if framed {
            r.skip(2)?;
        }
        event_payload(r, ty)?;
    }
    Ok(())
}

fn skip_emitter_mesh(r: &mut Reader<'_>) -> Result<()> {
    let p = r.u16()? as usize;
    r.skip_n(p, 12)?;
    let t = r.u16()? as usize;
    r.skip_n(t, 10)?;
    r.skip(12)
}

fn skip_emitter_colors(r: &mut Reader<'_>) -> Result<()> {
    let c = r.u16()? as usize;
    if c > 0 {
        r.skip(1)?;
        r.skip_n(c, 4)?;
    }
    Ok(())
}

impl<'a, 't> Walker<'a, 't> {
    fn options(&mut self, r: &mut Reader<'_>) -> Result<OptionList> {
        let l = OptionList::read(r, self.table)?;
        self.out.counts.options += l.count as u64;
        Ok(l)
    }

    fn list(&mut self, r: &mut Reader<'_>, f: Frame) -> Result<Vec<u32>> {
        let n = r.u16()?;
        let mut ids = Vec::with_capacity(n as usize);
        for _ in 0..n {
            ids.push(self.record(r, f)?);
        }
        Ok(ids)
    }

    fn record(&mut self, r: &mut Reader<'_>, f: Frame) -> Result<u32> {
        let offset = r.pos();
        let class = r.i16()?;
        let oid = r.i16()?;
        let script = r.i16()?;
        self.record_body(r, offset, class, oid, script, f)
    }

    fn record_body(
        &mut self,
        r: &mut Reader<'_>,
        offset: usize,
        class: i16,
        oid: i16,
        script: i16,
        f: Frame,
    ) -> Result<u32> {
        if f.depth > MAX_DEPTH {
            return Err(Error::Corrupt { offset, what: "object nesting too deep" });
        }
        let layout = layout_of(class).ok_or(Error::UnknownClass { offset, class })?;
        let template = f.template || (f.context == Context::TopLevel && STOCK_TEMPLATE_OIDS.contains(&oid));
        let idx = self.out.records.len() as u32;
        self.out.records.push(Record {
            offset,
            end: offset,
            class,
            oid,
            script,
            context: f.context,
            parent: f.parent,
            transform_parent: f.transform_parent,
            depth: f.depth,
            in_world: f.in_world,
            template,
            subtree_end: idx + 1,
            local: None,
            options: None,
            mesh_set: None,
            extension: None,
        });
        let inner = Frame {
            context: Context::Child,
            parent: Some(idx),
            transform_parent: Some(idx),
            depth: f.depth + 1,
            in_world: f.in_world,
            template,
        };
        self.body(r, idx, layout, inner)?;
        let end_idx = self.out.records.len() as u32;
        let rec = &mut self.out.records[idx as usize];
        rec.end = r.pos();
        rec.subtree_end = end_idx;
        Ok(idx)
    }

    fn set_local(&mut self, idx: u32, local: Affine) {
        self.out.records[idx as usize].local = Some(local);
    }

    fn set_options(&mut self, idx: u32, l: OptionList) {
        self.out.records[idx as usize].options = Some(l);
    }

    fn origin_kind(&self, idx: u32) -> Option<i16> {
        self.out.records[idx as usize].options.and_then(|o| o.origin_kind)
    }

    fn body(&mut self, r: &mut Reader<'_>, idx: u32, layout: Layout, inner: Frame) -> Result<()> {
        match layout {
            Layout::Base => {
                let l = self.options(r)?;
                self.set_options(idx, l);
            }
            Layout::Placed => self.placed(r, idx, inner)?,
            Layout::Solid => {
                self.solid_prefix(r)?;
                self.placed(r, idx, inner)?;
            }
            Layout::Conveyor => {
                self.solid_prefix(r)?;
                self.placed(r, idx, inner)?;
                r.skip(8)?;
            }
            Layout::Emitter => {
                self.placed(r, idx, inner)?;
                self.emitter_tail(r, idx)?;
            }
            Layout::Flash => {
                self.placed(r, idx, inner)?;
                self.volume(r, idx)?;
            }
            Layout::Dialogue => {
                self.placed(r, idx, inner)?;
                let n = count_i8(r)?;
                for _ in 0..n {
                    match r.i8()? {
                        0 | 7 => r.skip(32)?,
                        1 | 2 => r.skip(4)?,
                        3 => {
                            r.skip(4 + 4 + 1 + 1 + 8)?;
                            r.string()?;
                        }
                        4 => r.skip(2)?,
                        5 | 6 => {}
                        _ => return Err(Error::Corrupt { offset: r.pos() - 1, what: "unknown dialogue event kind" }),
                    }
                }
            }
            Layout::CameraZone => {
                let t = read_transform(r)?;
                self.set_local(idx, t);
                self.volume(r, idx)?;
                let kind = r.i8()?;
                if kind == -1 {
                    r.skip(48)?;
                } else {
                    skip_curve(r, kind as i32)?;
                }
                let l = self.options(r)?;
                self.set_options(idx, l);
            }
            Layout::Proxy => self.proxy(r, idx, inner)?,
            Layout::Shape => {
                let kind = r.u8()?;
                skip_curve(r, kind as i32)?;
                let l = self.options(r)?;
                self.set_options(idx, l);
            }
            Layout::Marker => {
                let t = read_transform(r)?;
                self.set_local(idx, t);
                let l = self.options(r)?;
                self.set_options(idx, l);
            }
            Layout::Warp => {
                let t = read_transform(r)?;
                self.set_local(idx, t);
                let l = self.options(r)?;
                self.set_options(idx, l);
                self.owned_animations(r, idx)?;
                r.string()?;
                let h = count_i16(r)?;
                r.skip_n(h, 2)?;
            }
            Layout::ExplosionGroup => {
                let t = read_transform(r)?;
                self.set_local(idx, t);
                let l = self.options(r)?;
                self.set_options(idx, l);
                let m = count_i16(r)?;
                let member = Frame { context: Context::GroupMember, in_world: false, ..inner };
                for _ in 0..m {
                    self.record(r, member)?;
                }
            }
            Layout::ExplosionEmitter => {
                let t = read_transform(r)?;
                self.set_local(idx, t);
                let l = self.options(r)?;
                self.set_options(idx, l);
                self.emitter_tail(r, idx)?;
            }
            Layout::Volume => {
                let t = read_transform(r)?;
                self.set_local(idx, t);
                self.volume(r, idx)?;
                let l = self.options(r)?;
                self.set_options(idx, l);
            }
            Layout::Exit => {
                r.skip(64)?;
                let t = read_transform(r)?;
                self.set_local(idx, t);
                self.volume(r, idx)?;
                let l = self.options(r)?;
                self.set_options(idx, l);
                self.owned_animations(r, idx)?;
            }
            Layout::Point | Layout::ScanPoint => {
                let p = r.vec3()?;
                self.set_local(idx, Affine { cols: Affine::IDENTITY.cols, t: p });
                let l = self.options(r)?;
                self.set_options(idx, l);
                if layout == Layout::ScanPoint {
                    r.string()?;
                }
            }
            Layout::AnimationSegment => {
                let l = self.animation_segment(r)?;
                self.out.counts.options += l.count as u64;
                self.set_options(idx, l);
            }
            Layout::VisibilityMap => {
                let n = r.u16()? as usize;
                let nodes = r.pos();
                r.skip_n(n, 24)?;
                for i in 0..n {
                    let mut nr = Reader::at(r.data(), nodes + i * 24 + 16);
                    for _ in 0..2 {
                        let side = nr.u32()?;
                        if side & 0x8000_0000 == 0 && side as usize >= n {
                            return Err(Error::Corrupt {
                                offset: nodes + i * 24,
                                what: "visibility node child out of range",
                            });
                        }
                    }
                }
                let z = r.u16()? as usize;
                if z > 32 {
                    return Err(corrupt(r, "more than 32 visibility zones"));
                }
                r.skip_n(z, 4)?;
                let l = self.options(r)?;
                self.set_options(idx, l);
            }
            Layout::StateMachine => {
                let s = r.u8()? as usize;
                r.skip_n(s, 4)?;
                let t = r.u8()? as usize;
                let mut with_anim = 0;
                for _ in 0..t {
                    if r.i8()? != 0 {
                        with_anim += 1;
                    }
                    r.skip(12)?;
                }
                let l = self.options(r)?;
                self.set_options(idx, l);
                r.skip(2)?;
                let owned = Frame { context: Context::StateMachine, in_world: false, ..inner };
                for _ in 0..with_anim {
                    self.record(r, owned)?;
                }
            }
            Layout::PathZone => {
                let l = self.options(r)?;
                self.set_options(idx, l);
                let v = r.u16()? as usize;
                let b = r.u16()? as usize;
                let i = r.u16()? as usize;
                let t = r.u16()? as usize;
                for _ in 0..v {
                    r.skip(12)?;
                    let n = r.u8()? as usize;
                    r.skip_n(n, 2)?;
                }
                r.skip_n(b + i, 4)?;
                r.skip_n(t, 6)?;
                let n = r.u16()? as usize;
                r.skip_n(n, 20)?;
            }
            Layout::ReachMap => {
                r.skip(48 + 2)?;
                let (p, _) = skip_collision_mesh(r)?;
                r.skip(p)?;
                let towers = count_i16(r)?;
                r.skip_n(towers, 8)?;
                let nodes = count_i16(r)?;
                r.skip_n(nodes, 6)?;
                r.skip(4)?;
                let l = self.options(r)?;
                self.set_options(idx, l);
            }
            Layout::Keyhole => {
                let l = self.options(r)?;
                self.set_options(idx, l);
                let p = r.u16()? as usize;
                r.skip_n(p, 8)?;
                r.skip(16)?;
                for _ in 0..5 {
                    let c = r.u16()? as usize;
                    r.skip_n(c, 6)?;
                }
            }
        }
        Ok(())
    }

    /// Warp and exit animations: `s16` count, then 6-byte headers with animation-segment bodies (not records).
    fn owned_animations(&mut self, r: &mut Reader<'_>, owner: u32) -> Result<()> {
        let a = count_i16(r)?;
        for _ in 0..a {
            let class = r.i16()?;
            let oid = r.i16()?;
            r.skip(2)?;
            self.out.owned_animations.push(OwnedAnimation { owner, class, oid, body: r.pos() });
            let l = self.animation_segment(r)?;
            self.out.counts.animation_options += l.count as u64;
            self.out.counts.options += l.count as u64;
            self.out.counts.warp_exit_animations += 1;
        }
        Ok(())
    }

    /// An animation-segment body; returns its option list (not counted here).
    fn animation_segment(&mut self, r: &mut Reader<'_>) -> Result<OptionList> {
        r.skip(4)?;
        let roots = r.u16()? as usize;
        r.skip_n(roots, 2)?;
        let channels = count_i8(r)?;
        for _ in 0..channels {
            r.skip(2)?;
            let flags = r.i8()? as u8;
            for bit in 0..3 {
                if flags >> bit & 1 != 0 {
                    let k = r.i8()?;
                    skip_vector_curve(r, k)?;
                }
            }
            if flags & 0x08 != 0 {
                let k = r.i8()?;
                skip_scalar_curve(r, k)?;
            }
            if flags & 0x10 != 0 {
                let n = r.u8()?;
                for _ in 0..n {
                    let k = r.i8()?;
                    if k != -1 {
                        skip_scalar_curve(r, k)?;
                    }
                }
            }
        }
        skip_events(r, false)?;
        skip_events(r, true)?;
        skip_events(r, false)?;
        OptionList::read(r, self.table)
    }

    fn volume(&mut self, r: &mut Reader<'_>, record: u32) -> Result<()> {
        self.out.volumes.push(VolumeRef { record, offset: r.pos() });
        skip_convex_volume(r)
    }

    fn emitter_tail(&mut self, r: &mut Reader<'_>, idx: u32) -> Result<()> {
        let kind = r.i8()?;
        if kind != -1 {
            skip_curve(r, kind as i32)?;
        }
        if self.origin_kind(idx) == Some(ORIGIN_KIND_MESH) {
            skip_emitter_mesh(r)?;
        }
        skip_emitter_colors(r)
    }

    /// The solid-object fields before the placed-object layout (classes/solid-object.md rows 1 to 12).
    fn solid_prefix(&mut self, r: &mut Reader<'_>) -> Result<()> {
        r.skip(1)?;
        let (_, tris) = skip_collision_mesh(r)?;
        skip_collision_tree(r)?;
        r.skip(4 + 36 + 12)?;
        let groups = r.u16()? as usize;
        r.skip_n(groups, 4)?;
        let map = r.u32()? as usize;
        r.skip_n(map, 2)?;
        let hits = r.u32()? as usize;
        r.skip_n(hits, 4)?;
        if hits > 0 {
            r.skip_n(tris + 1, 2)?;
        }
        skip_collision_mesh(r)?;
        skip_collision_tree(r)
    }

    /// The placed-object layout (classes/placed-object.md section 1; geometry.md 1.1).
    fn placed(&mut self, r: &mut Reader<'_>, idx: u32, inner: Frame) -> Result<()> {
        let local = read_transform(r)?;
        self.set_local(idx, local);
        r.skip(3 + 4 + 16)?;
        let l = self.options(r)?;
        self.set_options(idx, l);
        let set = MeshSet::read(r)?;
        let poses = set.pose_count() as usize;
        self.out.records[idx as usize].mesh_set = Some(set);
        let ext = self.extension(r)?;
        self.out.records[idx as usize].extension = Some(ext);
        let k = r.u8()? as usize;
        r.skip_n(k, 2 + 4 * poses)?;
        self.list(r, inner)?;
        Ok(())
    }

    fn extension(&mut self, r: &mut Reader<'_>) -> Result<Extension> {
        let at = r.pos();
        let flags = r.u32()?;
        if flags & !0xFF != 0 {
            return Err(Error::Corrupt { offset: at, what: "unknown extension flag bits" });
        }
        if flags & 0x01 != 0 {
            r.skip(36)?;
        }
        if flags & 0x02 != 0 {
            r.skip(36)?;
        }
        let basis = if flags & 0x0C != 0 { Some(r.i16()?) } else { None };
        if flags & 0x10 != 0 {
            r.skip(2)?;
        }
        if flags & 0x20 != 0 {
            r.skip(30)?;
        }
        if flags & 0x40 != 0 {
            r.skip(4)?;
        }
        if flags & 0x80 != 0 {
            r.skip(1)?;
        }
        Ok(Extension { flags, basis, offset: at })
    }

    fn proxy(&mut self, r: &mut Reader<'_>, idx: u32, inner: Frame) -> Result<()> {
        let local = read_transform(r)?;
        self.set_local(idx, local);
        let n = r.u8()? as usize;
        let mut entries = Vec::with_capacity(n);
        let source_frame = Frame { context: Context::ProxySource, ..inner };
        for _ in 0..n {
            let at = r.pos();
            let class = r.i16()?;
            if class == -1 {
                let si = r.i16()?;
                if si < 0 || si as usize >= self.out.sources.len() {
                    return Err(Error::Corrupt { offset: at, what: "proxy back-reference to a later source" });
                }
                entries.push(ProxyEntry::BackRef { source_index: si as u32 });
            } else {
                let oid = r.i16()?;
                let script = r.i16()?;
                let rec = self.record_body(r, at, class, oid, script, source_frame)?;
                let use_count = r.i16()?;
                let source_index = self.out.sources.len() as u32;
                self.out.sources.push(rec);
                entries.push(ProxyEntry::Source { record: rec, source_index, use_count });
            }
        }
        let m = r.u8()? as u32;
        for _ in 0..m {
            r.skip(2 + 2 + 4)?;
            if r.i8()? == 2 {
                r.skip(4)?;
            }
            self.options(r)?;
        }
        if n == 1 {
            self.list(r, Frame { context: Context::ProxyChild, ..inner })?;
        }
        self.out.proxies.push(Proxy { record: idx, entries, overrides: m });
        Ok(())
    }
}

/// A drawable object to place in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// The record whose mesh set is drawn.
    pub record: u32,
    /// Its world transform.
    pub world: Affine,
    /// For a clone, the in-world proxy whose back-reference created it (for a clone nested inside a clone, the
    /// outermost one).
    pub clone_of_proxy: Option<u32>,
}

/// Options for [`ObjectStream::world_transforms`] and [`ObjectStream::placements`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacementOptions {
    /// Honour the extension block's ancestor override (bits 2 and 3).
    pub ancestor_overrides: bool,
    /// Instantiate proxy back-references as clones.
    pub clones: bool,
}

impl Default for PlacementOptions {
    fn default() -> Self {
        PlacementOptions { ancestor_overrides: true, clones: true }
    }
}

impl ObjectStream {
    /// Walks the object stream starting at `start` (the top-level `u16` count) of the world file `data`.
    pub fn walk(data: &[u8], start: usize, table: &OptionTable) -> Result<ObjectStream> {
        let mut w = Walker { table, out: ObjectStream { start, ..Default::default() }, _data: data };
        let mut r = Reader::at(data, start);
        let top = Frame {
            context: Context::TopLevel,
            parent: None,
            transform_parent: None,
            depth: 0,
            in_world: true,
            template: false,
        };
        w.out.top_level = w.list(&mut r, top)?;
        w.out.end = r.pos();
        if w.out.cloned_records() > MAX_CLONED_RECORDS {
            return Err(Error::Corrupt { offset: start, what: "proxy back-references copy too many records" });
        }
        Ok(w.out)
    }

    /// How many records the clones of every back-reference copy, nested clones included (saturating). A
    /// back-reference inside a source always names a source completed before it, so one pass in source order
    /// resolves the nesting.
    pub fn cloned_records(&self) -> u64 {
        let pairs = self.back_reference_pairs();
        let index_of: alloc::collections::BTreeMap<u32, usize> =
            self.sources.iter().enumerate().map(|(i, &r)| (r, i)).collect();
        let mut per_source = vec![0u64; self.sources.len()];
        for (si, &s) in self.sources.iter().enumerate() {
            let range = self.clone_range(s);
            let nested = Self::backrefs_in(&pairs, range.clone())
                .iter()
                .fold(0u64, |acc, &(_, src)| acc.saturating_add(per_source[index_of[&src]]));
            per_source[si] = (range.len() as u64).saturating_add(nested);
        }
        pairs.iter().fold(0u64, |acc, &(_, src)| acc.saturating_add(per_source[index_of[&src]]))
    }

    /// Records read through object lists.
    pub fn list_loaded(&self) -> usize {
        self.records.iter().filter(|r| r.context.is_list_loaded()).count()
    }

    /// Deepest nesting.
    pub fn max_depth(&self) -> u8 {
        self.records.iter().map(|r| r.depth).max().unwrap_or(0)
    }

    /// Records with a drawable mesh set.
    pub fn drawable(&self) -> usize {
        self.records.iter().filter(|r| r.is_drawable()).count()
    }

    /// Drawable records outside stock templates.
    pub fn drawable_live(&self) -> usize {
        self.records.iter().filter(|r| r.is_drawable() && !r.template).count()
    }

    /// Stock templates (top-level records with OIDs 12 to 42).
    pub fn stock_templates(&self) -> usize {
        self.records.iter().filter(|r| r.context == Context::TopLevel && STOCK_TEMPLATE_OIDS.contains(&r.oid)).count()
    }

    /// Number of back-references.
    pub fn back_references(&self) -> usize {
        self.proxies.iter().flat_map(|p| &p.entries).filter(|e| matches!(e, ProxyEntry::BackRef { .. })).count()
    }

    /// Every back-reference as (proxy record, referenced source record), sorted by proxy record.
    pub fn back_reference_pairs(&self) -> Vec<(u32, u32)> {
        let mut v: Vec<(u32, u32)> = self
            .proxies
            .iter()
            .flat_map(|p| {
                p.entries.iter().filter_map(move |e| match e {
                    ProxyEntry::BackRef { source_index } => Some((p.record, self.sources[*source_index as usize])),
                    _ => None,
                })
            })
            .collect();
        v.sort_unstable();
        v
    }

    /// Back-references whose proxy record lies in `range`, from a list made by [`Self::back_reference_pairs`].
    fn backrefs_in(pairs: &[(u32, u32)], range: core::ops::Range<u32>) -> &[(u32, u32)] {
        let lo = pairs.partition_point(|&(p, _)| p < range.start);
        let hi = pairs.partition_point(|&(p, _)| p < range.end);
        &pairs[lo..hi]
    }

    /// For each full source (by source index), the number of drawable records a clone of it holds
    /// (classes/proxy.md 3): the records read inside the source's body, plus, for every back-reference inside it,
    /// what a clone of the referenced source holds (resolved once, when the source was loaded).
    pub fn clone_drawables(&self) -> Vec<usize> {
        let pairs = self.back_reference_pairs();
        let index_of: alloc::collections::BTreeMap<u32, usize> =
            self.sources.iter().enumerate().map(|(i, &r)| (r, i)).collect();
        let mut out = vec![0usize; self.sources.len()];
        // A back-reference inside a source always names a source completed before it, so source order works.
        for (si, &s) in self.sources.iter().enumerate() {
            let end = self.clone_range(s).end;
            let own = (s..end).filter(|&k| self.records[k as usize].is_drawable()).count();
            let nested: usize = Self::backrefs_in(&pairs, s..end).iter().map(|&(_, src)| out[index_of[&src]]).sum();
            out[si] = own + nested;
        }
        out
    }

    /// worlds.md "Drawable in clones": for each back-reference, the drawable records of a clone of the referenced
    /// source (with its nested clones), summed.
    pub fn drawable_in_clones(&self) -> usize {
        let per_source = self.clone_drawables();
        let index_of: alloc::collections::BTreeMap<u32, usize> =
            self.sources.iter().enumerate().map(|(i, &r)| (r, i)).collect();
        self.back_reference_pairs().iter().map(|&(_, src)| per_source[index_of[&src]]).sum()
    }

    /// The world transform of record `i`, given a lookup for already-computed world transforms of earlier
    /// records.
    fn compose(&self, i: u32, world_of: &dyn Fn(u32) -> Affine, overrides: bool) -> Affine {
        let rec = &self.records[i as usize];
        let local = rec.local_transform();
        let parent = rec.transform_parent.map_or(Affine::IDENTITY, world_of);
        let mut w = parent.mul(&local);
        if overrides
            && let Some(ext) = rec.extension
            && let Some(basis) = ext.basis
            && let Some(b) = self.find_ancestor(i, basis)
        {
            let bw = world_of(b);
            if ext.flags & Extension::ROTATION_FROM_BASIS != 0 {
                w.cols = bw.mul(&Affine { cols: local.cols, t: [0.0; 3] }).cols;
            }
            if ext.flags & Extension::POSITION_FROM_BASIS != 0 {
                w.t = bw.transform_point(local.t);
            }
        }
        w
    }

    /// The nearest ancestor (through transform parents) of record `i` with OID `oid`.
    pub fn find_ancestor(&self, i: u32, oid: i16) -> Option<u32> {
        let mut cur = self.records[i as usize].transform_parent;
        while let Some(p) = cur {
            if self.records[p as usize].oid == oid {
                return Some(p);
            }
            cur = self.records[p as usize].transform_parent;
        }
        None
    }

    /// The record with OID `oid` nearest to record `context`: the candidate whose ancestor chain meets the
    /// context's ancestor chain closest to the context (object-stream.md 7.4). The context itself and its descendants
    /// meet it at distance 0. Chains follow transform parents (proxies count as the parent of their sources and
    /// children, transforms.md 3). Among equally near candidates the first in stream order wins. Records outside the
    /// world are candidates only when `include_out_of_world` is set.
    pub fn find_nearest(&self, context: u32, oid: i16, include_out_of_world: bool) -> Option<u32> {
        let mut chain: Vec<u32> = Vec::with_capacity(16);
        let mut cur = Some(context);
        while let Some(c) = cur {
            chain.push(c);
            cur = self.records[c as usize].transform_parent;
        }
        let mut best: Option<(usize, u32)> = None;
        for (i, rec) in self.records.iter().enumerate() {
            if rec.oid != oid || !(rec.in_world || include_out_of_world) {
                continue;
            }
            let mut score = chain.len();
            let mut cur = Some(i as u32);
            while let Some(c) = cur {
                if let Some(k) = chain.iter().position(|&a| a == c) {
                    score = k;
                    break;
                }
                cur = self.records[c as usize].transform_parent;
            }
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, i as u32));
            }
        }
        best.map(|(_, i)| i)
    }

    /// World transforms of every record (transforms.md 2 to 4). Records without a stored transform take their
    /// parent's; owned records are placed relative to their owner.
    pub fn world_transforms(&self, ancestor_overrides: bool) -> Vec<Affine> {
        let mut world: Vec<Affine> = Vec::with_capacity(self.records.len());
        for i in 0..self.records.len() as u32 {
            let w = self.compose(i, &|p| world[p as usize], ancestor_overrides);
            world.push(w);
        }
        world
    }

    /// The records a clone of `source` copies (classes/proxy.md 3): the records read inside the source's body. The
    /// child list of the source's own proxy is not part of it (the pool of clones is made before that list is read;
    /// each back-referencing proxy carries its own child list). Nested back-references inside this range are cloned
    /// with it.
    pub fn clone_range(&self, source: u32) -> core::ops::Range<u32> {
        source..self.records[source as usize].subtree_end
    }

    /// World transforms of a clone of `source` placed by a proxy whose world transform is `proxy_world`: one
    /// transform per record of [`Self::clone_range`].
    pub fn clone_transforms(
        &self,
        world: &[Affine],
        proxy_world: &Affine,
        source: u32,
        overrides: bool,
    ) -> Vec<Affine> {
        let range = self.clone_range(source);
        let own_proxy = self.records[source as usize].transform_parent;
        let mut out: Vec<Affine> = vec![Affine::IDENTITY; range.len()];
        for i in range.clone() {
            let w = {
                let lookup = |p: u32| {
                    if range.contains(&p) {
                        out[(p - source) as usize]
                    } else if Some(p) == own_proxy {
                        *proxy_world
                    } else {
                        world[p as usize]
                    }
                };
                self.compose(i, &lookup, overrides)
            };
            out[(i - source) as usize] = w;
        }
        out
    }

    /// Whether each record is placed: false when its world position has a component above 1e30 in magnitude (the
    /// "not placed" sentinel of transforms.md 4) or its transform parent is not placed.
    pub fn placed_flags(&self, world: &[Affine]) -> Vec<bool> {
        let mut placed = Vec::with_capacity(self.records.len());
        for (i, r) in self.records.iter().enumerate() {
            let parent_ok = r.transform_parent.is_none_or(|p| placed[p as usize]);
            placed.push(parent_ok && !is_sentinel_position(world[i].t));
        }
        placed
    }

    /// Every drawable object in the world with its world transform: in-world, placed records outside stock
    /// templates, then (if enabled) the drawable records of each back-reference's clone, including the clones
    /// nested inside it (classes/proxy.md 3). Objects that are not placed are left out with their descendants.
    pub fn placements(&self, opts: PlacementOptions) -> Vec<Placement> {
        self.placements_where(opts, &Record::is_drawable)
    }

    /// Like [`Self::placements`], for the records `select` accepts instead of the drawable ones (for example every
    /// record with a solid layout, to place collision bodies and their clones).
    pub fn placements_where(&self, opts: PlacementOptions, select: &dyn Fn(&Record) -> bool) -> Vec<Placement> {
        let world = self.world_transforms(opts.ancestor_overrides);
        let placed = self.placed_flags(&world);
        let mut out: Vec<Placement> = self
            .records
            .iter()
            .enumerate()
            .filter(|&(i, r)| r.in_world && !r.template && select(r) && placed[i])
            .map(|(i, _)| Placement { record: i as u32, world: world[i], clone_of_proxy: None })
            .collect();
        if opts.clones {
            let pairs = self.back_reference_pairs();
            for &(p, src) in &pairs {
                let prec = &self.records[p as usize];
                if !prec.in_world || prec.template || !placed[p as usize] {
                    continue;
                }
                let clone =
                    CloneCtx { world: &world, pairs: &pairs, proxy: p, overrides: opts.ancestor_overrides, select };
                self.place_clone(&clone, &world[p as usize], src, &mut out);
            }
        }
        out
    }

    /// Places a clone of `source` under `proxy_world`, then the clones nested inside it, depth first in stream order.
    /// Iterative: the walk has already bounded the total expansion ([`MAX_CLONED_RECORDS`]).
    fn place_clone(&self, ctx: &CloneCtx<'_>, proxy_world: &Affine, source: u32, out: &mut Vec<Placement>) {
        let mut stack: Vec<(u32, Affine)> = vec![(source, *proxy_world)];
        while let Some((source, proxy_world)) = stack.pop() {
            self.place_one_clone(ctx, &proxy_world, source, out, &mut stack);
        }
    }

    /// Places one clone and pushes its nested back-references (in reverse, so they pop in stream order).
    fn place_one_clone(
        &self,
        ctx: &CloneCtx<'_>,
        proxy_world: &Affine,
        source: u32,
        out: &mut Vec<Placement>,
        stack: &mut Vec<(u32, Affine)>,
    ) {
        let end = self.clone_range(source).end;
        let t = self.clone_transforms(ctx.world, proxy_world, source, ctx.overrides);
        let mut placed = vec![false; t.len()];
        for i in source..end {
            let k = (i - source) as usize;
            let r = &self.records[i as usize];
            let parent_ok = match r.transform_parent {
                Some(p) if p >= source && p < end => placed[(p - source) as usize],
                _ => true,
            };
            placed[k] = parent_ok && !is_sentinel_position(t[k].t);
            if placed[k] && r.in_world && (ctx.select)(r) {
                out.push(Placement { record: i, world: t[k], clone_of_proxy: Some(ctx.proxy) });
            }
        }
        for &(p, src) in Self::backrefs_in(ctx.pairs, source..end).iter().rev() {
            let k = (p - source) as usize;
            if placed[k] && self.records[p as usize].in_world {
                stack.push((src, t[k]));
            }
        }
    }
}

/// What stays the same while clones are placed.
struct CloneCtx<'a> {
    world: &'a [Affine],
    pairs: &'a [(u32, u32)],
    proxy: u32,
    overrides: bool,
    select: &'a dyn Fn(&Record) -> bool,
}

/// One override entry of a proxy (classes/proxy.md 1.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProxyOverride {
    /// File offset of the entry.
    pub offset: usize,
    /// Target OID: the first source with it, or the first object with it below a source.
    pub target: i16,
    /// Script index, −1 for none.
    pub script: i16,
    /// Zone mask for a placed target.
    pub zone_mask: u32,
    /// Distance mode (2: whole-object distance culling with `max_distance`).
    pub distance_mode: i8,
    /// Maximum render distance, when the mode is 2.
    pub max_distance: Option<f32>,
    /// Options applied to the target.
    pub options: OptionList,
}

/// A warp point's name and hidden-object list (classes/markers-and-volumes.md 5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WarpInfo {
    /// Name bytes, as stored.
    pub name: Vec<u8>,
    /// OIDs hidden while the warp plays.
    pub hidden: Vec<i16>,
}

impl ObjectStream {
    /// The override entries of `proxy` (read from `data`, classes/proxy.md 1.2).
    pub fn proxy_overrides(&self, data: &[u8], proxy: &Proxy, table: &OptionTable) -> Result<Vec<ProxyOverride>> {
        let rec = &self.records[proxy.record as usize];
        // After the header, the transform and the source count, the entries in order: a back-reference is 4 bytes,
        // a full source runs to its record's end and is followed by its use count.
        let mut at = rec.offset + 6 + 48 + 1;
        for e in &proxy.entries {
            at = match *e {
                ProxyEntry::BackRef { .. } => at + 4,
                ProxyEntry::Source { record, .. } => self.records[record as usize].end + 2,
            };
        }
        let mut r = Reader::at(data, at);
        let m = r.u8()?;
        let mut out = Vec::with_capacity(m as usize);
        for _ in 0..m {
            let offset = r.pos();
            let target = r.i16()?;
            let script = r.i16()?;
            let zone_mask = r.u32()?;
            let distance_mode = r.i8()?;
            let max_distance = if distance_mode == 2 { Some(r.f32()?) } else { None };
            let options = OptionList::read(&mut r, table)?;
            out.push(ProxyOverride { offset, target, script, zone_mask, distance_mode, max_distance, options });
        }
        Ok(out)
    }
}

impl Record {
    /// A warp point's name and hidden list (class 116), read from `data`; `None` for other layouts.
    pub fn warp_info(&self, data: &[u8], table: &OptionTable) -> Result<Option<WarpInfo>> {
        if layout_of(self.class) != Some(Layout::Warp) {
            return Ok(None);
        }
        let Some(options) = self.options else { return Ok(None) };
        let mut r = Reader::at(data, options.end);
        let a = r.i16()?;
        for _ in 0..a.max(0) {
            r.skip(6)?;
            let seg = crate::anim::Segment::parse(data, r.pos(), table)?;
            r.set_pos(seg.end);
        }
        let name = Vec::from(r.string()?);
        let h = r.i16()?;
        let hidden = (0..h.max(0)).map(|_| r.i16()).collect::<Result<Vec<i16>>>()?;
        Ok(Some(WarpInfo { name, hidden }))
    }

    /// A level exit's destination and alternative destination (class 125), decoded; `None` for other layouts.
    pub fn exit_destinations(&self, data: &[u8]) -> Result<Option<[Option<crate::disc::Descriptor>; 2]>> {
        if layout_of(self.class) != Some(Layout::Exit) {
            return Ok(None);
        }
        let mut r = Reader::at(data, self.offset + 6);
        Ok(Some([crate::disc::Descriptor::read(&mut r)?, crate::disc::Descriptor::read(&mut r)?]))
    }
}

/// Whether a position holds the "not placed" sentinel (a component of magnitude above 1e30, or not finite).
pub fn is_sentinel_position(t: [f32; 3]) -> bool {
    t.iter().any(|x| !x.is_finite() || x.abs() > 1e30)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::geometry::tests::W;

    fn header(w: &mut W, class: i16, oid: i16) {
        w.i16(class).i16(oid).i16(-1);
    }

    /// A placed-object body: transform (rotation about Z by 90 degrees if `rot`), position, culling fields,
    /// options, a mesh set (one quad submesh if `mesh`), an empty extension, no pose copies, then `children`.
    pub(crate) fn placed_body(w: &mut W, rot: bool, p: [f32; 3], mesh: bool, ext: Option<(u32, i16)>) {
        if rot {
            w.v3([0., 1., 0.]).v3([-1., 0., 0.]).v3([0., 0., 1.]);
        } else {
            w.ident();
        }
        w.v3(p).bytes(&[1, 0, 0]).u32(0x0FFF_FFFF).f32(f32::MAX).f32(f32::MAX).f32(1.).f32(1.);
        w.i16(530).f32(1.0).i16(-1);
        w.u8(0).u8(0).u8(0).u8(0);
        if mesh {
            w.u16(1).u16(0).v3([0.; 3]).f32(1.).i16(1).bytes(&[0, 3, 0x10]).u16(1).quad_submesh(0, false).u16(0);
        } else {
            w.u16(0);
        }
        match ext {
            Some((flags, basis)) => {
                w.u32(flags).i16(basis);
            }
            None => {
                w.u32(0);
            }
        }
        w.u8(0);
    }

    fn empty_collision_mesh(w: &mut W) {
        // Collision mesh: radius, P = 0, T = 0, E = 0; then the collision tree: N = 0, full count = 0.
        w.f32(0.).u16(0).u16(0).u16(0).u16(0).u16(0);
    }

    /// A synthetic stream exercising most layouts. Returns (bytes, start offset).
    pub(crate) fn synthetic_stream() -> (Vec<u8>, usize) {
        let mut w = W::default();
        w.zeros(3); // odd start so submesh padding is exercised
        let start = w.0.len();
        synthetic_stream_into(&mut w);
        w.u8(0xEE);
        (w.0, start)
    }

    /// Appends the synthetic stream at the end of `w` (padding inside depends on where it starts).
    pub(crate) fn synthetic_stream_into(w: &mut W) {
        w.u16(6);
        // 0: world mesh (class 4, OID 1): solid prefix, placed body with a mesh, no children.
        header(w, 4, 1);
        w.u8(1);
        empty_collision_mesh(w);
        w.f32(0.).zeros(36 + 12).u16(0).u32(0).u32(0);
        empty_collision_mesh(w);
        placed_body(w, false, [0.; 3], true, None);
        w.u16(0);
        // 1: a stock template (OID 20) with a drawable child.
        header(w, 2, 20);
        placed_body(w, false, [0.; 3], true, None);
        w.u16(1);
        header(w, 2, 900);
        placed_body(w, false, [1., 0., 0.], true, None);
        w.u16(0);
        // 2: placed object rotated, at (10, 0, 0), with a point child, a drawable child and a basis-override
        //    grandchild.
        header(w, 2, 100);
        placed_body(w, true, [10., 0., 0.], true, None);
        w.u16(2);
        header(w, 126, 101);
        w.v3([0., 5., 0.]).i16(-1);
        header(w, 2, 102);
        placed_body(w, false, [1., 0., 0.], true, None);
        w.u16(1);
        header(w, 2, 103);
        placed_body(w, false, [0., 0., 1.], true, Some((Extension::ROTATION_FROM_BASIS, 100)));
        w.u16(0);
        // 3: proxy at (0, 100, 0) with one full source (a drawable placed object at (0, 0, 2)) and one child.
        header(w, 83, 200);
        w.ident().v3([0., 100., 0.]).u8(1);
        w.i16(2).i16(201).i16(-1);
        placed_body(w, false, [0., 0., 2.], true, None);
        w.u16(0);
        w.i16(2); // use count
        w.u8(1).i16(201).i16(-1).u32(0).i8(2).f32(5.).i16(-1); // one override with a distance
        w.u16(1);
        header(w, 115, 202);
        w.ident().v3([3., 0., 0.]).i16(-1);
        // 4: proxy at (0, 200, 0) with a back-reference to source 0 and a second full source (state machine):
        //    two sources, so no child list.
        header(w, 83, 300);
        w.ident().v3([0., 200., 0.]).u8(2).i16(-1).i16(0);
        w.i16(136).i16(301).i16(-1);
        w.u8(1).i16(1).i16(-1).u8(2).i8(1).zeros(12).i8(0).zeros(12).i16(-1).i16(0);
        // The state machine's one animation: an animation segment with a channel and events.
        header(w, 131, 302);
        w.i16(1).i16(60).u16(1).i16(5).i8(1);
        w.i16(1).i8(0x19).i8(2).i8(2).u16(1).zeros(6).u8(1).i8(-1); // pos (blend), twist (linear, 1 key), 1 pose
        w.i8(1).i8(4).i16(0).f32(1.); // apply events: type 4
        w.i8(1).i8(54).i16(10).i16(1).i16(2).u16(2).bytes(b"hi"); // framed: subtitle
        w.i8(0).i16(-1);
        w.i16(1); // use count
        w.u8(0);
        // 5: explosion group with one member (explosion emitter with mesh origin).
        header(w, 120, 400);
        w.ident().v3([0.; 3]).i16(-1).i16(1);
        header(w, 122, 401);
        w.ident().v3([0.; 3]).i16(366).i16(4).i16(-1).i8(0).u8(0).u8(1).zeros(16);
        w.u16(1).zeros(12).u16(0).zeros(12).u16(1).u8(0).u32(0x8080_8080);
    }

    #[test]
    fn walks_synthetic_stream() {
        let (data, start) = synthetic_stream();
        let s = ObjectStream::walk(&data, start, &OptionTable::sly1()).unwrap();
        assert_eq!(data[s.end], 0xEE);
        let classes: Vec<i16> = s.records.iter().map(|r| r.class).collect();
        assert_eq!(classes, [4, 2, 2, 2, 126, 2, 2, 83, 2, 115, 83, 136, 131, 120, 122]);
        assert_eq!(s.top_level, [0, 1, 3, 7, 10, 13]);
        assert_eq!(s.records.len(), 15);
        assert_eq!(s.list_loaded(), 11);
        assert_eq!(s.max_depth(), 2);
        assert_eq!(s.stock_templates(), 1);
        assert!(s.records[2].template && !s.records[3].template);
        assert_eq!(s.drawable(), 7);
        assert_eq!(s.drawable_live(), 5);
        assert_eq!(s.sources, [8, 11]);
        assert_eq!(s.back_references(), 1);
        assert_eq!(s.drawable_in_clones(), 1);
        assert_eq!(s.records[8].context, Context::ProxySource);
        assert_eq!(s.records[9].context, Context::ProxyChild);
        assert_eq!(s.records[9].transform_parent, Some(7));
        // A clone copies the source's own subtree, never its proxy's child list (classes/proxy.md 3).
        assert_eq!(s.clone_range(8), 8..9);
        assert_eq!(s.cloned_records(), 1);
        assert!(!s.records[12].in_world && !s.records[14].in_world);
        assert_eq!(s.records[0].subtree_end, 1);
        assert_eq!(s.records[3].subtree_end, 7);
        assert_eq!(s.proxies[0].overrides, 1);
        // Seven placed bodies with one option each, and the emitter's origin kind.
        assert_eq!(s.counts.options, 8);

        let world = s.world_transforms(true);
        // Point child of the rotated object: (10,0,0) + Rz90 * (0,5,0) = (5, 0, 0).
        assert_eq!(world[4].t, [5., 0., 0.]);
        // Drawable child at local (1,0,0): (10, 1, 0), rotated like its parent.
        assert_eq!(world[5].t, [10., 1., 0.]);
        assert_eq!(world[5].cols, world[3].cols);
        // The basis override takes the rotation from OID 100 (record 3), which is the same here; position is
        // from the parent.
        assert_eq!(world[6].t, [10., 1., 1.]);
        // Proxy source and child.
        assert_eq!(world[8].t, [0., 100., 2.]);
        assert_eq!(world[9].t, [3., 100., 0.]);

        let placements = s.placements(PlacementOptions::default());
        let recs: Vec<u32> = placements.iter().map(|p| p.record).collect();
        assert_eq!(recs, [0, 3, 5, 6, 8, 8]);
        let clone = placements.last().unwrap();
        assert_eq!(clone.clone_of_proxy, Some(10));
        assert_eq!(clone.world.t, [0., 200., 2.]);
        let no_clones = s.placements(PlacementOptions { clones: false, ..Default::default() });
        assert_eq!(no_clones.len(), 5);
    }

    #[test]
    fn basis_override_replaces_parent_rotation() {
        // Parent A (OID 50) rotated, child B unrotated with its own rotation, grandchild C takes rotation from A.
        let mut w = W::default();
        w.u16(1);
        header(&mut w, 2, 50);
        placed_body(&mut w, true, [0.; 3], false, None);
        w.u16(1);
        header(&mut w, 2, 51);
        // B rotated by 90 degrees again: world rotation 180.
        placed_body(&mut w, true, [0.; 3], false, None);
        w.u16(1);
        header(&mut w, 2, 52);
        placed_body(&mut w, false, [1., 0., 0.], false, Some((Extension::ROTATION_FROM_BASIS, 50)));
        w.u16(0);
        let s = ObjectStream::walk(&w.0, 0, &OptionTable::sly1()).unwrap();
        let with = s.world_transforms(true);
        let without = s.world_transforms(false);
        assert_eq!(s.find_ancestor(2, 50), Some(0));
        assert_eq!(with[2].cols, with[0].cols);
        assert_ne!(without[2].cols, with[2].cols);
        assert_eq!(with[2].t, without[2].t);
    }

    #[test]
    fn rejects_unknown_class_and_bad_backref() {
        let mut w = W::default();
        w.u16(1);
        header(&mut w, 108, 1);
        assert!(matches!(
            ObjectStream::walk(&w.0, 0, &OptionTable::sly1()),
            Err(Error::UnknownClass { class: 108, .. })
        ));
        let mut w = W::default();
        w.u16(1);
        header(&mut w, 83, 1);
        w.ident().v3([0.; 3]).u8(1).i16(-1).i16(0).u8(0);
        assert!(ObjectStream::walk(&w.0, 0, &OptionTable::sly1()).is_err());
    }

    #[test]
    fn layout_table_covers_the_seen_classes() {
        assert_eq!(layout_of(0), None);
        assert_eq!(layout_of(108), None);
        assert_eq!(layout_of(146), None);
        assert_eq!(layout_of(147), None);
        assert_eq!((1..146).filter(|&c| layout_of(c).is_some()).count(), 127);
        assert_eq!(layout_of(5), None);
        assert_eq!(layout_of(114), None);
        assert_eq!(layout_of(57), Some(Layout::Conveyor));
        assert_eq!(layout_of(99), Some(Layout::CameraZone));
    }

    #[test]
    fn nested_back_references_are_cloned_with_their_source() {
        let mut w = W::default();
        w.u16(4);
        // A: proxy with full source S1 (drawable, at (0, 0, 1)), used by three proxies.
        header(&mut w, 83, 110);
        w.ident().v3([0.; 3]).u8(1).i16(2).i16(111).i16(-1);
        placed_body(&mut w, false, [0., 0., 1.], true, None);
        w.u16(0).i16(3).u8(0).u16(0);
        // B at (100, 0, 0): full source S2 (not drawable) holding proxy C at (0, 10, 0) with a back-reference to S1.
        header(&mut w, 83, 120);
        w.ident().v3([100., 0., 0.]).u8(1).i16(2).i16(121).i16(-1);
        placed_body(&mut w, false, [0.; 3], false, None);
        w.u16(1);
        header(&mut w, 83, 122);
        w.ident().v3([0., 10., 0.]).u8(1).i16(-1).i16(0).u8(0).u16(0);
        w.i16(2).u8(0).u16(0);
        // D at (200, 0, 0): back-reference to S2.
        header(&mut w, 83, 130);
        w.ident().v3([200., 0., 0.]).u8(1).i16(-1).i16(1).u8(0).u16(0);
        // E parked at (0, 0, FLT_MAX): back-reference to S1.
        header(&mut w, 83, 140);
        w.ident().v3([0., 0., f32::MAX]).u8(1).i16(-1).i16(0).u8(0).u16(0);
        let s = ObjectStream::walk(&w.0, 0, &OptionTable::sly1()).unwrap();
        assert_eq!(s.end, w.0.len());
        assert_eq!(s.sources.len(), 2);
        assert_eq!(s.clone_drawables(), [1, 1]);
        // C -> S1, D -> S2 (holding a copy of C's clone), E -> S1.
        assert_eq!(s.drawable_in_clones(), 3);
        let world = s.world_transforms(true);
        let placed = s.placed_flags(&world);
        let e = s.records.iter().position(|r| r.oid == 140).unwrap();
        assert!(!placed[e]);
        let pl = s.placements(PlacementOptions::default());
        let mut at: Vec<[f32; 3]> = pl.iter().map(|p| p.world.t).collect();
        at.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(at, [[0., 0., 1.], [100., 10., 1.], [200., 10., 1.]]);
        assert!(pl.iter().all(|p| p.record == 1));
    }
}
