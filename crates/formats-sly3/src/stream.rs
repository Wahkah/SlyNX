//! Object lists and records (object-stream.md 2) with the body layout of every class on the disc
//! (object-stream.md 3 and `classes/`).
//!
//! Every body is parsed field by field inside a reader limited to the body and must end exactly at its declared
//! size. A body that does not is skipped by its size and reported in [`Walk::errors`], so one bad body never stops
//! the walk. On the disc exactly one body fails: a 360-byte class-157 body whose layout is open
//! (classes/effects-logic-ui.md 5).

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use formats::error::{Error, Result};
use formats::geometry::Affine;
use formats::reader::Reader;
use formats_sly2::exe::OptionTable;
use formats_sly2::record::FileRecord;
use formats_sly2::transforms::{TransformList, read_matrix};

use crate::anim::ClipData;
use crate::collision::{self, CollisionPart, read_mesh_and_tree};
use crate::filter::{Context, Filter};
use crate::geometry::MeshSet;
use crate::options::{self, OptionList};

/// Body layouts (object-stream.md 3.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Solid,
    SolidNamed,
    Placement,
    Options,
    HeaderOptions,
    HeaderOptionsName,
    OptionsName,
    Task,
    Volume,
    Shape,
    ShapeKeys,
    Path,
    MeshZone,
    Zone,
    Grouper,
    Emitter,
    Effect,
    KeyList,
    StreamList,
    PairList,
    Screen,
    Clip,
    StateMachine,
    CommandScript,
}

/// The placement class.
pub const PLACEMENT: i16 = 8;
/// Bind target naming the placement itself (classes/placement.md 1).
pub const BIND_SELF: i32 = 7;
/// Bind target naming the copy's first object.
pub const BIND_FIRST: i32 = 8;

/// The body layout of a class, `None` for a class with no known layout.
pub fn layout_of(class: i16) -> Option<Layout> {
    use Layout::*;
    Some(match class {
        2 | 133 | 134 | 136..=139 | 141 | 142 | 144 | 145 | 147..=150 | 152..=154 | 156 => Options,
        3..=6 | 15 | 17 | 18 | 33..=35 | 37 | 38 | 41..=44 | 46 => HeaderOptions,
        39 | 40 => HeaderOptionsName,
        168 | 170 => OptionsName,
        169 => Task,
        7 => Volume,
        8 => Placement,
        19..=21 | 23 | 25 => Shape,
        24 => ShapeKeys,
        9..=13 => Path,
        16 => MeshZone,
        14 => Zone,
        29 => Grouper,
        27 | 30 | 31 => Emitter,
        45 => Effect,
        143 => KeyList,
        151 => StreamList,
        157 => PairList,
        158..=167 => Screen,
        47 | 48 | 89..=94 | 96..=98 | 101 | 103..=129 => Solid,
        49 | 50 | 52..=88 => SolidNamed,
        130 | 131 => Clip,
        132 => StateMachine,
        135 => CommandScript,
        _ => return None,
    })
}

/// The transform header: parent reference, matrix, zone mask.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Header {
    pub parent: i16,
    pub matrix: Affine,
    pub zone_mask: u32,
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

/// A placement bind (classes/placement.md 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Bind {
    pub target: i32,
    pub class: i16,
    pub index: i16,
    pub options: OptionList,
}

/// A prefab placement (classes/placement.md).
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub group_id: i32,
    pub mode: i8,
    pub parent: i16,
    pub matrix: Affine,
    pub zone_mask: u32,
    pub binds: Vec<Bind>,
    /// Fresh (true) or copy (false).
    pub fresh: bool,
    /// The fresh data size, or the copy's leading word.
    pub word: i32,
    /// A copy's extra objects, or the objects of a fresh placement's data (which the game skips).
    pub objects: Vec<Record>,
}

impl Placement {
    /// Option 27 in a bind naming the placement or the copy's first object.
    pub fn removed_at_start(&self) -> bool {
        self.binds.iter().any(|b| matches!(b.target, BIND_SELF | BIND_FIRST) && b.options.has(options::REMOVE_AT_START))
    }
}

