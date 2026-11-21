//! The gang's clips (Sly 2: specs/sly2/characters.md 1.1, 1.2, 9 and 9.1): which clip each character state plays,
//! and the per-character playback that turns states into the driven blends of [`super::sly2_pose`].
//!
//! - **What plays** ([`Motion`]). Moving states play a **gait**: two clips blended with weight `1 − u` on the first
//!   and `u` on the second, time-warped to a common cycle (characters.md 1.1: each clip's time is the cycle phase
//!   times its own length; the cycle lasts `(1 − u)·T₀ + u·T₁`). Attacks and other moves play **once** from the
//!   state's start and hold their last frame; some loop. No cross-fades: the spec gives none, so a new motion cuts.
//! - **Rate.** A gait's playback speed is the character's horizontal speed over the gait's own speed at that weight
//!   (`consts::Gait::speed`), so the feet keep pace with the ground (for Sly's run, the spec's 832 / 640 = 1.3),
//!   kept within 0.5 to 2 (1 when the gait's speed is unknown or the character is stopped by a wall).
//! - **Overlays** ([`Track::overlay`]): moves that are events rather than states (Murray's throw, Bentley's crossbow
//!   swing, a landing) play once over the state's motion until they end or the state changes.
//! - **Standing** (9.2): the character's idle state machine (Sly 124, Murray and Bentley 418 of their own package)
//!   runs as a chain of one-shot clips ([`IdleRun`]): from the current state, a transition allowed by the mask (8: no
//!   look target; look targets are not modelled) is chosen at random by weight; its clip plays once and leads to the
//!   next state; a transition without a clip is taken at once. Sly starts at 373 (the default start), the others at
//!   their machine's first state. Without the machine a standing character loops Sly 5543, Murray 17782 or Bentley
//!   7602 (9.2's suggestion).
//! - **The paraglider** (9.3): the jump into state 19 plays the opening clip 1518, then the glide blends its five
//!   poses by the stick (9.1.1). The canopy and strings are meshes of Sly's
//!   body on his transforms 3 to 8; they are shown only while he glides ([`super::sly2_pose::Sly2Poser::gliding`],
//!   read by the skinning's accessory flag).
//! - **Jumps** (1.2): a launch plays the take-off set (Sly's double jump the second set) from index 0; within 0.15 s
//!   the first frame with the stick past 0.5 picks the direction ([`sub_kind`]: forward under 63°, back over 117°,
//!   else left or right) and the clip swaps at the same playback time. A plain fall plays take-off 0. A missing
//!   clip falls back to index 0 of its set, then to take-off 0. Landing plays the landing clip (`0x408`) as an
//!   overlay.
//! - **Blends** (9.1.1): the Stealth Slide leans 259 / 261 against 260 by the yaw rate ([`slide_weights`]); the
//!   glide blends 1519 (centre), 1520 (left), 1521 (right), 1523 (forward), 1522 (back) by the stick
//!   (`sly3_pose::glide_weights`, the same rule); Bentley's crossbow plays pair `2·mode + side` with the target's
//!   height weight, through mode changes, the side read from his weapon marker when the swing starts.
//! - **The hook swing** (9.1.1, [`SwingBlend`]): 165 / 166 / 167 at `(a·p, a·(1 − p), 1 − a)`, activity and pump eased
//!   toward the sub-state's targets (pumping ahead or back by the stick against the heading, coasting after a
//!   release or a long hold), the blend's time steered by the swing angle (0 until the gang swings Sly on a pendulum: the middle time).
//!
//! The clips are object IDs found in the character's own package (characters.md 9; `World::find_sly2_id` with the
//! character as context). Root motion: the driven blends hold transform 1's horizontal translation, since the gang
//! moves the object.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use super::sly2_machine::{MachineDef, Transition};
use super::sly2_pose::Driven;
use super::sly3_pose::glide_weights;
use crate::gang::consts::{Gait, gait};
use crate::gang::{Character, GangEvent, Who, bentley, murray, sly};
use crate::math::length;
use crate::object::Handle;
use crate::world::World;

/// Sly 2's clip class.
pub const CLIP_CLASS: i16 = 109;

/// Sly 2's state-machine class.
pub const MACHINE_CLASS: i16 = 111;
/// The idle machines (characters.md 9.2, lookup slot `0x1028`) per character [Sly, Murray, Bentley].
pub const IDLE_MACHINE: [i32; 3] = [124, 418, 418];
/// Sly's default start state in his idle machine.
pub const SLY_IDLE_START: i32 = 373;
/// The transition mask with no look target.
pub const NO_TARGET_MASK: i32 = 8;
/// The mask bits of a transition's flags (bits 2 to 15; bit 0 is two-way, animation.md 8).
pub const MASK_BITS: i32 = 0xFFFC;
/// A looping idle when the machine is missing (9.2's suggestion) per character.
pub const IDLE_LOOP: [i32; 3] = [5543, 17782, 7602];

/// A run of an idle machine (module notes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct IdleRun {
    /// The current state.
    pub state: i32,
    /// The transition playing: its clip, time, length and the state it leads to.
    pub step: Option<(i32, f32, f32, i32)>,
    /// The last clip played and its time (held when no transition leaves the state).
    pub last: Option<(i32, f32)>,
}

/// Whether a transition is allowed with no look target: its mask has bit 8, or it names no mask bit at all.
pub fn idle_allowed(t: &Transition) -> bool {
    let m = t.flags & MASK_BITS;
    m == 0 || m & NO_TARGET_MASK != 0
}

impl IdleRun {
    /// A run at `state`.
    pub fn new(state: i32) -> IdleRun {
        IdleRun { state, step: None, last: None }
    }

