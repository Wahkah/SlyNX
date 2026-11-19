//! Playback of a segment: time, speed, repeat modes, events (animation.md section 5), applying channels to targets
//! (section 4), root motion (4.2), several playbacks on shared targets (5.4) and run-time blends (section 6).

use alloc::vec::Vec;

use formats::anim::{Channel, Event, RepeatMode, Segment};

use super::curve::{EPSILON, eval_scalar, eval_vector, key_time, sample_vector};
use crate::math::{Mat3, Vec3, add, mat_mul, rem_euclid, scale, sub};

/// Which event list an event belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventList {
    /// Fired when the segment is applied.
    Apply,
    /// Fired as playback crosses the event's frame.
    Framed,
    /// Fired when the playback is removed.
    Retract,
}

/// An event that fired: its list and index in that list of the segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FiredEvent {
    /// The list.
    pub list: EventList,
    /// Index in the list.
    pub index: u16,
}

impl FiredEvent {
    /// The event itself.
    pub fn get<'s>(&self, seg: &'s Segment) -> Option<&'s Event> {
        let list = match self.list {
            EventList::Apply => &seg.on_apply,
            EventList::Framed => &seg.framed,
            EventList::Retract => &seg.on_retract,
        };
        list.get(self.index as usize)
    }
}

/// What one advance did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// A loop wrapped (repeat mode loop).
    pub wrapped: bool,
    /// A back-and-forth playback changed direction.
    pub turned: bool,
    /// A once playback reached an end (and stopped there).
    pub reached_end: bool,
}

/// What a playback does with one of the events that act on playback itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventEffect {
    /// Nothing (the event is for another system, or was applied in place).
    None,
    /// Event 8: the playback asks to be removed.
    Remove,
}

/// One playing segment (animation.md 5.2).
#[derive(Clone, Debug, PartialEq)]
pub struct Playback {
    /// Local time in seconds, inside `[0, duration]`.
    pub time: f32,
    /// Effective speed: caller speed times the segment's base speed. Negative plays backwards.
    speed: f32,
    scale: f32,
    duration: f32,
    repeat: RepeatMode,
    pause: f32,
    include_start: bool,
    /// Priority for shared targets (option 181).
    pub priority: i32,
    /// Advances on the real clock (option 189).
    pub real_clock: bool,
    /// The player has no control while it plays (option 180).
    pub hands_off: bool,
}

impl Playback {
    /// Applies `seg` at time 0 with caller speed `speed` and fires its apply list. The level's difficulty factor is
    /// taken as 0; use [`Self::start_with`] where options 183 and 184 matter.
    pub fn start(seg: &Segment, speed: f32, fired: &mut Vec<FiredEvent>) -> Playback {
        Playback::start_with(seg, 0.0, speed, 0.0, fired)
    }

    /// Applies `seg` at `time` (clamped into the segment), difficulty factor 0, and fires its apply list.
    pub fn start_at(seg: &Segment, time: f32, speed: f32, fired: &mut Vec<FiredEvent>) -> Playback {
        Playback::start_with(seg, time, speed, 0.0, fired)
    }

    /// Applies `seg` at `time` with caller speed `speed` for a level whose difficulty factor is `difficulty`
    /// (pickups-and-progress.md). Every speed of the playback, the caller's and those of events 4, 5 and 7, is
    /// multiplied by the base speed (182) and the difficulty factor (183, 184) (animation.md 5.1, 5.2). Fires the
    /// apply list; framed events at the start time fire on the first update that advances the time (5.3).
    pub fn start_with(seg: &Segment, time: f32, speed: f32, difficulty: f32, fired: &mut Vec<FiredEvent>) -> Playback {
        let s = &seg.settings;
        let duration = seg.duration().max(0.0);
        let scale = s.speed_scale(difficulty);
        let p = Playback {
            time: time.max(0.0).min(duration),
            speed: speed * scale,
            scale,
            duration,
            repeat: s.repeat,
            pause: 0.0,
            include_start: true,
            priority: s.priority,
            real_clock: s.real_clock,
            hands_off: s.hands_off,
        };
        fired.extend((0..seg.on_apply.len()).map(|i| FiredEvent { list: EventList::Apply, index: i as u16 }));
        p
    }

