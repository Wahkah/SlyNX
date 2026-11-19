//! Sly 2 clips applied to the objects' poses (specs/sly2/animation.md 2, 4 to 7): every frame, after the state
//! machines and clip playbacks advance (`game::sly2_anim`), each clip playing on a target ([`Sly2Anim::playing`])
//! is evaluated on the transform lists of the objects its channels name, through their [`XformRig`]s (channels by
//! transform kind, then mirror pairs and chains), and the result replaces those objects' poses
//! ([`crate::world::sly2::Sly2Level::pose`], relative to the object's transform). Whatever reads the poses (the
//! characters' skinning palettes, `Sly2Level::pose_of`) then sees the animated pose. An object whose clips stop keeps
//! its last pose.
//!
//! **Events** (animation.md 2): a playback appearing fires its clip's apply list, its time crossing a timed event's
//! frame (in either direction, frames of 1/60 s) fires that event, and a playback disappearing fires the retract
//! list. Every fired event is kept in [`Sly2Poser::fired`] for one frame, with its type, for the systems that act on
//! them (type 4 and the others; type 5's jump is the playback's, `anim::sly2::jump_of`).
//!
//! [`Sly2Anim::playing`]: crate::game::sly2_anim::Sly2Anim::playing

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use formats::geometry::Affine;
use formats_sly2::anim::Clip;
use formats_sly2::stream::Body;

use super::sly2::Fired;
use super::xform::XformRig;
use crate::object::Handle;
use crate::world::World;

/// One fired clip event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoseEvent {
    /// The playback's target.
    pub target: Handle,
    /// The clip object.
    pub clip: Handle,
    /// Which event.
    pub fired: Fired,
    /// Its type.
    pub kind: i8,
}

/// The clips' evaluation state.
#[derive(Clone, Debug, Default)]
pub struct Sly2Poser {
    /// Rigs by object slot, built from the object's transform list on first use.
    rigs: BTreeMap<u32, XformRig>,
    /// Parsed clips by clip slot (`None`: unreadable).
    clips: BTreeMap<u32, Option<Clip>>,
    /// Each playing (target, clip) pair's local time last frame.
    last: BTreeMap<(u32, u32), (Handle, Handle, f32)>,
    /// Events fired this frame.
    pub fired: Vec<PoseEvent>,
    /// Object slots posed this frame.
    pub posed: Vec<u32>,
    /// How many times each object's pose has changed (for caches of what is drawn).
    pub version: BTreeMap<u32, u32>,
}

impl Sly2Poser {
    fn clip<'a>(clips: &'a mut BTreeMap<u32, Option<Clip>>, world: &World, clip: Handle) -> Option<&'a Clip> {
        let slot = world.slot(clip)?;
        clips
            .entry(slot)
            .or_insert_with(|| {
                let table = &world.sly2.as_deref()?.table;
                Clip::of_record(&world.level.bytes, world.sly2_record(clip)?, table).ok()
            })
            .as_ref()
    }

    fn rig<'a>(rigs: &'a mut BTreeMap<u32, XformRig>, world: &World, h: Handle) -> Option<&'a mut XformRig> {
        let slot = world.slot(h)?;
        if let alloc::collections::btree_map::Entry::Vacant(e) = rigs.entry(slot) {
            let r = world.sly2_record(h)?;
            let Body::Solid(s) = &r.body else { return None };
            e.insert(XformRig::from_list(&s.transforms));
        }
        rigs.get_mut(&slot)
    }

    /// The pose version of object `slot` (0 until it is first animated).
    pub fn version_of(&self, slot: u32) -> u32 {
        self.version.get(&slot).copied().unwrap_or(0)
    }
}

/// Fires the timed events crossed between `from` and `to` (seconds), in either direction.
fn crossed(clip: &Clip, from: f32, to: f32, out: &mut Vec<(Fired, i8)>) {
    let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
    if lo == hi {
        return;
    }
    for (i, e) in clip.timed.iter().enumerate() {
        let Some(f) = e.frame else { continue };
        let at = f32::from(f) / 60.0;
        if at > lo && at <= hi {
            out.push((Fired::Timed(i as u16), e.kind));
        }
    }
}

