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
//! **Driven playbacks** ([`Sly2Poser::driven`]): systems outside the scripts (the gang's clip driver,
//! [`super::sly2_gang`]) hand in, each frame before the step, a blend of clips per target ([`Driven`]). They are
//! evaluated first, as weighted sums of the members' channels at their own times (`XformRig::blend_channels`), so a
//! script's playback on the same object writes over them. Driven playbacks fire no events, and can hold transform
//! 1's horizontal translation at its loaded value (root motion moves the object instead, characters.md 1.1).
//!
//! **Poses** are written relative to transform 0's loaded matrix, as the loader keeps them (`Sly2Level::pose`):
//! `pose_i = B_0⁻¹ · W_i`, with `B_0` transform 0's loaded local matrix and `W_i` the posed one (transform 0 hangs
//! from the rig's root frame).
//!
//! **Lens** (camera.md 2.1): a clip's extra-curve records of kind 0 whose object is a camera object (class 44) carry
//! its focal length: it is evaluated at the playback's time and kept per camera ([`Sly2Poser::fov_of`] gives the
//! field of view `2·atan(17.996 / f)`), the last value staying when the clip stops. On the disc all 151 extra-curve
//! records that name a camera are of kind 0, with values from 15 to 220 [D].
//!
//! [`Sly2Anim::playing`]: crate::game::sly2_anim::Sly2Anim::playing

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use formats::geometry::Affine;
use formats_sly2::anim::Clip;
use formats_sly2::stream::Body;

use super::curve::eval_scalar;
use super::sly2::Fired;
use super::xform::XformRig;
use crate::math::affine_inverse;
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

/// A blend of clips driven on one target this frame (module notes).
#[derive(Clone, Debug, PartialEq)]
pub struct Driven {
    /// The target object.
    pub target: Handle,
    /// Members: (clip object, local time in seconds, weight).
    pub clips: Vec<(Handle, f32, f32)>,
    /// Hold transform 1's horizontal translation at its loaded value.
    pub lock_root: bool,
}

/// The field of view a lens curve may give (camera.md 2.1).
pub const FOV_RANGE: (f32, f32) = (0.01, 3.0);

/// The camera objects' class (camera.md 7.2).
pub const CAMERA_CLASS: i16 = 44;

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
    /// Object-model hook: the loaded local transforms of objects whose transform 0 clips move (missions.md 4.1).
    pub bases: BTreeMap<u32, Affine>,
    /// How many times each object's pose has changed (for caches of what is drawn).
    pub version: BTreeMap<u32, u32>,
    /// Driven playbacks for the next step (consumed by it).
    pub driven: Vec<Driven>,
    /// Focal lengths of camera objects by slot, from their clips' lens curves.
    pub lens: BTreeMap<u32, f32>,
    /// Character slots whose paraglider is out (characters.md 9.3: shown only in the glide), set by the gang's
    /// clip driver; the skinning hides the glider meshes of the others.
    pub gliding: BTreeSet<u32>,
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

    /// Sly 2 hook (sequence starts): the clip event a fired event names, from the clips already read.
    pub fn event_of(&self, world: &World, e: &PoseEvent) -> Option<&formats::anim::Event> {
        let c = self.clips.get(&world.slot(e.clip)?)?.as_ref()?;
        e.fired.get(c)
    }

    /// The length of clip object `clip` in seconds (`None`: not a readable clip), read once.
    pub fn clip_length(&mut self, world: &World, clip: Handle) -> Option<f32> {
        Sly2Poser::clip(&mut self.clips, world, clip).map(|c| sane_length(c.duration))
    }

    /// The field of view a clip last gave camera object `slot` (camera.md 2.1), if any.
    pub fn fov_of(&self, slot: u32) -> Option<f32> {
        let f = *self.lens.get(&slot)?;
        (f > 0.0).then(|| crate::camera::views::fov_of_focal_length(f).clamp(FOV_RANGE.0, FOV_RANGE.1))
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

/// A clip length as playback uses it: a negative or non-finite duration (bad data) is 0.
pub fn sane_length(l: f32) -> f32 {
    if l.is_finite() && l > 0.0 { l } else { 0.0 }
}

/// A clip already read.
fn parsed<'a>(clips: &'a BTreeMap<u32, Option<Clip>>, world: &World, h: Handle) -> Option<&'a Clip> {
    clips.get(&world.slot(h)?)?.as_ref()
}