    /// Fires the retract list (call when the playback is removed).
    pub fn retract(&self, seg: &Segment, fired: &mut Vec<FiredEvent>) {
        fired.extend((0..seg.on_retract.len()).map(|i| FiredEvent { list: EventList::Retract, index: i as u16 }));
    }

    /// Effective speed.
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// Sets the caller speed (multiplied by the base speed and difficulty factor).
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed * self.scale;
    }

    /// Duration in seconds.
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Repeat mode.
    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }

    /// Overrides the repeat mode.
    pub fn set_repeat(&mut self, repeat: RepeatMode) {
        self.repeat = repeat;
    }

    /// Moves to `time` (clamped) without firing events; events at the new time fire on the next advance only when
    /// playback moves past them.
    pub fn seek(&mut self, time: f32) {
        self.time = time.max(0.0).min(self.duration);
        self.include_start = false;
    }

    /// Pauses for `seconds` of the playback's clock (`f32::INFINITY` until [`Self::resume`]).
    pub fn pause_for(&mut self, seconds: f32) {
        self.pause = seconds.max(0.0);
    }

    /// Ends a pause.
    pub fn resume(&mut self) {
        self.pause = 0.0;
    }

    /// Whether the playback is paused.
    pub fn is_paused(&self) -> bool {
        self.pause > 0.0
    }

    /// Whether a once playback has stopped at an end.
    pub fn is_stopped(&self) -> bool {
        self.speed.abs() < EPSILON
    }

    /// Advances by `dt` seconds of the playback's clock, wrapping per the repeat mode and firing the framed events
    /// crossed (in the order of travel).
    pub fn advance(&mut self, seg: &Segment, dt: f32, fired: &mut Vec<FiredEvent>) -> Status {
        let mut st = Status::default();
        if self.pause > 0.0 {
            self.pause -= dt;
            if self.pause <= 0.0 {
                self.pause = 0.0;
            }
            return st;
        }
        if self.repeat == RepeatMode::Manual || self.speed.abs() < EPSILON {
            return st;
        }
        let d = self.duration;
        let old = self.time;
        let new = old + dt * self.speed;
        if new == old {
            // Nothing advances: no event fires, and events at the start time wait for a real advance.
            return st;
        }
        if d <= 0.0 {
            self.time = 0.0;
            if self.repeat == RepeatMode::Once {
                self.speed = 0.0;
                st.reached_end = true;
            }
            return st;
        }
        match self.repeat {
            RepeatMode::Once | RepeatMode::Manual => {
                if new >= d {
                    self.scan(seg, old, d, fired);
                    self.time = d;
                    self.speed = 0.0;
                    st.reached_end = true;
                } else if new < 0.0 {
                    self.scan(seg, old, 0.0, fired);
                    self.time = 0.0;
                    self.speed = 0.0;
                    st.reached_end = true;
                } else {
                    self.scan(seg, old, new, fired);
                    self.time = new;
                }
            }
            RepeatMode::Loop => {
                if new >= d {
                    self.scan(seg, old, d, fired);
                    self.time = rem_euclid(new, d);
                    self.include_start = true;
                    self.scan(seg, 0.0, self.time, fired);
                    st.wrapped = true;
                } else if new < 0.0 {
                    self.scan(seg, old, 0.0, fired);
                    self.time = rem_euclid(new, d);
                    self.include_start = true;
                    self.scan(seg, d, self.time, fired);
                    st.wrapped = true;
                } else {
                    self.scan(seg, old, new, fired);
                    self.time = new;
                }
            }
            RepeatMode::PingPong => {
                if new >= d {
                    self.scan(seg, old, d, fired);
                    let f = rem_euclid(new, 2.0 * d);
                    if f >= d {
                        self.time = 2.0 * d - f;
                        self.speed = -self.speed;
                        st.turned = true;
                        self.scan(seg, d, self.time, fired);
                    } else {
                        self.time = f;
                        self.include_start = true;
                        self.scan(seg, 0.0, f, fired);
                    }
                } else if new < 0.0 {
                    self.scan(seg, old, 0.0, fired);
                    let f = rem_euclid(-new, 2.0 * d);
                    if f <= d {
                        self.time = f;
                        self.speed = -self.speed;
                        st.turned = true;
                        self.scan(seg, 0.0, f, fired);
                    } else {
                        self.time = 2.0 * d - f;
                        self.include_start = true;
                        self.scan(seg, d, self.time, fired);
                    }
                } else {
                    self.scan(seg, old, new, fired);
                    self.time = new;
                }
            }
        }
        st
    }

    /// Fires the framed events between `a` and `b` in the direction of travel: forward `a < t <= b`, backward
    /// `b <= t < a`; the start time itself is included right after an apply or a loop wrap.
    fn scan(&mut self, seg: &Segment, a: f32, b: f32, fired: &mut Vec<FiredEvent>) {
        let include = core::mem::replace(&mut self.include_start, false);
        let first = fired.len();
        for (i, e) in seg.framed.iter().enumerate() {
            let Some(t) = e.frame.map(key_time) else { continue };
            let hit = if b >= a {
                (t > a || (include && t == a)) && t <= b
            } else {
                (t < a || (include && t == a)) && t >= b
            };
            if hit {
                fired.push(FiredEvent { list: EventList::Framed, index: i as u16 });
            }
        }
        let time_of = |f: &FiredEvent| seg.framed[f.index as usize].frame.unwrap_or(0);
        if b >= a {
            fired[first..].sort_by_key(time_of);
        } else {
            fired[first..].sort_by_key(|f| core::cmp::Reverse(time_of(f)));
        }
    }

    /// Applies an event that acts on playback itself:
    /// - 4: seek to a frame and set the speed (scaled like every speed of the playback);
    /// - 5: set the speed;
    /// - 8: remove (reported to the caller);
    /// - 11: freeze at the event's own time (clamped to the segment) with speed 0, until something sets a speed.
    ///
    /// Event 7 is the same as 4 but only when a message arrives: the caller applies it with
    /// [`Self::apply_message_seek`]. Other types are left to the systems they belong to.
    pub fn apply_event(&mut self, ev: &Event) -> EventEffect {
        match ev.kind {
            4 => {
                self.apply_message_seek(ev);
                EventEffect::None
            }
            11 => {
                let t = ev.time().unwrap_or(self.time);
                self.time = t.max(0.0).min(self.duration);
                self.speed = 0.0;
                self.include_start = false;
                EventEffect::None
            }
            5 => {
                if let Some(speed) = ev.f32_at(0) {
                    self.set_speed(speed);
                }
                EventEffect::None
            }
            8 => EventEffect::Remove,
            _ => EventEffect::None,
        }
    }

    /// The seek of events 4 and 7: go to the payload's frame and set its speed.
    pub fn apply_message_seek(&mut self, ev: &Event) {
        if let (Some(frame), Some(speed)) = (ev.i16_at(0), ev.f32_at(2)) {
            self.seek(key_time(frame));
            self.set_speed(speed);
        }
    }
}

