//! The Sly 2 view stack's views besides the zones' follow view (specs/sly2/camera.md 2, 6, 7.1 and 7.2), used by
//! [`super::CameraSystem`] for Sly 2 and Sly 3 worlds. The highest-priority view drives the camera:
//!
//! | View | Priority | Here |
//! |---|---:|---|
//! | camera object (class 44), timed move | 10 | [`camera_object_view`], [`TimedMove`] |
//!
//! A camera object's view lives exactly as long as the object is in the world (camera.md 7.2.1): pushed when it
//! enters, ended only when it leaves. A push of equal priority goes beneath the views already there, so among equal
//! priorities the earliest pushed is on top.
//! | turret or vehicle mount | 9 | `World::shared.camera_mount` |
//! | binocucom | 8 [I: not established] | [`Binocucom`] |
//! | two-character framing | 6 | [`pair_view`] |
//! | follow (zones) | zone option 200 + 2 | the follow camera |
//!
//! Visibility volumes (class 5 in Sly 2, 7 in Sly 3; section 7.1) choose the object mask by the camera's position
//! ([`VisibilityVolume`], [`visibility_mask`]).

use alloc::vec::Vec;

use formats::geometry::Affine;

use super::sly2::TreeVolume;
use crate::math::{Vec3, add, atan2, cos, cross, length, normalize, scale, sin, sub};
use crate::object::Handle;
use crate::world::World;

/// View-stack priorities (camera.md 2).
pub mod priority {
    /// Camera objects and timed moves.
    pub const CAMERA_OBJECT: i32 = 10;
    /// Turret and vehicle mounts.
    pub const MOUNT: i32 = 9;
    /// The binocucom (not established in the spec; between mounts and two-character framing).
    pub const BINOCUCOM: i32 = 8;
    /// Two-character framing.
    pub const PAIR: i32 = 6;
}

/// One view's camera: eye, target, field of view and, when it sets one, the near clip.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewShot {
    /// Eye.
    pub eye: Vec3,
    /// Looked-at point.
    pub target: Vec3,
    /// Field of view (vertical, radians), if the view sets one.
    pub fov: Option<f32>,
    /// Near clip, if the view sets one.
    pub near: Option<f32>,
}

// ---------------------------------------------------------------- timed moves

/// The timed move's curves as (u³, u², u) coefficients, at 0x2BDA70 [D]: linear, two others, smoothstep (the third
/// reads oddly in the spec; kept as stored).
pub const MOVE_CURVES: [[f32; 3]; 4] = [[0.0, 0.0, 1.0], [-1.0, 2.0, 0.0], [-1.0, -1.0, 1.0], [-2.0, 3.0, 0.0]];

/// A camera state: position, Euler angles (Z·Y·X, the camera's forward along +x), field of view.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CamState {
    /// Position.
    pub position: Vec3,
    /// Euler angles.
    pub angles: Vec3,
    /// Field of view.
    pub fov: f32,
}

impl CamState {
    /// The state's shot.
    pub fn shot(&self) -> ViewShot {
        let m = crate::anim::curve::euler_zyx(self.angles);
        ViewShot { eye: self.position, target: add(self.position, m[0]), fov: Some(self.fov), near: None }
    }
}

/// A timed move between two camera states (camera.md 2): position, angles and field of view blend by one of
/// [`MOVE_CURVES`] over `duration`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimedMove {
    /// From.
    pub from: CamState,
    /// To.
    pub to: CamState,
    /// Duration in seconds.
    pub duration: f32,
    /// The curve (index into [`MOVE_CURVES`]).
    pub curve: usize,
    /// Time so far.
    pub time: f32,
}

impl TimedMove {
    /// The blend weight at time `t`.
    pub fn weight(&self, t: f32) -> f32 {
        let u = if self.duration > 0.0 { (t / self.duration).clamp(0.0, 1.0) } else { 1.0 };
        let c = MOVE_CURVES[self.curve.min(MOVE_CURVES.len() - 1)];
        c[0] * u * u * u + c[1] * u * u + c[2] * u
    }

    /// Advances; returns the shot and whether the move has ended.
    pub fn step(&mut self, dt: f32) -> (ViewShot, bool) {
        self.time += dt;
        let w = self.weight(self.time);
        let mix = |a: Vec3, b: Vec3| add(a, scale(sub(b, a), w));
        let s = CamState {
            position: mix(self.from.position, self.to.position),
            angles: mix(self.from.angles, self.to.angles),
            fov: self.from.fov + (self.to.fov - self.from.fov) * w,
        };
        // A duration that is not a number ends the move at once (review hook: bad data must not hold the view).
        (s.shot(), self.time >= self.duration || self.duration.is_nan())
    }
}

