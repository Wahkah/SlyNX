//! The Sly 3 crew's clips applied to their poses (specs/sly3/characters.md 12, animation.md; the playback rules are
//! Sly 2's, [`super::sly2_gang`]).
//!
//! - **Which clip** ([`sly3_motion`], characters.md 12.2 and 12.3):
//!   - standing: the idle state machine (Sly 172, Bentley and Murray 451);
//!   - moving: the full-speed gait (+0x1470: Sly 173/174, Bentley and Murray 449/447) at weight 1 in the run states,
//!     the stick-driven gait (Sly 307/308, Bentley and Murray 535/534) at the stick's deflection in the walk states,
//!     Murray's carry gait 528/529; each at Sly 2's footfall-weighted speed, played at the actual speed over it (Sly
//!     832 / 640 = 1.3);
//!   - jump and fall: the clip of the jump's sub-kind (`sly2_gang::sub_kind`, chosen at take-off), a missing clip
//!     falling back to sub-kind 0;
//!   - the combat dodge (Sly 312 left, 313 right); the hook swing's still pose (213 at 0.3333 s; the swing's
//!     sub-state and angle are not modelled by the gang); the paraglide (opening 1727, then the glide blend of 1728
//!     neutral, 1729 left, 1730 right, 1732 forward, 1731 back, weighted by the stick, [`glide_weights`]);
//!   - attacks: Sly's combo runner plays its definition's clip at the runner's time; Murray's punches play the five
//!     punch clips of his lookup table's records 5 to 9 (Square first 457, second 458, Triangle first 468, second
//!     467, the special 244) by punch index, once, as Sly 2's; Bentley's chair melee plays the clip its event names;
//!   - the ledge hang (Sly 217, Murray 706) only when a jump catches a ledge, which the gang does not do; Bentley
//!     has no ledge clip.
//! - **The idle machine** runs as Sly 2's (`sly2_gang::IdleRun`, Sly 2 characters.md 9.2): a random weighted chain
//!   of one-shot clips from its first state, by the transitions allowed with no look target (mask 8, assumed the same
//!   as Sly 2's). When the machine cannot be read, its first state's clip loops.
//! - **Where the clips are**: each character looks its IDs up inside its own group (12.2; the first record with the
//!   ID in stream order, at any depth). A state machine (class 132) has Sly 2's layout after its member list
//!   (classes/animation.md 2).
//! - **Poses** ([`Sly3Poser`], [`step`]): each driven blend is evaluated on the character's transform list
//!   (`XformRig`, the scalar track winning, then mirrors and chains), transform 1's horizontal translation held (the
//!   crew moves the object), and written to `Sly3Level::pose` relative to transform 0's loaded matrix, with a
//!   version per object for caches of what is drawn ([`Sly3Poser::version_of`]).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use formats::geometry::Affine;
use formats::reader::Reader;
use formats_sly3::anim::ClipData;
use formats_sly3::stream::{Body, Record, visit};

use super::sly2_gang::{
    IdleRun, Motion, SWING_STILL_TIME, Track, facing_angle, gait_rate, sub_kind, sub_kind_fallback,
};
use super::sly2_machine::{MachineDef, Transition};
use super::xform::XformRig;
use crate::gang::consts::Gait;
use crate::gang::{Character, GangEvent, Who, bentley, murray, sly};
use crate::math::affine_inverse;
use crate::object::Handle;
use crate::world::World;

/// Sly 3's clip and state-machine classes.
pub const CLIP_CLASS: i16 = 130;
pub const MACHINE_CLASS: i16 = 132;