    /// Advances by `dt`: returns the clip to show and its time. `random()` gives numbers in `[0, 1)`; `len` a
    /// clip's length (`None`: missing, the transition is skipped).
    pub fn advance(
        &mut self,
        def: &MachineDef,
        dt: f32,
        mut random: impl FnMut() -> f32,
        mut len: impl FnMut(i32) -> Option<f32>,
    ) -> Option<(i32, f32)> {
        let mut carry = dt;
        for _ in 0..16 {
            if let Some((clip, t, l, to)) = self.step.as_mut() {
                *t += carry;
                if *t < *l {
                    self.last = Some((*clip, *t));
                    return self.last;
                }
                carry = *t - *l;
                self.last = Some((*clip, *l));
                self.state = *to;
                self.step = None;
            }
            // Choose among the allowed transitions leaving the state, by weight (all 0: equal).
            let from = self.state;
            let options: Vec<&Transition> =
                def.transitions.iter().filter(|t| t.from == from && idle_allowed(t)).collect();
            if options.is_empty() {
                break;
            }
            let total: f32 = options.iter().map(|t| t.probability.max(0.0)).sum();
            let mut r = random() * if total > 0.0 { total } else { options.len() as f32 };
            let mut pick = options[options.len() - 1];
            for t in &options {
                let w = if total > 0.0 { t.probability.max(0.0) } else { 1.0 };
                if r < w {
                    pick = t;
                    break;
                }
                r -= w;
            }
            let to = if pick.to == -1 { from } else { pick.to };
            match (pick.object, if pick.object == -1 { None } else { len(pick.object) }) {
                (-1, _) => self.state = to,
                (clip, Some(l)) if l > 0.0 => self.step = Some((clip, 0.0, l, to)),
                _ => self.state = to,
            }
            carry = carry.min(1.0);
        }
        self.last
    }
}

/// The gang's clip playbacks in a Sly 2 world: one [`Track`] per character, handed to the poser each frame.
#[derive(Clone, Debug, Default)]
pub struct GangClips {
    /// Per character [Sly, Murray, Bentley].
    pub tracks: [Track; 3],
    /// The idle machines' runs while standing, per character.
    pub idle: [Option<IdleRun>; 3],
    /// Clip objects by (character slot, clip ID), found once.
    found: BTreeMap<(u32, i32), Option<Handle>>,
    /// Idle machines by character slot, read once.
    machines: BTreeMap<u32, Option<MachineDef>>,
    rng: u32,
    members: Vec<(i32, f32, f32)>,
}

/// A number in `[0, 1)` from `state` (xorshift; deterministic per session).
pub fn random(state: &mut u32) -> f32 {
    if *state == 0 {
        *state = 0x9E37_79B9;
    }
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    (x >> 8) as f32 / (1u32 << 24) as f32
}

impl GangClips {
    /// One frame (after the gang's update): starts the events' overlays, advances every member's track by `dt`
    /// and queues its blend on its object (`handles` per character) for the next pose step.
    pub fn drive(
        &mut self,
        world: &mut World,
        members: &[Character],
        handles: &[Option<Handle>; 3],
        events: &[(Who, GangEvent)],
        dt: f32,
    ) {
        let Some(mut poser) = world.sly2.as_deref_mut().map(|l| core::mem::take(&mut l.poser)) else { return };
        for (who, e) in events {
            if let Some(c) = members.iter().find(|c| c.who == *who) {
                if let (GangEvent::Crossbow { .. }, Some(h)) = (e, handles[who.index()]) {
                    self.tracks[who.index()].weapon_side = weapon_side(world, h);
                }
                sly2_event(c, e, &mut self.tracks[who.index()]);
            }
        }
        let w: &World = world;
        poser.gliding.clear();
        for c in members {
            let Some(h) = handles[c.who.index()] else { continue };
            let Some(slot) = w.slot(h) else { continue };
            let k = c.who.index();
            if c.gliding {
                poser.gliding.insert(slot);
            }
            let def = self.machines.entry(slot).or_insert_with(|| {
                let m = w.find_sly2_id(IDLE_MACHINE[k], Some(h)).filter(|&o| w.class(o) == Some(MACHINE_CLASS))?;
                MachineDef::read(&w.level.bytes, w.sly2_record(m)?)
            });
            let found = &mut self.found;
            let mut find = |id: i32| -> Option<Handle> {
                *found
                    .entry((slot, id))
                    .or_insert_with(|| w.find_sly2_id(id, Some(h)).filter(|&o| w.class(o) == Some(CLIP_CLASS)))
            };
            let track = &mut self.tracks[c.who.index()];
            track.observe(c, dt);
            let mut m = sly2_motion(c, track);
            let mut len = |id: i32| find(id).and_then(|o| poser.clip_length(w, o));
            // Standing: the idle machine's chain of clips (9.2).
            if m == idle(c.who)
                && let Some(def) = def.as_ref()
            {
                let start = if c.who == Who::Sly && def.transitions.iter().any(|t| t.from == SLY_IDLE_START) {
                    SLY_IDLE_START
                } else {
                    def.first_state().unwrap_or(0)
                };
                let run = self.idle[k].get_or_insert_with(|| IdleRun::new(start));
                let rng = &mut self.rng;
                if let Some((clip, t)) = run.advance(def, dt, || random(rng), &mut len) {
                    m = Motion::At(clip, t);
                }
            } else {
                self.idle[k] = None;
            }
            track.advance(m, c.state, dt, &mut len, &mut self.members);
            let mut fall_back = m;
            for _ in 0..2 {
                if !self.members.is_empty() {
                    break;
                }
                let Some(f) = sub_kind_fallback(fall_back, &[clip::SECOND[k], clip::TAKE_OFF[k]]) else { break };
                track.advance(f, c.state, dt, &mut len, &mut self.members);
                fall_back = f;
            }
            if self.members.is_empty() {
                track.advance(idle(c.who), c.state, dt, &mut len, &mut self.members);
            }
            let clips: Vec<(Handle, f32, f32)> =
                self.members.iter().filter_map(|&(id, t, wt)| Some((find(id)?, t, wt))).collect();
            if !clips.is_empty() {
                poser.driven.push(Driven { target: h, clips, lock_root: true });
            }
        }
        if let Some(l) = world.sly2.as_deref_mut() {
            l.poser = poser;
        }
    }
}

/// What a character plays now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Motion {
    /// A gait: clips 0 and 1, weight `u` on clip 1, `rate` its playback speed.
    Gait { clips: [i32; 2], u: f32, rate: f32 },
    /// One clip looping.
    Loop(i32),
    /// One clip once from the motion's start, holding its end.
    Once(i32),
    /// Two clips once together, weight `w` on the second.
    OnceBlend([i32; 2], f32),
    /// One clip at a given time (a stand-in pose, or a time kept by the caller).
    At(i32, f32),
    /// Up to five clips looping together at weights `w` (−1: no clip), as the glide blend.
    Blend5([i32; 5], [f32; 5]),
    /// Three clips at weights `w`, all at the time given (the swing blend, whose time is steered).
    BlendAt([i32; 3], [f32; 3], f32),
}