// ---------------------------------------------------------------- two-character framing

/// Two-character framing's tunables (the spec gives only "wider and farther as they part" [R]): the eye's distance
/// from the midpoint at no separation and per unit of separation, its height, and the field of view at no
/// separation, per unit of separation and at most.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PairTunables {
    /// Distance at no separation, and per unit of separation.
    pub distance: [f32; 2],
    /// Eye height above the midpoint.
    pub height: f32,
    /// Field of view at no separation, per unit of separation, and at most.
    pub fov: [f32; 3],
}

impl Default for PairTunables {
    fn default() -> Self {
        PairTunables { distance: [400.0, 0.8], height: 150.0, fov: [1.0, 0.0002, 1.4] }
    }
}

/// Two-character framing (camera.md 2): both in view, the eye to the side of the line between them (on the side of
/// `from`, the current eye), wider and farther as they part.
pub fn pair_view(a: Vec3, b: Vec3, from: Vec3, t: &PairTunables) -> ViewShot {
    let mid = scale(add(a, b), 0.5);
    let d = sub(b, a);
    let sep = length([d[0], d[1], 0.0]);
    let along = if sep > 1e-3 { scale([d[0], d[1], 0.0], 1.0 / sep) } else { [1.0, 0.0, 0.0] };
    let mut side = cross([0.0, 0.0, 1.0], along);
    if crate::math::dot(side, sub(from, mid)) < 0.0 {
        side = scale(side, -1.0);
    }
    let dist = t.distance[0] + t.distance[1] * sep;
    let eye = add(add(mid, scale(side, dist)), [0.0, 0.0, t.height]);
    let fov = (t.fov[0] + t.fov[1] * sep).min(t.fov[2]);
    ViewShot { eye, target: mid, fov: Some(fov), near: None }
}

// ---------------------------------------------------------------- binocucom

/// The binocucom's values (camera.md 6) [D 0x2A5000, 0x2BD950..0x2BD9B0].
pub mod binocucom {
    /// Eye raise above a character focus.
    pub const RAISE: f32 = 150.0;
    /// Eye distance behind the focus along the view.
    pub const BACK: f32 = 50.0;
    /// Near clip while on.
    pub const NEAR: f32 = 49.0;
    /// Limits for a character: pan, tilt.
    pub const CHARACTER_LIMITS: [f32; 2] = [10.0, 1.0];
    /// Limits for another object: pan around its heading, tilt.
    pub const OBJECT_LIMITS: [f32; 2] = [1.0, 1.5];
    /// Target turn speeds per unit of x·|x| and y·|y| (pan, tilt).
    pub const SPEED: [f32; 2] = [0.9, 0.8];
    /// Turn acceleration (rad/s²).
    pub const ACCEL: f32 = 10.0;
    /// Slowing near the limits (fast, slow, fast time).
    pub const LIMIT_EASE: [f32; 3] = [1.0, 0.0, 0.2];
    /// The turn rate's scale per unit of field of view.
    pub const FOV_SCALE: f32 = 1.3;
    /// The narrowest field of view, as a fraction of the normal one.
    pub const ZOOM_MIN: f32 = 1.0 / 7.0;
}

/// The binocucom view (camera.md 6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Binocucom {
    /// The focus object.
    pub focus: Handle,
    /// The focus is a playable character.
    pub character: bool,
    /// Pan (absolute heading) and tilt.
    pub angles: [f32; 2],
    /// Pan and tilt speeds.
    pub speed: [f32; 2],
    /// The focus's heading when the view started (the centre of an object's pan limits).
    pub base: f32,
    /// Zoom, 0 normal to 1 narrowest.
    pub zoom: f32,
    /// Inverted tilt ("Invert Binocucom Controls").
    pub invert: bool,
}

impl Binocucom {
    /// Starts the view on `focus`, looking along its heading.
    pub fn new(focus: Handle, character: bool, heading: f32, invert: bool) -> Binocucom {
        Binocucom { focus, character, angles: [heading, 0.0], speed: [0.0; 2], base: heading, zoom: 0.0, invert }
    }

