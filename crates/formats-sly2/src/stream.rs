//! Object lists and records (object-stream.md 2) with the body layout of every class (object-stream.md 3 and
//! `classes/`).
//!
//! Every body is parsed field by field and must end exactly at its declared size. A body that does not is skipped
//! by its size and reported in [`Walk::errors`], so one bad body never stops the walk.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use formats::error::{Error, Result};
use formats::geometry::Affine;
use formats::reader::Reader;

use crate::collision::{self, CollisionPart};
use crate::exe::{OptionTable, option_ids};
use crate::geometry::MeshSet;
use crate::options::OptionList;
use crate::transforms::{TransformList, read_matrix};

/// Body layouts (object-stream.md 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Solid,
    SolidNamed,
    Placement,
    Options,
    HeaderOptions,
    HeaderOptionsName,
    Curve,
    Track,
    PathNetwork,
    TriangleSurface,
    Zone,
    Volume,
    Warp,
    Emitter,
    ExplosionGroup,
    Job,
    Task,
    ScreenMovie,
    Clip,
    StateMachine,
}

/// The body layout of a class, `None` for a class with no known layout.
pub fn layout_of(class: i16) -> Option<Layout> {
    use Layout::*;
    Some(match class {
        2 | 112..=117 | 119..=121 | 123 | 124 => Options,
        3 | 4 | 16 | 18..=21 | 35..=40 | 43..=47 => HeaderOptions,
        41 | 42 => HeaderOptionsName,
        5 => Zone,
        6 | 7 => Placement,
        9 => TriangleSurface,
        10..=14 => Curve,
        15 => Track,
        17 => PathNetwork,
        22..=25 | 27 => Volume,
        26 => Warp,
        29 | 32 | 33 => Emitter,
        31 => ExplosionGroup,
        48 | 49 | 79 | 81..=83 | 85 | 86 | 88 | 89 | 91..=108 => Solid,
        50 | 53..=55 | 57..=70 | 72..=78 => SolidNamed,
        109 => Clip,
        111 => StateMachine,
        126..=131 => ScreenMovie,
        132 => Job,
        133 => Task,
        _ => return None,
    })
}

/// A solid body (classes/solid-object.md).
#[derive(Clone, Debug, PartialEq)]
pub struct Solid {
    pub transforms: TransformList,
    pub zone_mask: u32,
    pub meshes: MeshSet,
    pub collision: Vec<CollisionPart>,
    pub options: OptionList,
    pub children: Vec<Record>,
    /// Name string (offset, length) for "solid + name" classes.
    pub name: Option<(u32, u32)>,
}

/// A prefab placement (classes/placement.md).
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub set_id: i32,
    pub kind: i8,
    pub parent: i16,
    pub matrix: Affine,
    pub zone_mask: u32,
    /// Fresh (true) or copy (false).
    pub fresh: bool,
    /// Option 26 applies to the placed root (a fresh option block, or a copy patch for object 5 or 6).
    pub remove_at_start: bool,
    /// Extra objects of a copy placement.
    pub extra: Vec<Record>,
    /// A fresh placement's blocks (classes/placement.md 1.1).
    pub blocks: Vec<PlacementBlock>,
    /// A copy placement's patches (classes/placement.md 1.2).
    pub patches: Vec<Patch>,
}

/// Block kind 5 of a fresh placement: an option list for the new object (classes/placement.md 1.1).
pub const BLOCK_OPTIONS: i32 = 5;

/// One block of a fresh placement.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacementBlock {
    pub kind: i32,
    pub class: i16,
    pub index: i16,
    /// The block's bytes: (absolute offset, size).
    pub bytes: (u32, u32),
    /// The option list of a kind-5 block.
    pub options: Option<OptionList>,
}

/// One patch of a copy placement: options for the copied object with `target` (5 and 6: the copy's root).
#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub target: i32,
    pub class: i16,
    pub index: i16,
    pub options: OptionList,
}