impl Motion {
    /// The clips it plays.
    pub fn clips(&self) -> [Option<i32>; 5] {
        match *self {
            Motion::Gait { clips, .. } | Motion::OnceBlend(clips, _) => {
                [Some(clips[0]), Some(clips[1]), None, None, None]
            }
            Motion::Loop(c) | Motion::Once(c) | Motion::At(c, _) => [Some(c), None, None, None, None],
            Motion::Blend5(c, _) => c.map(|c| (c >= 0).then_some(c)),
            Motion::BlendAt(c, _, _) => [Some(c[0]), Some(c[1]), Some(c[2]), None, None],
        }
    }

    /// Whether `other` continues this motion (same kind and clips; weights, rates and times may differ).
    pub fn same(&self, other: &Motion) -> bool {
        core::mem::discriminant(self) == core::mem::discriminant(other) && self.clips() == other.clips()
    }
}

/// An overlay that the state's changes do not end (the crossbow swing keeps its time across a mode change).
pub const ANY_STATE: u8 = u8::MAX;
/// Sly 2: the stick picks a jump direction during this long after a launch, once past [`DIRECTION_STICK`].
pub const DIRECTION_WINDOW: f32 = 0.15;
pub const DIRECTION_STICK: f32 = 0.5;

/// One character's playback.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Track {
    /// The motion playing.
    pub motion: Option<Motion>,
    /// Time since the motion started (seconds).
    pub time: f32,
    /// A gait's cycle phase (0 to 1).
    pub phase: f32,
    /// An overlay: its motion, the state it started in, its time.
    pub overlay: Option<(Motion, u8, f32)>,
    /// The character is in the air after a jump (not a fall).
    pub jumped: bool,
    /// The jump's direction index (0 to 4).
    pub sub_kind: u8,
    /// Sly 2: the jump is Sly's second (double) jump, which plays the second set.
    pub second: bool,
    /// Sly 2: time since the launch, and what is left of the window in which the stick picks the direction.
    pub launch: f32,
    pub window: f32,
    /// The facing last frame and the yaw rate (rad/s, counter-clockwise positive).
    pub yaw: Option<f32>,
    pub yaw_rate: f32,
    /// The hook swing's blend while swinging.
    pub swing: Option<SwingBlend>,
    /// Bentley's weapon side at the crossbow swing's start (set by the driver, [`weapon_side`]).
    pub weapon_side: i32,
}

impl Track {
    /// Sly 2's per-frame bookkeeping before the motion is chosen: the launch time, the jump direction picked in
    /// its window (1.2: the first frame with the stick past 0.5 within 0.15 s), and the yaw rate.
    pub fn observe(&mut self, c: &Character, dt: f32) {
        self.launch += dt;
        if self.window > 0.0 {
            self.window -= dt;
            if c.last_deflection > DIRECTION_STICK {
                self.sub_kind = sub_kind(c);
                self.window = 0.0;
            }
        }
        if let Some(p) = self.yaw
            && dt > 0.0
        {
            use core::f32::consts::PI;
            self.yaw_rate = (crate::math::rem_euclid(c.yaw - p + PI, 2.0 * PI) - PI) / dt;
        }
        self.yaw = Some(c.yaw);
        if c.who == Who::Sly && c.state == sly::HOOK {
            let stick = (c.last_deflection > SWING_STICK).then(|| facing_angle(c));
            // Gang hook: the pendulum's swing angle θ (`Character::swing_angle`).
            self.swing.get_or_insert_with(SwingBlend::enter).step(dt, stick, c.swing_angle, SWING_LENGTH);
        } else {
            self.swing = None;
        }
    }

    /// Starts an overlay that plays until it ends or the state leaves `state`.
    pub fn overlay(&mut self, m: Motion, state: u8) {
        self.overlay = Some((m, state, 0.0));
    }

    /// A launch (a jump, or Sly's double jump with `second`): direction index 0, the direction window open.
    pub fn launch(&mut self, second: bool) {
        self.jumped = true;
        self.second = second;
        self.sub_kind = 0;
        self.launch = 0.0;
        self.window = DIRECTION_WINDOW;
        self.restart();
    }

    /// Restarts the current motion from its start.
    pub fn restart(&mut self) {
        self.time = 0.0;
    }

    /// Advances by `dt` with the state's motion `m` (in state `state`) and writes the members to play,
    /// `(clip, time, weight)`, into `out`. `len` gives a clip's length (`None`: not present, left out).
    pub fn advance(
        &mut self,
        m: Motion,
        state: u8,
        dt: f32,
        mut len: impl FnMut(i32) -> Option<f32>,
        out: &mut Vec<(i32, f32, f32)>,
    ) {
        out.clear();
        // Bad lengths (negative, infinite or not a number) read as 0.
        let mut len = |id: i32| len(id).map(super::sly2_pose::sane_length);
        if let Some((om, st, t)) = self.overlay.as_mut() {
            *t += dt;
            let end = match *om {
                Motion::Once(c) => len(c).unwrap_or(0.0),
                Motion::OnceBlend(c, _) => len(c[0]).unwrap_or(0.0).max(len(c[1]).unwrap_or(0.0)),
                _ => f32::MAX,
            };
            if (*st != state && *st != ANY_STATE) || *t >= end {
                self.overlay = None;
            } else {
                let (om, t) = (*om, *t);
                emit(om, t, 0.0, &mut len, out);
                if !out.is_empty() {
                    return;
                }
            }
        }
        let was_gait = matches!(self.motion, Some(Motion::Gait { .. }));
        if !self.motion.is_some_and(|p| p.same(&m)) {
            self.time = 0.0;
            if !(was_gait && matches!(m, Motion::Gait { .. })) {
                self.phase = 0.0;
            }
        } else {
            self.time += dt;
        }
        self.motion = Some(m);
        if let Motion::Gait { clips, u, rate } = m {
            let (t0, t1) = (len(clips[0]), len(clips[1]));
            let u = u.clamp(0.0, 1.0);
            let cycle = (1.0 - u) * t0.unwrap_or(0.0) + u * t1.unwrap_or(0.0);
            let cycle = match (t0, t1) {
                (Some(_), Some(_)) => cycle,
                (Some(a), None) => a,
                (None, Some(b)) => b,
                (None, None) => 0.0,
            };
            if cycle > 0.0 && self.time > 0.0 {
                self.phase = crate::math::rem_euclid(self.phase + dt * rate / cycle, 1.0);
            }
        }
        emit(m, self.time, self.phase, &mut len, out);
    }
}