    /// One step: the left stick turns, the right stick's vertical axis zooms. `normal_fov` is the lens's field of
    /// view; returns the shot.
    pub fn step(&mut self, dt: f32, left: [f32; 2], right: [f32; 2], at: Vec3, normal_fov: f32) -> ViewShot {
        use binocucom::*;
        // Review hook: a step that is not a finite, non-negative time moves nothing.
        let dt = if dt.is_finite() && dt > 0.0 { dt } else { 0.0 };
        let fov = normal_fov * (1.0 - self.zoom * (1.0 - ZOOM_MIN));
        let rate = FOV_SCALE * fov;
        let y = if self.invert { -left[1] } else { left[1] };
        let goal = [-left[0] * left[0].abs() * SPEED[0] * rate, y * y.abs() * SPEED[1] * rate];
        let limits = if self.character { CHARACTER_LIMITS } else { OBJECT_LIMITS };
        let centre = [if self.character { self.angles[0] } else { self.base }, 0.0];
        for k in 0..2 {
            let dv = goal[k] - self.speed[k];
            self.speed[k] += dv.clamp(-ACCEL * dt, ACCEL * dt);
            // Slowing near the limits: the speed toward a limit is capped by the room left, eased.
            let lo = centre[k] - limits[k];
            let hi = centre[k] + limits[k];
            let room = if self.speed[k] > 0.0 { hi - self.angles[k] } else { self.angles[k] - lo };
            let p = crate::physics::SmoothParams {
                fast: LIMIT_EASE[0],
                slow: LIMIT_EASE[1],
                fast_time: LIMIT_EASE[2],
                max_speed_change: None,
            };
            let cap = p.speed_at(room.max(0.0));
            if k == 1 || !self.character {
                self.speed[k] = self.speed[k].clamp(-cap.max(0.0), cap.max(0.0));
            }
            let a = self.angles[k] + self.speed[k] * dt;
            // Review hook: limits that are not numbers (a bad heading) leave the angle unclamped instead of panicking.
            self.angles[k] = if lo <= hi { a.clamp(lo, hi) } else { a };
        }
        // Zoom: speed (2 − u²)·y·|y| on the right stick's vertical axis.
        let zy = right[1];
        self.zoom = (self.zoom + (2.0 - self.zoom * self.zoom) * zy * zy.abs() * dt).clamp(0.0, 1.0);
        let [yaw, pitch] = self.angles;
        let dir = [cos(pitch) * cos(yaw), cos(pitch) * sin(yaw), sin(pitch)];
        let base = if self.character { add(at, [0.0, 0.0, RAISE]) } else { at };
        let eye = sub(base, scale(dir, BACK));
        ViewShot { eye, target: add(eye, dir), fov: Some(fov), near: Some(NEAR) }
    }

    /// Where the camera cuts to when the view ends: looking back the way the view looked.
    pub fn exit_cut(&self, shot: &ViewShot) -> (Vec3, Vec3) {
        (shot.eye, shot.target)
    }
}

// ---------------------------------------------------------------- camera objects

/// Sly 2's camera-object class.
pub const CAMERA_OBJECT_CLASS: i16 = 44;

/// Camera-object options (camera.md 7.2).
pub mod opt {
    /// Field of view.
    pub const FOV: u16 = 1054;
    /// Target object.
    pub const TARGET: u16 = 1055;
    /// Cut when the view ends.
    pub const CUT_AT_END: u16 = 1059;
}

/// The shot of camera object `h`: at its transform, looking at its target (1055) or along its forward axis, with
/// its field of view (1054).
pub fn camera_object_view(world: &World, h: Handle) -> Option<ViewShot> {
    let w = world.world_transform(h)?;
    // Sly 3's cutscene cameras (class 42) carry 1153 to 1158 for Sly 2's 1054 to 1059.
    let translated = world.sly3.is_some().then(|| {
        let mut t = crate::object::OptionStore::new();
        for (id, v) in world.options(h).map(|o| o.entries()).unwrap_or(&[]) {
            if let Some(to) = super::sly2::sly3_cutscene_option(*id) {
                t.push(to, v.clone());
            }
        }
        t
    });
    let o = translated.as_ref().or_else(|| world.options(h));
    let target = o
        .and_then(|o| o.i32(opt::TARGET))
        .and_then(|id| world.find_sly2_id(id, Some(h)))
        .and_then(|t| world.position(t))
        .unwrap_or_else(|| add(w.t, normalize(w.cols[0])));
    // Sly 2 pose hook: a clip's focal-length curve on the camera wins over option 1054 (camera.md 2.1).
    let lens = world.sly2.as_deref().zip(world.slot(h)).and_then(|(l, s)| l.poser.fov_of(s));
    Some(ViewShot { eye: w.t, target, fov: lens.or_else(|| o.and_then(|o| o.f32(opt::FOV))), near: None })
}