/// Records the focal length a clip's lens curves give the camera objects they name, at time `t`.
fn lens_of(world: &World, target: Handle, clip: &Clip, t: f32, lens: &mut BTreeMap<u32, f32>) {
    for e in clip.extra.iter().filter(|e| e.a == 0) {
        let Some(cam) = world.find_sly2_id(e.b, Some(target)) else { continue };
        if world.class(cam) != Some(CAMERA_CLASS) {
            continue;
        }
        if let (Some(slot), Some(f)) = (world.slot(cam), eval_scalar(&e.curve, t)) {
            lens.insert(slot, f);
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
    // Poses: every object a driven blend's or a playing clip's channels name, reset once; the driven blends first,
    // then each script clip in order.
    let mut reset: BTreeSet<u32> = BTreeSet::new();
    let driven = core::mem::take(&mut p.driven);
    for d in &driven {
        for &(clip, _, _) in &d.clips {
            Sly2Poser::clip(&mut p.clips, world, clip);
        }
    }
    let Sly2Poser { rigs, clips, lens, .. } = &mut p;
    for d in &driven {
        let mut ids: Vec<i32> = Vec::new();
        for &(clip, _, _) in &d.clips {
            if let Some(c) = parsed(clips, world, clip) {
                ids.extend(c.channels.iter().map(|ch| ch.target));
            }
        }
        ids.sort_unstable();
        ids.dedup();
        let own = world.sly2_id(d.target);
        for id in ids {
            let object = if own == Some(id) { Some(d.target) } else { world.find_sly2_id(id, Some(d.target)) };
            let Some(object) = object else { continue };
            let Some(slot) = world.slot(object) else { continue };
            let Some(rig) = Sly2Poser::rig(rigs, world, object) else { continue };
            if reset.insert(slot) {
                rig.reset();
            }
            let members = d
                .clips
                .iter()
                .filter_map(|&(clip, t, w)| parsed(clips, world, clip).map(|c| (XformRig::clip_channels(c, id), t, w)));
            rig.blend_channels(members, false);
            if d.lock_root && object == d.target && rig.nodes.len() > 1 {
                let b = rig.nodes[1].bind.t;
                rig.local[1].t[0] = b[0];
                rig.local[1].t[1] = b[1];
            }
        }
        for &(clip, t, _) in &d.clips {
            if let Some(c) = parsed(clips, world, clip) {
                lens_of(world, d.target, c, t, lens);
            }
        }
    }
    // Object-model hook (missions.md 4.1): objects whose transform 0 a script clip drives are moved as a whole.
    let mut moved: BTreeSet<u32> = BTreeSet::new();
    for &(target, clip, t) in &playing {
        let Some(c) = Sly2Poser::clip(clips, world, clip) else { continue };
        lens_of(world, target, c, t, lens);
        for ch in c.channels.iter().filter(|ch| ch.transform == 0) {
            let object = if world.sly2_id(target) == Some(ch.target) {
                Some(target)
            } else {
                world.find_sly2_id(ch.target, Some(target))
            };
            if let Some(s) = object.and_then(|o| world.slot(o)) {
                moved.insert(s);
            }
        }
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
    // The loaded local transform of every moved object, kept once.
    for &s in &moved {
        if !p.bases.contains_key(&s)
            && let Some(l) = world.handle(s).and_then(|h| world.local(h))
        {
            p.bases.insert(s, l);
        }
    }
    let mut moves: Vec<(u32, Affine)> = Vec::new();
    let Some(level) = world.sly2.as_deref_mut() else { return };
    for slot in reset {
        let Some(rig) = p.rigs.get_mut(&slot) else { continue };
        rig.update(&Affine::IDENTITY);
        let Some(o) = level.object(slot) else { continue };
        let (a, b) = (o.pose.0 as usize, o.pose.1 as usize);
        if b - a != rig.world.len() || b > level.pose.len() {
            continue;
        }
        // Relative to transform 0's loaded matrix, as the loader keeps the list. A moved object takes transform 0's
        // new matrix as its own (its loaded local transform times the change), carrying what is placed under it, and
        // the list is kept relative to the new transform 0.
        let bind0 = rig.nodes.first().and_then(|n| affine_inverse(&n.bind)).unwrap_or(Affine::IDENTITY);
        let base = if moved.contains(&slot)
            && let (Some(w0), Some(load)) = (rig.world.first(), p.bases.get(&slot))
        {
            moves.push((slot, load.mul(&bind0.mul(w0))));
            affine_inverse(w0).unwrap_or(Affine::IDENTITY)
        } else {
            bind0
        };
        let mut changed = false;
        for (dst, w) in level.pose[a..b].iter_mut().zip(&rig.world) {
            let m = base.mul(w);
            if *dst != m {
                *dst = m;
                changed = true;
            }
        }
        if changed {
            *p.version.entry(slot).or_default() += 1;
        }
        p.posed.push(slot);
    }
    level.poser = p;
    // The moved objects keep their last place when their clips end (missions.md 4.1: the van's carrier).
    for (s, local) in moves {
        if let Some(h) = world.handle(s) {
            world.set_local(h, local);
        }
    }
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
