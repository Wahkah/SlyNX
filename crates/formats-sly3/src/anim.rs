//! Sly 3 clip data (animation.md 2): shared key-time lists, vector and scalar tracks (plain and packed), parts and
//! events. Tracks become `formats::anim` curves, the types the engine evaluates, as for Sly 2:
//!
//! | Sly 3 track | `formats::anim` curve |
//! |---|---|
//! | vector 0, 1, 2, 3 (constants) | `VectorCurve::Components { mask: 0, base, .. }` |
//! | vector 4 to 7 (keyed axes on a shared time list) | `VectorCurve::Components` with a `ScalarCurve::Hermite` per keyed axis |
//! | scalar 0 | `ScalarCurve::Hermite` with one key |
//! | scalar 1 | `ScalarCurve::Hermite` |
//! | scalar 2 | `ScalarCurve::TwistBlend` |
//!
//! Packed values are `s16 × scale` (1/16 for scale index 0, 1/1024 for 1 and 2); packed tangents are
//! `±tan(i·π/32) × multiplier` with the executable's multipliers (60, 10, 10), in units per second.

use alloc::vec::Vec;

use formats::anim::{Event, HermiteKey, ScalarCurve, VectorCurve};
use formats::error::{Error, Result};
use formats::reader::Reader;

/// Key frames are 1/60 s.
pub const FRAMES_PER_SECOND: f32 = 60.0;
/// Packed-value scales by scale index.
pub const PACKED_SCALES: [f32; 3] = [1.0 / 16.0, 1.0 / 1024.0, 1.0 / 1024.0];
/// The multipliers the executable stores (used when none are given).
pub const DEFAULT_MULTIPLIERS: [f32; 3] = [60.0, 10.0, 10.0];

/// A channel: a transform of a target object, driven by a vector and/or a scalar track.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    /// Target object ID (low 20 bits of the stored word, sign-extended).
    pub target: i32,
    /// Transform (low 10 bits, sign-extended).
    pub transform: i16,
    pub vector_kind: i8,
    pub vector: Option<VectorCurve>,
    pub scalar_kind: i8,
    pub scalar: Option<ScalarCurve>,
    pub flags: u8,
}

/// A part record (purpose open).
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub kind: u8,
    pub value: i32,
    pub curve: Option<ScalarCurve>,
}

/// A clip's data.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipData {
    pub frames: u16,
    /// The shared key-time lists (frame numbers).
    pub time_lists: Vec<Vec<u16>>,
    pub pool: u16,
    pub channels: Vec<Channel>,
    pub parts: Vec<Part>,
    pub on_apply: Vec<Event>,
    pub timed: Vec<Event>,
    pub on_retract: Vec<Event>,
}

impl ClipData {
    /// Duration in seconds.
    pub fn duration(&self) -> f32 {
        self.frames as f32 / FRAMES_PER_SECOND
    }
}

/// Event payload sizes (animation.md 2.3), `None` for a kind that does not occur.
pub fn event_payload_size(kind: i8) -> Option<usize> {
    Some(match kind {
        0 => 26,
        1 | 21 => 2,
        2 => 6,
        3 => 8,
        4 | 11 | 12 | 22 | 25 | 26 => 4,
        5 => 6,
        7 | 8 => 8,
        13..=16 => 8,
        17 => 12,
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
        let frame = if timed { Some(r.u16()?.min(i16::MAX as u16) as i16) } else { None };
        let size = event_payload_size(kind).ok_or_else(|| r.corrupt("unknown event kind"))?;
        let offset = r.pos();
        let payload = r.bytes(size)?.to_vec();
        out.push(Event { kind, frame, offset, payload });
    }
    Ok(out)
}

fn key(frame: u16, value: f32, slope_in: f32, slope_out: f32) -> HermiteKey {
    HermiteKey { frame: frame.min(i16::MAX as u16) as i16, value, kind_in: 0, kind_out: 0, slope_in, slope_out }
}

/// `tan(i·π/32)` for `i` = 0..15.
fn tan_steps() -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for (i, o) in out.iter_mut().enumerate() {
        let x = i as f64 * core::f64::consts::PI / 32.0;
        // Taylor series of sin and cos; x is below π/2.
        let (mut s, mut c, mut ts, mut tc) = (0.0f64, 0.0f64, x, 1.0f64);
        for n in 0..20 {
            s += ts;
            c += tc;
            ts *= -x * x / ((2 * n + 2) as f64 * (2 * n + 3) as f64);
            tc *= -x * x / ((2 * n + 1) as f64 * (2 * n + 2) as f64);
        }
        *o = (s / c) as f32;
    }
    out
}

/// A packed tangent of one axis: the axis's 5 bits of `word` (x bits 0..4, y 5..9, z 10..14).
pub fn packed_tangent(word: u16, axis: usize, multiplier: f32) -> f32 {
    let bits = (word >> (axis * 5)) & 0x1F;
    let t = tan_steps()[(bits & 0xF) as usize] * multiplier;
    if bits & 0x10 != 0 { -t } else { t }
}