/// The lens a focal length `f` gives (camera.md 7.2): field of view = 2·atan(17.996 / f).
pub fn fov_of_focal_length(f: f32) -> f32 {
    2.0 * atan2(17.996, f)
}

// ---------------------------------------------------------------- visibility volumes

/// Sly 2's visibility-volume class.
pub const VISIBILITY_CLASS_SLY2: i16 = 5;
/// Sly 3's.
pub const VISIBILITY_CLASS_SLY3: i16 = 7;
/// The mask when the camera is in no volume.
pub const DEFAULT_MASK: u32 = 0xFF;
/// The most volumes the camera is in at once.
pub const MAX_VOLUMES: usize = 32;

/// A visibility volume (camera.md 7.1).
#[derive(Clone, Debug, PartialEq)]
pub struct VisibilityVolume {
    /// The object.
    pub object: Handle,
    /// Its volume.
    pub tree: TreeVolume,
    /// The object mask it selects.
    pub mask: u32,
    /// Its priority (Sly 2 option 175, Sly 3 option 202).
    pub priority: i32,
}

/// The visibility volumes of a Sly 2 or Sly 3 world.
pub fn visibility_volumes(world: &World) -> Vec<VisibilityVolume> {
    let (class, sly3, prio) = if world.sly2.is_some() {
        (VISIBILITY_CLASS_SLY2, false, 175)
    } else if world.sly3.is_some() {
        (VISIBILITY_CLASS_SLY3, true, 202)
    } else {
        return Vec::new();
    };
    let data = &world.level.bytes;
    let mut out = Vec::new();
    for h in world.handles().filter(|&h| world.class(h) == Some(class)) {
        let body = if sly3 {
            world.sly3_record(h).map(|r| r.body_offset as usize)
        } else {
            world.sly2_record(h).map(|r| r.body_offset as usize)
        };
        let (Some(b), Some(w)) = (body, world.world_transform(h)) else { continue };
        let at = b + 2 + 48 + 4;
        let Some(m) = data.get(at..at + 4) else { continue };
        let mask = u32::from_le_bytes([m[0], m[1], m[2], m[3]]);
        let Some(tree) = TreeVolume::read(data, at + 4, &w) else { continue };
        let priority = world.options(h).and_then(|o| o.i32(prio)).unwrap_or(0);
        out.push(VisibilityVolume { object: h, tree, mask, priority });
    }
    out
}

/// The visibility state: the volumes the camera is in, most recent entry last.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Visibility {
    /// Indices into the volumes, in order of entry.
    pub inside: Vec<usize>,
    /// The mask in force.
    pub mask: u32,
}

/// One step: the volumes containing `eye` (up to 32), then the highest priority, the most recent entry on a tie;
/// its mask, or [`DEFAULT_MASK`] in none.
pub fn visibility_mask(volumes: &[VisibilityVolume], eye: Vec3, state: &mut Visibility) -> u32 {
    state.inside.retain(|&i| volumes.get(i).is_some_and(|v| v.tree.contains(eye)));
    for (i, v) in volumes.iter().enumerate() {
        if state.inside.len() >= MAX_VOLUMES {
            break;
        }
        if !state.inside.contains(&i) && v.tree.contains(eye) {
            state.inside.push(i);
        }
    }
    let top = state.inside.iter().map(|&i| volumes[i].priority).max();
    let best = top.and_then(|top| state.inside.iter().rev().copied().find(|&i| volumes[i].priority == top));
    state.mask = best.map_or(DEFAULT_MASK, |i| volumes[i].mask);
    state.mask
}

/// Whether a view (priority, push order) is above another in the stack (camera.md 7.2.1): a higher priority, or an
/// equal one pushed earlier (a push goes beneath every entry of equal or higher priority).
pub fn above(view: (i32, u64), other: (i32, u64)) -> bool {
    view.0 > other.0 || (view.0 == other.0 && view.1 < other.1)
}

