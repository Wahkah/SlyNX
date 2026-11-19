//! Sly 2 animation data (animation.md, classes/animation.md): clips, state machines and the characters' lookup
//! tables.
//!
//! Curves are returned as `formats::anim` curves, the types `engine::anim::curve` evaluates, because Sly 2 keys are
//! the same kind of data as Sly 1's: 1/60 s frames, Hermite keys with slopes in units per second, the outgoing slope
//! of the earlier key and the incoming slope of the later key (animation.md 3.3):
//!
//! | Sly 2 curve | `formats::anim` curve |
//! |---|---|
//! | vector 0, constant | `VectorCurve::Components { mask: 0, base, curves: [None; 3] }` |
//! | vector 1 and 2 (two or one slope) | `VectorCurve::Hermite` (one slope: incoming = outgoing) |
//! | vector 3 and 4 (per axis, shared times) | `VectorCurve::Components` with a `ScalarCurve::Hermite` per animated axis |
//! | vector 5, scalar 2 (blend placeholders) | `VectorCurve::Blend`, `ScalarCurve::TwistBlend` |
//! | scalar 0, constant | `ScalarCurve::Hermite` with one key |
//! | scalar 1 | `ScalarCurve::Hermite` |
//!
//! Tangent kinds do not exist in Sly 2 (no step rule); keys get kind 0 on both sides.

use alloc::vec::Vec;

use formats::anim::{Event, HermiteKey, ScalarCurve, VectorCurve, VectorKey};
use formats::error::{Error, Result};
use formats::reader::Reader;

use crate::exe::{Executable, OptionTable};
use crate::options::OptionList;
use crate::stream::{Body, Record, visit};

/// Key times are 1/60 s frames.
pub const FRAMES_PER_SECOND: f32 = 60.0;

/// One channel of a clip: a transform of a target object, driven by a vector or a scalar curve.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipChannel {
    /// Target object ID (resolved below the character).
    pub target: i32,
    /// Index into the target's transform list.
    pub transform: i16,
    /// Vector curve: its meaning comes from the transform's kind (animation.md 4).
    pub vector: Option<VectorCurve>,
    /// Scalar curve: the swivel of a two-bone chain whose upper bone is this transform (animation.md 4, 6).
    pub scalar: Option<ScalarCurve>,
    /// The channel flag byte (meaning open).
    pub flag: u8,
}

/// An extra-curve record (purpose open).
#[derive(Clone, Debug, PartialEq)]
pub struct ExtraCurve {
    pub a: u8,
    pub b: i32,
    pub curve: ScalarCurve,
}

/// A clip (class 109).
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    /// Duration in seconds.
    pub duration: f32,
    pub channels: Vec<ClipChannel>,
    pub extra: Vec<ExtraCurve>,
    /// Fired when the clip is applied.
    pub on_apply: Vec<Event>,
    /// Fired as playback crosses their frame (`Event::frame`).
    pub timed: Vec<Event>,
    /// Fired when the playback is removed.
    pub on_retract: Vec<Event>,
    pub options: OptionList,
    /// Time entries of keyed curves (animation.md 11: vector kinds 1 to 4 and scalar kind 1, one per key).
    pub keys: usize,
}

/// Event payload sizes (animation.md 2), `None` for a type that does not occur.
pub fn event_payload_size(kind: i8) -> Option<usize> {
    Some(match kind {
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
        _ => return None,
    })
}

fn events(r: &mut Reader<'_>, timed: bool) -> Result<Vec<Event>> {
    let n = r.u8()?;
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let kind = r.i8()?;
        let frame = if timed { Some(r.i16()?) } else { None };
        let size = event_payload_size(kind).ok_or_else(|| r.corrupt("unknown event type"))?;
        let offset = r.pos();
        let payload = r.bytes(size)?.to_vec();
        out.push(Event { kind, frame, offset, payload });
    }
    Ok(out)
}

fn hermite_key(frame: u16, value: f32, slope_in: f32, slope_out: f32) -> HermiteKey {
    HermiteKey { frame: frame.min(i16::MAX as u16) as i16, value, kind_in: 0, kind_out: 0, slope_in, slope_out }
}

/// Reads a scalar curve of `kind` (animation.md 3.2). Adds its keys to `keys`.
pub fn read_scalar(r: &mut Reader<'_>, kind: i8, keys: &mut usize) -> Result<ScalarCurve> {
    match kind {
        0 => Ok(ScalarCurve::Hermite(alloc::vec![hermite_key(0, r.f32()?, 0.0, 0.0)])),
        1 => {
            let n = r.u16()? as usize;
            *keys += n;
            let mut k = Vec::with_capacity(n);
            for _ in 0..n {
                let frame = r.u16()?;
                k.push(hermite_key(frame, r.f32()?, r.f32()?, r.f32()?));
            }
            Ok(ScalarCurve::Hermite(k))
        }
        2 => Ok(ScalarCurve::TwistBlend),
        _ => Err(r.corrupt("unknown scalar curve kind")),
    }
}