/// The crew's clips and machines (object IDs, characters.md 12.2 and 12.3), per character [Sly, Murray, Bentley].
pub mod clip {
    /// The idle state machine.
    pub const IDLE: [i32; 3] = [172, 451, 451];
    /// The full-speed gait (+0x1470, slow and fast clips), played at weight 1.
    pub const FULL: [[i32; 2]; 3] = [[173, 174], [449, 447], [449, 447]];
    /// The stick-driven gait (the one `0x00173068` returns; Sly's from his second table).
    pub const STICK: [[i32; 2]; 3] = [[307, 308], [535, 534], [535, 534]];
    /// Murray's carry gait (+0x15A0).
    pub const MURRAY_CARRY: [i32; 2] = [528, 529];
    /// Jump and fall clips by sub-kind 0 to 4 (−1: none).
    pub const JUMP: [[i32; 5]; 3] = [[200, 204, 203, 201, 202], [696, 700, 699, 697, 698], [696, 700, 699, 697, 698]];
    pub const FALL: [[i32; 5]; 3] = [[208, 212, 211, 209, 210], [701, 705, 704, 702, 703], [707, 710, -1, 708, 709]];
    /// Murray's punches (his table's records 5 to 9): Square first, Square second, Triangle first, Triangle second,
    /// the special punch; 0.6667 s each, 244 0.9167 s.
    pub const MURRAY_PUNCHES: [i32; 5] = [457, 458, 468, 467, 244];
    /// Ledge hang (only when a jump catches a ledge; Bentley has none).
    pub const LEDGE_HANG: [i32; 3] = [217, 706, -1];
    /// Sly's combat dodge, left and right.
    pub const DODGE: [i32; 2] = [312, 313];
    /// Sly's hook swing blend (213, 214, 215) and the hang machine.
    pub const SWING: [i32; 3] = [213, 214, 215];
    pub const HANG_MACHINE: i32 = 216;
    /// Paraglide: the opening clip, then the glide blend: neutral, left, right, forward, back.
    pub const GLIDE_OPEN: i32 = 1727;
    pub const GLIDE: [i32; 5] = [1728, 1729, 1730, 1732, 1731];
}

/// The gaits' (root travel, length) per clip in v_apt (characters.md 12.3), per character [Sly, Murray, Bentley].
pub mod gait {
    use crate::gang::consts::Gait;

    /// The full-speed gaits.
    pub const FULL: [Gait; 3] = [
        Gait { slow: [160.0, 1.0], fast: [320.0, 0.5] },
        Gait { slow: [170.0, 1.0 / 3.0], fast: [400.0, 0.5] },
        Gait { slow: [100.0, 1.0], fast: [250.0, 1.0 / 3.0] },
    ];
    /// The stick-driven gaits.
    pub const STICK: [Gait; 3] = [
        Gait { slow: [300.8, 4.0 / 3.0], fast: [340.0, 2.0 / 3.0] },
        Gait { slow: [300.0, 4.0 / 3.0], fast: [255.0, 0.5] },
        Gait { slow: [220.0, 2.0], fast: [255.0, 0.5] },
    ];
    /// Murray's carry gait.
    pub const MURRAY_CARRY: Gait = Gait { slow: [125.0, 1.0], fast: [310.0, 0.6] };
}

/// The glide blend's weights (12.3) for stick deflection `m` and `a` = the stick's angle − the heading: neutral
/// `1 − m`; the rest `m` shared by |m·sin a| (left or right) and |m·cos a| (forward or back).
pub fn glide_weights(m: f32, a: f32) -> [f32; 5] {
    let m = m.clamp(0.0, 1.0);
    let mut w = [1.0 - m, 0.0, 0.0, 0.0, 0.0];
    if m > 0.0 {
        let (s, c) = (m * crate::math::sin(a), m * crate::math::cos(a));
        let total = s.abs() + c.abs();
        if total > 0.0 {
            let q = m / total;
            if s >= 0.0 {
                w[1] = s * q;
            } else {
                w[2] = -s * q;
            }
            if c > 0.0 {
                w[3] = c * q;
            } else {
                w[4] = -c * q;
            }
        }
    }
    w
}

/// The lookup records with flag `0x20` (parts or meshes of the character, not looked up by object ID), per
/// character [Sly, Murray, Bentley], and the one all three share (characters.md 12.3). Which are accessories shown
/// only in some states, and which meshes they name, is open, so nothing hides them yet.
pub const ACCESSORY_RECORDS: [&[i32]; 3] = [
    &[254, 256, 257, 258, 259, 260, 252, 253, 255],
    &[714, 715, 542, 543, 544, 545, 753, 754, 755, 756],
    &[611, 714, 715, 753, 754, 755, 756],
];
/// The accessory record all three carry.
pub const SHARED_ACCESSORY: i32 = 1710;

/// One driven blend for the next step: the target, its members (clip ID in the target's group, time, weight).
#[derive(Clone, Debug, PartialEq)]
pub struct Driven3 {
    pub target: Handle,
    pub clips: Vec<(i32, f32, f32)>,
}