/// The top of a stack of views (priority, push order), if any.
pub fn top(views: &[(i32, u64)]) -> Option<usize> {
    (0..views.len()).reduce(|best, i| if above(views[i], views[best]) { i } else { best })
}

/// The placement whose forward axis looks from `eye` to `target` (for the follow camera's placement mode).
pub fn placement(eye: Vec3, target: Vec3) -> Affine {
    let f = normalize(sub(target, eye));
    let l = {
        let c = cross([0.0, 0.0, 1.0], f);
        if length(c) < 1e-5 { [0.0, 1.0, 0.0] } else { normalize(c) }
    };
    let u = cross(f, l);
    Affine { cols: [f, l, u], t: eye }
}

// ---------------------------------------------------------------- the stack

/// The Sly 2 / Sly 3 view stack's state (camera.md 2): what the views above the follow view need from frame to
/// frame, and the visibility volumes.
#[derive(Clone, Debug, Default)]
pub struct ViewStack {
    /// Camera objects, each with whether it was in the world last frame.
    pub objects: Vec<(Handle, bool)>,
    /// The camera objects whose view is pushed, oldest first, with the order they were pushed in.
    pub active_objects: Vec<Handle>,
    /// See [`ViewStack::active_objects`].
    pub pushed: Vec<u64>,
    /// When the timed move was pushed, and the next push's number (camera.md 7.2.1: an equal-priority push goes
    /// beneath).
    timed_pushed: u64,
    next_push: u64,
    /// The timed move running, and the last one started from the world.
    pub timed: Option<TimedMove>,
    seen_move: Option<TimedMove>,
    /// The binocucom view while on.
    pub binocucom: Option<Binocucom>,
    /// Two-character framing's tunables.
    pub pair: PairTunables,
    /// The shot of the view on top last frame, and whether leaving it cuts.
    last: Option<(ViewShot, bool)>,
    cut: Option<(Vec3, Vec3)>,
    /// Visibility volumes, the state, and the mask in force.
    pub volumes: Vec<VisibilityVolume>,
    /// See [`ViewStack::volumes`].
    pub visibility: Visibility,
    /// See [`ViewStack::volumes`].
    pub mask: u32,
}

impl ViewStack {
    /// The stack of a Sly 2 or Sly 3 world: its camera objects (in the world at load: not pushed; a view is pushed
    /// when one enters later [I]) and visibility volumes.
    pub fn new(world: &World) -> ViewStack {
        let class = if world.sly2.is_some() { CAMERA_OBJECT_CLASS } else { super::sly2::CUTSCENE_CLASS_SLY3 };
        let objects =
            world.handles().filter(|&h| world.class(h) == Some(class)).map(|h| (h, world.in_world(h))).collect();
        ViewStack { objects, volumes: visibility_volumes(world), mask: DEFAULT_MASK, ..ViewStack::default() }
    }

    /// The cut to make when the view on top has ended (the binocucom, or a camera object with option 1059).
    pub fn take_cut(&mut self) -> Option<(Vec3, Vec3)> {
        self.cut.take()
    }