/// The transform header of the simple layouts: parent reference, matrix, zone mask (classes/simple-layouts.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Header {
    pub parent: i16,
    pub matrix: Affine,
    pub zone_mask: u32,
}

/// A parsed body.
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Solid(Box<Solid>),
    Placement(Box<Placement>),
    /// Any other layout: its transform header (when it has one), its options (when it has a list) and nested
    /// objects (tracks, explosion groups, state machines).
    Other {
        layout: Layout,
        header: Option<Header>,
        options: Option<OptionList>,
        children: Vec<Record>,
    },
    /// Skipped: no layout, or the layout failed (see [`Walk::errors`]).
    Unparsed,
}

/// An object record.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub kind: i8,
    pub conditions: Vec<i16>,
    pub class: i16,
    pub id: i32,
    pub script: i16,
    pub header_offset: u32,
    pub body_offset: u32,
    pub body_size: u32,
    pub body: Body,
}

impl Record {
    /// The object's own option list, when its layout has one.
    pub fn options(&self) -> Option<&OptionList> {
        match &self.body {
            Body::Solid(s) => Some(&s.options),
            Body::Other { options, .. } => options.as_ref(),
            _ => None,
        }
    }

    /// Nested records: solid children, copy-placement extras, embedded lists.
    pub fn nested(&self) -> &[Record] {
        match &self.body {
            Body::Solid(s) => &s.children,
            Body::Placement(p) => &p.extra,
            Body::Other { children, .. } => children,
            Body::Unparsed => &[],
        }
    }

    /// Marked to be removed when the level starts (object-stream.md 1.4).
    pub fn removed_at_start(&self) -> bool {
        self.options().is_some_and(|o| o.has(option_ids::REMOVE_AT_START))
    }

    /// Whether the record is loaded for level row `row` (object-stream.md 2.3; classes 132 and 133 always load).
    pub fn loaded_for(&self, row: Option<i16>) -> bool {
        self.conditions.is_empty()
            || matches!(self.class, 132 | 133)
            || row.is_none_or(|r| self.conditions.contains(&r))
    }
}

/// A body that did not parse.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BodyError {
    pub class: i16,
    pub body_offset: u32,
    pub body_size: u32,
    pub error: Error,
}

/// The walking context: the world data, the option table and the package "shared" flags by set ID.
pub struct Walk<'a, 't> {
    pub data: &'a [u8],
    pub table: &'t OptionTable,
    pub shared: &'t BTreeMap<i32, bool>,
    pub errors: Vec<BodyError>,
    /// Bodies read (every depth); those that did not parse field-exactly are in `errors`.
    pub bodies: u64,
    /// Bodies the spec's rule could not decide, parsed with the other form because it ends exactly at the body's
    /// declared size: (class, body offset, what).
    pub undecided: Vec<(i16, u32, &'static str)>,
}

fn header(r: &mut Reader<'_>) -> Result<Header> {
    let parent = r.i16()?;
    let matrix = read_matrix(r)?;
    Ok(Header { parent, matrix, zone_mask: r.u32()? })
}

fn skip_string(r: &mut Reader<'_>) -> Result<(u32, u32)> {
    let n = r.u16()? as u32;
    let at = r.pos() as u32;
    r.skip(n as usize)?;
    Ok((at, n))
}

impl<'a, 't> Walk<'a, 't> {
    pub fn new(data: &'a [u8], table: &'t OptionTable, shared: &'t BTreeMap<i32, bool>) -> Walk<'a, 't> {
        Walk { data, table, shared, errors: Vec::new(), bodies: 0, undecided: Vec::new() }
    }

    fn options(&self, r: &mut Reader<'_>) -> Result<OptionList> {
        OptionList::read(r, self.table)
    }

