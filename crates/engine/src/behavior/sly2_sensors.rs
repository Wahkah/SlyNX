//! Sly 2 lasers, touch pads and stand-alone detectors (specs/sly2/alarms-sensors.md).
//!
//! - **Laser beams** (class 85, section 2): each occurrence of option 455 is a beam between two ends; an end on a
//!   curve (classes 10 to 14) moves along it by its mode (8: loop, 16: back and forth with eased turns of radius 100,
//!   or 0.2 L on curves shorter than 300; anything else as 8), with speed and period related by the curve's length;
//!   an end on another object sits at its position, with no anchor at the laser's centre. A playable character
//!   touching a beam takes a hit of 10 units of the stun kind (`0x1000`), every frame it touches (its own flashing
//!   stops repeats). Solids cutting the beam are not tested yet (no Sly 2 physics contacts).
//! - **Touch pads** (class 83, section 3): kind 0 is pressed while a playable character stands on it, kinds 1 and 2
//!   by a hit (2 toggles, at most once per re-arm time, option 419, default 1 s). Pressing raises script event 5,
//!   releasing event 6.
//! - **Stand-alone detectors** (class 16, section 4): watch the targets of option 349 (the three characters) with the
//!   guards' view-volume box (option 345); every change of a target's state (0 unseen, 1 seen, 2 lost) raises
//!   script event 28 with the target, the old and the new state.
//!
//! Characters are the objects of classes 53 to 55 (Sly, Murray, Bentley); a character's touch is a sphere of radius
//! [`CHARACTER_REACH`] centred [`CHARACTER_CENTRE`] above its position. *Placeholder* until the characters' collision
//! shapes are used.

use alloc::vec::Vec;

use formats::geometry::Affine;

use super::{ClassBehavior, Cx};
use crate::math::{dot, length, madd, sub};
use crate::object::{Arg, Handle};
use crate::world::{Notice, World};

/// Option IDs.
pub mod opt {
    /// A laser beam (typed argument list, one per beam).
    pub const BEAM: u16 = 455;
    /// Touch pad: how it is pressed; objects and classes that press it; re-arm time.
    pub const PAD_KIND: u16 = 413;
    /// See [`PAD_KIND`].
    pub const PAD_OBJECT: u16 = 417;
    /// See [`PAD_KIND`].
    pub const PAD_CLASS: u16 = 418;
    /// See [`PAD_KIND`].
    pub const PAD_REARM: u16 = 419;
    /// Detector: view volume; watched targets.
    pub const DETECT_VIEW: u16 = 345;
    /// See [`DETECT_VIEW`].
    pub const DETECT_TARGET: u16 = 349;
    /// Detector: always sees, never sees.
    pub const DETECT_ALWAYS: u16 = 343;
    /// See [`DETECT_ALWAYS`].
    pub const DETECT_NEVER: u16 = 344;
}

/// Script events.
pub mod event {
    /// A touch pad was pressed.
    pub const PRESSED: u32 = 5;
    /// A touch pad was released.
    pub const RELEASED: u32 = 6;
    /// A detector's target changed state.
    pub const DETECT: u32 = 28;
}

/// Classes of the playable characters.
pub const CHARACTERS: [i16; 3] = [53, 54, 55];
/// A character's touch sphere: radius, and its centre's height above the position. *Placeholder* (module docs).
pub const CHARACTER_REACH: f32 = 50.0;
/// See [`CHARACTER_REACH`].
pub const CHARACTER_CENTRE: f32 = 60.0;
/// Hit of a laser beam: amount and kind (stun).
pub const LASER_HIT: (f32, u32) = (10.0, 0x1000);
/// Beams per laser (more are ignored).
pub const MAX_BEAMS: usize = 64;
/// Turn radius of a back-and-forth end (alarms-sensors.md 2.1).
pub const TURN_RADIUS: f32 = 100.0;

