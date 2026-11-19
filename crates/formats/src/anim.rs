//! Animation segments: channels, curves, events and settings (classes/animation-and-state.md section 1,
//! classes/shared-structures.md "Scalar animation curve", "Vector animation curve", "Animation events",
//! animation.md sections 2 to 5 and 7).
//!
//! [`Segment::parse`] reads a segment body into owned data: key values are copied out of the world file (they are
//! unaligned), so a parsed segment no longer borrows it. Evaluation lives in the engine; this module only reads.

use alloc::vec::Vec;

use crate::disc::Executable;
use crate::error::{Error, Result};
use crate::options::{OptionList, OptionTable};
use crate::reader::{Reader, Vec3};

/// Frames per second of key times and durations (animation.md 1).
pub const FRAMES_PER_SECOND: f32 = 60.0;

/// The outgoing tangent kind that holds a key's value over the whole segment (animation.md 2.3).
pub const TANGENT_STEP: i8 = 5;

/// A Hermite scalar key (kind 0, "Bezier keys" in the layout files).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HermiteKey {
    /// Time in 1/60 s frames.
    pub frame: i16,
    /// Value.
    pub value: f32,
    /// Incoming tangent kind.
    pub kind_in: i8,
    /// Outgoing tangent kind ([`TANGENT_STEP`] holds the value).
    pub kind_out: i8,
    /// Incoming slope, value units per second.
    pub slope_in: f32,
    /// Outgoing slope, value units per second.
    pub slope_out: f32,
}

/// A weighted Bezier scalar key (kind 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BezierKey {
    /// Time in 1/60 s frames.
    pub frame: i16,
    /// Value.
    pub value: f32,
    /// Incoming kind.
    pub kind_in: i8,
    /// Outgoing kind ([`TANGENT_STEP`] holds the value).
    pub kind_out: i8,
    /// Incoming handle length (seconds before the key).
    pub len_in: f32,
    /// Incoming handle drop (value below the key).
    pub drop_in: f32,
    /// Outgoing handle length (seconds after the key).
    pub len_out: f32,
    /// Outgoing handle rise (value above the key).
    pub rise_out: f32,
}

/// A linear scalar key (kind 2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearKey {
    /// Time in 1/60 s frames.
    pub frame: i16,
    /// Value.
    pub value: f32,
}

/// A vector Hermite key (vector kind 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VectorKey {
    /// Time in 1/60 s frames.
    pub frame: i16,
    /// Value.
    pub value: Vec3,
    /// Incoming tangent, units per second.
    pub tangent_in: Vec3,
    /// Outgoing tangent, units per second.
    pub tangent_out: Vec3,
}

/// A scalar animation curve.
#[derive(Clone, Debug, PartialEq)]
pub enum ScalarCurve {
    /// Kind 0.
    Hermite(Vec<HermiteKey>),
    /// Kind 1.
    Bezier(Vec<BezierKey>),
    /// Kind 2.
    Linear(Vec<LinearKey>),
    /// Kind 3: supplied by a run-time twist blend; no data.
    TwistBlend,
    /// Kind 4: supplied by a run-time pose blend; no data.
    PoseBlend,
}