    /// Reads an object list (2.1).
    pub fn object_list(&mut self, r: &mut Reader<'a>) -> Result<Vec<Record>> {
        let n = r.i16()?;
        if n < 0 {
            return Err(r.corrupt("negative object count"));
        }
        (0..n).map(|_| self.record(r)).collect()
    }

    /// Reads one object record (2.2) and parses its body.
    pub fn record(&mut self, r: &mut Reader<'a>) -> Result<Record> {
        let header_offset = r.pos() as u32;
        let kind = r.i8()?;
        let conditions = if kind > 0 { (0..kind).map(|_| r.i16()).collect::<Result<Vec<_>>>()? } else { Vec::new() };
        let class = r.i16()?;
        let id = r.i32()?;
        let script = r.i16()?;
        let size = r.i32()?;
        if size < 0 {
            return Err(r.corrupt("negative body size"));
        }
        let body_offset = r.pos();
        let end = body_offset + size as usize;
        if end > self.data.len() {
            return Err(r.corrupt("body runs past the end of the world"));
        }
        // Parse within a reader limited to the body, so a layout can never read past it.
        let data: &'a [u8] = self.data;
        let mut b = Reader::at(&data[..end], body_offset);
        self.bodies += 1;
        let body = match self.body(&mut b, class) {
            Ok(body) if b.pos() == end => body,
            Ok(_) => {
                self.errors.push(BodyError {
                    class,
                    body_offset: body_offset as u32,
                    body_size: size as u32,
                    error: Error::Corrupt { offset: b.pos(), what: "body ends before its declared size" },
                });
                Body::Unparsed
            }
            Err(error) => {
                self.errors.push(BodyError { class, body_offset: body_offset as u32, body_size: size as u32, error });
                Body::Unparsed
            }
        };
        r.set_pos(end);
        Ok(Record {
            kind,
            conditions,
            class,
            id,
            script,
            header_offset,
            body_offset: body_offset as u32,
            body_size: size as u32,
            body,
        })
    }