/// The crew's poses.
#[derive(Clone, Debug, Default)]
pub struct Sly3Poser {
    /// Rigs by object slot.
    rigs: BTreeMap<u32, XformRig>,
    /// Clips by (group, ID) (`None`: none with data).
    clips: BTreeMap<(u16, i32), Option<ClipData>>,
    /// A state machine's first state's clip by (group, machine ID).
    idle: BTreeMap<(u16, i32), Option<i32>>,
    /// State machines by (group, machine ID).
    machines: BTreeMap<(u16, i32), Option<MachineDef>>,
    /// Blends for the next step (consumed by it).
    pub driven: Vec<Driven3>,
    /// Object slots posed in the last step.
    pub posed: Vec<u32>,
    /// How many times each object's pose has changed.
    pub version: BTreeMap<u32, u32>,
}

/// The records of a group, at every depth.
fn find_in_group(world: &World, group: u16, mut f: impl FnMut(&Record) -> bool) -> Option<&Record> {
    let body = world.sly3.as_deref()?.bodies.get(group as usize)?;
    let mut out = None;
    visit(&body.objects, &mut |r| {
        if out.is_none() && f(r) {
            out = Some(r);
        }
    });
    out
}

/// A state machine record (class 132): Sly 2's layout after the member list (classes/animation.md 2): the states,
/// then the transitions (from, to, flags, a word, a probability, a clip); a transition whose clip is not a member is
/// dropped, as in Sly 2.
pub fn machine_def(data: &[u8], r: &Record) -> Option<MachineDef> {
    let members: Vec<i32> = r.nested().iter().map(|c| c.id).collect();
    let last = r.nested().last()?;
    let mut rd = Reader::at(data, (last.body_offset + last.body_size) as usize);
    let n = rd.u8().ok()? as usize;
    let states = (0..n).map(|_| Some((rd.i32().ok()?, rd.i32().ok()?))).collect::<Option<Vec<_>>>()?;
    let m = rd.u8().ok()? as usize;
    let mut transitions = Vec::with_capacity(m);
    for _ in 0..m {
        let t = Transition {
            from: rd.i32().ok()?,
            to: rd.i32().ok()?,
            flags: rd.i32().ok()?,
            extra: rd.i32().ok()?,
            probability: rd.f32().ok()?,
            object: rd.i32().ok()?,
        };
        if t.object == -1 || members.contains(&t.object) {
            transitions.push(t);
        }
    }
    Some(MachineDef { members, states, transitions })
}

/// A state machine's states (state, object) after its member list (classes/animation.md 2).
pub fn machine_states(data: &[u8], r: &Record) -> Option<Vec<(i32, i32)>> {
    let last = r.nested().last()?;
    let mut rd = Reader::at(data, (last.body_offset + last.body_size) as usize);
    let n = rd.u8().ok()? as usize;
    (0..n).map(|_| Some((rd.i32().ok()?, rd.i32().ok()?))).collect()
}

impl Sly3Poser {
    /// The clip with ID `id` in group `group`, read once.
    pub fn clip(&mut self, world: &World, group: u16, id: i32) -> Option<&ClipData> {
        self.clips
            .entry((group, id))
            .or_insert_with(|| {
                let r = find_in_group(world, group, |r| r.id == id && r.class == CLIP_CLASS)?;
                match &r.body {
                    Body::Clip(c) => c.data.clone(),
                    _ => None,
                }
            })
            .as_ref()
    }

    /// A state machine of group `group`, read once.
    pub fn machine(&mut self, world: &World, group: u16, machine: i32) -> Option<&MachineDef> {
        self.machines
            .entry((group, machine))
            .or_insert_with(|| {
                let r = find_in_group(world, group, |r| r.id == machine && r.class == MACHINE_CLASS)?;
                machine_def(&world.level.bytes, r)
            })
            .as_ref()
    }

    /// The clip a state machine's first state plays (falling back to its first member clip with data).
    pub fn machine_clip(&mut self, world: &World, group: u16, machine: i32) -> Option<i32> {
        *self.idle.entry((group, machine)).or_insert_with(|| {
            let r = find_in_group(world, group, |r| r.id == machine && r.class == MACHINE_CLASS)?;
            let members: Vec<i32> = r
                .nested()
                .iter()
                .filter(|c| matches!(&c.body, Body::Clip(k) if k.data.is_some()))
                .map(|c| c.id)
                .collect();
            let first =
                machine_states(&world.level.bytes, r).and_then(|s| s.iter().map(|s| s.1).find(|o| members.contains(o)));
            first.or_else(|| members.first().copied())
        })
    }