impl ScalarCurve {
    /// Reads a curve of `kind` (the kind byte already read).
    pub fn read(r: &mut Reader<'_>, kind: i8) -> Result<ScalarCurve> {
        let at = r.pos().saturating_sub(1);
        match kind {
            0 => {
                let n = r.u16()? as usize;
                let mut keys = Vec::with_capacity(n);
                for _ in 0..n {
                    keys.push(HermiteKey {
                        frame: r.i16()?,
                        value: r.f32()?,
                        kind_in: r.i8()?,
                        kind_out: r.i8()?,
                        slope_in: r.f32()?,
                        slope_out: r.f32()?,
                    });
                }
                Ok(ScalarCurve::Hermite(keys))
            }
            1 => {
                let n = r.u16()? as usize;
                let mut keys = Vec::with_capacity(n);
                for _ in 0..n {
                    keys.push(BezierKey {
                        frame: r.i16()?,
                        value: r.f32()?,
                        kind_in: r.i8()?,
                        kind_out: r.i8()?,
                        len_in: r.f32()?,
                        drop_in: r.f32()?,
                        len_out: r.f32()?,
                        rise_out: r.f32()?,
                    });
                }
                Ok(ScalarCurve::Bezier(keys))
            }
            2 => {
                let n = r.u16()? as usize;
                let mut keys = Vec::with_capacity(n);
                for _ in 0..n {
                    keys.push(LinearKey { frame: r.i16()?, value: r.f32()? });
                }
                Ok(ScalarCurve::Linear(keys))
            }
            3 => Ok(ScalarCurve::TwistBlend),
            4 => Ok(ScalarCurve::PoseBlend),
            _ => Err(Error::Corrupt { offset: at, what: "unknown scalar curve kind" }),
        }
    }

    /// The stream kind code.
    pub fn kind(&self) -> i8 {
        match self {
            ScalarCurve::Hermite(_) => 0,
            ScalarCurve::Bezier(_) => 1,
            ScalarCurve::Linear(_) => 2,
            ScalarCurve::TwistBlend => 3,
            ScalarCurve::PoseBlend => 4,
        }
    }

    /// Number of keys (0 for the blend placeholders).
    pub fn key_count(&self) -> usize {
        match self {
            ScalarCurve::Hermite(k) => k.len(),
            ScalarCurve::Bezier(k) => k.len(),
            ScalarCurve::Linear(k) => k.len(),
            _ => 0,
        }
    }

    /// The value of the first key, or `None` for the blend placeholders and empty curves.
    pub fn first_value(&self) -> Option<f32> {
        match self {
            ScalarCurve::Hermite(k) => k.first().map(|k| k.value),
            ScalarCurve::Bezier(k) => k.first().map(|k| k.value),
            ScalarCurve::Linear(k) => k.first().map(|k| k.value),
            _ => None,
        }
    }

    /// The value of the last key, or `None` for the blend placeholders and empty curves.
    pub fn last_value(&self) -> Option<f32> {
        match self {
            ScalarCurve::Hermite(k) => k.last().map(|k| k.value),
            ScalarCurve::Bezier(k) => k.last().map(|k| k.value),
            ScalarCurve::Linear(k) => k.last().map(|k| k.value),
            _ => None,
        }
    }
}

/// A vector animation curve.
#[derive(Clone, Debug, PartialEq)]
pub enum VectorCurve {
    /// Kind 0: a base vector with some components replaced by scalar curves.
    Components {
        /// Bit 0 x, bit 1 y, bit 2 z.
        mask: u8,
        /// Base value.
        base: Vec3,
        /// The scalar curve of each animated component.
        curves: [Option<ScalarCurve>; 3],
    },
    /// Kind 1: vector Hermite keys.
    Hermite(Vec<VectorKey>),
    /// Kind 2: supplied by a run-time blend; no data.
    Blend,
}

impl VectorCurve {
    /// Reads a curve of `kind` (the kind byte already read).
    pub fn read(r: &mut Reader<'_>, kind: i8) -> Result<VectorCurve> {
        let at = r.pos().saturating_sub(1);
        match kind {
            0 => {
                let mask = r.u8()?;
                let base = r.vec3()?;
                let mut curves = [None, None, None];
                for (bit, c) in curves.iter_mut().enumerate() {
                    if mask >> bit & 1 != 0 {
                        let k = r.i8()?;
                        *c = Some(ScalarCurve::read(r, k)?);
                    }
                }
                Ok(VectorCurve::Components { mask, base, curves })
            }
            1 => {
                let n = r.u16()? as usize;
                let mut keys = Vec::with_capacity(n);
                for _ in 0..n {
                    keys.push(VectorKey {
                        frame: r.i16()?,
                        value: r.vec3()?,
                        tangent_in: r.vec3()?,
                        tangent_out: r.vec3()?,
                    });
                }
                Ok(VectorCurve::Hermite(keys))
            }
            2 => Ok(VectorCurve::Blend),
            _ => Err(Error::Corrupt { offset: at, what: "unknown vector curve kind" }),
        }
    }