    /// One frame: the shot of the highest view above `floor` (`None`: the follow view drives). `eye` is the follow
    /// camera's eye, `fov` the lens's field of view.
    pub fn step(
        &mut self,
        world: &World,
        input: &crate::game::systems::FrameInput,
        dt: f32,
        eye: Vec3,
        fov: f32,
        floor: i32,
    ) -> Option<ViewShot> {
        // Camera objects entering and leaving the world.
        for (h, was) in &mut self.objects {
            let now = world.in_world(*h);
            if now && !*was {
                self.active_objects.push(*h);
                self.pushed.push(self.next_push);
                self.next_push += 1;
            } else if !now
                && *was
                && let Some(k) = self.active_objects.iter().position(|a| a == h)
            {
                self.active_objects.remove(k);
                self.pushed.remove(k);
            }
            *was = now;
        }
        // A new timed move from the world.
        if world.shared.camera_move != self.seen_move {
            self.seen_move = world.shared.camera_move;
            self.timed = self.seen_move.map(|m| TimedMove { time: 0.0, ..m });
            self.timed_pushed = self.next_push;
            self.next_push += 1;
        }
        // The binocucom.
        match (world.shared.camera_binocucom, &self.binocucom) {
            (Some((h, invert)), None) => {
                let t = world.world_transform(h);
                let heading = t.map_or(0.0, |t| atan2(t.cols[0][1], t.cols[0][0]));
                let character =
                    world.class(h).is_some_and(|c| crate::behavior::sly3::character_classes(world).contains(&c));
                self.binocucom = Some(Binocucom::new(h, character, heading, invert));
            }
            (None, Some(_)) => self.binocucom = None,
            _ => {}
        }
        // The highest priority wins; among equal priorities the earliest push (a later one went beneath).
        let mut best: Option<(i32, ViewShot, bool, u64)> = None;
        let mut offer = |p: i32, s: Option<ViewShot>, cuts: bool, order: u64| {
            if let Some(s) = s
                && p > floor
                && best.as_ref().is_none_or(|b| above((p, order), (b.0, b.3)))
            {
                best = Some((p, s, cuts, order));
            }
        };
        // Every camera object's view is in the stack; the earliest pushed of them is above the others.
        if let (Some(&h), Some(&order)) = (self.active_objects.first(), self.pushed.first()) {
            let cut_option = if world.sly3.is_some() { 1158 } else { opt::CUT_AT_END };
            let cuts = world.options(h).and_then(|o| o.i32(cut_option)).is_some_and(|v| v != 0);
            offer(priority::CAMERA_OBJECT, camera_object_view(world, h), cuts, order);
        }
        if let Some(m) = &mut self.timed {
            let (s, done) = m.step(dt);
            if done {
                self.timed = None;
            }
            offer(priority::CAMERA_OBJECT, Some(s), false, self.timed_pushed);
        }
        if let Some(b) = &mut self.binocucom {
            let at = world.position(b.focus).unwrap_or([0.0; 3]);
            offer(priority::BINOCUCOM, Some(b.step(dt, input.left, input.right, at, fov)), true, 0);
        }
        if let Some((a, b)) = world.shared.camera_pair
            && let (Some(pa), Some(pb)) = (world.position(a), world.position(b))
        {
            offer(priority::PAIR, Some(pair_view(pa, pb, eye, &self.pair)), false, 0);
        }
        match best {
            Some((_, s, cuts, _)) => {
                self.last = Some((s, cuts));
                Some(s)
            }
            None => {
                if let Some((s, true)) = self.last.take() {
                    self.cut = Some((s.eye, s.target));
                }
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn equal_priority_pushes_go_beneath() {
        // A camera object (priority 10) pushed first; a camera block's timed view (10) pushed later goes beneath it.
        let object = (priority::CAMERA_OBJECT, 0);
        let block = (priority::CAMERA_OBJECT, 1);
        assert!(above(object, block) && !above(block, object));
        assert_eq!(top(&[object, block]), Some(0));
        assert_eq!(top(&[block, object]), Some(1), "the order of the list does not matter, the push order does");
        // A strictly higher priority covers whatever was pushed first; a lower one never does.
        assert_eq!(top(&[(priority::PAIR, 0), (priority::BINOCUCOM, 5)]), Some(1));
        assert_eq!(top(&[object, (priority::MOUNT, 0)]), Some(0));
        // When the covering view leaves, the earliest of the equal ones is on top again.
        assert_eq!(top(&[block, (priority::CAMERA_OBJECT, 2), object]), Some(2));
        assert_eq!(top(&[]), None);
    }

    use super::*;

    #[test]
    fn timed_move_curves() {
        let a = CamState { position: [0.0; 3], angles: [0.0; 3], fov: 1.0 };
        let b = CamState { position: [100.0, 0.0, 0.0], angles: [0.0, 0.0, 1.0], fov: 0.5 };
        let mut m = TimedMove { from: a, to: b, duration: 2.0, curve: 3, time: 0.0 };
        // Smoothstep: half way at half time, slow at the ends.
        assert!((m.weight(1.0) - 0.5).abs() < 1e-6 && m.weight(0.2) < 0.1);
        let (s, done) = m.step(1.0);
        assert!(!done && (s.eye[0] - 50.0).abs() < 1e-3 && (s.fov.unwrap() - 0.75).abs() < 1e-6);
        let (s, done) = m.step(1.0);
        assert!(done && (s.eye[0] - 100.0).abs() < 1e-3);
        let lin = TimedMove { curve: 0, ..m };
        assert!((lin.weight(0.5) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn bad_numbers_do_not_panic_or_hold_the_view() {
        // A timed move with a duration that is not a number ends at once.
        let a = CamState { position: [0.0; 3], angles: [0.0; 3], fov: 1.0 };
        let mut m = TimedMove { from: a, to: a, duration: f32::NAN, curve: 3, time: 0.0 };
        assert!(m.step(1.0 / 60.0).1);
        // The binocucom with a heading that is not a number, or a bad step, does not panic.
        let h = Handle::new(0, 0);
        let mut b = Binocucom::new(h, false, f32::NAN, false);
        for dt in [1.0 / 60.0, f32::NAN, -1.0, f32::INFINITY] {
            b.step(dt, [-1.0, 1.0], [0.0, 1.0], [0.0; 3], 1.0);
        }
        let mut c = Binocucom::new(h, true, 0.0, false);
        let s = c.step(f32::NAN, [1.0, 0.0], [0.0; 2], [0.0; 3], 1.0);
        assert!(s.eye.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn pair_view_widens_as_they_part() {
        let t = PairTunables::default();
        let near = pair_view([0.0; 3], [200.0, 0.0, 0.0], [100.0, -500.0, 0.0], &t);
        let far = pair_view([0.0; 3], [1200.0, 0.0, 0.0], [100.0, -500.0, 0.0], &t);
        assert_eq!(near.target, [100.0, 0.0, 0.0]);
        assert!(near.eye[1] < 0.0, "on the current eye's side");
        assert!(
            far.fov.unwrap() > near.fov.unwrap()
                && length(sub(far.eye, far.target)) > length(sub(near.eye, near.target))
        );
    }

    #[test]
    fn binocucom_limits_zoom_and_eye() {
        let h = Handle::new(0, 0);
        let mut b = Binocucom::new(h, false, 0.0, false);
        let mut s = b.step(0.0, [0.0; 2], [0.0; 2], [0.0; 3], 1.0);
        // An object's focus: 50 behind along the view, no raise; near clip 49.
        assert_eq!((s.eye, s.near), ([-50.0, 0.0, 0.0], Some(49.0)));
        for _ in 0..600 {
            s = b.step(1.0 / 60.0, [-1.0, 1.0], [0.0, 1.0], [0.0; 3], 1.0);
        }
        // Pan within ±1 of the object's heading, tilt within ±1.5; zoomed to 1/7 of the field of view.
        assert!(b.angles[0] <= 1.0 + 1e-4 && b.angles[1] <= 1.5 + 1e-4, "{:?}", b.angles);
        assert!((s.fov.unwrap() - 1.0 / 7.0).abs() < 1e-4);
        // A character focus: 150 above.
        let mut c = Binocucom::new(h, true, 0.0, false);
        let s = c.step(0.0, [0.0; 2], [0.0; 2], [0.0; 3], 1.0);
        assert_eq!(s.eye, [-50.0, 0.0, 150.0]);
        assert!((fov_of_focal_length(17.996) - core::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn visibility_prefers_priority_then_the_latest() {
        let cube = |c: f32| TreeVolume {
            planes: alloc::vec![
                ([1.0, 0.0, 0.0], c),
                ([-1.0, 0.0, 0.0], c),
                ([0.0, 1.0, 0.0], c),
                ([0.0, -1.0, 0.0], c),
                ([0.0, 0.0, 1.0], c),
                ([0.0, 0.0, -1.0], c),
            ],
            nodes: (0..6u16).map(|i| (i, if i < 5 { u32::from(i) + 1 } else { u32::MAX }, u32::MAX)).collect(),
            to_local: Affine::IDENTITY,
        };
        let h = Handle::new(0, 0);
        let vols = alloc::vec![
            VisibilityVolume { object: h, tree: cube(10.0), mask: 1, priority: 0 },
            VisibilityVolume { object: h, tree: cube(5.0), mask: 2, priority: 0 },
            VisibilityVolume { object: h, tree: cube(2.0), mask: 4, priority: 1 },
        ];
        let mut st = Visibility::default();
        assert_eq!(visibility_mask(&vols, [100.0; 3], &mut st), DEFAULT_MASK);
        assert_eq!(visibility_mask(&vols, [8.0, 0.0, 0.0], &mut st), 1);
        // Into the second (same priority): the most recent wins.
        assert_eq!(visibility_mask(&vols, [4.0, 0.0, 0.0], &mut st), 2);
        // Into the third: higher priority.
        assert_eq!(visibility_mask(&vols, [1.0, 0.0, 0.0], &mut st), 4);
    }
}