    /// The pose version of object `slot` (0 until it is first posed).
    pub fn version_of(&self, slot: u32) -> u32 {
        self.version.get(&slot).copied().unwrap_or(0)
    }
}

/// One step: evaluates the driven blends onto the crew's transform lists and writes the poses.
pub fn step(world: &mut World) {
    let Some(mut p) = world.sly3.as_deref_mut().map(|l| core::mem::take(&mut l.poser)) else { return };
    p.posed.clear();
    let driven = core::mem::take(&mut p.driven);
    let w: &World = world;
    let mut done = BTreeSet::new();
    for d in &driven {
        let (Some(slot), Some(o)) = (w.slot(d.target), w.sly3_object(d.target).copied()) else { continue };
        for &(id, _, _) in &d.clips {
            p.clip(w, o.group, id);
        }
        if let alloc::collections::btree_map::Entry::Vacant(e) = p.rigs.entry(slot) {
            let Some(Body::Solid(s)) = w.sly3_record(d.target).map(|r| &r.body) else { continue };
            e.insert(XformRig::from_list(&s.transforms));
        }
        let Sly3Poser { rigs, clips, .. } = &mut p;
        let Some(rig) = rigs.get_mut(&slot) else { continue };
        rig.reset();
        let members = d.clips.iter().filter_map(|&(id, t, wt)| {
            let c = clips.get(&(o.group, id))?.as_ref()?;
            Some((XformRig::sly3_clip_channels(c, o.id), t, wt))
        });
        rig.blend_channels(members, true);
        if rig.nodes.len() > 1 {
            let b = rig.nodes[1].bind.t;
            rig.local[1].t[0] = b[0];
            rig.local[1].t[1] = b[1];
        }
        rig.update(&Affine::IDENTITY);
        done.insert(slot);
    }
    let Some(level) = world.sly3.as_deref_mut() else { return };
    for slot in done {
        let Some(rig) = p.rigs.get(&slot) else { continue };
        let Some(o) = level.object(slot) else { continue };
        let (a, b) = (o.pose.0 as usize, o.pose.1 as usize);
        if b - a != rig.world.len() || b > level.pose.len() {
            continue;
        }
        let base = rig.nodes.first().and_then(|n| affine_inverse(&n.bind)).unwrap_or(Affine::IDENTITY);
        let mut changed = false;
        for (dst, m) in level.pose[a..b].iter_mut().zip(&rig.world) {
            let m = base.mul(m);
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
}

/// The motion of a Sly 3 core character in its current state (module notes); `idle` is the clip its idle machine
/// plays, when found.
pub fn sly3_motion(c: &Character, track: &Track, idle: Option<i32>) -> Motion {
    let k = c.who.index();
    let u = c.last_deflection.clamp(0.0, 1.0);
    let g = |clips: [i32; 2], gt: &Gait, u: f32| Motion::Gait { clips, u, rate: gait_rate(c, Some(gt), u) };
    let full = || g(clip::FULL[k], &gait::FULL[k], 1.0);
    let stick = |u: f32| g(clip::STICK[k], &gait::STICK[k], u);
    let still = idle.map_or(Motion::At(clip::STICK[k][0], 0.0), Motion::Loop);
    let air = || {
        let i = usize::from(track.sub_kind.min(4));
        if c.gliding {
            Motion::Blend5(clip::GLIDE, glide_weights(u, facing_angle(c)))
        } else if track.jumped {
            Motion::Once(clip::JUMP[k][i])
        } else {
            Motion::Once(clip::FALL[k][i])
        }
    };
    let moving = u > 0.0;
    match c.who {
        Who::Sly => match c.state {
            sly::RUN => full(),
            sly::SNEAK | sly::SNEAK_RUN if moving => stick(u),
            sly::AIR | sly::AIR_SQUARE | sly::AIR_TRIANGLE => air(),
            sly::DODGE => Motion::Once(clip::DODGE[usize::from(c.dodge_side < 0.0)]),
            sly::HOOK => Motion::At(clip::SWING[0], SWING_STILL_TIME),
            sly::COMBO if c.combo_runner.clip > 0 => Motion::At(c.combo_runner.clip, c.combo_runner.time),
            _ if c.grounded && moving => stick(u),
            _ => still,
        },
        Who::Murray => match c.state {
            murray::RUN => full(),
            murray::WALK if moving => stick(u),
            murray::CARRY if moving => g(clip::MURRAY_CARRY, &gait::MURRAY_CARRY, u),
            murray::AIR => air(),
            murray::PUNCH => Motion::Once(clip::MURRAY_PUNCHES[usize::from(c.combo.index).min(4)]),
            _ => still,
        },
        Who::Bentley => match c.state {
            bentley::MOVE | bentley::TIMED => full(),
            bentley::WALK if moving => stick(u),
            bentley::AIR => air(),
            _ => still,
        },
    }
}

/// Starts what a Sly 3 character's event plays over its state.
pub fn sly3_event(c: &Character, e: &GangEvent, track: &mut Track) {
    match *e {
        GangEvent::Jumped | GangEvent::DoubleJump => {
            track.jumped = true;
            track.sub_kind = sub_kind(c);
            track.restart();
        }
        // Paraglide (jump kind 19): the opening clip, then the glide.
        GangEvent::SpecialJump(19) => track.overlay(Motion::Once(clip::GLIDE_OPEN), c.state),
        GangEvent::SpecialJump(_) => {
            track.jumped = true;
            track.sub_kind = sub_kind(c);
            track.restart();
        }
        GangEvent::Landed => {
            track.jumped = false;
            track.sub_kind = 0;
        }
        GangEvent::ChairMelee { clip, .. } => track.overlay(Motion::Once(clip), c.state),
        _ => {}
    }
}

/// The crew's clip playbacks: one [`Track`] per core character.
#[derive(Clone, Debug, Default)]
pub struct CrewClips {
    /// Per character [Sly, Murray, Bentley].
    pub tracks: [Track; 3],
    /// The idle machines' runs while standing, per character.
    pub idle: [Option<IdleRun>; 3],
    rng: u32,
    members: Vec<(i32, f32, f32)>,
}

impl CrewClips {
    /// One frame (after the crew's update): the events' overlays, every core member's track advanced by `dt`,
    /// then the poses written ([`step`]).
    pub fn drive(
        &mut self,
        world: &mut World,
        members: &[Character],
        handles: &[Option<Handle>; 3],
        events: &[(Who, GangEvent)],
        dt: f32,
    ) {
        let Some(mut poser) = world.sly3.as_deref_mut().map(|l| core::mem::take(&mut l.poser)) else { return };
        for (who, e) in events {
            if let Some(c) = members.iter().find(|c| c.who == *who) {
                sly3_event(c, e, &mut self.tracks[who.index()]);
            }
        }
        let w: &World = world;
        for c in members {
            let Some(h) = handles[c.who.index()] else { continue };
            let Some(group) = w.sly3_object(h).map(|o| o.group) else { continue };
            let k = c.who.index();
            let idle = poser.machine_clip(w, group, clip::IDLE[k]);
            let track = &mut self.tracks[k];
            let mut m = sly3_motion(c, track, idle);
            let still = idle.map_or(Motion::At(clip::STICK[k][0], 0.0), Motion::Loop);
            // Standing: the idle machine's chain of clips.
            let def = if m == still { poser.machine(w, group, clip::IDLE[k]).cloned() } else { None };
            match def {
                Some(def) => {
                    let run = self.idle[k].get_or_insert_with(|| IdleRun::new(def.first_state().unwrap_or(0)));
                    let rng = &mut self.rng;
                    let mut len = |id: i32| poser.clip(w, group, id).map(ClipData::duration);
                    if let Some((clip, t)) = run.advance(&def, dt, || crate::anim::sly2_gang::random(rng), &mut len) {
                        m = Motion::At(clip, t);
                    }
                }
                None => self.idle[k] = None,
            }
            let mut len = |id: i32| poser.clip(w, group, id).map(ClipData::duration);
            track.advance(m, c.state, dt, &mut len, &mut self.members);
            if self.members.is_empty()
                && let Some(f) = sub_kind_fallback(m, &[clip::FALL[k], clip::JUMP[k]])
            {
                track.advance(f, c.state, dt, &mut len, &mut self.members);
            }
            if self.members.is_empty() {
                track.advance(still, c.state, dt, &mut len, &mut self.members);
            }
            if !self.members.is_empty() {
                poser.driven.push(Driven3 { target: h, clips: self.members.clone() });
            }
        }
        if let Some(l) = world.sly3.as_deref_mut() {
            l.poser = poser;
        }
        step(world);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crew_states_pick_their_clips() {
        let t = Track::default();
        let mut s = Character::new(Who::Sly, [0.0; 3], 0.0);
        assert_eq!(sly3_motion(&s, &t, Some(8184)), Motion::Loop(8184));
        assert_eq!(sly3_motion(&s, &t, None), Motion::At(307, 0.0));
        s.state = sly::RUN;
        s.velocity = [832.0, 0.0, 0.0];
        let Motion::Gait { clips, u, rate } = sly3_motion(&s, &t, None) else { panic!() };
        assert_eq!((clips, u), ([173, 174], 1.0));
        assert!((rate - 1.3).abs() < 1e-3);
        s.state = sly::SNEAK_RUN;
        s.last_deflection = 0.5;
        assert!(matches!(sly3_motion(&s, &t, None), Motion::Gait { clips: [307, 308], u: 0.5, .. }));
        s.state = sly::DODGE;
        s.dodge_side = -1.0;
        assert_eq!(sly3_motion(&s, &t, None), Motion::Once(313));
        s.state = sly::HOOK;
        assert_eq!(sly3_motion(&s, &t, None), Motion::At(213, SWING_STILL_TIME));
        // Jumps by sub-kind: the stick to the left of the facing (+y with yaw 0) is 2.
        s.state = sly::AIR;
        s.last_stick = [0.0, 1.0, 0.0];
        let mut j = Track::default();
        sly3_event(&s, &GangEvent::Jumped, &mut j);
        assert_eq!(j.sub_kind, 2);
        assert_eq!(sly3_motion(&s, &j, None), Motion::Once(203));
        sly3_event(&s, &GangEvent::Landed, &mut j);
        assert_eq!(sly3_motion(&s, &j, None), Motion::Once(208));
        let mut m = Character::new(Who::Murray, [0.0; 3], 0.0);
        m.state = murray::CARRY;
        m.last_deflection = 1.0;
        assert!(matches!(sly3_motion(&m, &t, None), Motion::Gait { clips: [528, 529], .. }));
        m.state = murray::PUNCH;
        m.combo.index = 2;
        assert_eq!(sly3_motion(&m, &t, None), Motion::Once(468));
        let mut b = Character::new(Who::Bentley, [0.0; 3], 0.0);
        b.state = bentley::AIR;
        assert_eq!(sly3_motion(&b, &t, None), Motion::Once(707));
        b.state = bentley::MOVE;
        assert!(matches!(sly3_motion(&b, &t, None), Motion::Gait { clips: [449, 447], u: 1.0, .. }));
    }

    #[test]
    fn sub_kinds_follow_the_stick_and_glide_weights_sum_to_one() {
        use core::f32::consts::PI;
        let mut c = Character::new(Who::Sly, [0.0; 3], 0.5);
        assert_eq!(sub_kind(&c), 0);
        c.last_deflection = 1.0;
        for (angle, want) in [(0.0, 3), (1.5, 2), (-1.5, 1), (3.0, 4), (-3.0, 4), (1.0, 3), (2.1, 4)] {
            let a: f32 = 0.5 + angle;
            c.last_stick = [crate::math::cos(a), crate::math::sin(a), 0.0];
            assert_eq!(sub_kind(&c), want, "angle {angle}");
        }
        for (m, a) in [(0.0, 0.0), (1.0, 0.0), (0.5, PI / 2.0), (0.7, -2.5), (1.0, PI / 4.0)] {
            let w = glide_weights(m, a);
            assert!((w.iter().sum::<f32>() - 1.0).abs() < 1e-5, "{w:?}");
            assert!((w[0] - (1.0 - m)).abs() < 1e-6);
        }
        assert!((glide_weights(1.0, 0.0)[3] - 1.0).abs() < 1e-6);
        assert!((glide_weights(1.0, PI / 2.0)[1] - 1.0).abs() < 1e-5);
        let w = glide_weights(1.0, -PI / 4.0);
        assert!((w[2] - 0.5).abs() < 1e-5 && (w[3] - 0.5).abs() < 1e-5, "{w:?}");
    }
}