    /// The stream kind code.
    pub fn kind(&self) -> i8 {
        match self {
            VectorCurve::Components { .. } => 0,
            VectorCurve::Hermite(_) => 1,
            VectorCurve::Blend => 2,
        }
    }
}

/// One channel of a segment: the curves that drive one target object.
#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    /// Target object OID.
    pub target: i16,
    /// Flag byte: bits 0 to 4 say which curves follow; bit 5 marks the root-motion channel.
    pub flags: u8,
    /// Local position.
    pub position: Option<VectorCurve>,
    /// Local rotation as Euler angles (x, y, z), radians, composed Rz · Ry · Rx.
    pub rotation: Option<VectorCurve>,
    /// Local scale.
    pub scale: Option<VectorCurve>,
    /// Inverse-kinematics twist angle, radians.
    pub twist: Option<ScalarCurve>,
    /// One entry per pose of the target's mesh set; `None` where the stream has kind −1 (not animated).
    pub poses: Vec<Option<ScalarCurve>>,
}

impl Channel {
    /// Bit 5: the root-motion channel (animation.md 4.2).
    pub const ROOT_MOTION: u8 = 0x20;

    /// Whether this is the root-motion channel.
    pub fn is_root_motion(&self) -> bool {
        self.flags & Self::ROOT_MOTION != 0
    }

    fn read(r: &mut Reader<'_>) -> Result<Channel> {
        let target = r.i16()?;
        let flags = r.u8()?;
        let mut vectors = [None, None, None];
        for (bit, v) in vectors.iter_mut().enumerate() {
            if flags >> bit & 1 != 0 {
                let k = r.i8()?;
                *v = Some(VectorCurve::read(r, k)?);
            }
        }
        let twist = if flags & 0x08 != 0 {
            let k = r.i8()?;
            Some(ScalarCurve::read(r, k)?)
        } else {
            None
        };
        let mut poses = Vec::new();
        if flags & 0x10 != 0 {
            let n = r.u8()? as usize;
            poses.reserve(n);
            for _ in 0..n {
                let k = r.i8()?;
                poses.push(if k == -1 { None } else { Some(ScalarCurve::read(r, k)?) });
            }
        }
        let [position, rotation, scale] = vectors;
        Ok(Channel { target, flags, position, rotation, scale, twist, poses })
    }
}

/// One animation event (shared-structures.md, animation events; animation.md 5.3).
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    /// Event type.
    pub kind: i8,
    /// Frame (1/60 s) for events of the framed list, `None` for the apply and retract lists.
    pub frame: Option<i16>,
    /// File offset of the payload.
    pub offset: usize,
    /// Payload bytes, as stored.
    pub payload: Vec<u8>,
}

impl Event {
    /// Payload size of an event type, or `None` for types that do not occur. Type 54 has a fixed 4-byte part and a
    /// string; this returns the fixed part.
    pub fn payload_size(kind: i8) -> Option<usize> {
        Some(match kind {
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
            54 => 4,
            _ => return None,
        })
    }

    fn read(r: &mut Reader<'_>, framed: bool) -> Result<Event> {
        let at = r.pos();
        let kind = r.i8()?;
        let frame = if framed { Some(r.i16()?) } else { None };
        let size =
            Event::payload_size(kind).ok_or(Error::Corrupt { offset: at, what: "unknown animation event type" })?;
        let offset = r.pos();
        let mut payload = Vec::from(r.bytes(size)?);
        if kind == 54 {
            let len = r.u16()?;
            payload.extend_from_slice(&len.to_le_bytes());
            payload.extend_from_slice(r.bytes(len as usize)?);
        }
        Ok(Event { kind, frame, offset, payload })
    }