/// A hit on one of the gang, for the game's gang system to apply (characters.md 2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GangHit {
    /// The character's object.
    pub target: Handle,
    /// Amount (health units).
    pub amount: f32,
    /// Hit kind bits.
    pub kind: u32,
    /// Where it came from.
    pub source: [f32; 3],
}

/// The live playable characters with their touch centres.
pub fn characters(world: &World) -> Vec<(Handle, [f32; 3])> {
    super::sly3::character_classes(world) // Sly 3 hook: Sly 3's character classes (52 to 54)
        .iter()
        .flat_map(|&c| world.slots_of_class(c).iter().copied())
        .filter_map(|s| world.handle(s))
        .filter(|&h| world.in_world(h))
        .filter_map(|h| world.position(h).map(|p| (h, [p[0], p[1], p[2] + CHARACTER_CENTRE])))
        .collect()
}

/// The points of a Sly 2 curve object (classes 10 to 14) in world space (specs/sly2/classes/paths-and-volumes.md 1).
pub fn sly2_curve(world: &World, h: Handle) -> Vec<[f32; 3]> {
    let Some(r) = world.sly2_record(h) else { return Vec::new() };
    if !(10..=14).contains(&r.class) {
        return Vec::new();
    }
    let w = world.world_transform(h).unwrap_or(Affine::IDENTITY);
    let data = &world.level.bytes;
    let mut rd = formats::reader::Reader::at(data, r.body_offset as usize);
    let parsed = (|| -> formats::Result<Vec<[f32; 3]>> {
        rd.skip(2 + 48 + 4)?;
        let _tangents = rd.u8()?;
        let _unknown = rd.u8()?;
        let n = rd.u16()? as usize;
        rd.skip(4 * n)?;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(w.transform_point(rd.vec3()?));
        }
        Ok(v)
    })();
    parsed.unwrap_or_default()
}

fn polyline_length(p: &[[f32; 3]]) -> f32 {
    p.windows(2).map(|w| length(sub(w[1], w[0]))).sum()
}

fn point_at(p: &[[f32; 3]], d: f32) -> [f32; 3] {
    let mut left = d.max(0.0);
    for w in p.windows(2) {
        let l = length(sub(w[1], w[0]));
        if l >= left && l > 0.0 {
            return madd(w[0], sub(w[1], w[0]), left / l);
        }
        left -= l;
    }
    p.last().copied().unwrap_or([0.0; 3])
}

/// Distance along a curve of length `l` at time `t` for an end (start fraction `f0`, speed, period, mode).
pub fn end_distance(l: f32, f0: f32, speed: f32, period: f32, mode: i32, t: f32) -> f32 {
    if l <= 0.0 {
        return 0.0;
    }
    let period = if period != 0.0 {
        period.abs()
    } else if speed != 0.0 {
        l / speed.abs()
    } else {
        return f0.clamp(0.0, 1.0) * l;
    };
    let backwards = speed < 0.0;
    let phase = (f0.clamp(0.0, 1.0) * period + t) / period;
    let d = if mode == 16 {
        // Back and forth: one period out, one back, with eased turns of radius r at both ends.
        let r = if l >= 300.0 { TURN_RADIUS } else { 0.2 * l };
        let u = crate::math::rem_euclid(phase, 2.0);
        let (u, flip) = if u > 1.0 { (2.0 - u, true) } else { (u, false) };
        let turn = 2.0 * r / (l + 2.0 * r);
        let tt = turn * 0.5;
        // Ease the first and last `tt` of the run: distance grows quadratically there.
        let s = if u < tt {
            r * (u / tt) * (u / tt)
        } else if u > 1.0 - tt {
            l - r * ((1.0 - u) / tt) * ((1.0 - u) / tt)
        } else {
            r + (l - 2.0 * r) * (u - tt) / (1.0 - 2.0 * tt).max(1e-6)
        };
        let _ = flip;
        s
    } else {
        crate::math::rem_euclid(phase, 1.0) * l
    };
    if backwards { l - d } else { d }
}