    fn body(&mut self, r: &mut Reader<'a>, class: i16) -> Result<Body> {
        let Some(layout) = layout_of(class) else {
            return Err(Error::UnknownClass { offset: r.pos(), class });
        };
        let other = |header, options, children| Ok(Body::Other { layout, header, options: Some(options), children });
        match layout {
            Layout::Solid | Layout::SolidNamed => {
                let transforms = TransformList::read(r)?;
                let zone_mask = r.u32()?;
                let meshes = MeshSet::read(r)?;
                let c = r.i16()?;
                let mut collision = Vec::new();
                for _ in 0..c.max(0) {
                    collision.push(collision::read_part(r)?);
                }
                if c > 0 {
                    r.skip(4 + 36)?;
                }
                let options = self.options(r)?;
                let children = self.object_list(r)?;
                let name = if layout == Layout::SolidNamed { Some(skip_string(r)?) } else { None };
                Ok(Body::Solid(Box::new(Solid { transforms, zone_mask, meshes, collision, options, children, name })))
            }
            Layout::Placement => self.placement(r),
            Layout::Options => {
                let o = self.options(r)?;
                other(None, o, Vec::new())
            }
            Layout::HeaderOptions => {
                let h = header(r)?;
                let o = self.options(r)?;
                other(Some(h), o, Vec::new())
            }
            Layout::HeaderOptionsName => {
                let h = header(r)?;
                let o = self.options(r)?;
                skip_string(r)?;
                other(Some(h), o, Vec::new())
            }
            Layout::Curve => {
                let h = header(r)?;
                let tangents = r.i8()? != 0;
                r.skip(1)?;
                let n = r.u16()? as usize;
                r.skip(n * 4 + n * 12)?;
                if tangents {
                    r.skip(n * 24)?;
                }
                let o = self.options(r)?;
                other(Some(h), o, Vec::new())
            }
            Layout::Track => {
                let h = header(r)?;
                collision::read_mesh_and_tree(r)?;
                let o = self.options(r)?;
                let children = self.object_list(r)?;
                other(Some(h), o, children)
            }
            Layout::PathNetwork => {
                let h = header(r)?;
                let o = self.options(r)?;
                let v = r.u16()?;
                let t = r.u16()? as usize;
                for _ in 0..v {
                    r.skip(12)?;
                    let k = r.u8()? as usize;
                    r.skip(k * 2)?;
                }
                r.skip(t * 6)?;
                let p = r.u16()? as usize;
                r.skip(p * 16)?;
                other(Some(h), o, Vec::new())
            }
            Layout::TriangleSurface => {
                let h = header(r)?;
                let v = r.i16()?.max(0) as usize;
                r.skip(v * 12)?;
                let t = r.i16()?.max(0) as usize;
                r.skip(t * 12)?;
                let o = self.options(r)?;
                other(Some(h), o, Vec::new())
            }
            Layout::Zone => {
                let h = header(r)?;
                r.skip(4)?;
                collision::read_mesh_and_tree(r)?;
                let o = self.options(r)?;
                other(Some(h), o, Vec::new())
            }
            Layout::Volume | Layout::Warp => {
                let h = header(r)?;
                let o = self.options(r)?;
                collision::read_mesh_and_tree(r)?;
                if layout == Layout::Warp {
                    r.skip(64)?;
                    let n = r.u8()? as usize;
                    r.skip(n * 64)?;
                }
                other(Some(h), o, Vec::new())
            }
            Layout::Emitter => {
                let h = header(r)?;
                let o = self.options(r)?;
                let origin = |id| o.u32_value(self.data, id);
                let mesh = match class {
                    // classes/effects-logic-ui.md 1: class 29 when option 483 is 3, class 33 when option 542 is 5,
                    // class 32 never.
                    29 => origin(option_ids::EMITTER_MESH_KIND_29) == Some(3),
                    33 => origin(option_ids::EMITTER_ORIGIN_33) == Some(5),
                    _ => false,
                };
                let start = r.pos();
                let end = r.data().len();
                let fits = |mesh: bool| {
                    let mut t = Reader::at(r.data(), start);
                    emitter_tail(&mut t, mesh).is_ok() && t.pos() == end
                };
                let mesh = if fits(mesh) || !fits(!mesh) {
                    mesh
                } else {
                    self.undecided.push((class, start as u32, "emitter origin mesh present without its options"));
                    !mesh
                };
                emitter_tail(r, mesh)?;
                other(Some(h), o, Vec::new())
            }
            Layout::ExplosionGroup => {
                let h = header(r)?;
                let o = self.options(r)?;
                let children = self.object_list(r)?;
                other(Some(h), o, children)
            }
            Layout::Job => {
                let o = self.options(r)?;
                skip_string(r)?;
                other(None, o, Vec::new())
            }
            Layout::Task => {
                let o = self.options(r)?;
                for _ in 0..2 {
                    let n = r.u8()? as usize;
                    r.skip(n * 4)?;
                }
                skip_string(r)?;
                other(None, o, Vec::new())
            }
            Layout::ScreenMovie => {
                let o = self.options(r)?;
                let streamed = r.i8()? != 0;
                r.skip(4 + 4 + 4 + 4)?;
                if streamed {
                    r.skip(64)?;
                } else {
                    dictionary(r)?;
                    sprites(r)?;
                }
                other(None, o, Vec::new())
            }
            Layout::Clip => {
                r.skip(4)?;
                let channels = r.i16()?;
                for _ in 0..channels.max(0) {
                    r.skip(4 + 2)?;
                    let v = r.i8()?;
                    if v != -1 {
                        vector_curve(r, v)?;
                    }
                    let s = r.i8()?;
                    if s != -1 {
                        scalar_curve(r, s)?;
                    }
                    r.skip(1)?;
                }
                let extra = r.u8()?;
                for _ in 0..extra {
                    r.skip(1 + 4)?;
                    let s = r.i8()?;
                    scalar_curve(r, s)?;
                }
                event_list(r, false)?;
                event_list(r, true)?;
                event_list(r, false)?;
                let o = self.options(r)?;
                other(None, o, Vec::new())
            }
            Layout::StateMachine => {
                let children = self.object_list(r)?;
                let n = r.u8()? as usize;
                r.skip(n * 8)?;
                let m = r.u8()? as usize;
                r.skip(m * 24)?;
                let o = self.options(r)?;
                other(None, o, children)
            }
        }
    }