    /// The `s16` at byte `at` of the payload.
    pub fn i16_at(&self, at: usize) -> Option<i16> {
        self.payload.get(at..at + 2).map(|b| i16::from_le_bytes([b[0], b[1]]))
    }

    /// The `u16` at byte `at` of the payload.
    pub fn u16_at(&self, at: usize) -> Option<u16> {
        self.payload.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }

    /// The `f32` at byte `at` of the payload.
    pub fn f32_at(&self, at: usize) -> Option<f32> {
        self.payload.get(at..at + 4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// The frame as seconds (framed events only).
    pub fn time(&self) -> Option<f32> {
        self.frame.map(|f| f as f32 / FRAMES_PER_SECOND)
    }
}

/// Repeat mode (option 178).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RepeatMode {
    /// 0: play once and stop at the end.
    #[default]
    Once,
    /// 1: loop.
    Loop,
    /// 2: back and forth.
    PingPong,
    /// 3: the caller sets the time.
    Manual,
}

impl RepeatMode {
    /// From the option value; unknown values give `None`.
    pub fn from_code(v: i16) -> Option<RepeatMode> {
        Some(match v {
            0 => RepeatMode::Once,
            1 => RepeatMode::Loop,
            2 => RepeatMode::PingPong,
            3 => RepeatMode::Manual,
            _ => return None,
        })
    }
}

/// The settings a segment's option list carries (animation.md 5.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentSettings {
    /// Option 178 (default once).
    pub repeat: RepeatMode,
    /// Option 179: apply once the world has loaded.
    pub auto_play: bool,
    /// Option 180: the player has no control while it plays.
    pub hands_off: bool,
    /// Option 181: priority for shared targets (higher wins); 0 when absent.
    pub priority: i32,
    /// Option 182: base speed multiplier (default 1).
    pub base_speed: f32,
    /// Option 183: the quadratic's coefficients for the difficulty-dependent speed.
    pub difficulty_speed: Option<Vec3>,
    /// Option 184: the clamp range of the difficulty-dependent speed.
    pub difficulty_clamp: Option<[f32; 2]>,
    /// Option 189: advances on the real clock (keeps playing while game time is paused).
    pub real_clock: bool,
}

impl Default for SegmentSettings {
    fn default() -> Self {
        SegmentSettings {
            repeat: RepeatMode::Once,
            auto_play: false,
            hands_off: false,
            priority: 0,
            base_speed: 1.0,
            difficulty_speed: None,
            difficulty_clamp: None,
            real_clock: false,
        }
    }
}