/// Reads a vector curve of `kind` (animation.md 3.1). Adds its keys to `keys`.
pub fn read_vector(r: &mut Reader<'_>, kind: i8, keys: &mut usize) -> Result<VectorCurve> {
    match kind {
        0 => Ok(VectorCurve::Components { mask: 0, base: r.vec3()?, curves: [None, None, None] }),
        1 | 2 => {
            let n = r.u16()? as usize;
            *keys += n;
            let mut k = Vec::with_capacity(n);
            for _ in 0..n {
                let frame = r.u16()?.min(i16::MAX as u16) as i16;
                let value = r.vec3()?;
                let tangent_in = r.vec3()?;
                let tangent_out = if kind == 1 { r.vec3()? } else { tangent_in };
                k.push(VectorKey { frame, value, tangent_in, tangent_out });
            }
            Ok(VectorCurve::Hermite(k))
        }
        3 | 4 => {
            let base = r.vec3()?;
            let n = r.u16()? as usize;
            *keys += n;
            let frames = (0..n).map(|_| r.u16()).collect::<Result<Vec<_>>>()?;
            let mask = r.u8()?;
            let mut curves = [None, None, None];
            for (axis, c) in curves.iter_mut().enumerate() {
                if mask & (1 << axis) == 0 {
                    continue;
                }
                let mut k = Vec::with_capacity(n);
                for &frame in &frames {
                    let value = r.f32()?;
                    let slope_in = r.f32()?;
                    let slope_out = if kind == 3 { r.f32()? } else { slope_in };
                    k.push(hermite_key(frame, value, slope_in, slope_out));
                }
                *c = Some(ScalarCurve::Hermite(k));
            }
            Ok(VectorCurve::Components { mask: mask & 7, base, curves })
        }
        5 => Ok(VectorCurve::Blend),
        _ => Err(r.corrupt("unknown vector curve kind")),
    }
}

impl Clip {
    /// Parses a clip body (`data[body..end]`).
    pub fn parse(data: &[u8], body: usize, end: usize, table: &OptionTable) -> Result<Clip> {
        let mut r = Reader::at(&data[..end], body);
        let duration = r.f32()?;
        let n = r.i16()?;
        let mut keys = 0usize;
        let mut channels = Vec::with_capacity(n.max(0) as usize);
        for _ in 0..n.max(0) {
            let target = r.i32()?;
            let transform = r.i16()?;
            let vk = r.i8()?;
            let vector = if vk == -1 { None } else { Some(read_vector(&mut r, vk, &mut keys)?) };
            let sk = r.i8()?;
            let scalar = if sk == -1 { None } else { Some(read_scalar(&mut r, sk, &mut keys)?) };
            let flag = r.u8()?;
            channels.push(ClipChannel { target, transform, vector, scalar, flag });
        }
        let mut extra = Vec::new();
        for _ in 0..r.u8()? {
            let a = r.u8()?;
            let b = r.i32()?;
            let k = r.i8()?;
            let mut unused = 0;
            extra.push(ExtraCurve { a, b, curve: read_scalar(&mut r, k, &mut unused)? });
        }
        let on_apply = events(&mut r, false)?;
        let timed = events(&mut r, true)?;
        let on_retract = events(&mut r, false)?;
        let options = OptionList::read(&mut r, table)?;
        if r.pos() != end {
            return Err(Error::Corrupt { offset: r.pos(), what: "clip ends before its body size" });
        }
        Ok(Clip { duration, channels, extra, on_apply, timed, on_retract, options, keys })
    }

    /// Parses the clip of a class-109 record.
    pub fn of_record(data: &[u8], record: &Record, table: &OptionTable) -> Result<Clip> {
        if record.class != 109 {
            return Err(Error::Corrupt { offset: record.body_offset as usize, what: "not a clip record" });
        }
        let b = record.body_offset as usize;
        Clip::parse(data, b, b + record.body_size as usize, table)
    }

    /// Duration in 1/60 s frames, rounded.
    pub fn frames(&self) -> i32 {
        (self.duration * FRAMES_PER_SECOND + 0.5) as i32
    }