fn time_list<'a>(lists: &'a [Vec<u16>], r: &Reader<'_>, i: u16) -> Result<&'a [u16]> {
    lists.get(i as usize).map(|l| l.as_slice()).ok_or_else(|| r.corrupt("time-list index out of range"))
}

/// Reads a scalar track of `kind` (animation.md 2.2).
pub fn read_scalar(r: &mut Reader<'_>, kind: i8, lists: &[Vec<u16>]) -> Result<ScalarCurve> {
    match kind {
        0 => Ok(ScalarCurve::Hermite(alloc::vec![key(0, r.f32()?, 0.0, 0.0)])),
        1 => {
            let list = r.u16()?;
            r.skip(2)?;
            let frames = time_list(lists, r, list)?;
            let mut k = Vec::with_capacity(frames.len());
            for &f in frames {
                let value = r.f32()?;
                let slope_in = r.f32()?;
                let slope_out = r.f32()?;
                k.push(key(f, value, slope_in, slope_out));
            }
            Ok(ScalarCurve::Hermite(k))
        }
        2 => Ok(ScalarCurve::TwistBlend),
        _ => Err(r.corrupt("unknown scalar track kind")),
    }
}

fn constant(base: [f32; 3]) -> VectorCurve {
    VectorCurve::Components { mask: 0, base, curves: [None, None, None] }
}

/// Reads a vector track of `kind` (animation.md 2.1).
pub fn read_vector(r: &mut Reader<'_>, kind: i8, lists: &[Vec<u16>], mult: &[f32; 3]) -> Result<VectorCurve> {
    match kind {
        0 => Ok(constant([0.0; 3])),
        1 => Ok(constant([1.0; 3])),
        2 => Ok(constant(r.vec3()?)),
        3 => {
            let s = PACKED_SCALES.get(r.i8()? as usize).copied().ok_or_else(|| r.corrupt("scale index"))?;
            Ok(constant([r.i16()? as f32 * s, r.i16()? as f32 * s, r.i16()? as f32 * s]))
        }
        4..=7 => {
            let size = r.u32()? as usize;
            let list = r.u16()?;
            r.skip(2)?;
            let packed = kind >= 6;
            let mask = r.u8()? & 7;
            let si = if packed { r.i8()? as usize } else { 0 };
            let start = r.pos();
            let scale = PACKED_SCALES.get(si).copied().ok_or_else(|| r.corrupt("scale index"))?;
            let m = mult.get(si).copied().unwrap_or(1.0);
            let frames = time_list(lists, r, list)?;
            let mut base = [0.0f32; 3];
            for (axis, b) in base.iter_mut().enumerate() {
                if mask & (1 << axis) == 0 {
                    *b = if packed { r.i16()? as f32 * scale } else { r.f32()? };
                }
            }
            let keyed: Vec<usize> = (0..3).filter(|a| mask & (1 << a) != 0).collect();
            let mut keys: [Vec<HermiteKey>; 3] = [Vec::new(), Vec::new(), Vec::new()];
            for &f in frames {
                if packed {
                    let tin = r.u16()?;
                    let tout = if kind == 6 { r.u16()? } else { tin };
                    for &a in &keyed {
                        let v = r.i16()? as f32 * scale;
                        keys[a].push(key(f, v, packed_tangent(tin, a, m), packed_tangent(tout, a, m)));
                    }
                } else {
                    for &a in &keyed {
                        let (sin, sout, v) = if kind == 4 {
                            (r.f32()?, r.f32()?, r.f32()?)
                        } else {
                            let t = r.f32()?;
                            (t, t, r.f32()?)
                        };
                        keys[a].push(key(f, v, sin, sout));
                    }
                }
            }
            if r.pos() - start != size {
                return Err(Error::Corrupt { offset: r.pos(), what: "vector track does not end at its size" });
            }
            let [x, y, z] = keys;
            let wrap = |k: Vec<HermiteKey>, a: usize| (mask & (1 << a) != 0).then_some(ScalarCurve::Hermite(k));
            Ok(VectorCurve::Components { mask, base, curves: [wrap(x, 0), wrap(y, 1), wrap(z, 2)] })
        }
        _ => Err(r.corrupt("unknown vector track kind")),
    }
}

fn sign_extend(v: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    ((v << shift) as i32) >> shift
}