/// An emission mesh (classes/effects-logic-ui.md 1).
#[derive(Clone, Debug, PartialEq)]
pub struct EmissionMesh {
    pub positions: Vec<[f32; 3]>,
    /// Triangles with their areas.
    pub triangles: Vec<([u16; 3], f32)>,
    pub extra: [f32; 3],
}

/// An emitter (classes 27, 30, 31).
#[derive(Clone, Debug, PartialEq)]
pub struct Emitter {
    pub header: Header,
    pub options: OptionList,
    pub mesh: Option<EmissionMesh>,
    pub colors: Vec<u32>,
}

/// An effect component (class 191 or 192).
#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub mode: i8,
    pub filter: Filter,
    pub class: i16,
    pub id: i32,
    pub script: i16,
    pub body_offset: u32,
    pub body_size: u32,
    pub options: OptionList,
    pub mesh: Option<EmissionMesh>,
}

/// An effect (class 45).
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub header: Header,
    pub options: OptionList,
    pub components: Vec<Component>,
}

/// A screen-object element (classes/effects-logic-ui.md 6.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    /// The set index, or the character code inside a kind-8 element.
    pub index: u16,
    pub kind: u32,
    pub offset: u32,
    /// Kind 8: the glyph elements.
    pub nested: Vec<Element>,
}

/// A screen object (classes 158 to 167).
#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub options: OptionList,
    /// External element data.
    pub external: Option<FileRecord>,
    pub canvas: (u32, u32),
    pub elements: Vec<Element>,
    pub groups: u16,
}

/// A clip (classes 130, 131).
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub options: OptionList,
    pub deferred: bool,
    pub data: Option<ClipData>,
}

/// A parsed body.
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Solid(Box<Solid>),
    Placement(Box<Placement>),
    Emitter(Box<Emitter>),
    Effect(Box<Effect>),
    Screen(Box<Screen>),
    Clip(Box<Clip>),
    /// Any other layout: its header, options and nested objects (zones, groupers, state machines).
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
    pub mode: i8,
    pub filter: Filter,
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
            Body::Emitter(e) => Some(&e.options),
            Body::Effect(e) => Some(&e.options),
            Body::Screen(s) => Some(&s.options),
            Body::Clip(c) => Some(&c.options),
            Body::Other { options, .. } => options.as_ref(),
            Body::Placement(_) | Body::Unparsed => None,
        }
    }

    /// Nested records: solid children, placement objects, zone, grouper and state-machine lists.
    pub fn nested(&self) -> &[Record] {
        match &self.body {
            Body::Solid(s) => &s.children,
            Body::Placement(p) => &p.objects,
            Body::Other { children, .. } => children,
            _ => &[],
        }
    }

    /// Marked to be removed when the level starts (option 27).
    pub fn removed_at_start(&self) -> bool {
        match &self.body {
            Body::Placement(p) => p.removed_at_start(),
            _ => self.options().is_some_and(|o| o.has(options::REMOVE_AT_START)),
        }
    }

    /// Whether the record loads in `ctx`: mode-1 records need their world filter and conditions to match.
    pub fn loaded(&self, ctx: &Context) -> bool {
        self.mode != 1 || self.filter.matches(ctx)
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

/// The walking context.
pub struct Walk<'a, 't> {
    pub data: &'a [u8],
    pub table: &'t OptionTable,
    /// Group streaming flags by group id (decides fresh or copy placements).
    pub streaming: &'t BTreeMap<i32, bool>,
    /// Packed-tangent multipliers.
    pub multipliers: [f32; 3],
    pub errors: Vec<BodyError>,
    /// Object bodies read (every depth, effect components not included).
    pub bodies: u64,
    /// Effect component bodies read.
    pub components: u64,
    /// Object lists entered and not yet left.
    depth: u32,
}

/// Deepest nesting of object lists read (the disc's are a few levels deep).
pub const MAX_OBJECT_DEPTH: u32 = 64;

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

fn emission_mesh(r: &mut Reader<'_>) -> Result<EmissionMesh> {
    let n = r.u16()?;
    let positions = (0..n).map(|_| r.vec3()).collect::<Result<Vec<_>>>()?;
    let t = r.u16()?;
    let mut triangles = Vec::with_capacity(t as usize);
    for _ in 0..t {
        let tri = [r.u16()?, r.u16()?, r.u16()?];
        triangles.push((tri, r.f32()?));
    }
    Ok(EmissionMesh { positions, triangles, extra: r.vec3()? })
}

/// Argument letters of a command (classes/animation.md 3).
pub fn command_args(id: u8) -> &'static [u8] {
    match id {
        0x00 => b"u",
        0x0C | 0x53 => b"B",
        0x0D => b"Bw",
        0x19 => b"wwwb",
        0x1D => b"cef",
        0x1F => b"bb",
        0x22 => b"wbb",
        0x2E => b"www",
        0x2F => b"ew",
        0x30 => b"wffwb",
        0x33 => b"wbbfw",
        0x34 => b"wfbfw",
        0x35 | 0x37 => b"wff",
        0x36 => b"wbbb",
        0x40 => b"fb",
        0x43 => b"hwwwfff",
        0x45 => b"vvfffuwwvw",
        0x46 => b"wwvvfwww",
        0x4A => b"vvf",
        0x4E => b"ww",
        0x52 => b"Bbwwwf",
        0x58 => b"fw",
        0x5C => b"wbwb",
        0x6B => b"wwubw",
        0x6C => b"wP",
        0x76 => b"wbbbb",
        0x3E | 0x44 | 0x59 | 0x5A => b"h",
        0x55 | 0x56 => b"hb",
        0x3F | 0x4F | 0x57 | 0x73 => b"b",
        0x4B | 0x5B => b"f",
        0x41 | 0x42 => b"wwf",
        0x26 | 0x38 | 0x39 | 0x60 => b"wf",
        0x04 | 0x07 | 0x08 | 0x0F | 0x10 | 0x1A | 0x28 | 0x2A | 0x3A..=0x3D | 0x62 | 0x63 | 0x68 => b"ww",
        0x0E | 0x13 | 0x1C | 0x1E | 0x6F | 0x70 | 0x74 => b"wb",
        0x16 | 0x75 => b"wwb",
        0x0B
        | 0x11
        | 0x12
        | 0x15
        | 0x17
        | 0x18
        | 0x1B
        | 0x20
        | 0x21
        | 0x24
        | 0x25
        | 0x29
        | 0x2B..=0x2D
        | 0x48
        | 0x49
        | 0x4D
        | 0x51
        | 0x61
        | 0x65..=0x67
        | 0x6A
        | 0x71
        | 0x72
        | 0x77 => b"w",
        _ => b"",
    }
}