    fn placement(&mut self, r: &mut Reader<'a>) -> Result<Body> {
        let set_id = r.i32()?;
        let kind = r.i8()?;
        let parent = r.i16()?;
        let matrix = read_matrix(r)?;
        let zone_mask = r.u32()?;
        // The package's "shared" flag decides only for kind -1 (classes/placement.md 1). A kind -1 placement whose
        // set ID names no package of the world (a few nested ones) is read in whichever form ends at its size.
        let fresh = match kind {
            1 => true,
            -1 => match self.shared.get(&set_id) {
                Some(&shared) => shared,
                None => {
                    let start = r.pos();
                    let end = r.data().len();
                    let mut probe = Walk::new(self.data, self.table, self.shared);
                    let mut t = Reader::at(r.data(), start);
                    let copy_fits =
                        probe.placement_rest(&mut t, false).is_ok() && t.pos() == end && probe.errors.is_empty();
                    self.undecided.push((6, start as u32, "placement set ID names no package"));
                    !copy_fits
                }
            },
            _ => false,
        };
        let rest = self.placement_rest(r, fresh)?;
        Ok(Body::Placement(Box::new(Placement {
            set_id,
            kind,
            parent,
            matrix,
            zone_mask,
            fresh,
            remove_at_start: rest.remove_at_start,
            extra: rest.extra,
            blocks: rest.blocks,
            patches: rest.patches,
        })))
    }