/// The members of motion `m` at time `t` (gait phase `phase`).
fn emit(m: Motion, t: f32, phase: f32, len: &mut impl FnMut(i32) -> Option<f32>, out: &mut Vec<(i32, f32, f32)>) {
    // `time(l)` gives the member's time from its clip's length.
    let mut push = |c: i32, w: f32, time: &dyn Fn(f32) -> f32| {
        if w > 0.0
            && let Some(l) = len(c)
        {
            // Lengths are sanitised where they are read; this guard keeps a bad one from panicking here too.
            let l = super::sly2_pose::sane_length(l);
            out.push((c, time(l).clamp(0.0, l), w));
        }
    };
    let solo = |out: &mut Vec<(i32, f32, f32)>| {
        // A missing member leaves the other at full weight.
        if out.len() == 1 {
            out[0].2 = 1.0;
        }
    };
    match m {
        Motion::Gait { clips, u, .. } => {
            let u = u.clamp(0.0, 1.0);
            // Each clip's own time is the phase times its length.
            push(clips[0], 1.0 - u, &|l| phase * l);
            push(clips[1], u, &|l| phase * l);
            solo(out);
        }
        Motion::Loop(c) => push(c, 1.0, &|l| if l > 0.0 { crate::math::rem_euclid(t, l) } else { 0.0 }),
        Motion::Once(c) => push(c, 1.0, &|_| t),
        Motion::At(c, at) => push(c, 1.0, &|_| at),
        Motion::OnceBlend(c, w) => {
            push(c[0], 1.0 - w, &|_| t);
            push(c[1], w, &|_| t);
            solo(out);
        }
        Motion::BlendAt(c, w, at) => {
            for (c, w) in c.into_iter().zip(w) {
                push(c, w, &|_| at);
            }
            let sum: f32 = out.iter().map(|m| m.2).sum();
            if sum > 0.0 {
                out.iter_mut().for_each(|m| m.2 /= sum);
            }
        }
        Motion::Blend5(c, w) => {
            for (c, w) in c.into_iter().zip(w) {
                if c >= 0 {
                    push(c, w, &|l| if l > 0.0 { crate::math::rem_euclid(t, l) } else { 0.0 });
                }
            }
            // Missing members: the others share their weight.
            let sum: f32 = out.iter().map(|m| m.2).sum();
            if sum > 0.0 {
                out.iter_mut().for_each(|m| m.2 /= sum);
            }
        }
    }
}

/// The jump and fall sub-kind (Sly 3 characters.md 12.3; Sly 2 the same rule and thresholds): from the stick's
/// angle minus the facing, `a` in −π … π (positive: stick to the left): 3 forward (|a| < 1.0996), 4 backward
/// (|a| > 2.0420), else 1 right (a ≤ 0) or 2 left (a > 0); 0 with the stick at rest.
pub fn sub_kind(c: &Character) -> u8 {
    let st = c.last_stick;
    if c.last_deflection <= 0.0 || st[0] * st[0] + st[1] * st[1] < 1e-8 {
        return 0;
    }
    let a = facing_angle(c);
    let m = if a < 0.0 { -a } else { a };
    if m < SUB_KIND_FORWARD {
        3
    } else if m > SUB_KIND_BACKWARD {
        4
    } else if a <= 0.0 {
        1
    } else {
        2
    }
}

/// |a| below this is forward (`0x3F8CBE4C`).
pub const SUB_KIND_FORWARD: f32 = 1.0996;
/// |a| above this is backward (`0x4002B0B4`).
pub const SUB_KIND_BACKWARD: f32 = 2.0420;

/// The stick's angle minus the facing, in −π … π (positive: to the left).
pub fn facing_angle(c: &Character) -> f32 {
    use core::f32::consts::PI;
    let a = crate::math::atan2(c.last_stick[1], c.last_stick[0]) - c.yaw;
    crate::math::rem_euclid(a + PI, 2.0 * PI) - PI
}

/// The fallback of a missing jump or fall clip (Sly 2 1.2, Sly 3 12.3): index 0 of its row of `tables`, then index 0
/// of the last row (the take-off set). `None` when `m` is not such a clip or nothing is left to try.
pub fn sub_kind_fallback(m: Motion, tables: &[[i32; 5]]) -> Option<Motion> {
    let (c, at) = match m {
        Motion::Once(c) => (c, None),
        Motion::At(c, t) => (c, Some(t)),
        _ => return None,
    };
    let base = tables.last()?[0];
    let to = match tables.iter().find(|row| row.contains(&c)) {
        Some(row) if row[0] != c && row[0] >= 0 => row[0],
        Some(_) if c != base && base >= 0 => base,
        _ => return None,
    };
    Some(at.map_or(Motion::Once(to), |t| Motion::At(to, t)))
}

/// A gait's playback speed: horizontal speed over the gait's speed at `u`, within 0.5 to 2 (1 when either is
/// near 0).
pub fn gait_rate(c: &Character, g: Option<&Gait>, u: f32) -> f32 {
    let v = length([c.velocity[0], c.velocity[1], 0.0]);
    let s = g.map_or(0.0, |g| g.speed(u));
    if s < 1.0 || v < 1.0 { 1.0 } else { (v / s).clamp(0.5, 2.0) }
}