/// One end of a beam.
#[derive(Clone, Debug, PartialEq)]
struct End {
    /// The anchor object, resolved at load.
    anchor: Option<Handle>,
    /// The anchor's curve points (world space, as loaded) and length, when it is a curve.
    curve: Vec<[f32; 3]>,
    length: f32,
    fraction: f32,
    speed: f32,
    period: f32,
    mode: i32,
}

/// A laser beam object (class 85).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Laser {
    beams: Vec<(End, End)>,
    /// Seconds since the world started.
    pub time: f32,
    /// Hits dealt.
    pub hits: u32,
}

fn num(a: &Arg) -> Option<f32> {
    match *a {
        Arg::Float(f) => Some(f),
        Arg::Int(n) => Some(n as f32),
        Arg::Short(_, n) => Some(f32::from(n)),
        _ => None,
    }
}

fn int(a: &Arg) -> Option<i32> {
    match *a {
        Arg::Int(n) => Some(n),
        Arg::Short(_, n) => Some(i32::from(n)),
        Arg::Float(f) => Some(f as i32),
        _ => None,
    }
}

impl Laser {
    /// The beams of laser `h` (option 455 occurrences).
    pub fn new(world: &World, h: Handle) -> Laser {
        let Some(l) = world.sly2.as_deref() else { return Laser::default() };
        let slot = h.index();
        let beams = l
            .actions
            .iter()
            .filter(|a| a.0 == slot && a.1 == opt::BEAM && a.2.len() >= 10)
            .take(MAX_BEAMS)
            .filter_map(|a| {
                let e = |k: usize| -> Option<End> {
                    let anchor = int(&a.2[k]).filter(|&id| id > 0);
                    let anchor = anchor.and_then(|id| world.find_sly2_id(id, Some(h)));
                    let curve = anchor.map(|a| sly2_curve(world, a)).unwrap_or_default();
                    let length = polyline_length(&curve);
                    Some(End {
                        anchor,
                        curve,
                        length,
                        fraction: num(&a.2[k + 1])?,
                        speed: num(&a.2[k + 2])?,
                        period: num(&a.2[k + 3])?,
                        mode: int(&a.2[k + 4])?,
                    })
                };
                Some((e(0)?, e(5)?))
            })
            .collect();
        Laser { beams, time: 0.0, hits: 0 }
    }

    /// Number of beams.
    pub fn beam_count(&self) -> usize {
        self.beams.len()
    }

    fn end_point(&self, world: &World, me: Handle, e: &End) -> [f32; 3] {
        let centre = world.position(me).unwrap_or([0.0; 3]);
        let Some(a) = e.anchor else { return centre };
        if e.curve.len() < 2 {
            return world.position(a).unwrap_or(centre);
        }
        point_at(&e.curve, end_distance(e.length, e.fraction, e.speed, e.period, e.mode, self.time))
    }

    /// The beams' end points now.
    pub fn segments(&self, world: &World, me: Handle) -> Vec<([f32; 3], [f32; 3])> {
        self.beams.iter().map(|(a, b)| (self.end_point(world, me, a), self.end_point(world, me, b))).collect()
    }
}

/// Distance from `p` to the segment `a`–`b`.
pub fn segment_distance(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> f32 {
    let ab = sub(b, a);
    let l2 = dot(ab, ab);
    let t = if l2 > 0.0 { (dot(sub(p, a), ab) / l2).clamp(0.0, 1.0) } else { 0.0 };
    length(sub(p, madd(a, ab, t)))
}

impl ClassBehavior for Laser {
    fn ticks(&self) -> bool {
        !self.beams.is_empty()
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        self.time += cx.dt;
        let segs = self.segments(cx.world, cx.me);
        let source = cx.my_position();
        for (h, c) in characters(cx.world) {
            if segs.iter().any(|&(a, b)| segment_distance(c, a, b) <= CHARACTER_REACH) {
                cx.world.shared.gang_hits.push(GangHit { target: h, amount: LASER_HIT.0, kind: LASER_HIT.1, source });
                self.hits += 1;
            }
        }
    }
}

/// A touch pad (class 83).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TouchPad {
    /// Pressed.
    pub pressed: bool,
    /// Seconds since the last change.
    pub since: f32,
}

