//! Clips played on Sly 3 objects for their add and remove events (specs/sly3/guards.md 4b; the design is Sly 2's
//! `engine::behavior::sly2_commands::clip_object_events`).
//!
//! - **Events**: kind 7 adds an object and kind 8 removes one; the payload is (object ID, fade time). The object is
//!   looked up near the clip's owner (`World::find_sly3_id` with the owner as context), so a pooled guard removes
//!   its own instance. The clip's apply events fire at its start, its timed events at their frame (60 a second) and
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

/// The clips playing and the fades running.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clips {
    /// Playing clips.
    pub plays: Vec<ClipPlay>,
    fades: Vec<Fade>,
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
            let Some(h) = world.find_sly3_id(id, Some(owner)) else { continue };
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
        if let Some(l) = world.sly3.as_deref_mut() {
            for p in &self.plays {
                l.poser.driven.push(Driven3 { target: p.owner, clips: alloc::vec![(p.clip, p.time, 1.0)] });
            }
        }
        self.plays.retain(|p| p.time < p.length || p.next < p.events.len());
    }
}