/// Sly 2's clips (object IDs) for each character (characters.md 1.1, 1.2, 9.1).
pub mod clip {
    /// Gaits (slow, fast): Sly's run and move; Murray's run, walk and carrying; Bentley's move and walk.
    pub const SLY_RUN: [i32; 2] = [125, 126];
    pub const SLY_MOVE: [i32; 2] = [276, 277];
    pub const MURRAY_RUN: [i32; 2] = [416, 414];
    pub const MURRAY_WALK: [i32; 2] = [489, 488];
    pub const MURRAY_CARRY: [i32; 2] = [482, 483];
    pub const BENTLEY_RUN: [i32; 2] = [416, 414];
    pub const BENTLEY_WALK: [i32; 2] = [489, 488];
    /// Bentley's timed state (14).
    pub const BENTLEY_TIMED: i32 = 588;
    /// The take-off set (`0x3E0` to `0x3F0`) and the second set (`0x3F4` to `0x404`, Sly's double jump) by direction
    /// index 0 straight, 1 right, 2 left, 3 forward, 4 back (1.2), and the landing (`0x408`; −1 none), per character
    /// [Sly, Murray, Bentley]; −1 where the hub has no clip (index 0 plays).
    pub const TAKE_OFF: [[i32; 5]; 3] = [[152, 156, 155, 153, 154], [590, 594, 593, 591, 592], [590, 594, 593, -1, -1]];
    pub const SECOND: [[i32; 5]; 3] = [[160, 164, 163, 161, 162], [595, -1, -1, -1, -1], [595, -1, -1, -1, -1]];
    pub const LANDING: [i32; 3] = [158, -1, 478];
    /// Sly: Knockout Dive, air attacks (Square, Triangle), pickpocket, juggle strike, combat dodge (left, right:
    /// order assumed), Stealth Slide (first clip of its blend), hook swing (first clip of its blend).
    pub const SLY_DIVE: i32 = 189;
    pub const SLY_AIR_ATTACK: [i32; 2] = [190, 191];
    pub const SLY_PICKPOCKET: i32 = 280;
    pub const SLY_JUGGLE: i32 = 196;
    pub const SLY_DODGE: [i32; 2] = [281, 282];
    pub const SLY_SLIDE: [i32; 5] = [259, 260, 261, -1, -1];
    pub const SLY_SWING: i32 = 165;
    /// The swing blend's three clips (9.1.1): 165 = a·p, 166 = a·(1 − p), 167 = 1 − a.
    pub const SLY_SWING_BLEND: [i32; 3] = [165, 166, 167];
    /// The paraglider (9.3): the opening clip (slot `0x438`) and the five glide poses (`0x440` to `0x470`).
    pub const SLY_GLIDE_OPEN: i32 = 1518;
    /// In slot order: centre, left, right, forward, back (9.1.1: 1523 before 1522).
    pub const SLY_GLIDE: [i32; 5] = [1519, 1520, 1521, 1523, 1522];
    /// Murray: the five punches, Thunder Flop, throw, carry slam, picking up, Berserker Charge (charge, stop),
    /// Turnbuckle Launch.
    pub const MURRAY_PUNCHES: [i32; 5] = [424, 425, 435, 434, 196];
    pub const MURRAY_FLOP: i32 = 417;
    pub const MURRAY_THROW: i32 = 490;
    pub const MURRAY_SLAM: i32 = 492;
    pub const MURRAY_PICK_UP: i32 = 452;
    pub const MURRAY_CHARGE: [i32; 2] = [441, 443];
    pub const MURRAY_TURNBUCKLE: i32 = 509;
    /// Bentley: held by Murray (still), picking up, the crossbow swing's first clip (pairs of consecutive IDs).
    pub const BENTLEY_HELD: i32 = 522;
    pub const BENTLEY_PICK_UP: i32 = 447;
    pub const BENTLEY_SWING: i32 = 560;
}

/// The swing blend's still pose time (0.3333 s of its 0.6667 s clips).
pub const SWING_STILL_TIME: f32 = 1.0 / 3.0;
/// The swing blend's clips' length (9.1: 165 to 167, 0.6667 s; Sly 3's 213 to 215 the same).
pub const SWING_LENGTH: f32 = 2.0 / 3.0;
/// The stick counts as pushed past this (9.1.1).
pub const SWING_STICK: f32 = 0.3;
/// Pumping ahead within this angle of the heading, back beyond the other (9.1.1).
pub const SWING_AHEAD: f32 = core::f32::consts::FRAC_PI_3;
pub const SWING_BACK: f32 = 2.0 * core::f32::consts::FRAC_PI_3;
/// Coasting after the stick is released this long, or held ahead or back longer than the other.
pub const SWING_RELEASE: f32 = 0.25;
pub const SWING_HOLD: f32 = 1.4;
/// The easings of activity and pump (characters.md 9.1.1): (fast, slow, fast time).
pub const SWING_ACTIVITY_EASE: [f32; 3] = [1.0, 0.0, 0.1];
pub const SWING_PUMP_EASE: [f32; 3] = [4.0, 0.0, 0.1];
/// The swing angle mapped onto the blend's time and the time's rate limit (characters.md 9.1.1).
pub const SWING_ANGLE_RANGE: f32 = 1.7;
pub const SWING_TIME_RATE: f32 = 3.0;

/// The hook swing's blend (characters.md 9.1.1): activity `a` and pump `p` eased toward targets set by the
/// sub-state, weights `(a·p, a·(1 − p), 1 − a)`, and the blend's time steered by the swing angle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SwingBlend {
    /// Sub-state: 0 pumping ahead, 1 coasting, 2 pumping back.
    pub sub: u8,
    /// Activity and pump.
    pub active: f32,
    pub pump: f32,
    /// The blend's time.
    pub time: f32,
    /// How long the stick has been released, and held in the current pumping direction.
    pub released: f32,
    pub held: f32,
    ease: [crate::physics::Smoother; 2],
}

impl SwingBlend {
    /// Entering the swing: pumping ahead, at the blend's middle.
    pub fn enter() -> SwingBlend {
        SwingBlend { active: 1.0, pump: 1.0, time: SWING_LENGTH * 0.5, ..SwingBlend::default() }
    }

    /// One step: `stick` is the stick's angle against the heading when pushed past 0.3, `theta` the swing angle
    /// (the hook direction's pitch from straight up, in the heading's frame), `length` the blend's length.
    pub fn step(&mut self, dt: f32, stick: Option<f32>, theta: f32, length: f32) {
        let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
        match stick {
            None => {
                self.released += dt;
                self.held = 0.0;
                if self.released >= SWING_RELEASE {
                    self.sub = 1;
                }
            }
            Some(a) => {
                self.released = 0.0;
                let a = if a < 0.0 { -a } else { a };
                let dir = if a < SWING_AHEAD {
                    Some(0)
                } else if a > SWING_BACK {
                    Some(2)
                } else {
                    None
                };
                if let Some(d) = dir {
                    if self.sub != d && !(self.sub == 1 && self.held > SWING_HOLD) {
                        self.held = 0.0;
                        self.sub = d;
                    }
                    self.held += dt;
                    if self.held > SWING_HOLD {
                        self.sub = 1;
                    }
                }
            }
        }
        let (a_goal, p_goal) = match self.sub {
            0 => (1.0, 1.0),
            2 => (1.0, 0.0),
            _ => (0.0, if self.pump > 0.5 { 1.0 } else { 0.0 }),
        };
        let p = |e: [f32; 3]| crate::physics::SmoothParams {
            fast: e[0],
            slow: e[1],
            fast_time: e[2],
            max_speed_change: None,
        };
        self.active = self.ease[0].step(&p(SWING_ACTIVITY_EASE), self.active, a_goal, dt).clamp(0.0, 1.0);
        self.pump = self.ease[1].step(&p(SWING_PUMP_EASE), self.pump, p_goal, dt).clamp(0.0, 1.0);
        // The time follows the swing angle: D·(clamp(θ / 1.7, −1, 1) + 1) / 2, at most 3 per second.
        let length = super::sly2_pose::sane_length(length);
        let theta = if theta.is_finite() { theta } else { 0.0 };
        let goal = length * ((theta / SWING_ANGLE_RANGE).clamp(-1.0, 1.0) + 1.0) * 0.5;
        let max = SWING_TIME_RATE * dt;
        self.time = (self.time + (goal - self.time).clamp(-max, max)).clamp(0.0, length);
    }