/// One command of a command script (scripts.md 7, classes/animation.md 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    /// Command ID.
    pub id: u8,
    /// Object ID (−1 none).
    pub object: i32,
    /// Flags (bit 0: wait for the command's completion).
    pub flags: i8,
    /// First child and next sibling (low 11 bits used; 0 none).
    pub child: u16,
    pub sibling: u16,
    /// The argument bytes: (absolute offset, length).
    pub args: (u32, u32),
}

/// Reads a command list (`s16` count, then the commands) at the reader's position.
pub fn read_commands(r: &mut Reader<'_>) -> Result<Vec<Command>> {
    let n = r.i16()?;
    let mut out = Vec::with_capacity(n.max(0) as usize);
    for _ in 0..n.max(0) {
        let id = r.u8()?;
        let object = r.i32()?;
        let flags = r.i8()?;
        let child = r.u16()?;
        let sibling = r.u16()?;
        let at = r.pos();
        command_arguments(r, id)?;
        out.push(Command { id, object, flags, child, sibling, args: (at as u32, (r.pos() - at) as u32) });
    }
    Ok(out)
}

/// The option list and commands of a command-script body (class 135) at `body_offset`.
pub fn command_script(data: &[u8], body_offset: u32, table: &OptionTable) -> Result<(OptionList, Vec<Command>)> {
    let mut r = Reader::at(data, body_offset as usize);
    let o = options::read(&mut r, table)?;
    Ok((o, read_commands(&mut r)?))
}

/// Sly 3 hook (guards.md 4a): a path's points (classes 9 to 13, classes/paths-and-volumes.md 4), in the path's own
/// space.
pub fn path_points(data: &[u8], body_offset: u32) -> Result<Vec<[f32; 3]>> {
    let mut r = Reader::at(data, body_offset as usize);
    header(&mut r)?;
    r.skip(2)?;
    let n = r.u16()? as usize;
    r.skip(n * 4)?;
    (0..n).map(|_| Ok([r.f32()?, r.f32()?, r.f32()?])).collect()
}