impl ClipData {
    /// Reads clip data at the reader's position (after the options and a zero deferred flag).
    pub fn read(r: &mut Reader<'_>, mult: &[f32; 3]) -> Result<ClipData> {
        let frames = r.u16()?;
        let t = r.u16()?;
        let pool = r.u16()?;
        let mut time_lists = Vec::with_capacity(t as usize);
        for _ in 0..t {
            let wide = r.i8()? != 0;
            let c = r.u8()?;
            let l = (0..c).map(|_| if wide { r.u16() } else { r.u8().map(u16::from) }).collect::<Result<Vec<_>>>()?;
            time_lists.push(l);
        }
        let n = r.i16()?;
        let mut channels = Vec::with_capacity(n.max(0) as usize);
        for _ in 0..n.max(0) {
            let target = sign_extend(r.u32()?, 20);
            let transform = sign_extend(r.u16()? as u32, 10) as i16;
            let vector_kind = r.i8()?;
            let vector = if vector_kind == -1 { None } else { Some(read_vector(r, vector_kind, &time_lists, mult)?) };
            let scalar_kind = r.i8()?;
            let scalar = if scalar_kind == -1 { None } else { Some(read_scalar(r, scalar_kind, &time_lists)?) };
            let flags = r.u8()?;
            channels.push(Channel { target, transform, vector_kind, vector, scalar_kind, scalar, flags });
        }
        let mut parts = Vec::new();
        for _ in 0..r.u8()? {
            let kind = r.u8()?;
            let value = r.i32()?;
            let sk = r.i8()?;
            let curve = if sk == -1 { None } else { Some(read_scalar(r, sk, &time_lists)?) };
            parts.push(Part { kind, value, curve });
        }
        let on_apply = events(r, false)?;
        let timed = events(r, true)?;
        let on_retract = events(r, false)?;
        Ok(ClipData { frames, time_lists, pool, channels, parts, on_apply, timed, on_retract })
    }
}

/// The key search (animation.md 3): for time `t` seconds and key frames `frames`, the segment `(i − 1, i)`.
pub fn segment(frames: &[u16], t: f32) -> Option<(usize, usize)> {
    let n = frames.len();
    if n < 2 {
        return None;
    }
    let f = (t * FRAMES_PER_SECOND) as i64;
    let f = if (t * FRAMES_PER_SECOND) < f as f32 { f - 1 } else { f };
    let i = (1..n - 1).find(|&i| frames[i] as i64 > f).unwrap_or(n - 1);
    Some((i - 1, i))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tangents_and_extension() {
        let t = tan_steps();
        assert!((t[0]).abs() < 1e-7 && (t[8] - 1.0).abs() < 1e-6);
        // x axis index 8 positive, y index 8 negative, multiplier 60.
        let w = 8 | ((0x10 | 8) << 5);
        assert!((packed_tangent(w, 0, 60.0) - 60.0).abs() < 1e-4);
        assert!((packed_tangent(w, 1, 60.0) + 60.0).abs() < 1e-4);
        assert_eq!(packed_tangent(w, 2, 60.0), 0.0);
        assert_eq!(sign_extend(0xFFFFF, 20), -1);
        assert_eq!(sign_extend(0x00018, 20), 24);
        assert_eq!(sign_extend(0x3FF, 10), -1);
    }

    #[test]
    fn clip_with_shared_times_and_packed_track() {
        let mut d = alloc::vec![];
        d.extend_from_slice(&30u16.to_le_bytes()); // frames
        d.extend_from_slice(&1u16.to_le_bytes()); // one time list
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&[0, 2, 0, 29]); // narrow, 2 keys at 0 and 29
        d.extend_from_slice(&1i16.to_le_bytes()); // one channel
        d.extend_from_slice(&0x0010_0018u32.to_le_bytes()); // target 24 (high bits are not part of it)
        d.extend_from_slice(&0x0401u16.to_le_bytes()); // transform 1
        d.push(7); // packed, one tangent per key
        // Unkeyed axes x and z first, then per key one tangent word and the keyed y.
        let mut t2 = alloc::vec![];
        t2.extend_from_slice(&16i16.to_le_bytes());
        t2.extend_from_slice(&32i16.to_le_bytes());
        for v in [0i16, 1024] {
            t2.extend_from_slice(&(8u16 << 5).to_le_bytes());
            t2.extend_from_slice(&v.to_le_bytes());
        }
        d.extend_from_slice(&(t2.len() as u32).to_le_bytes());
        d.extend_from_slice(&0u16.to_le_bytes());
        d.extend_from_slice(&0u16.to_le_bytes());
        d.push(0b010);
        d.push(0);
        d.extend_from_slice(&t2);
        d.push(0xFF); // no scalar
        d.push(0); // flags
        d.push(0); // parts
        d.extend_from_slice(&[0, 0, 0]); // three empty event lists
        let mut r = Reader::new(&d);
        let c = ClipData::read(&mut r, &DEFAULT_MULTIPLIERS).unwrap();
        assert_eq!(r.pos(), d.len());
        assert_eq!(c.time_lists, [alloc::vec![0, 29]]);
        let ch = &c.channels[0];
        assert_eq!((ch.target, ch.transform), (24, 1));
        let Some(VectorCurve::Components { mask, base, curves }) = &ch.vector else { panic!() };
        assert_eq!(*mask, 0b010);
        assert_eq!(base[0], 1.0);
        assert_eq!(base[2], 2.0);
        let Some(ScalarCurve::Hermite(k)) = &curves[1] else { panic!() };
        assert_eq!((k[1].frame, k[1].value), (29, 64.0));
        assert!((k[0].slope_in - 60.0).abs() < 1e-3);
        assert_eq!(segment(&[0, 10, 20], 0.2), Some((1, 2)));
        assert_eq!(segment(&[0, 10, 20], 0.0), Some((0, 1)));
    }
}