    /// The explicit loop (animation.md 7): a timed type-5 event at the end of the clip, its frame f satisfying
    /// f >= 60 x duration - 1. Returns (its frame, the frame it jumps to, the speed).
    pub fn explicit_loop(&self) -> Option<(i16, i16, f32)> {
        let end = FRAMES_PER_SECOND * self.duration - 1.0 - 1e-3; // durations are stored as floats
        self.timed.iter().find(|e| e.kind == 5 && e.frame.is_some_and(|f| f as f32 >= end)).map(|e| {
            let target = i16::from_le_bytes([e.payload[0], e.payload[1]]);
            let speed = f32::from_le_bytes(e.payload[2..6].try_into().unwrap());
            (e.frame.unwrap(), target, speed)
        })
    }
}

/// A state of a state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State {
    pub id: i32,
    /// The state's clip or helper object; −1 none; 32 a state without an object of its own.
    pub object: i32,
}

/// A transition of a state machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition {
    pub from: i32,
    pub to: i32,
    pub flags: i32,
    pub unknown: i32,
    pub value: f32,
    /// −1 none; a transition whose object is not among the members is dropped at load.
    pub object: i32,
}

impl Transition {
    /// Flag bit 0: usable in both directions.
    pub fn both_ways(&self) -> bool {
        self.flags & 1 != 0
    }

    /// Group bits (2 to 12).
    pub fn groups(&self) -> u32 {
        (self.flags as u32 >> 2) & 0x7FF
    }
}

/// A state machine (class 111), without its member objects (they are the record's nested records).
#[derive(Clone, Debug, PartialEq)]
pub struct StateMachine {
    pub states: Vec<State>,
    pub transitions: Vec<Transition>,
}