    /// The weights of 165, 166, 167 (Sly 3: 213, 214, 215).
    pub fn weights(&self) -> [f32; 3] {
        [self.active * self.pump, self.active * (1.0 - self.pump), 1.0 - self.active]
    }
}

/// Standing: a looping idle clip (9.2); the driver plays the idle machine instead when it finds it.
fn idle(who: Who) -> Motion {
    Motion::Loop(IDLE_LOOP[who.index()])
}

/// Index of a character in the per-character clip arrays ([Sly, Murray, Bentley]).
fn k(who: Who) -> usize {
    who.index()
}

/// The motion of a Sly 2 character in its current state (module notes; `track` says whether it jumped).
pub fn sly2_motion(c: &Character, track: &Track) -> Motion {
    let u = c.last_deflection.clamp(0.0, 1.0);
    let g = |clips: [i32; 2], gt: &Gait, u: f32| Motion::Gait { clips, u, rate: gait_rate(c, Some(gt), u) };
    // A jump plays the take-off set (Sly's double jump the second set) at the launch's time, so a direction picked
    // in the window swaps the clip at the same time; a plain fall plays take-off 0.
    let air = || {
        let i = usize::from(track.sub_kind.min(4));
        let row = if track.second { clip::SECOND[k(c.who)] } else { clip::TAKE_OFF[k(c.who)] };
        if track.jumped { Motion::At(row[i], track.launch) } else { Motion::Once(clip::TAKE_OFF[k(c.who)][0]) }
    };
    match c.who {
        Who::Sly => match c.state {
            sly::RUN => g(clip::SLY_RUN, &gait::SLY_RUN, 1.0),
            sly::SNEAK | sly::SNEAK_RUN if u > 0.0 => g(clip::SLY_MOVE, &gait::SLY_MOVE, u),
            sly::AIR if c.gliding => Motion::Blend5(clip::SLY_GLIDE, glide_weights(u, facing_angle(c))),
            sly::AIR => air(),
            sly::DODGE => Motion::Once(clip::SLY_DODGE[usize::from(c.dodge_side < 0.0)]),
            sly::PICKPOCKET => Motion::Once(clip::SLY_PICKPOCKET),
            sly::KNOCKOUT_DIVE => Motion::Once(clip::SLY_DIVE),
            sly::AIR_SQUARE => Motion::Once(clip::SLY_AIR_ATTACK[0]),
            sly::AIR_TRIANGLE => Motion::Once(clip::SLY_AIR_ATTACK[1]),
            sly::JUGGLE_STRIKE => Motion::Once(clip::SLY_JUGGLE),
            sly::SLIDE => Motion::Blend5(clip::SLY_SLIDE, slide_weights(track.yaw_rate)),
            // The swing blend (9.1.1): its weights and steered time; its still pose until the first step.
            sly::HOOK => match track.swing {
                Some(b) => Motion::BlendAt(clip::SLY_SWING_BLEND, b.weights(), b.time),
                None => Motion::At(clip::SLY_SWING, SWING_STILL_TIME),
            },
            _ => idle(c.who),
        },
        Who::Murray => match c.state {
            murray::RUN => g(clip::MURRAY_RUN, &gait::MURRAY_RUN, 1.0),
            murray::WALK if u > 0.0 => g(clip::MURRAY_WALK, &gait::MURRAY_WALK, u),
            murray::CARRY if u > 0.0 => g(clip::MURRAY_CARRY, &gait::MURRAY_CARRY, u),
            murray::CARRY => Motion::At(clip::MURRAY_CARRY[0], 0.0),
            murray::AIR => air(),
            murray::FLOP => Motion::Once(clip::MURRAY_FLOP),
            murray::LIFT => Motion::Once(clip::MURRAY_PICK_UP),
            murray::SLAM => Motion::Once(clip::MURRAY_SLAM),
            murray::PUNCH => Motion::Once(clip::MURRAY_PUNCHES[usize::from(c.combo.index).min(4)]),
            murray::CHARGE if c.state_time < gait::CHARGE_TIME => Motion::Loop(clip::MURRAY_CHARGE[0]),
            murray::CHARGE => Motion::Once(clip::MURRAY_CHARGE[1]),
            murray::TURNBUCKLE => Motion::Loop(clip::MURRAY_TURNBUCKLE),
            _ => idle(c.who),
        },
        Who::Bentley => match c.state {
            bentley::MOVE => g(clip::BENTLEY_RUN, &gait::BENTLEY_RUN, 1.0),
            bentley::WALK if u > 0.0 => g(clip::BENTLEY_WALK, &gait::BENTLEY_WALK, u),
            bentley::TIMED => Motion::Loop(clip::BENTLEY_TIMED),
            bentley::AIR => air(),
            bentley::CARRIED => Motion::Loop(clip::BENTLEY_HELD),
            _ => idle(c.who),
        },
    }
}

/// Bentley's weapon part (lookup slot `0x17F4`): object 580, a marker on his transform 52 (5.3).
pub const WEAPON_MARKER: i32 = 580;
/// The weapon side when the marker cannot be read: left (1), its side at rest (5.3).
pub const WEAPON_SIDE_AT_REST: i32 = 1;

/// The weapon side for Bentley's crossbow pairs (5.3): 1 (left) when the weapon marker's origin, in Bentley's object
/// frame under his current pose, has y > 0, else 0.
pub fn weapon_side(world: &World, bentley: Handle) -> i32 {
    let side = || -> Option<i32> {
        let m = world.find_sly2_id(WEAPON_MARKER, Some(bentley))?;
        let o = world.sly2_object(m)?;
        let (slot, n) = o.bone?;
        let pose = world.sly2.as_deref()?.pose_of(slot);
        let at = pose.get(usize::from(n))?.mul(&o.bone_local).t;
        Some(i32::from(at[1] > 0.0))
    };
    side().unwrap_or(WEAPON_SIDE_AT_REST)
}