/// Whether a looping channel's position or rotation ends away from where it starts (first and last values differ
/// by 1 unit or more, or 0.01 radian; animation.md 5.2). Such objects snap on a wrap instead of easing.
pub fn jumps_on_wrap(channel: &Channel, duration: f32) -> (bool, bool) {
    let jump = |c: &Option<formats::anim::VectorCurve>, limit: f32| {
        c.as_ref().is_some_and(|c| match (eval_vector(c, 0.0), eval_vector(c, duration)) {
            (Some(a), Some(b)) => sub(a, b).iter().any(|d| d.abs() >= limit),
            _ => false,
        })
    };
    (jump(&channel.position, 1.0), jump(&channel.rotation, 0.01))
}

/// Receives the values a segment writes. Node ids are whatever the caller's [`Binding`] resolved channels to.
pub trait AnimTarget {
    /// Local position.
    fn set_position(&mut self, node: u32, position: Vec3);
    /// Local rotation as Euler angles (x, y, z); the target composes `Post · Rz · Ry · Rx · Pre`
    /// ([`rotation_matrix`]).
    fn set_rotation(&mut self, node: u32, euler: Vec3);
    /// Local scale.
    fn set_scale(&mut self, node: u32, scale: Vec3);
    /// Inverse-kinematics twist angle.
    fn set_twist(&mut self, node: u32, twist: f32);
    /// Pose weight `pose` of the node's mesh set.
    fn set_pose_weight(&mut self, node: u32, pose: usize, weight: f32);
}