/// A level exit's body (class 24, level-flow.md 4, classes/paths-and-volumes.md 3): the trigger volume and the file
/// records it leads to.
#[derive(Clone, Debug, PartialEq)]
pub struct ExitBody {
    /// The transform header.
    pub header: Header,
    /// The option list.
    pub options: OptionList,
    /// The trigger volume's mesh, in the exit's space.
    pub mesh: collision::Mesh,
    /// Its tree (inside is decided by it).
    pub tree: formats::collision::WideCollisionTree,
    /// The destination world's file record (its identity is the world id).
    pub destination: FileRecord,
    /// The extra file records (movies played on the way).
    pub extra: Vec<FileRecord>,
}

/// Reads an exit body (class 24) at `body_offset`.
pub fn exit_body(data: &[u8], body_offset: u32, table: &OptionTable) -> Result<ExitBody> {
    let mut r = Reader::at(data, body_offset as usize);
    let header = header(&mut r)?;
    let options = options::read(&mut r, table)?;
    let mesh = collision::read_mesh(&mut r)?;
    let at = r.pos();
    let (nodes, _) = collision::read_raw_tree(&mut r)?;
    let tree = collision::convert_tree(&nodes, mesh.triangles.len())
        .ok_or(Error::Corrupt { offset: at, what: "exit volume tree that does not convert" })?;
    let destination = FileRecord::decode(r.bytes(FileRecord::SIZE)?);
    let n = r.u8()? as usize;
    let extra = (0..n).map(|_| r.bytes(FileRecord::SIZE).map(FileRecord::decode)).collect::<Result<Vec<_>>>()?;
    Ok(ExitBody { header, options, mesh, tree, destination, extra })
}

fn command_arguments(r: &mut Reader<'_>, id: u8) -> Result<()> {
    for &letter in command_args(id) {
        match letter {
            b'b' => r.skip(1)?,
            b'h' | b'u' => r.skip(2)?,
            b'w' | b'f' => r.skip(4)?,
            b'v' | b'c' => r.skip(12)?,
            b'e' => r.skip(8)?,
            b'B' => {
                let n = r.i8()?.max(0) as usize;
                r.skip(n * 4)?;
            }
            _ => {
                let n = r.i8()?.max(0) as usize;
                r.skip(n * 8)?;
            }
        }
    }
    Ok(())
}

fn element(r: &mut Reader<'_>, index: u16, depth: u32) -> Result<Element> {
    if depth > 8 {
        return Err(r.corrupt("screen elements nested too deeply"));
    }
    let offset = r.pos() as u32;
    let kind = r.u32()?;
    let mut nested = Vec::new();
    match kind {
        1 => {
            r.skip(4)?;
            for _ in 0..r.u16()? {
                let v = r.u8()? as usize;
                r.skip(4)?;
                r.skip_n(v, 4 + 1 + 2 + 2)?;
            }
        }
        3 => {
            r.skip(4 + 12 + 1)?;
            let text = r.i8()?;
            r.skip(1)?;
            if matches!(text, 0 | 1) {
                r.skip(4)?;
            }
        }
        4 => r.skip(4 + 2)?,
        6 | 7 => r.skip(8 + 4)?,
        8 => {
            r.skip(1)?;
            for _ in 0..r.u16()? {
                let c = r.u16()?;
                r.skip(4)?;
                nested.push(element(r, c, depth + 1)?);
            }
        }
        0 | 2 | 5 | 9 => {}
        _ => return Err(r.corrupt("unknown screen element kind")),
    }
    Ok(Element { index, kind, offset, nested })
}

fn group_tree(r: &mut Reader<'_>) -> Result<u16> {
    let groups = r.u16()?;
    for _ in 0..groups {
        r.skip(4)?;
        for _ in 0..r.u16()? {
            r.skip(8)?;
            for _ in 0..r.u16()? {
                r.skip(4)?;
                let i = r.u8()? as usize;
                let c = r.u8()? as usize;
                let w = r.u8()? as usize;
                r.skip(2 + 4)?;
                r.skip(i * (2 + 4 + 4 + 36) + c * 8 + w)?;
            }
        }
    }
    Ok(groups)
}