/// The Stealth Slide's weights for 259, 260, 261 (9.1.1): `L = 0.5·clamp(ω / 2.5, −1, 1)` with `ω` the yaw rate;
/// `max(0, L)`, `1 − |L|`, `max(0, −L)`.
pub fn slide_weights(yaw_rate: f32) -> [f32; 5] {
    let l = 0.5 * (yaw_rate / 2.5).clamp(-1.0, 1.0);
    [l.max(0.0), 1.0 - l.abs(), (-l).max(0.0), 0.0, 0.0]
}

/// Starts what a Sly 2 character's event plays over its state (throws, crossbow swings, landings, jumps).
pub fn sly2_event(c: &Character, e: &GangEvent, track: &mut Track) {
    match *e {
        // Paraglide (jump kind 19): the opening clip, then the glide.
        GangEvent::SpecialJump(19) if c.who == Who::Sly => track.overlay(Motion::Once(clip::SLY_GLIDE_OPEN), c.state),
        GangEvent::Jumped | GangEvent::SpecialJump(_) => track.launch(false),
        GangEvent::DoubleJump => track.launch(true),
        GangEvent::Landed => {
            track.jumped = false;
            track.second = false;
            track.sub_kind = 0;
            let l = clip::LANDING[k(c.who)];
            if l >= 0 {
                track.overlay(Motion::Once(l), c.state);
            }
        }
        GangEvent::Thrown { .. } if c.who == Who::Murray => track.overlay(Motion::Once(clip::MURRAY_THROW), c.state),
        GangEvent::Lift(_) if c.who == Who::Bentley => track.overlay(Motion::Once(clip::BENTLEY_PICK_UP), c.state),
        GangEvent::Crossbow { high, .. } => {
            // 5.3: blend 2·mode + side of the six low/high pairs from 560, weights 1 − u and u (the event's `high`,
            // (z + 50) / 100 clamped, 0.5 with no target). Mode from the state: 0 standing, 2 near an object ahead,
            // else 1. The side is the weapon marker's, read when the swing starts (`Track::weapon_side`).
            let mode = match c.state {
                bentley::STAND => 0,
                bentley::NEAR => 2,
                _ => 1,
            };
            let lo = clip::BENTLEY_SWING + 2 * (2 * mode + track.weapon_side);
            track.overlay(Motion::OnceBlend([lo, lo + 1], high), ANY_STATE);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lens(id: i32) -> Option<f32> {
        match id {
            1 => Some(1.0),
            2 => Some(0.5),
            3 => Some(2.0),
            _ => None,
        }
    }

    #[test]
    fn gait_members_share_a_cycle() {
        let mut t = Track::default();
        let mut out = Vec::new();
        let m = Motion::Gait { clips: [1, 2], u: 0.5, rate: 1.0 };
        t.advance(m, 1, 1.0 / 60.0, lens, &mut out);
        assert_eq!(out.len(), 2);
        // Cycle 0.75 s: after 0.375 s the phase is a half, each clip at half its length.
        for _ in 0..15 {
            t.advance(m, 1, 1.0 / 40.0, lens, &mut out);
        }
        assert!((t.phase - 0.5).abs() < 1e-4, "{}", t.phase);
        assert!((out[0].1 - 0.5).abs() < 0.03 && (out[1].1 - 0.25).abs() < 0.02, "{out:?}");
        assert!((out[0].2 - 0.5).abs() < 1e-6 && (out[1].2 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn once_holds_and_loop_wraps_and_missing_clips_drop() {
        let mut t = Track::default();
        let mut out = Vec::new();
        for _ in 0..90 {
            t.advance(Motion::Once(2), 3, 1.0 / 60.0, lens, &mut out);
        }
        assert_eq!(out, [(2, 0.5, 1.0)]);
        for _ in 0..90 {
            t.advance(Motion::Loop(1), 3, 1.0 / 60.0, lens, &mut out);
        }
        assert!(out[0].1 < 0.5, "{out:?}");
        t.advance(Motion::Gait { clips: [9, 3], u: 0.2, rate: 1.0 }, 1, 1.0 / 60.0, lens, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].2, 1.0);
        t.advance(Motion::Once(9), 1, 1.0 / 60.0, lens, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn swing_blend_follows_the_stick_and_the_angle() {
        let mut b = SwingBlend::enter();
        assert_eq!(b.weights(), [1.0, 0.0, 0.0]);
        assert!((b.time - SWING_LENGTH * 0.5).abs() < 1e-6);
        // Pumping back: the pump eases to 0, so 166 takes over.
        for _ in 0..60 {
            b.step(1.0 / 60.0, Some(3.0), 0.0, SWING_LENGTH);
        }
        assert_eq!(b.sub, 2);
        assert!(b.weights()[1] > 0.9, "{:?}", b.weights());
        // Released for 0.25 s: coasting; the activity eases to 0, so 167 takes over.
        for _ in 0..90 {
            b.step(1.0 / 60.0, None, 0.0, SWING_LENGTH);
        }
        assert_eq!(b.sub, 1);
        assert!(b.weights()[2] > 0.9, "{:?}", b.weights());
        // The time follows the swing angle at most 3 per second: θ = 1.7 is the end.
        let t0 = b.time;
        b.step(0.05, None, 1.7, SWING_LENGTH);
        assert!((b.time - (t0 + 0.15).min(SWING_LENGTH)).abs() < 1e-5);
        for _ in 0..30 {
            b.step(1.0 / 60.0, None, 5.0, SWING_LENGTH);
        }
        assert!((b.time - SWING_LENGTH).abs() < 1e-5);
        // Held ahead for more than 1.4 s: coasting.
        let mut h = SwingBlend::enter();
        for _ in 0..90 {
            h.step(1.0 / 60.0, Some(0.2), 0.0, SWING_LENGTH);
        }
        assert_eq!(h.sub, 1);
        // Bad numbers do not panic.
        h.step(f32::NAN, Some(f32::NAN), f32::NAN, f32::NAN);
        assert!(h.weights().iter().all(|w| w.is_finite()));
    }

    #[test]
    fn bad_clip_lengths_do_not_panic() {
        let bad = |id: i32| match id {
            1 => Some(-1.0),
            2 => Some(f32::NAN),
            _ => Some(f32::INFINITY),
        };
        let mut out = Vec::new();
        let mut t = Track::default();
        for m in
            [Motion::Once(1), Motion::Loop(2), Motion::Gait { clips: [1, 3], u: 0.5, rate: 1.0 }, Motion::At(2, 5.0)]
        {
            t.advance(m, 0, 1.0 / 60.0, bad, &mut out);
            assert!(out.iter().all(|&(_, at, _)| at == 0.0), "{m:?}: {out:?}");
        }
        assert_eq!(super::super::sly2_pose::sane_length(f32::NAN), 0.0);
        assert_eq!(super::super::sly2_pose::sane_length(-2.0), 0.0);
        assert_eq!(super::super::sly2_pose::sane_length(0.5), 0.5);
    }

    #[test]
    fn overlays_end_with_their_clip_or_state() {
        let mut t = Track::default();
        let mut out = Vec::new();
        t.overlay(Motion::Once(2), 0);
        t.advance(Motion::Loop(1), 0, 0.1, lens, &mut out);
        assert_eq!(out[0].0, 2);
        t.advance(Motion::Loop(1), 0, 0.5, lens, &mut out);
        assert_eq!(out[0].0, 1);
        t.overlay(Motion::Once(3), 0);
        t.advance(Motion::Loop(1), 1, 0.1, lens, &mut out);
        assert_eq!(out[0].0, 1);
    }

    #[test]
    fn jump_directions_swap_in_their_window_and_fall_back() {
        let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
        c.state = sly::AIR;
        let mut t = Track::default();
        sly2_event(&c, &GangEvent::Jumped, &mut t);
        t.observe(&c, 0.05);
        assert_eq!(sly2_motion(&c, &t), Motion::At(152, 0.05));
        // The stick past 0.5 to the right within 0.15 s: index 1, same time.
        c.last_stick = [0.0, -1.0, 0.0];
        c.last_deflection = 0.8;
        t.observe(&c, 0.05);
        assert_eq!(sly2_motion(&c, &t), Motion::At(156, 0.1));
        // After the window the stick changes nothing.
        let mut t2 = Track::default();
        sly2_event(&c, &GangEvent::DoubleJump, &mut t2);
        c.last_deflection = 0.0;
        t2.observe(&c, 0.25);
        c.last_deflection = 1.0;
        t2.observe(&c, 0.125);
        assert_eq!(sly2_motion(&c, &t2), Motion::At(160, 0.375));
        // Missing clips: index 0 of the set, then take-off 0.
        let rows = [clip::SECOND[2], clip::TAKE_OFF[2]];
        assert_eq!(sub_kind_fallback(Motion::At(-1, 0.3), &rows), Some(Motion::At(595, 0.3)));
        assert_eq!(sub_kind_fallback(Motion::At(595, 0.3), &rows), Some(Motion::At(590, 0.3)));
        assert_eq!(sub_kind_fallback(Motion::At(590, 0.3), &rows), None);
        // A plain fall plays take-off 0.
        let fall = Track::default();
        assert_eq!(sly2_motion(&c, &fall), Motion::Once(152));
    }

    #[test]
    fn slide_and_crossbow_weights() {
        assert_eq!(slide_weights(0.0), [0.0, 1.0, 0.0, 0.0, 0.0]);
        assert_eq!(slide_weights(10.0), [0.5, 0.5, 0.0, 0.0, 0.0]);
        let w = slide_weights(-1.25);
        assert!((w[2] - 0.25).abs() < 1e-6 && (w[1] - 0.75).abs() < 1e-6);
        let mut b = Character::new(Who::Bentley, [0.0; 3], 0.0);
        b.state = bentley::NEAR;
        let mut t = Track::default();
        sly2_event(&b, &GangEvent::Crossbow { target: None, high: 0.5, mode: 0 }, &mut t);
        assert_eq!(t.overlay.map(|o| o.0), Some(Motion::OnceBlend([568, 569], 0.5)));
        t.weapon_side = 1;
        b.state = bentley::WALK;
        sly2_event(&b, &GangEvent::Crossbow { target: None, high: 0.25, mode: 0 }, &mut t);
        assert_eq!(t.overlay.map(|o| o.0), Some(Motion::OnceBlend([566, 567], 0.25)));
    }

    #[test]
    fn idle_runs_chain_one_shot_clips() {
        use super::super::sly2_machine::Transition;
        let t = |from, to, flags, probability, object| Transition { from, to, flags, extra: 0, probability, object };
        // 1 → 2 by clip 1 (mask 8), 2 → 2 by clip 2 or 3 (mask 8), 2 → 1 for look targets only (mask 0x1FF0).
        let def = MachineDef {
            members: alloc::vec![1, 2, 3],
            states: alloc::vec![(1, -1), (2, -1)],
            transitions: alloc::vec![
                t(1, 2, 8, 1.0, 1),
                t(2, 2, 8, 1.0, 2),
                t(2, 2, 8, 1.0, 3),
                t(2, 1, 0x1FF0, 1.0, -1)
            ],
        };
        let mut run = IdleRun::new(1);
        let mut seed = 1u32;
        let mut seen = alloc::collections::BTreeSet::new();
        let mut first = None;
        for _ in 0..600 {
            let (c, at) = run.advance(&def, 1.0 / 60.0, || random(&mut seed), lens).unwrap();
            first.get_or_insert(c);
            assert!(at <= lens(c).unwrap());
            seen.insert(c);
        }
        assert_eq!(first, Some(1));
        assert_eq!(seen, [1, 2, 3].into_iter().collect());
        assert_eq!(run.state, 2, "the look-target transition back to 1 is never taken");
    }

    #[test]
    fn sly2_states_pick_their_clips() {
        let mut c = Character::new(Who::Sly, [0.0; 3], 0.0);
        let t = Track::default();
        c.state = sly::RUN;
        assert!(matches!(sly2_motion(&c, &t), Motion::Gait { clips: clip::SLY_RUN, .. }));
        c.state = sly::PICKPOCKET;
        assert_eq!(sly2_motion(&c, &t), Motion::Once(280));
        c.state = sly::STAND;
        assert_eq!(sly2_motion(&c, &t), Motion::Loop(5543));
        let mut m = Character::new(Who::Murray, [0.0; 3], 0.0);
        m.state = murray::PUNCH;
        m.combo.index = 2;
        assert_eq!(sly2_motion(&m, &t), Motion::Once(435));
        // Sly's run is played 1.3 times as fast when he moves at 832.
        c.state = sly::RUN;
        c.velocity = [832.0, 0.0, 0.0];
        let Motion::Gait { rate, .. } = sly2_motion(&c, &t) else { panic!() };
        assert!((rate - 1.3).abs() < 1e-3);
    }
}