/// The rotation written to an object for Euler angles: `Post · R · Pre`, either factor omitted when absent
/// (animation.md 4.1).
pub fn rotation_matrix(euler: Vec3, pre: Option<&Mat3>, post: Option<&Mat3>) -> Mat3 {
    let mut r = super::curve::euler_zyx(euler);
    if let Some(pre) = pre {
        r = mat_mul(&r, pre);
    }
    if let Some(post) = post {
        r = mat_mul(post, &r);
    }
    r
}

/// The target node of each channel of a segment (`None`: skipped, animation.md 4.3; always `None` for the
/// root-motion channel).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Binding {
    /// One entry per channel of the segment.
    pub targets: Vec<Option<u32>>,
}

impl Binding {
    /// Resolves each driving channel's target OID with `resolve` (the caller applies the lookup order of 4.3).
    pub fn resolve(seg: &Segment, mut resolve: impl FnMut(i16) -> Option<u32>) -> Binding {
        Binding {
            targets: seg.channels.iter().map(|c| if c.is_root_motion() { None } else { resolve(c.target) }).collect(),
        }
    }
}

/// Writes the values of every bound channel of `seg` at time `t` into `target`. Curves that are blend
/// placeholders write nothing.
pub fn apply_segment(seg: &Segment, binding: &Binding, t: f32, target: &mut impl AnimTarget) {
    for (c, node) in seg.channels.iter().zip(&binding.targets) {
        let Some(node) = *node else { continue };
        if let Some(p) = c.position.as_ref().and_then(|v| eval_vector(v, t)) {
            target.set_position(node, p);
        }
        if let Some(r) = c.rotation.as_ref().and_then(|v| eval_vector(v, t)) {
            target.set_rotation(node, r);
        }
        if let Some(s) = c.scale.as_ref().and_then(|v| eval_vector(v, t)) {
            target.set_scale(node, s);
        }
        if let Some(a) = c.twist.as_ref().and_then(|v| eval_scalar(v, t)) {
            target.set_twist(node, a);
        }
        for (i, p) in c.poses.iter().enumerate() {
            if let Some(w) = p.as_ref().and_then(|v| eval_scalar(v, t)) {
                target.set_pose_weight(node, i, w);
            }
        }
    }
}

/// The root-motion velocity of `seg` at time `t` for a playback of effective speed `speed`: the derivative of the
/// root-motion channel's position curve times the speed, in the character's facing frame (animation.md 4.2). The
/// caller turns it into world space. `None` when the segment has no root-motion channel.
pub fn root_motion_velocity(seg: &Segment, t: f32, speed: f32) -> Option<Vec3> {
    let c = seg.root_motion()?;
    let s = sample_vector(c.position.as_ref()?, t)?;
    Some(scale(s.rate, speed))
}

/// One member of a run-time blend (animation.md 6).
#[derive(Clone, Copy, Debug)]
pub struct BlendMember<'a> {
    /// The segment.
    pub segment: &'a Segment,
    /// Its binding.
    pub binding: &'a Binding,
    /// Weight (members with weight 0 are skipped).
    pub weight: f32,
}

#[derive(Clone, Copy, Default)]
struct BlendAcc {
    node: u32,
    position: Option<Vec3>,
    euler: Option<Vec3>,
    twist: Option<f32>,
}