    /// The fresh (1.1) or copy (1.2) part of a placement.
    fn placement_rest(&mut self, r: &mut Reader<'a>, fresh: bool) -> Result<PlacementRest> {
        let mut rest = PlacementRest::default();
        if fresh {
            let blocks = r.u8()?;
            for _ in 0..blocks {
                let block_kind = r.i32()?;
                let class = r.i16()?;
                let index = r.i16()?;
                let size = r.u16()? as usize;
                let start = r.pos();
                let options = if block_kind == BLOCK_OPTIONS {
                    let o = self.options(r)?;
                    if r.pos() - start != size {
                        return Err(r.corrupt("placement option block size mismatch"));
                    }
                    rest.remove_at_start |= o.has(option_ids::REMOVE_AT_START);
                    Some(o)
                } else {
                    r.skip(size)?;
                    None
                };
                rest.blocks.push(PlacementBlock {
                    kind: block_kind,
                    class,
                    index,
                    bytes: (start as u32, size as u32),
                    options,
                });
            }
            let s = r.i32()?;
            if s < 2 {
                return Err(r.corrupt("placement skip size below 2"));
            }
            r.skip(2 + (s as usize - 2))?;
        } else {
            let patches = r.u8()?;
            for _ in 0..patches {
                let target = r.i32()?;
                let class = r.i16()?;
                let index = r.i16()?;
                let size = r.u16()? as usize;
                let start = r.pos();
                let options = self.options(r)?;
                if r.pos() - start != size {
                    return Err(r.corrupt("placement patch size mismatch"));
                }
                if target == 5 || target == 6 {
                    rest.remove_at_start |= options.has(option_ids::REMOVE_AT_START);
                }
                rest.patches.push(Patch { target, class, index, options });
            }
            r.skip(4)?;
            rest.extra = self.object_list(r)?;
        }
        Ok(rest)
    }
}

/// What [`Walk::placement_rest`] reads.
#[derive(Default)]
struct PlacementRest {
    remove_at_start: bool,
    extra: Vec<Record>,
    blocks: Vec<PlacementBlock>,
    patches: Vec<Patch>,
}

/// The part of an emitter body after its options: the origin mesh (when present) and the colours.
fn emitter_tail(r: &mut Reader<'_>, mesh: bool) -> Result<()> {
    if mesh {
        let p = r.u16()? as usize;
        r.skip(p * 12)?;
        let t = r.u16()? as usize;
        r.skip(t * 10 + 12)?;
    }
    let c = r.u16()? as usize;
    if c > 0 {
        r.skip(1 + c * 4)?;
    }
    Ok(())
}

/// Vector curve of `kind` (animation.md 3.1).
fn vector_curve(r: &mut Reader<'_>, kind: i8) -> Result<()> {
    match kind {
        0 => r.skip(12),
        1 => {
            let n = r.u16()? as usize;
            r.skip(n * (2 + 36))
        }
        2 => {
            let n = r.u16()? as usize;
            r.skip(n * (2 + 24))
        }
        3 | 4 => {
            r.skip(12)?;
            let n = r.u16()? as usize;
            r.skip(n * 2)?;
            let mask = r.u8()?;
            let per = if kind == 3 { 12 } else { 8 };
            r.skip((mask & 7).count_ones() as usize * n * per)
        }
        5 => Ok(()),
        _ => Err(r.corrupt("unknown vector curve kind")),
    }
}

/// Scalar curve of `kind` (animation.md 3.2).
fn scalar_curve(r: &mut Reader<'_>, kind: i8) -> Result<()> {
    match kind {
        0 => r.skip(4),
        1 => {
            let n = r.u16()? as usize;
            r.skip(n * 14)
        }
        2 => Ok(()),
        _ => Err(r.corrupt("unknown scalar curve kind")),
    }
}

/// Event list (animation.md 2); `timed` lists carry an `s16` frame after each type byte.
fn event_list(r: &mut Reader<'_>, timed: bool) -> Result<()> {
    let n = r.u8()?;
    for _ in 0..n {
        let t = r.i8()?;
        if timed {
            r.skip(2)?;
        }
        let size = match t {
            0 => 26,
            1 | 2 | 21 => 2,
            3 => 4,
            4 | 11 | 12 | 22 => 4,
            5 => 6,
            7 | 8 => 8,
            13..=16 => 8,
            17 => 16,
            20 => 12,
            24 => 24,
            6 | 9 | 10 | 18 | 19 | 23 => 0,
            _ => return Err(r.corrupt("unknown event type")),
        };
        r.skip(size)?;
    }
    Ok(())
}

/// Screen-movie dictionary (classes/effects-logic-ui.md 4).
fn dictionary(r: &mut Reader<'_>) -> Result<()> {
    let n = r.u16()?;
    for _ in 0..n {
        r.skip(2)?;
        match r.u32()? {
            0 | 5 | 9 => {}
            1 => {
                let prims = r.u16()?;
                for _ in 0..prims {
                    let k = r.u8()? as usize;
                    r.skip(4)?;
                    r.skip(k * 9)?;
                }
            }
            2 => {
                r.skip(36)?;
                let m = r.u16()? as usize;
                r.skip(m * 24)?;
            }
            3 => {
                r.skip(4 + 4 + 12 + 1)?;
                let kind = r.i8()?;
                if kind == 0 || kind == 1 {
                    r.skip(4)?;
                }
            }
            4 => r.skip(2)?,
            6 | 7 => r.skip(8 + 4)?,
            8 => {
                dictionary(r)?;
                let c = r.u32()? as usize;
                r.skip_n(c, 4)?;
                let c = r.u32()? as usize;
                r.skip_n(c, 4)?;
            }
            _ => return Err(r.corrupt("unknown screen-movie item kind")),
        }
    }
    Ok(())
}

/// Screen-movie sprites (classes/effects-logic-ui.md 4).
fn sprites(r: &mut Reader<'_>) -> Result<()> {
    let n = r.u16()?;
    for _ in 0..n {
        r.skip(4)?;
        let labels = r.u16()?;
        for _ in 0..labels {
            r.skip(8)?;
            let keys = r.u16()?;
            for _ in 0..keys {
                r.skip(4)?;
                for size in [50usize, 8, 2, 1, 5] {
                    let c = r.u8()? as usize;
                    r.skip(c * size)?;
                }
            }
        }
    }
    Ok(())
}

/// Visits every record, depth first (records before their nested ones).
pub fn visit<'r>(records: &'r [Record], f: &mut impl FnMut(&'r Record)) {
    for rec in records {
        f(rec);
        visit(rec.nested(), f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts() {
        assert_eq!(layout_of(49), Some(Layout::Solid));
        assert_eq!(layout_of(53), Some(Layout::SolidNamed));
        assert_eq!(layout_of(6), Some(Layout::Placement));
        assert_eq!(layout_of(126), Some(Layout::ScreenMovie));
        assert_eq!(layout_of(1), None);
        assert_eq!(layout_of(110), None);
    }

    #[test]
    fn record_framing_and_options_body() {
        let table = OptionTable::default();
        let shared = BTreeMap::new();
        // kind 2 with conditions (3, 7), class 2 (options only), object 9, no script, body "FF FF".
        let mut d = alloc::vec![1u8, 0]; // object count 1
        d.push(2);
        d.extend_from_slice(&3i16.to_le_bytes());
        d.extend_from_slice(&7i16.to_le_bytes());
        d.extend_from_slice(&2i16.to_le_bytes());
        d.extend_from_slice(&9i32.to_le_bytes());
        d.extend_from_slice(&(-1i16).to_le_bytes());
        d.extend_from_slice(&2i32.to_le_bytes());
        d.extend_from_slice(&[0xFF, 0xFF]);
        // A second record of an unknown class: skipped by size, reported.
        d[0] = 2;
        d.push(0);
        d.extend_from_slice(&1i16.to_le_bytes());
        d.extend_from_slice(&1i32.to_le_bytes());
        d.extend_from_slice(&(-1i16).to_le_bytes());
        d.extend_from_slice(&3i32.to_le_bytes());
        d.extend_from_slice(&[1, 2, 3]);
        let mut w = Walk::new(&d, &table, &shared);
        let mut r = Reader::new(&d);
        let list = w.object_list(&mut r).unwrap();
        assert_eq!(r.pos(), d.len());
        assert_eq!(list[0].conditions, [3, 7]);
        assert!(list[0].loaded_for(Some(7)) && !list[0].loaded_for(Some(4)) && list[0].loaded_for(None));
        assert!(matches!(list[0].body, Body::Other { .. }));
        assert_eq!(list[1].body, Body::Unparsed);
        assert_eq!(w.errors.len(), 1);
        assert_eq!(w.bodies, 2);
    }

    #[test]
    fn events_and_curves() {
        // One timed event of type 5 (frame, then s16 + f32), one of type 9 (nothing).
        let d = [2u8, 5, 10, 0, 1, 0, 0, 0, 0, 0, 9, 0, 0];
        let mut r = Reader::new(&d);
        event_list(&mut r, true).unwrap();
        assert_eq!(r.pos(), d.len());
        // Vector curve kind 4 with 2 keys animating x and z.
        let mut c = alloc::vec![0u8; 12];
        c.extend_from_slice(&2u16.to_le_bytes());
        c.extend_from_slice(&[0, 0, 5, 0]);
        c.push(0b101);
        c.extend_from_slice(&[0u8; 2 * 2 * 8]);
        let mut r = Reader::new(&c);
        vector_curve(&mut r, 4).unwrap();
        assert_eq!(r.pos(), c.len());
    }
}