fn f32_le(b: &[u8], at: usize) -> f32 {
    b.get(at..at + 4).map_or(0.0, |s| f32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

impl SegmentSettings {
    /// The difficulty speed factor (options 183, 184; animation.md 5.1): `clamp(g0 + g1·u + g2·u², min, max)` for the
    /// level's difficulty factor `u`, with defaults (1, 0, 0) and (0, 1), so 1 when the options are absent.
    pub fn difficulty_factor(&self, u: f32) -> f32 {
        let [g0, g1, g2] = self.difficulty_speed.unwrap_or([1.0, 0.0, 0.0]);
        let [lo, hi] = self.difficulty_clamp.unwrap_or([0.0, 1.0]);
        (g0 + g1 * u + g2 * u * u).max(lo).min(hi)
    }

    /// The factor every speed of a playback is multiplied by: base speed (182) times the difficulty factor.
    pub fn speed_scale(&self, u: f32) -> f32 {
        self.base_speed * self.difficulty_factor(u)
    }
}

/// Expected value size of the segment options this module reads (option-encodings.md: `h`, `b`, `b`, `i`, `f`,
/// `v`, `p`, `b`). Option 188 (the duration) has no stream encoding: it comes from the record (animation.md 5.1).
fn setting_size(id: i16) -> Option<usize> {
    Some(match id {
        178 => 2,
        179 | 180 | 189 => 1,
        181 | 182 => 4,
        183 => 12,
        184 => 8,
        _ => return None,
    })
}

impl SegmentSettings {
    /// Reads the settings from a segment's option list. A setting whose value does not have the size its option
    /// has in option-encodings.md (possible only with a custom option table) is reported as corrupt.
    pub fn read(data: &[u8], options: &OptionList, table: &OptionTable) -> Result<SegmentSettings> {
        let mut s = SegmentSettings::default();
        for item in options.iter(data, table) {
            let (id, v) = item?;
            let Some(size) = setting_size(id) else { continue };
            if v.len() != size {
                return Err(Error::Corrupt { offset: options.start, what: "segment option value has the wrong size" });
            }
            match id {
                178 => {
                    let code = i16::from_le_bytes([v[0], v[1]]);
                    s.repeat = RepeatMode::from_code(code)
                        .ok_or(Error::Corrupt { offset: options.start, what: "unknown repeat mode" })?;
                }
                179 => s.auto_play = v[0] != 0,
                180 => s.hands_off = v[0] != 0,
                181 => s.priority = i32::from_le_bytes([v[0], v[1], v[2], v[3]]),
                182 => s.base_speed = f32_le(v, 0),
                183 => s.difficulty_speed = Some([f32_le(v, 0), f32_le(v, 4), f32_le(v, 8)]),
                184 => s.difficulty_clamp = Some([f32_le(v, 0), f32_le(v, 4)]),
                _ => s.real_clock = v[0] != 0,
            }
        }
        Ok(s)
    }
}

/// An animation segment body (class 131).
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    /// File offset of the body (just after the 6-byte record header).
    pub offset: usize,
    /// File offset just past the body.
    pub end: usize,
    /// Root object OID.
    pub root: i16,
    /// Duration in 1/60 s frames.
    pub duration_frames: i16,
    /// Search roots (OIDs).
    pub search_roots: Vec<i16>,
    /// Channels in stream order, the root-motion channel included.
    pub channels: Vec<Channel>,
    /// Events fired when the segment is applied.
    pub on_apply: Vec<Event>,
    /// Events fired as playback crosses their frame.
    pub framed: Vec<Event>,
    /// Events fired when the playback is removed.
    pub on_retract: Vec<Event>,
    /// The option list.
    pub options: OptionList,
    /// The settings read from the option list.
    pub settings: SegmentSettings,
}

fn read_events(r: &mut Reader<'_>, framed: bool) -> Result<Vec<Event>> {
    let at = r.pos();
    let n = r.i8()?;
    if n < 0 {
        return Err(Error::Corrupt { offset: at, what: "negative event count" });
    }
    (0..n).map(|_| Event::read(r, framed)).collect()
}

impl Segment {
    /// Parses a segment body at `offset` (the record's offset + 6, or the body of a warp or exit animation).
    pub fn parse(data: &[u8], offset: usize, table: &OptionTable) -> Result<Segment> {
        let mut r = Reader::at(data, offset);
        let root = r.i16()?;
        let duration_frames = r.i16()?;
        let nroots = r.u16()? as usize;
        let mut search_roots = Vec::with_capacity(nroots);
        for _ in 0..nroots {
            search_roots.push(r.i16()?);
        }
        let at = r.pos();
        let nch = r.i8()?;
        if nch < 0 {
            return Err(Error::Corrupt { offset: at, what: "negative channel count" });
        }
        let mut channels = Vec::with_capacity(nch as usize);
        for _ in 0..nch {
            channels.push(Channel::read(&mut r)?);
        }
        let on_apply = read_events(&mut r, false)?;
        let framed = read_events(&mut r, true)?;
        let on_retract = read_events(&mut r, false)?;
        let options = OptionList::read(&mut r, table)?;
        let settings = SegmentSettings::read(data, &options, table)?;
        Ok(Segment {
            offset,
            end: r.pos(),
            root,
            duration_frames,
            search_roots,
            channels,
            on_apply,
            framed,
            on_retract,
            options,
            settings,
        })
    }

    /// Duration in seconds.
    pub fn duration(&self) -> f32 {
        self.duration_frames as f32 / FRAMES_PER_SECOND
    }

    /// The root-motion channel, if any (at most one per segment).
    pub fn root_motion(&self) -> Option<&Channel> {
        self.channels.iter().find(|c| c.is_root_motion())
    }

    /// The channels that drive objects (all but the root-motion channel), with their index in [`Self::channels`].
    pub fn driving_channels(&self) -> impl Iterator<Item = (usize, &Channel)> + '_ {
        self.channels.iter().enumerate().filter(|(_, c)| !c.is_root_motion())
    }
}