/// Evaluates every member at the same phase (local time / own duration, `phase` in `[0, 1]`) and writes the
/// weighted sums `Σ wᵢ · valueᵢ` of positions, Euler angles and twists (animation.md 6). Weights are used as given.
/// `scratch` is reused between calls to avoid allocating.
pub fn apply_blend(
    members: &[BlendMember<'_>],
    phase: f32,
    scratch: &mut Vec<BlendAccumulator>,
    target: &mut impl AnimTarget,
) {
    scratch.clear();
    for m in members.iter().filter(|m| m.weight != 0.0) {
        let t = phase * m.segment.duration();
        for (c, node) in m.segment.channels.iter().zip(&m.binding.targets) {
            let Some(node) = *node else { continue };
            let idx = match scratch.iter().position(|a| a.0.node == node) {
                Some(i) => i,
                None => {
                    scratch.push(BlendAccumulator(BlendAcc { node, ..Default::default() }));
                    scratch.len() - 1
                }
            };
            let acc = &mut scratch[idx].0;
            let w = m.weight;
            if let Some(p) = c.position.as_ref().and_then(|v| eval_vector(v, t)) {
                acc.position = Some(add(acc.position.unwrap_or([0.0; 3]), scale(p, w)));
            }
            if let Some(r) = c.rotation.as_ref().and_then(|v| eval_vector(v, t)) {
                acc.euler = Some(add(acc.euler.unwrap_or([0.0; 3]), scale(r, w)));
            }
            if let Some(a) = c.twist.as_ref().and_then(|v| eval_scalar(v, t)) {
                acc.twist = Some(acc.twist.unwrap_or(0.0) + a * w);
            }
        }
    }
    for a in scratch.iter().map(|a| a.0) {
        if let Some(p) = a.position {
            target.set_position(a.node, p);
        }
        if let Some(r) = a.euler {
            target.set_rotation(a.node, r);
        }
        if let Some(t) = a.twist {
            target.set_twist(a.node, t);
        }
    }
}

/// Scratch storage for [`apply_blend`].
#[derive(Clone, Copy, Default)]
pub struct BlendAccumulator(BlendAcc);

/// Identifies a playback in an [`Animator`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlaybackId(pub u32);

/// An event fired by a playback of an [`Animator`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimatorEvent {
    /// The playback.
    pub playback: PlaybackId,
    /// Index of its segment in the library passed to the animator.
    pub segment: u32,
    /// The event.
    pub event: FiredEvent,
}

struct Active {
    id: PlaybackId,
    segment: u32,
    binding: Binding,
    playback: Playback,
    order: u64,
}

/// Several playbacks driving shared targets (animation.md 5.4): each frame every playback advances, then they are
/// applied in increasing (priority, start order), so on a property driven by several playbacks the highest priority
/// wins and, at equal priority, the newest. When a playback is removed, the next one that drives the property takes
/// over on the next frame.
///
/// Segments live in a library slice owned by the caller (`lib[segment]`), which must stay the same between calls.
#[derive(Default)]
pub struct Animator {
    /// The level's difficulty factor, for segments with options 183 and 184 (applied to playbacks started later).
    pub difficulty: f32,
    active: Vec<Active>,
    next_id: u32,
    next_order: u64,
    fired: Vec<FiredEvent>,
}

impl Animator {
    /// An empty animator.
    pub fn new() -> Animator {
        Animator::default()
    }

    /// Applies `lib[segment]` with `binding` at caller speed `speed`; its apply events go to `events`.
    pub fn play(
        &mut self,
        lib: &[Segment],
        segment: u32,
        binding: Binding,
        speed: f32,
        events: &mut Vec<AnimatorEvent>,
    ) -> PlaybackId {
        let id = PlaybackId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.fired.clear();
        let playback = Playback::start_with(&lib[segment as usize], 0.0, speed, self.difficulty, &mut self.fired);
        events.extend(self.fired.iter().map(|&event| AnimatorEvent { playback: id, segment, event }));
        self.active.push(Active { id, segment, binding, playback, order: self.next_order });
        self.next_order += 1;
        id
    }

    /// Removes a playback, firing its retract events.
    pub fn remove(&mut self, lib: &[Segment], id: PlaybackId, events: &mut Vec<AnimatorEvent>) -> bool {
        let Some(i) = self.active.iter().position(|a| a.id == id) else { return false };
        let a = self.active.remove(i);
        self.fired.clear();
        a.playback.retract(&lib[a.segment as usize], &mut self.fired);
        events.extend(self.fired.iter().map(|&event| AnimatorEvent { playback: id, segment: a.segment, event }));
        true
    }

    /// The playback with `id`.
    pub fn get(&self, id: PlaybackId) -> Option<&Playback> {
        self.active.iter().find(|a| a.id == id).map(|a| &a.playback)
    }

    /// The playback with `id`, mutably.
    pub fn get_mut(&mut self, id: PlaybackId) -> Option<&mut Playback> {
        self.active.iter_mut().find(|a| a.id == id).map(|a| &mut a.playback)
    }

    /// Number of playbacks.
    pub fn len(&self) -> usize {
        self.active.len()
    }