impl<'a, 't> Walk<'a, 't> {
    pub fn new(
        data: &'a [u8],
        table: &'t OptionTable,
        streaming: &'t BTreeMap<i32, bool>,
        multipliers: [f32; 3],
    ) -> Walk<'a, 't> {
        Walk { data, table, streaming, multipliers, errors: Vec::new(), bodies: 0, components: 0, depth: 0 }
    }

    fn options(&self, r: &mut Reader<'_>) -> Result<OptionList> {
        options::read(r, self.table)
    }

    /// Reads an object list (2.1).
    pub fn object_list(&mut self, r: &mut Reader<'a>) -> Result<Vec<Record>> {
        let n = r.i16()?;
        if n < 0 {
            return Err(r.corrupt("negative object count"));
        }
        // Lists nest through bodies; corrupt data must not recurse without bound.
        if self.depth >= MAX_OBJECT_DEPTH {
            return Err(r.corrupt("object lists nested too deeply"));
        }
        self.depth += 1;
        let list = (0..n).map(|_| self.record(r)).collect();
        self.depth -= 1;
        list
    }

    /// Reads a record header (2.2): mode, filter, class, ID, script, size; returns the body's end.
    fn record_header(&self, r: &mut Reader<'_>) -> Result<(i8, Filter, i16, i32, i16, usize, usize)> {
        let mode = r.i8()?;
        let filter = if mode == 1 {
            let f = r.u8()? as usize;
            let c = r.u8()? as usize;
            Filter::read(r, f, c)?
        } else {
            Filter::default()
        };
        let class = r.i16()?;
        let id = r.i32()?;
        let script = r.i16()?;
        let size = r.i32()?;
        if size < 0 {
            return Err(r.corrupt("negative body size"));
        }
        let start = r.pos();
        let end = start.saturating_add(size as usize);
        if end > r.data().len() {
            return Err(r.corrupt("body runs past its container"));
        }
        Ok((mode, filter, class, id, script, start, end))
    }

    /// Reads one object record (2.2) and parses its body.
    pub fn record(&mut self, r: &mut Reader<'a>) -> Result<Record> {
        let header_offset = r.pos() as u32;
        let (mode, filter, class, id, script, start, end) = self.record_header(r)?;
        let data: &'a [u8] = self.data;
        let mut b = Reader::at(&data[..end], start);
        self.bodies += 1;
        let size = (end - start) as u32;
        let body = match self.body(&mut b, class) {
            Ok(body) if b.pos() == end => body,
            Ok(_) => {
                self.errors.push(BodyError {
                    class,
                    body_offset: start as u32,
                    body_size: size,
                    error: Error::Corrupt { offset: b.pos(), what: "body ends before its declared size" },
                });
                Body::Unparsed
            }
            Err(error) => {
                self.errors.push(BodyError { class, body_offset: start as u32, body_size: size, error });
                Body::Unparsed
            }
        };
        r.set_pos(end);
        Ok(Record { mode, filter, class, id, script, header_offset, body_offset: start as u32, body_size: size, body })
    }

    fn other(layout: Layout, header: Option<Header>, options: OptionList, children: Vec<Record>) -> Body {
        Body::Other { layout, header, options: Some(options), children }
    }