/// One entry of a character's animation lookup list (animation.md 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LookupEntry {
    /// Flags (see the constants).
    pub flags: u32,
    /// OID to look up.
    pub oid: i16,
}

impl LookupEntry {
    /// Bit 0: search the whole world, not only below the character.
    pub const WHOLE_WORLD: u32 = 0x01;
    /// Bit 1: optional.
    pub const OPTIONAL: u32 = 0x02;
    /// Bit 2: leave the object in the world.
    pub const KEEP_IN_WORLD: u32 = 0x04;
    /// Bit 3: look up without keeping a reference.
    pub const NO_REFERENCE: u32 = 0x08;
    /// Bit 4: subscribe the character to the object's messages.
    pub const SUBSCRIBE: u32 = 0x10;
    /// Bit 5: also find objects not in the world.
    pub const INCLUDE_OUT_OF_WORLD: u32 = 0x20;

    /// Whether the entry is optional.
    pub fn is_optional(&self) -> bool {
        self.flags & Self::OPTIONAL != 0
    }
}

/// Virtual address of the player character's lookup list in SCUS_971.98 (animation.md 7).
pub const PLAYER_LOOKUP_VA: u32 = 0x0026_2E40;
/// Record count of the player character's lookup list.
pub const PLAYER_LOOKUP_COUNT: usize = 151;

/// Reads a lookup list of `count` 12-byte records at `va` from the executable. Refuses an executable whose
/// SHA-256 is not the supported one (the address belongs to that executable only).
pub fn read_lookup_list(exe: &Executable, va: u32, count: usize) -> Result<Vec<LookupEntry>> {
    if !exe.is_supported() {
        return Err(Error::UnknownExecutable);
    }
    let raw = exe.bytes_at(va, count * 12)?;
    let mut out = Vec::with_capacity(count);
    for c in raw.as_chunks::<12>().0 {
        let flags = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        let oid = u32::from_le_bytes([c[4], c[5], c[6], c[7]]);
        let oid = i16::try_from(oid).map_err(|_| Error::BadExecutable("lookup OID out of range"))?;
        out.push(LookupEntry { flags, oid });
    }
    Ok(out)
}