impl StateMachine {
    /// Reads the states and transitions after the member list of a class-111 record.
    pub fn of_record(data: &[u8], record: &Record) -> Result<StateMachine> {
        if record.class != 111 {
            return Err(Error::Corrupt { offset: record.body_offset as usize, what: "not a state machine" });
        }
        let end = (record.body_offset + record.body_size) as usize;
        let members = record.nested();
        let after = members.last().map_or(record.body_offset as usize + 2, |m| (m.body_offset + m.body_size) as usize);
        let mut r = Reader::at(&data[..end], after);
        let n = r.u8()?;
        let states = (0..n).map(|_| Ok(State { id: r.i32()?, object: r.i32()? })).collect::<Result<Vec<_>>>()?;
        let m = r.u8()?;
        let transitions = (0..m)
            .map(|_| {
                Ok(Transition {
                    from: r.i32()?,
                    to: r.i32()?,
                    flags: r.i32()?,
                    unknown: r.i32()?,
                    value: r.f32()?,
                    object: r.i32()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(StateMachine { states, transitions })
    }

    /// The transitions kept at load: those without an object, or whose object is among `members` (by object ID).
    pub fn kept_transitions(&self, members: &[Record]) -> Vec<Transition> {
        self.transitions
            .iter()
            .filter(|t| t.object == -1 || members.iter().any(|m| m.id == t.object))
            .copied()
            .collect()
    }
}

/// The clip records of a character loaded for level row `row` (`None`: all variants): class 109 among its own
/// nested objects and among the members of the state machines nested there (animation.md 8), in walk order.
/// Records whose level conditions exclude the row are not loaded (object-stream.md 2.3).
pub fn character_clips(character: &Record, row: Option<i16>) -> Vec<&Record> {
    fn collect<'r>(list: &'r [Record], row: Option<i16>, out: &mut Vec<&'r Record>) {
        for r in list {
            if !r.loaded_for(row) {
                continue;
            }
            match r.class {
                109 => out.push(r),
                111 => collect(r.nested(), row, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    collect(character.nested(), row, &mut out);
    out
}

/// A lookup-table record (animation.md 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lookup {
    pub flags: u32,
    pub key: u32,
    pub slot: u32,
}

impl Lookup {
    /// `0x10`: the key is a transform ID.
    pub fn is_transform(&self) -> bool {
        self.flags & 0x10 != 0
    }

    /// `0x20` or `0x40`: a part or mesh of the character (the key is not an object ID).
    pub fn is_part(&self) -> bool {
        !self.is_transform() && self.flags & 0x60 != 0
    }

    /// The key is an object ID.
    pub fn is_object(&self) -> bool {
        !self.is_transform() && !self.is_part()
    }
}

/// The character tables: virtual address and record count (animation.md 9).
pub const SLY_TABLES: [(u32, usize); 2] = [(0x002B_E990, 173), (0x002B_F1B0, 18)];
pub const BENTLEY_TABLE: (u32, usize) = (0x002B_C390, 118);
pub const MURRAY_TABLE: (u32, usize) = (0x002C_0270, 137);
pub const SHARED_TABLE: (u32, usize) = (0x002B_C9F0, 53);

/// Body classes of the playable characters.
pub const SLY_CLASS: i16 = 53;
pub const MURRAY_CLASS: i16 = 54;
pub const BENTLEY_CLASS: i16 = 55;

/// Reads a lookup table. Refuses an executable that is not the supported release.
pub fn read_lookup_table(exe: &Executable, (va, count): (u32, usize)) -> Result<Vec<Lookup>> {
    if !exe.is_supported() {
        return Err(Error::UnknownExecutable);
    }
    let raw = exe.bytes_at(va, count * 12)?;
    Ok(raw
        .as_chunks::<12>()
        .0
        .iter()
        .map(|c| Lookup {
            flags: u32::from_le_bytes(c[0..4].try_into().unwrap()),
            key: u32::from_le_bytes(c[4..8].try_into().unwrap()),
            slot: u32::from_le_bytes(c[8..12].try_into().unwrap()),
        })
        .collect())
}

/// What a lookup resolved to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Resolved<'r> {
    /// A transform of the character's own list (its index).
    Transform(usize),
    /// An object below the character.
    Object(&'r Record),
    /// A part or mesh lookup (not an object ID; not resolved here).
    Part,
    /// Not found: the game skips the record.
    NotFound,
}

/// Resolves a lookup against a character's body record: transform IDs in its transform list, object IDs among its
/// nested objects (state-machine members included), depth first.
pub fn resolve_lookup<'r>(character: &'r Record, l: &Lookup) -> Resolved<'r> {
    if l.is_part() {
        return Resolved::Part;
    }
    if l.is_transform() {
        let Body::Solid(s) = &character.body else { return Resolved::NotFound };
        return s
            .transforms
            .transforms
            .iter()
            .position(|t| t.id == l.key as i32)
            .map_or(Resolved::NotFound, Resolved::Transform);
    }
    let mut found = None;
    visit(character.nested(), &mut |r| {
        if found.is_none() && r.id == l.key as i32 {
            found = Some(r);
        }
    });
    found.map_or(Resolved::NotFound, Resolved::Object)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(v: &[f32]) -> Vec<u8> {
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    #[test]
    fn curves_map_to_formats_types() {
        // Vector kind 4: default, 2 frames, mask x|z, then (value, slope) per frame for x and for z.
        let mut d = le(&[1.0, 2.0, 3.0]);
        d.extend_from_slice(&2u16.to_le_bytes());
        d.extend_from_slice(&[0, 0, 30, 0]);
        d.push(0b101);
        d.extend(le(&[0.0, 10.0, 5.0, 10.0, 7.0, 0.0, 9.0, 0.0]));
        let mut keys = 0;
        let v = read_vector(&mut Reader::new(&d), 4, &mut keys).unwrap();
        assert_eq!(keys, 2);
        let VectorCurve::Components { mask, base, curves } = v else { panic!("components") };
        assert_eq!((mask, base), (0b101, [1.0, 2.0, 3.0]));
        assert!(curves[1].is_none());
        let ScalarCurve::Hermite(x) = curves[0].as_ref().unwrap() else { panic!() };
        assert_eq!((x[1].frame, x[1].value, x[1].slope_in, x[1].slope_out), (30, 5.0, 10.0, 10.0));
        let ScalarCurve::Hermite(z) = curves[2].as_ref().unwrap() else { panic!() };
        assert_eq!(z[0].value, 7.0);

        // Vector kind 2: one slope used on both sides.
        let mut d = 1u16.to_le_bytes().to_vec();
        d.extend_from_slice(&6u16.to_le_bytes());
        d.extend(le(&[1.0, 2.0, 3.0, 0.5, 0.5, 0.5]));
        let VectorCurve::Hermite(k) = read_vector(&mut Reader::new(&d), 2, &mut keys).unwrap() else { panic!() };
        assert_eq!((k[0].frame, k[0].tangent_in, k[0].tangent_out), (6, [0.5; 3], [0.5; 3]));

        // Scalar constant: one key.
        let ScalarCurve::Hermite(k) = read_scalar(&mut Reader::new(&le(&[0.25])), 0, &mut keys).unwrap() else {
            panic!()
        };
        assert_eq!((k.len(), k[0].value), (1, 0.25));
        assert!(read_vector(&mut Reader::new(&[]), 9, &mut keys).is_err());
    }

    #[test]
    fn lookup_kinds() {
        let l = |flags| Lookup { flags, key: 1, slot: 0 };
        assert!(l(0x110).is_transform());
        assert!(l(0x120).is_part() && l(0x140).is_part());
        assert!(l(0x103).is_object() && l(0x200).is_object());
    }

    #[test]
    fn transitions() {
        let t = Transition { from: 1, to: 2, flags: 0b1_0101, unknown: 0, value: 0.0, object: -1 };
        assert!(t.both_ways());
        assert_eq!(t.groups(), 0b101);
    }
}