    fn body(&mut self, r: &mut Reader<'a>, class: i16) -> Result<Body> {
        let Some(layout) = layout_of(class) else {
            return Err(Error::UnknownClass { offset: r.pos(), class });
        };
        use Layout::*;
        Ok(match layout {
            Solid | SolidNamed => {
                let transforms = TransformList::read(r)?;
                let zone_mask = r.u32()?;
                let meshes = MeshSet::read(r)?;
                let c = r.i16()?;
                let mut parts = Vec::new();
                for _ in 0..c.max(0) {
                    parts.push(collision::read_part(r)?);
                }
                if c > 0 {
                    r.skip(4 + 36)?;
                }
                let options = self.options(r)?;
                let children = self.object_list(r)?;
                let name = if layout == SolidNamed { Some(skip_string(r)?) } else { None };
                Body::Solid(Box::new(self::Solid {
                    transforms,
                    zone_mask,
                    meshes,
                    collision: parts,
                    options,
                    children,
                    name,
                }))
            }
            Placement => self.placement(r)?,
            Options => Self::other(layout, None, self.options(r)?, Vec::new()),
            HeaderOptions => {
                let h = header(r)?;
                Self::other(layout, Some(h), self.options(r)?, Vec::new())
            }
            HeaderOptionsName => {
                let h = header(r)?;
                let o = self.options(r)?;
                skip_string(r)?;
                Self::other(layout, Some(h), o, Vec::new())
            }
            OptionsName => {
                let o = self.options(r)?;
                skip_string(r)?;
                Self::other(layout, None, o, Vec::new())
            }
            Task => {
                let o = self.options(r)?;
                for _ in 0..2 {
                    let n = r.u8()? as usize;
                    r.skip(n * 4)?;
                }
                skip_string(r)?;
                Self::other(layout, None, o, Vec::new())
            }
            Volume => {
                let h = header(r)?;
                r.skip(4)?;
                read_mesh_and_tree(r)?;
                Self::other(layout, Some(h), self.options(r)?, Vec::new())
            }
            Shape | ShapeKeys => {
                let h = header(r)?;
                let o = self.options(r)?;
                read_mesh_and_tree(r)?;
                if layout == ShapeKeys {
                    r.skip(FileRecord::SIZE)?;
                    let n = r.u8()? as usize;
                    r.skip(n * FileRecord::SIZE)?;
                }
                Self::other(layout, Some(h), o, Vec::new())
            }
            Path => {
                let h = header(r)?;
                let tangents = r.i8()? != 0;
                r.skip(1)?;
                let n = r.u16()? as usize;
                r.skip(n * 4 + n * 12)?;
                if tangents {
                    r.skip(n * 24)?;
                }
                Self::other(layout, Some(h), self.options(r)?, Vec::new())
            }
            MeshZone => {
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
                Self::other(layout, Some(h), o, Vec::new())
            }
            Zone => {
                let h = header(r)?;
                read_mesh_and_tree(r)?;
                let o = self.options(r)?;
                let children = self.object_list(r)?;
                Self::other(layout, Some(h), o, children)
            }
            Grouper => {
                let h = header(r)?;
                let o = self.options(r)?;
                let children = self.object_list(r)?;
                Self::other(layout, Some(h), o, children)
            }
            Emitter => {
                let h = header(r)?;
                let o = self.options(r)?;
                let id = if class == 27 { options::EMITTER_POSITION_27 } else { options::EMITTER_POSITION_30 };
                let mesh = if options::first_argument(&o, self.data, self.table, id) == Some(3) {
                    Some(emission_mesh(r)?)
                } else {
                    None
                };
                let n = r.u16()?;
                let mut colors = Vec::new();
                if n > 0 {
                    r.skip(1)?;
                    for _ in 0..n {
                        colors.push(r.u32()?);
                    }
                }
                Body::Emitter(Box::new(self::Emitter { header: h, options: o, mesh, colors }))
            }
            Effect => {
                let h = header(r)?;
                let o = self.options(r)?;
                let n = r.i16()?;
                let mut components = Vec::with_capacity(n.max(0) as usize);
                for _ in 0..n.max(0) {
                    components.push(self.component(r)?);
                }
                Body::Effect(Box::new(self::Effect { header: h, options: o, components }))
            }
            KeyList => {
                let o = self.options(r)?;
                let n = r.u8()? as usize;
                r.skip(n * (4 + 1 + 1 + 4 + 4 + FileRecord::SIZE))?;
                Self::other(layout, None, o, Vec::new())
            }
            StreamList => {
                let o = self.options(r)?;
                let n = r.i32()?.max(0) as usize;
                r.skip(n * (FileRecord::SIZE + 8))?;
                for _ in 0..r.i32()?.max(0) {
                    match r.i8()? {
                        1 => r.skip(8)?,
                        0 | 2 => r.skip(4)?,
                        _ => {}
                    }
                }
                Self::other(layout, None, o, Vec::new())
            }
            PairList => {
                let o = self.options(r)?;
                let n = r.u16()? as usize;
                r.skip(n * 8)?;
                Self::other(layout, None, o, Vec::new())
            }
            Screen => {
                let o = self.options(r)?;
                let external = r.i8()? != 0;
                r.skip(4)?;
                let canvas = (r.u32()?, r.u32()?);
                r.skip(4)?;
                let (external, elements, groups) = if external {
                    (Some(FileRecord::decode(r.bytes(FileRecord::SIZE)?)), Vec::new(), 0)
                } else {
                    let n = r.u16()?;
                    let mut elements = Vec::with_capacity(n as usize);
                    for _ in 0..n {
                        let index = r.u16()?;
                        elements.push(element(r, index, 0)?);
                    }
                    (None, elements, group_tree(r)?)
                };
                Body::Screen(Box::new(self::Screen { options: o, external, canvas, elements, groups }))
            }
            Clip => {
                let o = self.options(r)?;
                let deferred = r.i8()? != 0;
                let data = if deferred { None } else { Some(ClipData::read(r, &self.multipliers)?) };
                Body::Clip(Box::new(self::Clip { options: o, deferred, data }))
            }
            StateMachine => {
                let children = self.object_list(r)?;
                let a = r.u8()? as usize;
                r.skip(a * 8)?;
                let b = r.u8()? as usize;
                r.skip(b * 24)?;
                let o = self.options(r)?;
                Self::other(layout, None, o, children)
            }
            CommandScript => {
                let o = self.options(r)?;
                read_commands(r)?;
                Self::other(layout, None, o, Vec::new())
            }
        })
    }