    /// Whether there are no playbacks.
    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }

    /// Advances every playback (by `dt` on the game clock, or `real_dt` for real-clock segments), applies events 4,
    /// 5 and 8 to their playback (event 8 removes it, firing its retract list), reports every fired event, then
    /// writes all playbacks into `target` by priority.
    pub fn update(
        &mut self,
        lib: &[Segment],
        dt: f32,
        real_dt: f32,
        target: &mut impl AnimTarget,
        events: &mut Vec<AnimatorEvent>,
    ) {
        let mut remove: Vec<PlaybackId> = Vec::new();
        for a in &mut self.active {
            let seg = &lib[a.segment as usize];
            self.fired.clear();
            let step = if a.playback.real_clock { real_dt } else { dt };
            a.playback.advance(seg, step, &mut self.fired);
            for &event in &self.fired {
                events.push(AnimatorEvent { playback: a.id, segment: a.segment, event });
                if let Some(ev) = event.get(seg)
                    && a.playback.apply_event(ev) == EventEffect::Remove
                    && !remove.contains(&a.id)
                {
                    remove.push(a.id);
                }
            }
        }
        for id in remove {
            self.remove(lib, id, events);
        }
        self.apply(lib, target);
    }

    /// Writes every playback into `target`, lowest (priority, start order) first.
    pub fn apply(&mut self, lib: &[Segment], target: &mut impl AnimTarget) {
        self.active.sort_by_key(|a| (a.playback.priority, a.order));
        for a in &self.active {
            apply_segment(&lib[a.segment as usize], &a.binding, a.playback.time, target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use formats::anim::{LinearKey, ScalarCurve, SegmentSettings, VectorCurve};
    use formats::options::OptionList;

    fn linear(v0: f32, v1: f32, frames: i16) -> ScalarCurve {
        ScalarCurve::Linear(vec![LinearKey { frame: 0, value: v0 }, LinearKey { frame: frames, value: v1 }])
    }

    fn event(kind: i8, frame: i16, payload: Vec<u8>) -> Event {
        Event { kind, frame: Some(frame), offset: 0, payload }
    }

    fn seg(repeat: RepeatMode, priority: i32) -> Segment {
        let x = VectorCurve::Components {
            mask: 1,
            base: [0.0, 5.0, 0.0],
            curves: [Some(linear(0.0, 60.0, 60)), None, None],
        };
        Segment {
            offset: 0,
            end: 0,
            root: 0,
            duration_frames: 60,
            search_roots: vec![],
            channels: vec![
                Channel {
                    target: 1,
                    flags: 0x01 | 0x08,
                    position: Some(x.clone()),
                    rotation: None,
                    scale: None,
                    twist: Some(linear(0.0, 1.0, 60)),
                    poses: vec![None, Some(linear(1.0, 0.0, 60))],
                },
                Channel {
                    target: 2,
                    flags: 0x21,
                    position: Some(x),
                    rotation: None,
                    scale: None,
                    twist: None,
                    poses: vec![],
                },
            ],
            on_apply: vec![Event { kind: 39, frame: None, offset: 0, payload: vec![1, 0] }],
            framed: vec![event(39, 30, vec![2, 0]), event(39, 0, vec![3, 0]), event(39, 60, vec![4, 0])],
            on_retract: vec![Event { kind: 39, frame: None, offset: 0, payload: vec![5, 0] }],
            options: OptionList::default(),
            settings: SegmentSettings { repeat, priority, ..Default::default() },
        }
    }

    fn labels(seg: &Segment, fired: &[FiredEvent]) -> Vec<i16> {
        fired.iter().map(|f| f.get(seg).unwrap().i16_at(0).unwrap()).collect()
    }

    #[derive(Default)]
    struct Rec {
        pos: Vec<(u32, Vec3)>,
        twist: Vec<(u32, f32)>,
        poses: Vec<(u32, usize, f32)>,
    }
    impl AnimTarget for Rec {
        fn set_position(&mut self, n: u32, p: Vec3) {
            self.pos.push((n, p));
        }
        fn set_rotation(&mut self, _: u32, _: Vec3) {}
        fn set_scale(&mut self, _: u32, _: Vec3) {}
        fn set_twist(&mut self, n: u32, a: f32) {
            self.twist.push((n, a));
        }
        fn set_pose_weight(&mut self, n: u32, i: usize, w: f32) {
            self.poses.push((n, i, w));
        }
    }

    #[test]
    fn once_stops_at_the_end_and_fires_events_in_order() {
        let s = seg(RepeatMode::Once, 0);
        let mut f = Vec::new();
        let mut p = Playback::start(&s, 1.0, &mut f);
        assert_eq!(labels(&s, &f), [1]);
        f.clear();
        p.advance(&s, 0.25, &mut f);
        assert_eq!(labels(&s, &f), [3]);
        f.clear();
        let st = p.advance(&s, 1.0, &mut f);
        assert!(st.reached_end && p.is_stopped());
        assert_eq!(p.time, 1.0);
        assert_eq!(labels(&s, &f), [2, 4]);
        f.clear();
        p.advance(&s, 1.0, &mut f);
        assert!(f.is_empty());
        p.retract(&s, &mut f);
        assert_eq!(labels(&s, &f), [5]);
    }

    #[test]
    fn loop_wraps_and_rescans_from_the_start() {
        let s = seg(RepeatMode::Loop, 0);
        let mut f = Vec::new();
        let mut p = Playback::start(&s, 1.0, &mut f);
        f.clear();
        p.advance(&s, 0.75, &mut f);
        assert_eq!(labels(&s, &f), [3, 2]);
        f.clear();
        let st = p.advance(&s, 0.5, &mut f);
        assert!(st.wrapped);
        assert!((p.time - 0.25).abs() < 1e-6);
        assert_eq!(labels(&s, &f), [4, 3]);
        // Backwards: events in descending order, wrap to the end.
        p.set_speed(-1.0);
        f.clear();
        let st = p.advance(&s, 0.5, &mut f);
        assert!(st.wrapped);
        assert!((p.time - 0.75).abs() < 1e-6);
        assert_eq!(labels(&s, &f), [3, 4]);
    }

    #[test]
    fn ping_pong_turns_around() {
        let s = seg(RepeatMode::PingPong, 0);
        let mut f = Vec::new();
        let mut p = Playback::start(&s, 1.0, &mut f);
        let st = p.advance(&s, 1.25, &mut f);
        assert!(st.turned);
        assert!((p.time - 0.75).abs() < 1e-6);
        assert!(p.speed() < 0.0);
        let st = p.advance(&s, 1.0, &mut f);
        assert!(st.turned);
        assert!((p.time - 0.25).abs() < 1e-6);
        assert!(p.speed() > 0.0);
    }

    #[test]
    fn manual_and_paused_do_not_advance() {
        let s = seg(RepeatMode::Manual, 0);
        let mut f = Vec::new();
        let mut p = Playback::start(&s, 1.0, &mut f);
        p.advance(&s, 0.5, &mut f);
        assert_eq!(p.time, 0.0);
        p.seek(0.4);
        assert_eq!(p.time, 0.4);
        let s = seg(RepeatMode::Loop, 0);
        let mut p = Playback::start(&s, 1.0, &mut f);
        p.pause_for(0.1);
        p.advance(&s, 0.05, &mut f);
        assert_eq!(p.time, 0.0);
        p.advance(&s, 0.05, &mut f);
        p.advance(&s, 0.5, &mut f);
        assert_eq!(p.time, 0.5);
    }

    #[test]
    fn playback_events_act_on_the_playback() {
        let s = seg(RepeatMode::Loop, 0);
        let mut f = Vec::new();
        let mut p = Playback::start(&s, 1.0, &mut f);
        let mut payload = 30i16.to_le_bytes().to_vec();
        payload.extend_from_slice(&2.0f32.to_le_bytes());
        assert_eq!(p.apply_event(&Event { kind: 4, frame: None, offset: 0, payload }), EventEffect::None);
        assert_eq!((p.time, p.speed()), (0.5, 2.0));
        p.apply_event(&Event { kind: 5, frame: None, offset: 0, payload: 0.5f32.to_le_bytes().to_vec() });
        assert_eq!(p.speed(), 0.5);
        assert_eq!(p.apply_event(&Event { kind: 8, frame: None, offset: 0, payload: vec![] }), EventEffect::Remove);
        // Event 11 freezes the playback at its own frame.
        p.apply_event(&Event { kind: 11, frame: Some(45), offset: 0, payload: vec![] });
        assert_eq!((p.time, p.speed()), (0.75, 0.0));
        p.advance(&s, 0.5, &mut f);
        assert_eq!(p.time, 0.75);
        // Speeds are scaled by the base speed and the difficulty factor.
        let mut d = seg(RepeatMode::Loop, 0);
        d.settings.base_speed = 2.0;
        d.settings.difficulty_speed = Some([0.0, 1.0, 0.0]);
        d.settings.difficulty_clamp = Some([0.0, 1.0]);
        let mut p = Playback::start_with(&d, 0.0, 1.0, 0.25, &mut f);
        assert_eq!(p.speed(), 0.5);
        p.apply_event(&Event { kind: 5, frame: None, offset: 0, payload: 2.0f32.to_le_bytes().to_vec() });
        assert_eq!(p.speed(), 1.0);
        // A zero step fires nothing, not even frame-0 events.
        f.clear();
        let mut p = Playback::start(&s, 1.0, &mut f);
        f.clear();
        p.advance(&s, 0.0, &mut f);
        assert!(f.is_empty());
        p.advance(&s, 0.01, &mut f);
        assert_eq!(f.len(), 1);
    }

    #[test]
    fn applies_channels_and_root_motion() {
        let s = seg(RepeatMode::Once, 0);
        let b = Binding::resolve(&s, |oid| Some(oid as u32 * 10));
        assert_eq!(b.targets, [Some(10), None]);
        let mut r = Rec::default();
        apply_segment(&s, &b, 0.5, &mut r);
        assert_eq!(r.pos, [(10, [30.0, 5.0, 0.0])]);
        assert_eq!(r.twist, [(10, 0.5)]);
        assert_eq!(r.poses, [(10, 1, 0.5)]);
        let v = root_motion_velocity(&s, 0.5, 2.0).unwrap();
        assert!((v[0] - 120.0).abs() < 1e-3 && v[1] == 0.0);
        assert_eq!(jumps_on_wrap(&s.channels[0], 1.0), (true, false));
    }

    #[test]
    fn animator_priority_and_newest_wins() {
        let lib = vec![seg(RepeatMode::Loop, 1), seg(RepeatMode::Loop, 0), seg(RepeatMode::Loop, 1)];
        let mut a = Animator::new();
        let mut ev = Vec::new();
        let bind = |s: &Segment| Binding::resolve(s, |_| Some(0));
        let p0 = a.play(&lib, 0, bind(&lib[0]), 1.0, &mut ev);
        let p1 = a.play(&lib, 1, bind(&lib[1]), 1.0, &mut ev);
        a.get_mut(p1).unwrap().seek(0.9);
        let mut r = Rec::default();
        a.update(&lib, 0.0, 0.0, &mut r, &mut ev);
        // The priority-1 playback is written last.
        assert_eq!(r.pos.last().unwrap().1[0], 0.0);
        let p2 = a.play(&lib, 2, bind(&lib[2]), 1.0, &mut ev);
        a.get_mut(p2).unwrap().seek(0.5);
        let mut r = Rec::default();
        a.apply(&lib, &mut r);
        assert_eq!(r.pos.last().unwrap().1[0], 30.0);
        ev.clear();
        assert!(a.remove(&lib, p2, &mut ev));
        assert_eq!(ev.len(), 1);
        assert_eq!(a.len(), 2);
        assert!(a.get(p0).is_some());
    }

    #[test]
    fn blend_is_a_weighted_sum() {
        let s0 = seg(RepeatMode::Loop, 0);
        let mut s1 = seg(RepeatMode::Loop, 0);
        s1.duration_frames = 120;
        let b0 = Binding::resolve(&s0, |_| Some(7));
        let b1 = Binding::resolve(&s1, |_| Some(7));
        let members = [
            BlendMember { segment: &s0, binding: &b0, weight: 0.25 },
            BlendMember { segment: &s1, binding: &b1, weight: 0.75 },
            BlendMember { segment: &s1, binding: &b1, weight: 0.0 },
        ];
        let mut r = Rec::default();
        let mut scratch = Vec::new();
        apply_blend(&members, 0.5, &mut scratch, &mut r);
        // Phase 0.5: s0 at 0.5 s (x = 30), s1 at 1 s (x = 60, the end of its keys).
        assert_eq!(r.pos.len(), 1);
        assert!((r.pos[0].1[0] - (0.25 * 30.0 + 0.75 * 60.0)).abs() < 1e-4);
        assert!((r.pos[0].1[1] - 5.0).abs() < 1e-6);
    }
}