/// One frame: evaluates every clip playing on the Sly 2 world's objects and writes the poses.
pub fn step(world: &mut World) {
    let Some(level) = world.sly2.as_deref_mut() else { return };
    let mut p = core::mem::take(&mut level.poser);
    let playing = world.sly2.as_deref().map(|l| l.anim.playing(world)).unwrap_or_default();
    p.fired.clear();
    p.posed.clear();
    // Events: apply lists for new playbacks, timed events crossed, retract lists for ended ones.
    let mut now: BTreeMap<(u32, u32), (Handle, Handle, f32)> = BTreeMap::new();
    let mut events = Vec::new();
    for &(target, clip, t) in &playing {
        let (Some(ts), Some(cs)) = (world.slot(target), world.slot(clip)) else { continue };
        now.insert((ts, cs), (target, clip, t));
        let Some(c) = Sly2Poser::clip(&mut p.clips, world, clip) else { continue };
        events.clear();
        match p.last.get(&(ts, cs)) {
            None => events.extend(c.on_apply.iter().enumerate().map(|(i, e)| (Fired::Apply(i as u16), e.kind))),
            Some(&(_, _, last)) => crossed(c, last, t, &mut events),
        }
        p.fired.extend(events.iter().map(|&(fired, kind)| PoseEvent { target, clip, fired, kind }));
    }
    for (key, &(target, clip, _)) in &p.last {
        if now.contains_key(key) {
            continue;
        }
        if let Some(c) = Sly2Poser::clip(&mut p.clips, world, clip) {
            p.fired.extend(c.on_retract.iter().enumerate().map(|(i, e)| PoseEvent {
                target,
                clip,
                fired: Fired::Retract(i as u16),
                kind: e.kind,
            }));
        }
    }
    p.last = now;
    // Poses: every object a playing clip's channels name, reset once, then each clip applied in order.
    let mut reset: BTreeSet<u32> = BTreeSet::new();
    let Sly2Poser { rigs, clips, .. } = &mut p;
    for &(target, clip, t) in &playing {
        let Some(c) = Sly2Poser::clip(clips, world, clip) else { continue };
        let mut ids: Vec<i32> = c.channels.iter().map(|ch| ch.target).collect();
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            let object =
                if world.sly2_id(target) == Some(id) { Some(target) } else { world.find_sly2_id(id, Some(target)) };
            let Some(object) = object else { continue };
            let Some(slot) = world.slot(object) else { continue };
            let Some(rig) = Sly2Poser::rig(rigs, world, object) else { continue };
            if reset.insert(slot) {
                rig.reset();
            }
            rig.apply_clip(c, id, t);
        }
    }
    let Some(level) = world.sly2.as_deref_mut() else { return };
    for slot in reset {
        let Some(rig) = p.rigs.get_mut(&slot) else { continue };
        rig.update(&Affine::IDENTITY);
        let Some(o) = level.object(slot) else { continue };
        let (a, b) = (o.pose.0 as usize, o.pose.1 as usize);
        if b - a != rig.world.len() || b > level.pose.len() {
            continue;
        }
        if level.pose[a..b] != rig.world[..] {
            level.pose[a..b].copy_from_slice(&rig.world);
            *p.version.entry(slot).or_default() += 1;
        }
        p.posed.push(slot);
    }
    level.poser = p;
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::anim::Event;

    #[test]
    fn timed_events_fire_when_crossed_either_way() {
        let ev = |frame: i16, kind: i8| Event { kind, frame: Some(frame), offset: 0, payload: Vec::new() };
        let clip = Clip {
            duration: 1.0,
            channels: Vec::new(),
            extra: Vec::new(),
            on_apply: Vec::new(),
            timed: alloc::vec![ev(6, 4), ev(30, 11)],
            on_retract: Vec::new(),
            options: Default::default(),
            keys: 0,
        };
        let mut out = Vec::new();
        crossed(&clip, 0.0, 0.2, &mut out);
        assert_eq!(out, [(Fired::Timed(0), 4)]);
        out.clear();
        crossed(&clip, 0.6, 0.4, &mut out);
        assert_eq!(out, [(Fired::Timed(1), 11)]);
        out.clear();
        crossed(&clip, 0.5, 0.5, &mut out);
        assert!(out.is_empty());
    }
}