    fn component(&mut self, r: &mut Reader<'a>) -> Result<Component> {
        let (mode, filter, class, id, script, start, end) = self.record_header(r)?;
        self.components += 1;
        let data: &'a [u8] = self.data;
        let mut b = Reader::at(&data[..end], start);
        let options = self.options(&mut b)?;
        let mesh = if options::first_argument(&options, data, self.table, options::COMPONENT_POSITION) == Some(3) {
            Some(emission_mesh(&mut b)?)
        } else {
            None
        };
        if b.pos() != end {
            return Err(Error::Corrupt { offset: b.pos(), what: "effect component does not end at its size" });
        }
        r.set_pos(end);
        Ok(Component {
            mode,
            filter,
            class,
            id,
            script,
            body_offset: start as u32,
            body_size: (end - start) as u32,
            options,
            mesh,
        })
    }

    fn placement(&mut self, r: &mut Reader<'a>) -> Result<Body> {
        let group_id = r.i32()?;
        let mode = r.i8()?;
        let parent = r.i16()?;
        let matrix = read_matrix(r)?;
        let zone_mask = r.u32()?;
        let n = r.u8()?;
        let mut binds = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let target = r.i32()?;
            let class = r.i16()?;
            let index = r.i16()?;
            let size = r.u16()? as usize;
            let start = r.pos();
            let options = self.options(r)?;
            if r.pos() - start != size {
                return Err(Error::Corrupt { offset: start, what: "bind option list does not match its size" });
            }
            binds.push(Bind { target, class, index, options });
        }
        let streaming = self.streaming.get(&group_id).copied().unwrap_or(false);
        let fresh = mode > 0 || (mode == -1 && streaming);
        let word = r.i32()?;
        let objects = if fresh {
            if word < 0 {
                return Err(r.corrupt("negative fresh-placement size"));
            }
            let start = r.pos();
            let end = start.saturating_add(word as usize);
            if end > r.data().len() {
                return Err(r.corrupt("fresh-placement data runs past the body"));
            }
            let data: &'a [u8] = self.data;
            let mut b = Reader::at(&data[..end], start);
            let list = self.object_list(&mut b)?;
            if b.pos() != end {
                return Err(Error::Corrupt { offset: b.pos(), what: "fresh-placement data does not end at its size" });
            }
            r.set_pos(end);
            list
        } else {
            self.object_list(r)?
        };
        Ok(Body::Placement(Box::new(Placement {
            group_id,
            mode,
            parent,
            matrix,
            zone_mask,
            binds,
            fresh,
            word,
            objects,
        })))
    }
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
        assert_eq!(layout_of(47), Some(Layout::Solid));
        assert_eq!(layout_of(52), Some(Layout::SolidNamed));
        assert_eq!(layout_of(8), Some(Layout::Placement));
        assert_eq!(layout_of(158), Some(Layout::Screen));
        assert_eq!(layout_of(130), Some(Layout::Clip));
        assert_eq!(layout_of(1), None);
        assert_eq!(layout_of(95), None);
        assert_eq!(command_args(0x45), b"vvfffuwwvw");
        assert_eq!(command_args(0x3B), b"ww");
        assert_eq!(command_args(0x78), b"");
    }

    fn rec(d: &mut Vec<u8>, mode: i8, filter: &[i16], class: i16, id: i32, body: &[u8]) {
        d.push(mode as u8);
        if mode == 1 {
            d.extend_from_slice(&[filter.len() as u8, 0]);
            for s in filter {
                d.extend_from_slice(&s.to_le_bytes());
            }
        }
        d.extend_from_slice(&class.to_le_bytes());
        d.extend_from_slice(&id.to_le_bytes());
        d.extend_from_slice(&(-1i16).to_le_bytes());
        d.extend_from_slice(&(body.len() as i32).to_le_bytes());
        d.extend_from_slice(body);
    }

    #[test]
    fn record_framing_and_filters() {
        let table = OptionTable::default();
        let streaming = BTreeMap::new();
        let mut d = alloc::vec![2u8, 0];
        rec(&mut d, 1, &[5, 6], 2, 9, &[0xFF, 0xFF]);
        rec(&mut d, 0, &[], 1, 1, &[1, 2, 3]); // unknown class: skipped by size, reported
        let mut w = Walk::new(&d, &table, &streaming, crate::anim::DEFAULT_MULTIPLIERS);
        let mut r = Reader::new(&d);
        let list = w.object_list(&mut r).unwrap();
        assert_eq!(r.pos(), d.len());
        assert_eq!(list[0].filter.slots, [5, 6]);
        let mut ctx = Context::any();
        assert!(list[0].loaded(&ctx));
        ctx.slot = Some(4);
        assert!(!list[0].loaded(&ctx) && list[1].loaded(&ctx));
        assert_eq!(list[1].body, Body::Unparsed);
        assert_eq!((w.errors.len(), w.bodies), (1, 2));
    }

    #[test]
    fn copy_placement_with_extra_object() {
        let table = OptionTable::default();
        let streaming = BTreeMap::new();
        let mut body = alloc::vec![];
        body.extend_from_slice(&77i32.to_le_bytes());
        body.push(0xFF); // mode −1
        body.extend_from_slice(&(-1i16).to_le_bytes());
        for f in [1.0f32, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 10.0, 20.0, 30.0] {
            body.extend_from_slice(&f.to_le_bytes());
        }
        body.extend_from_slice(&255u32.to_le_bytes());
        body.push(0); // no binds
        body.extend_from_slice(&0i32.to_le_bytes());
        body.extend_from_slice(&1i16.to_le_bytes());
        rec(&mut body, 0, &[], 2, 3, &[0xFF, 0xFF]);
        let mut d = alloc::vec![1u8, 0];
        rec(&mut d, 0, &[], PLACEMENT, 1, &body);
        let mut w = Walk::new(&d, &table, &streaming, crate::anim::DEFAULT_MULTIPLIERS);
        let list = w.object_list(&mut Reader::new(&d)).unwrap();
        assert!(w.errors.is_empty());
        let Body::Placement(p) = &list[0].body else { panic!() };
        assert!(!p.fresh);
        assert_eq!((p.group_id, p.matrix.t, p.objects.len()), (77, [10.0, 20.0, 30.0], 1));
        assert_eq!(w.bodies, 2);
    }

    #[test]
    fn nesting_is_capped() {
        let table = OptionTable::default();
        let streaming = BTreeMap::new();
        let d = 0i16.to_le_bytes();
        let mut w = Walk::new(&d, &table, &streaming, crate::anim::DEFAULT_MULTIPLIERS);
        w.depth = MAX_OBJECT_DEPTH - 1;
        assert!(w.object_list(&mut Reader::new(&d)).is_ok());
        w.depth = MAX_OBJECT_DEPTH;
        assert!(w.object_list(&mut Reader::new(&d)).is_err());
    }
}