/// The player character's lookup list ([`PLAYER_LOOKUP_VA`]).
pub fn player_lookup_list(exe: &Executable) -> Result<Vec<LookupEntry>> {
    read_lookup_list(exe, PLAYER_LOOKUP_VA, PLAYER_LOOKUP_COUNT)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::geometry::tests::W;

    /// Writes a small segment: one channel with a per-component rotation (z Hermite) and a vector-Hermite position,
    /// a twist with linear keys, two poses (one not animated), one framed event and options 178 = 1, 181 = 7.
    pub fn sample_segment(w: &mut W) {
        w.i16(10).i16(60).u16(1).i16(11);
        w.i8(2);
        // Channel 0: rotation + twist + poses.
        w.i16(20).u8(0x02 | 0x08 | 0x10);
        w.i8(0).u8(0b100).v3([0.0, 0.0, 0.0]).i8(0).u16(2);
        w.i16(0).f32(0.0).i8(2).i8(2).f32(0.0).f32(1.0);
        w.i16(60).f32(1.0).i8(1).i8(1).f32(1.0).f32(1.0);
        w.i8(2).u16(2).i16(0).f32(0.0).i16(60).f32(2.0);
        w.u8(2).i8(-1).i8(2).u16(2).i16(0).f32(0.5).i16(60).f32(0.25);
        // Channel 1: root motion, vector Hermite position.
        w.i16(21).u8(0x01 | 0x20).i8(1).u16(2);
        w.i16(0).v3([0.0; 3]).v3([0.0; 3]).v3([0.0; 3]);
        w.i16(60).v3([10.0, 0.0, 0.0]).v3([0.0; 3]).v3([0.0; 3]);
        // Events: apply none, framed one (type 39 label, frame 30), retract one (type 8).
        w.i8(0);
        w.i8(1).i8(39).i16(30).i16(400);
        w.i8(1).i8(8);
        // Options.
        w.i16(178).i16(1).i16(181).u32(7).i16(-1);
    }

    #[test]
    fn parses_a_segment() {
        let mut w = W::default();
        w.zeros(6);
        sample_segment(&mut w);
        let data = w.0.clone();
        let s = Segment::parse(&data, 6, &OptionTable::sly1()).unwrap();
        assert_eq!(s.end, data.len());
        assert_eq!((s.root, s.duration_frames, s.search_roots.as_slice()), (10, 60, &[11][..]));
        assert_eq!(s.duration(), 1.0);
        assert_eq!(s.channels.len(), 2);
        let c = &s.channels[0];
        assert!(matches!(&c.rotation, Some(VectorCurve::Components { mask: 4, curves: [None, None, Some(_)], .. })));
        assert!(matches!(&c.twist, Some(ScalarCurve::Linear(k)) if k.len() == 2));
        assert_eq!(c.poses.len(), 2);
        assert!(c.poses[0].is_none());
        assert_eq!(s.root_motion().unwrap().target, 21);
        assert_eq!(s.driving_channels().count(), 1);
        assert_eq!(s.framed[0].kind, 39);
        assert_eq!(s.framed[0].frame, Some(30));
        assert_eq!(s.framed[0].i16_at(0), Some(400));
        assert_eq!(s.on_retract[0].kind, 8);
        assert_eq!(s.settings.repeat, RepeatMode::Loop);
        assert_eq!(s.settings.priority, 7);
        assert_eq!(s.settings.base_speed, 1.0);
    }

    #[test]
    fn settings_with_a_wrong_value_size_are_corrupt() {
        // A custom table that gives option 178 a 1-byte value instead of `h`.
        let mut t = OptionTable::sly1();
        t.set(178, b'b');
        let mut w = W::default();
        w.i16(178).u8(1).i16(-1);
        let list = OptionList::read(&mut Reader::new(&w.0), &t).unwrap();
        assert!(SegmentSettings::read(&w.0, &list, &t).is_err());
        let mut w = W::default();
        w.i16(181).u32(3).i16(-1);
        let t = OptionTable::sly1();
        let list = OptionList::read(&mut Reader::new(&w.0), &t).unwrap();
        assert_eq!(SegmentSettings::read(&w.0, &list, &t).unwrap().priority, 3);
    }

    #[test]
    fn difficulty_factor_multiplies_the_base_speed() {
        let mut s = SegmentSettings { base_speed: 2.0, ..Default::default() };
        assert_eq!(s.speed_scale(0.7), 2.0);
        s.difficulty_speed = Some([0.5, 1.0, 1.0]);
        s.difficulty_clamp = Some([0.0, 1.5]);
        assert_eq!(s.difficulty_factor(0.5), 1.25);
        assert_eq!(s.difficulty_factor(2.0), 1.5);
        assert_eq!(s.speed_scale(0.5), 2.5);
    }

    #[test]
    fn rejects_bad_kinds() {
        let d = [0u8, 0];
        assert!(ScalarCurve::read(&mut Reader::new(&d), 9).is_err());
        assert!(VectorCurve::read(&mut Reader::new(&d), 5).is_err());
        assert!(Event::payload_size(60).is_none());
    }
}
