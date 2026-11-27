//! Clips played on Sly 3 objects for their add and remove events (specs/sly3/guards.md 4b; the design is Sly 2's
//! `engine::behavior::sly2_commands::clip_object_events`).
//!
//! - **Events**: kind 7 adds an object and kind 8 removes one; the payload is (object ID, fade time). The object is
//!   looked up near the clip's owner: in the owner's copy or below it, never another copy's object of that ID, so a
//!   pooled guard removes its own instance. The clip's apply events fire at its start, its timed events at their frame (60 a second) and
//!   its retract events at its end.
//! - **Fades**: a removal with a fade above 0 keeps the object in the world while its alpha ([`Clips::alpha`]) eases
//!   from 1 to 0 over the fade, then removes it; 0 removes it at once. The Sly 3 drawing has no per-object alpha
//!   yet, so the fade shows as the wait only.
//! - **The knock-out clip** (guards.md 4b: a knocked-out guard leaves the world at the end of its knock-out clip):
//!   stand-in, a working choice [I] (the spec does not say which clip plays for which hit): the first clip, in
//!   stream order of the guard's group, with a kind-8 event on the guard's own ID. v_gauntlet: `ecto_swarmer`
//!   (56250) plays 56542 (removed at its end, no fade), the guards of group 40 (19900) play 24874 (removed at
//!   0.33 s with a 0.5 s fade).
//! - The clip also drives the owner's pose (`anim::sly3_pose`), at weight 1.

use alloc::vec::Vec;

use engine::anim::sly3_pose::Driven3;
use engine::object::Handle;
use engine::world::World;
use formats::geometry::Affine;
use formats_sly3::anim::{ClipData, FRAMES_PER_SECOND};
use formats_sly3::stream::Body;

/// The add event.
pub const EVENT_ADD: i8 = 7;
/// The remove event.
pub const EVENT_REMOVE: i8 = 8;
/// The clip class.
const CLIP_CLASS: i16 = engine::anim::sly3_pose::CLIP_CLASS;

/// One add or remove event: (time, add, object ID, fade).
type ObjectEvent = (f32, bool, i32, f32);

/// A clip playing on an object.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipPlay {
    /// The object it plays on.
    pub owner: Handle,
    /// The clip's ID (in the owner's group).
    pub clip: i32,
    /// Seconds played.
    pub time: f32,
    /// The clip's length.
    pub length: f32,
    events: Vec<ObjectEvent>,
    next: usize,
}

/// An object fading out before it is removed.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fade {
    object: Handle,
    left: f32,
    total: f32,
}

/// An animation segment playing on its own (`0x16`; scripts.md 7.3): its position tracks move the objects they key.
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentPlay {
    /// The segment (class 130).
    pub segment: Handle,
    /// Seconds played.
    pub time: f32,
    /// Its length.
    pub length: f32,
    /// It starts over at its end.
    pub looping: bool,
}

/// The clips playing and the fades running.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clips {
    /// Playing clips.
    pub plays: Vec<ClipPlay>,
    /// Playing segments.
    pub segments: Vec<SegmentPlay>,
    fades: Vec<Fade>,
}

/// The object with ID `id` near clip owner `owner`: in the owner's copy, or below the owner. None otherwise: an
/// event never reaches another copy's object of the same ID (v_gauntlet's timed buttons each hold a use point 50450;
/// one button's clip must not remove another's).
pub fn near(world: &World, id: i32, owner: Handle) -> Option<Handle> {
    let copy = world.sly3_object(owner)?.copy;
    let below = |h: Handle| core::iter::successors(world.parent(h), |&q| world.parent(q)).any(|q| q == owner);
    world
        .sly3_id_slots(id)
        .filter_map(|k| world.handle(k))
        .find(|&h| world.sly3_object(h).is_some_and(|o| o.copy == copy) || below(h))
}

/// The data of a clip record.
fn clip_data(world: &World, h: Handle) -> Option<&ClipData> {
    match &world.sly3_record(h)?.body {
        Body::Clip(c) => c.data.as_ref(),
        _ => None,
    }
}