impl TouchPad {
    /// Reach of a contact pad around its centre, horizontally, and how far above it a character stands on it.
    /// *Placeholder* until the pads' collision is used.
    pub const REACH: (f32, f32) = (100.0, 120.0);

    fn presses(world: &World, me: Handle, by: Handle) -> bool {
        let o = world.options(me);
        let objects: Vec<i32> =
            o.map(|o| o.all(opt::PAD_OBJECT).filter_map(|v| v.as_i32()).collect()).unwrap_or_default();
        let classes: Vec<i32> =
            o.map(|o| o.all(opt::PAD_CLASS).filter_map(|v| v.as_i32()).collect()).unwrap_or_default();
        if objects.is_empty() && classes.is_empty() {
            return true;
        }
        let character = world.class(by).is_some_and(|c| CHARACTERS.contains(&c));
        objects.iter().any(|&id| world.sly2_id(by) == Some(id))
            // The playable characters belong to the class-51/52 families (alarms-sensors.md 2.2, 3).
            || classes.iter().any(|&c| world.class(by) == Some(c as i16) || (character && (c == 51 || c == 52)))
    }

    fn set(&mut self, cx: &mut Cx<'_>, on: bool) {
        if self.pressed != on {
            self.pressed = on;
            self.since = 0.0;
            cx.raise(if on { event::PRESSED } else { event::RELEASED }, Vec::new());
        }
    }
}

impl ClassBehavior for TouchPad {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        self.since += cx.dt;
        let o = cx.world.options(cx.me);
        let kind = o.and_then(|o| o.i32(opt::PAD_KIND)).unwrap_or(0);
        let rearm = o.and_then(|o| o.f32(opt::PAD_REARM)).unwrap_or(1.0);
        let notices: Vec<Notice> = cx.world.notices(cx.me).to_vec();
        match kind {
            1 | 2 => {
                for n in notices {
                    if let Notice::Attack { by, .. } = n
                        && Self::presses(cx.world, cx.me, by)
                    {
                        let on = if kind == 2 { !self.pressed } else { true };
                        if kind == 1 || self.since >= rearm {
                            self.set(cx, on);
                        }
                    }
                }
                if kind == 1 && self.pressed && self.since >= rearm {
                    self.set(cx, false);
                }
            }
            _ => {
                let me = cx.my_position();
                let (reach, above) = Self::REACH;
                let on = characters(cx.world).iter().any(|&(h, c)| {
                    let d = sub(c, me);
                    let feet = d[2] - CHARACTER_CENTRE;
                    length([d[0], d[1], 0.0]) <= reach
                        && (-20.0..=above).contains(&feet)
                        && Self::presses(cx.world, cx.me, h)
                });
                if on || self.since >= rearm.min(0.25) {
                    self.set(cx, on);
                }
            }
        }
    }
}

/// A stand-alone detector (class 16).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Detector {
    /// Each watched target and its state (0 unseen, 1 seen, 2 lost).
    pub targets: Vec<(Handle, u8)>,
    /// State changes reported.
    pub changes: u32,
}

impl Detector {
    /// A detector watching option 349's targets.
    pub fn new(world: &World, h: Handle) -> Detector {
        let targets = world
            .options(h)
            .map(|o| o.all(opt::DETECT_TARGET).filter_map(|v| v.as_i32()).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| world.find_sly2_id(id, Some(h)))
            .map(|t| (t, 0))
            .collect();
        Detector { targets, changes: 0 }
    }
}