fn object_events(d: &ClipData) -> Vec<ObjectEvent> {
    let decode = |e: &formats::anim::Event, at: f32| {
        let p = e.payload.get(..8)?;
        let id = i32::from_le_bytes([p[0], p[1], p[2], p[3]]);
        let fade = f32::from_le_bytes([p[4], p[5], p[6], p[7]]);
        Some((at, e.kind == EVENT_ADD, id, if fade.is_finite() { fade.max(0.0) } else { 0.0 }))
    };
    let ours = |e: &&formats::anim::Event| e.kind == EVENT_ADD || e.kind == EVENT_REMOVE;
    let length = d.duration();
    let mut out: Vec<ObjectEvent> = d.on_apply.iter().filter(ours).filter_map(|e| decode(e, 0.0)).collect();
    out.extend(
        d.timed
            .iter()
            .filter(ours)
            .filter_map(|e| decode(e, (e.frame.unwrap_or(0).max(0) as f32 / FRAMES_PER_SECOND).min(length))),
    );
    out.extend(d.on_retract.iter().filter(ours).filter_map(|e| decode(e, length)));
    // Stable: events at the same time keep their order in the clip.
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// The clip records of `group`, in stream order, at every depth.
fn group_clips(world: &World, group: u16) -> Vec<(i32, &ClipData)> {
    let Some(body) = world.sly3.as_deref().and_then(|l| l.bodies.get(group as usize)) else { return Vec::new() };
    let mut out = Vec::new();
    formats_sly3::stream::visit(&body.objects, &mut |r| {
        if r.class == CLIP_CLASS
            && let Body::Clip(c) = &r.body
            && let Some(d) = &c.data
        {
            out.push((r.id, d));
        }
    });
    out
}

/// The knock-out clip of guard `guard` (module notes: stand-in): the first clip of its group removing its own ID.
pub fn knockout_clip(world: &World, guard: Handle) -> Option<i32> {
    let id = world.sly3_id(guard)?;
    let group = world.sly3_object(guard)?.group;
    group_clips(world, group)
        .into_iter()
        .find(|(_, d)| object_events(d).iter().any(|e| !e.1 && e.2 == id))
        .map(|(clip, _)| clip)
}

impl Clips {
    /// Plays clip `clip` (an ID in `owner`'s group) on `owner` from its start; false when there is no such clip with
    /// data. A clip already playing on `owner` is replaced.
    pub fn play(&mut self, world: &World, owner: Handle, clip: i32) -> bool {
        let Some(group) = world.sly3_object(owner).map(|o| o.group) else { return false };
        let Some((_, d)) = group_clips(world, group).into_iter().find(|(id, _)| *id == clip) else { return false };
        let play =
            ClipPlay { owner, clip, time: 0.0, length: d.duration().max(0.0), events: object_events(d), next: 0 };
        self.plays.retain(|p| p.owner != owner);
        self.plays.push(play);
        true
    }

    /// Clip `clip`'s add and remove events (an ID in `owner`'s group) applied at once, with no fades and no pose: a
    /// state-machine transition snapped (`machines`). False when there is no such clip with data.
    pub fn apply_now(&mut self, world: &mut World, owner: Handle, clip: i32) -> bool {
        let Some(group) = world.sly3_object(owner).map(|o| o.group) else { return false };
        let Some(events) =
            group_clips(world, group).into_iter().find(|(id, _)| *id == clip).map(|(_, d)| object_events(d))
        else {
            return false;
        };
        for (_, add, id, _) in events {
            if let Some(h) = near(world, id, owner) {
                self.fades.retain(|f| f.object != h);
                world.set_in_world(h, add);
            }
        }
        true
    }

    /// Plays animation segment `segment` (a class-130 clip record) on its own from its start (`0x16`, missions.md
    /// 4.1a): each channel on transform 0 places the object it keys (by ID, its translation) along its track; the
    /// object's y axis faces the way it moves (inference, marked [I] in the spec: the canal boat, turned on its
    /// owner, then fires along the track). A segment already playing starts over.
    pub fn play_segment(&mut self, world: &World, segment: Handle, looping: bool) {
        let Some(d) = clip_data(world, segment) else { return };
        let length = d.duration().max(0.0);
        self.segments.retain(|p| p.segment != segment);
        self.segments.push(SegmentPlay { segment, time: 0.0, length, looping });
    }

    /// Where segment-keyed object `target` (by ID) will be `ahead` seconds from now on a playing segment, looping
    /// with it.
    pub fn segment_point(&self, world: &World, target: i32, ahead: f32) -> Option<[f32; 3]> {
        self.segments.iter().find_map(|p| {
            let d = clip_data(world, p.segment)?;
            let c = d.channels.iter().find(|c| c.target == target && c.transform == 0)?;
            let mut t = p.time + ahead;
            if p.length > 0.0 {
                t = if p.looping { t % p.length } else { t.min(p.length) };
            }
            engine::anim::curve::eval_vector(c.vector.as_ref()?, t)
        })
    }

    /// Whether a clip is playing on `owner`.
    pub fn playing(&self, owner: Handle) -> bool {
        self.plays.iter().any(|p| p.owner == owner)
    }

    /// The alpha of an object fading out (1 when it is not fading).
    pub fn alpha(&self, object: Handle) -> f32 {
        self.fades.iter().find(|f| f.object == object).map_or(1.0, |f| f.left / f.total)
    }

    /// One frame of `dt` seconds: the clips advance and fire their events, the fades run, and each playing clip
    /// drives its owner's pose for the next pose step.
    pub fn step(&mut self, world: &mut World, dt: f32) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let mut fired: Vec<(Handle, bool, i32, f32)> = Vec::new();
        for p in &mut self.plays {
            p.time = (p.time + dt).min(p.length);
            while let Some(&(at, add, id, fade)) = p.events.get(p.next) {
                if at > p.time {
                    break;
                }
                fired.push((p.owner, add, id, fade));
                p.next += 1;
            }
        }
        for (owner, add, id, fade) in fired {
            let Some(h) = near(world, id, owner) else { continue };
            self.fades.retain(|f| f.object != h);
            if add {
                world.set_in_world(h, true);
            } else if fade > 0.0 {
                self.fades.push(Fade { object: h, left: fade, total: fade });
            } else {
                world.set_in_world(h, false);
            }
        }
        for f in &mut self.fades {
            f.left -= dt;
            if f.left <= 0.0 {
                world.set_in_world(f.object, false);
            }
        }
        self.fades.retain(|f| f.left > 0.0);
        // Segments: each keyed object at its track's position, facing along it.
        for p in &mut self.segments {
            p.time += dt;
            if p.time > p.length {
                p.time = if p.looping && p.length > 0.0 { p.time % p.length } else { p.length };
            }
        }
        for p in &self.segments {
            let Some(d) = clip_data(world, p.segment) else { continue };
            let moves: Vec<(i32, [f32; 3], [f32; 3])> = d
                .channels
                .iter()
                .filter(|c| c.transform == 0)
                .filter_map(|c| {
                    let v = c.vector.as_ref()?;
                    let at = engine::anim::curve::eval_vector(v, p.time)?;
                    let ahead = engine::anim::curve::eval_vector(v, (p.time + 0.1).min(p.length))?;
                    Some((c.target, at, ahead))
                })
                .collect();
            for (id, at, ahead) in moves {
                let Some(h) = world.find_sly3_id(id, None) else { continue };
                let (dx, dy) = (ahead[0] - at[0], ahead[1] - at[1]);
                let mut m = world.world_transform(h).unwrap_or(Affine::IDENTITY);
                if dx * dx + dy * dy > 1e-6 {
                    // The object's y axis along the track: v_canal's boat sits on its owner turned so that its own
                    // forward (its x axis, where its missiles fly) is the owner's y.
                    let yaw = engine::math::atan2(dy, dx) - core::f32::consts::FRAC_PI_2;
                    let (s, c) = (engine::math::sin(yaw), engine::math::cos(yaw));
                    m.cols = [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]];
                }
                m.t = at;
                world.set_world_transform(h, m);
            }
        }
        self.segments.retain(|p| p.looping || p.time < p.length);
        if let Some(l) = world.sly3.as_deref_mut() {
            for p in &self.plays {
                l.poser.driven.push(Driven3 { target: p.owner, clips: alloc::vec![(p.clip, p.time, 1.0)] });
            }
        }
        self.plays.retain(|p| p.time < p.length || p.next < p.events.len());
    }
}