impl ClassBehavior for Detector {
    fn ticks(&self) -> bool {
        !self.targets.is_empty()
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let o = cx.world.options(cx.me);
        let never = o.is_some_and(|o| o.bool_or(opt::DETECT_NEVER, false));
        let always = o.is_some_and(|o| o.bool_or(opt::DETECT_ALWAYS, false));
        let view = o.and_then(|o| o.i32(opt::DETECT_VIEW)).and_then(|id| cx.world.find_sly2_id(id, Some(cx.me)));
        let shape = view.and_then(|v| super::sly2_props::volume_box(cx.world, v));
        let frame = view.and_then(|v| cx.world.world_transform(v)).or_else(|| cx.world.world_transform(cx.me));
        let eye = cx.my_position();
        for k in 0..self.targets.len() {
            let (t, old) = self.targets[k];
            let Some(p) = cx.world.position(t) else { continue };
            let p = [p[0], p[1], p[2] + CHARACTER_CENTRE];
            let inside = match &shape {
                Some(s) => super::sly2_props::in_volume(s, p),
                None => frame.is_some_and(|f| {
                    let (lo, hi) = super::sly2::VIEW_BOX;
                    let l = to_local(&f, p);
                    (0..3).all(|i| l[i] >= lo[i] && l[i] <= hi[i])
                }),
            };
            let hidden = cx.world.player == Some(t) && cx.world.shared.player.hiding != crate::world::Hiding::None;
            let mut ignore = alloc::vec![cx.me, t];
            ignore.extend(cx.world.children(cx.me));
            let seen = !never
                && (always || inside && !hidden && cx.collision.line_clear(cx.world, eye, p, &ignore).unwrap_or(true));
            let new = if seen {
                1
            } else if old == 1 || (old == 2 && inside) {
                2
            } else {
                0
            };
            if new != old {
                self.targets[k].1 = new;
                self.changes += 1;
                cx.raise(
                    event::DETECT,
                    alloc::vec![
                        script::Value::Object(Some(t.to_script())),
                        script::Value::Int(i32::from(old)),
                        script::Value::Int(i32::from(new)),
                    ],
                );
            }
        }
    }
}

fn to_local(frame: &Affine, p: [f32; 3]) -> [f32; 3] {
    let d = sub(p, frame.t);
    let c = &frame.cols;
    let n = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).max(1e-12);
    [0, 1, 2].map(|k| (d[0] * c[k][0] + d[1] * c[k][1] + d[2] * c[k][2]) / n(c[k]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ends_move_along_their_curves() {
        // Still: at the start fraction.
        assert_eq!(end_distance(1000.0, 0.25, 0.0, 0.0, 16, 3.0), 250.0);
        // Loop (mode 8): one length per period, then back to the start.
        assert!((end_distance(1000.0, 0.0, 0.0, 2.0, 8, 1.0) - 500.0).abs() < 1e-3);
        assert!(end_distance(1000.0, 0.0, 0.0, 2.0, 8, 2.1) < 100.0);
        // Back and forth (mode 16): out in one period, back in the next; never past either end.
        for k in 0..100 {
            let d = end_distance(1000.0, 0.0, 0.0, 2.0, 16, k as f32 * 0.04);
            assert!((0.0..=1000.0).contains(&d), "{d}");
        }
        assert!((end_distance(1000.0, 0.0, 0.0, 2.0, 16, 2.0) - 1000.0).abs() < 1.0, "at the far end after a period");
        // Speed sets the period from the length; a negative speed runs backwards.
        assert!((end_distance(1000.0, 0.0, 500.0, 0.0, 8, 1.0) - 500.0).abs() < 1e-3);
        assert!((end_distance(1000.0, 0.0, -500.0, 0.0, 8, 0.5) - 750.0).abs() < 1e-3);
        assert_eq!(segment_distance([0.0, 5.0, 0.0], [-10.0, 0.0, 0.0], [10.0, 0.0, 0.0]), 5.0);
    }
}
