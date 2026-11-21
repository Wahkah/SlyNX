//! Sly 2 state-machine and clip playbacks on world objects (animation.md 8, options-by-class.md 2: options 73, 74,
//! 106, 368; dialogue-and-voice.md 4: the speaker's state machines; missions.md 4: task goals).
//!
//! - **Playbacks are objects.** Applying a state machine (class 111, option 368) or a clip (class 109, option 106)
//!   to a target makes a playback, a run-time object of class 136 (machine) or 134 (clip) that scripts keep and
//!   set options on [D for the result types `0x2088` and `0x2086`: object results of classes 136 and 134]. Option
//!   74 on a target finds the playback of a machine by the machine's object ID, 73 that of a clip; a machine with no
//!   playback on the target yet gets one at its first state [working choice: the scripts set a goal on what 74
//!   returns at once, so the game's machines exist from the start].
//! - **Machine playback options** (class 136, stream IDs): 372 the goal (setting it starts the walk; the scripts'
//!   478), 375 the current state (the safe house compares it with 7, 8, 9), 376 the goal read back, 371 the target,
//!   377 the speed, 378 the clip playing; method 379 jumps to a state, walking the route at once; method 380
//!   removes the playback. Clip playback options (class 134): 116 the clip, 117 the target, 118 the time, 119 the
//!   speed; method 123 seeks (mode 0 set, 1 add, 2 from the end; time; speed); method 124 removes it. Other options
//!   are stored like any option. (Script ID 236 is 123 on clip playbacks and 379 on machine playbacks.)
//! - **Events** (scripts.md 6): a playback runs its clip's or machine's module. It gets event 2 when applied and 3
//!   when removed; a machine playback gets event 8 (from, to) on every state change, its first state (from −1)
//!   included.
//! - **Runs** ([`crate::anim::sly2_machine`]) advance every frame with the game clock. A clip playback follows the
//!   flags of option 106 (animation.md 7): 8 loops, else 16 plays back and forth, else 32 holds at the end; with
//!   none of them it ends at the clip's end and is removed. Flags 1 (ease), 2 (fire the start events) and 4 (snap
//!   to the start pose) concern the pose and the clip events, which this module does not apply.
//! - **Goals from elsewhere**: the job system's state-machine goals (`Jobs::machine_goals`), and voice lines' timed
//!   records: targets 0, 1, 2, 4, 5 set goals on the speaker's machines 1548, 1549, 1550, 1551 and 1552 (5 is the
//!   mouth), target 3 how far the speaker's eyes are closed ([`Sly2Anim::eyes`]).
//! - **Applying the clips to the targets' transforms** is the animation system's (`engine::anim`): this module
//!   reports what plays where ([`Sly2Anim::playing`]); see `docs/engine.md` ("Sly 2 state machines").

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use formats::geometry::Affine;

use crate::anim::sly2_machine::{MachineDef, MachineRun, RunEvent};
use crate::object::{Handle, OptionValue};
use crate::world::{NONE, World};

/// Classes.
pub const CLIP_CLASS: i16 = 109;
/// The other clip class (options-by-class.md: option 106 on classes 109 and 110).
pub const CLIP_CLASS_2: i16 = 110;
pub const MACHINE_CLASS: i16 = 111;
pub const CLIP_PLAYBACK_CLASS: i16 = 134;
pub const MACHINE_PLAYBACK_CLASS: i16 = 136;

/// Option IDs (stream).
pub mod opt {
    /// Target: find a clip's playback by the clip's ID.
    pub const FIND_CLIP: u16 = 73;
    /// Target: find a machine's playback by the machine's ID.
    pub const FIND_MACHINE: u16 = 74;
    /// Clip: apply to (target, start time, speed, flags).
    pub const APPLY_CLIP: u16 = 106;
    /// Machine: apply to (target, goal, flags).
    pub const APPLY_MACHINE: u16 = 368;
    /// Clip playback: the clip, target, time, speed; seek; remove.
    pub const CLIP: u16 = 116;
    pub const CLIP_TARGET: u16 = 117;
    pub const CLIP_TIME: u16 = 118;
    pub const CLIP_SPEED: u16 = 119;
    pub const CLIP_SEEK: u16 = 123;
    pub const CLIP_REMOVE: u16 = 124;
    /// Machine playback: target, goal, state, goal read back, speed, clip playing; jump to a state; remove.
    pub const TARGET: u16 = 371;
    pub const GOAL: u16 = 372;
    pub const STATE: u16 = 375;
    pub const GOAL_READ: u16 = 376;
    pub const SPEED: u16 = 377;
    pub const PLAYING: u16 = 378;
    pub const JUMP: u16 = 379;
    pub const REMOVE: u16 = 380;
}

/// The speaker's machines for a voice line's targets 0 to 5 (executable 0x2C3C40; −1: target 3, the eyes).
pub const SPEAKER_MACHINES: [i32; 6] = [1548, 1549, 1550, -1, 1551, 1552];
/// Target 3: the eyes.
pub const TARGET_EYES: i16 = 3;
/// Flags of option 106 (animation.md 7).
pub mod flag {
    pub const EASE: i32 = 1;
    pub const START_EVENTS: i32 = 2;
    pub const SNAP: i32 = 4;
    pub const LOOP: i32 = 8;
    pub const PING_PONG: i32 = 16;
    pub const HOLD: i32 = 32;
}

/// Script events of playbacks.
pub mod event {
    /// Applied.
    pub const APPLIED: u32 = 2;
    /// Removed.
    pub const REMOVED: u32 = 3;
    /// A machine playback changed state: (from, to).
    pub const STATE: u32 = 8;
}

/// How a clip playback ends (option 106's flags).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrap {
    /// Ends at the clip's end and is removed.
    Once,
    Loop,
    PingPong,
    Hold,
}

impl Wrap {
    /// From option 106's flags: 8 wins over 16, 16 over 32.
    pub fn of(flags: i32) -> Wrap {
        if flags & flag::LOOP != 0 {
            Wrap::Loop
        } else if flags & flag::PING_PONG != 0 {
            Wrap::PingPong
        } else if flags & flag::HOLD != 0 {
            Wrap::Hold
        } else {
            Wrap::Once
        }
    }
}

/// A machine playback.
#[derive(Clone, Debug, PartialEq)]
pub struct MachinePlayback {
    /// The playback object.
    pub object: Handle,
    /// The machine object and the target.
    pub machine: Handle,
    pub target: Handle,
    /// The run.
    pub run: MachineRun,
}

/// A clip playback.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipPlayback {
    pub object: Handle,
    pub clip: Handle,
    pub target: Handle,
    pub time: f32,
    pub speed: f32,
    pub length: f32,
    pub wrap: Wrap,
}

/// Something playing on a target: (target, clip object, local time in seconds).
pub type Playing = (Handle, Handle, f32);

/// The world's playbacks.
#[derive(Clone, Debug, Default)]
pub struct Sly2Anim {
    /// Machine records read so far, by machine slot.
    defs: BTreeMap<u32, MachineDef>,
    /// Clip durations, by clip slot.
    lengths: BTreeMap<u32, f32>,
    /// Machine playbacks, by playback slot.
    pub machines: BTreeMap<u32, MachinePlayback>,
    /// Clip playbacks, by playback slot.
    pub clips: BTreeMap<u32, ClipPlayback>,
    /// How far each speaker's eyes are closed (0 to 1), by speaker slot.
    pub eyes: BTreeMap<u32, f32>,
    /// State entries, oldest first: (playback, from, to), for the curious (tests, debug views).
    pub entered: Vec<(Handle, i32, i32)>,
    /// Playbacks removed: destroyed on the next step, after their event 3 has run.
    doomed: Vec<Handle>,
    rng: u32,
}

/// A run-time playback object running `module` (its clip's or machine's).
fn new_object(world: &mut World, class: i16, module: i16) -> Option<Handle> {
    let s = world.create(class, -1, u32::MAX, module, Affine::IDENTITY, NONE, u64::MAX, 0);
    world.handle(s)
}

fn next_random(rng: &mut u32) -> f32 {
    *rng = rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*rng >> 8) as f32 / (1u32 << 24) as f32
}

/// Raises a machine playback's state-change event.
fn entered(world: &mut World, log: &mut Vec<(Handle, i32, i32)>, playback: Handle, from: i32, to: i32) {
    world.raise(playback, event::STATE, alloc::vec![script::Value::Int(from), script::Value::Int(to)]);
    log.push((playback, from, to));
}

impl Sly2Anim {
    fn def(&mut self, world: &World, machine: Handle) -> Option<&MachineDef> {
        let slot = world.slot(machine)?;
        if let alloc::collections::btree_map::Entry::Vacant(e) = self.defs.entry(slot) {
            e.insert(MachineDef::read(&world.level.bytes, world.sly2_record(machine)?)?);
        }
        self.defs.get(&slot)
    }

    /// The duration of clip object `clip`.
    pub fn length(&mut self, world: &World, clip: Handle) -> Option<f32> {
        length_in(&mut self.lengths, world, clip)
    }

    /// A member of a machine by object ID: among its children, else searched from it.
    fn member(world: &World, machine: Handle, id: i32) -> Option<Handle> {
        world
            .children(machine)
            .find(|&c| world.sly2_id(c) == Some(id))
            .or_else(|| world.find_sly2_id(id, Some(machine)))
    }

    /// The playback of `machine` on `target`, made at the machine's first state when there is none.
    pub fn machine_playback(&mut self, world: &mut World, machine: Handle, target: Handle) -> Option<Handle> {
        if let Some(p) = self.machines.values().find(|p| p.machine == machine && p.target == target) {
            return Some(p.object);
        }
        let run = MachineRun::new(self.def(world, machine)?);
        let module = world.module(machine);
        let object = new_object(world, MACHINE_PLAYBACK_CLASS, module)?;
        let first = run.state;
        self.machines.insert(object.index(), MachinePlayback { object, machine, target, run });
        world.raise(object, event::APPLIED, Vec::new());
        entered(world, &mut self.entered, object, -1, first);
        Some(object)
    }

    /// Finds machine `id` for `target` (below the target first, then the whole world) and its playback.
    pub fn find_machine(&mut self, world: &mut World, target: Handle, id: i32) -> Option<Handle> {
        let machine = world
            .slots_with_sly2_id(id)
            .into_iter()
            .filter_map(|s| world.handle(s))
            .filter(|&h| world.class(h) == Some(MACHINE_CLASS))
            .min_by_key(|&h| !below(world, h, target))?;
        self.machine_playback(world, machine, target)
    }

    /// Sets a goal on machine `id` of `target`.
    pub fn set_goal(&mut self, world: &mut World, target: Handle, id: i32, goal: i32, instant: bool) -> bool {
        let Some(p) = self.find_machine(world, target, id) else { return false };
        if instant { self.jump(world, p, goal) } else { self.goal(p, goal) }
    }

    /// Sets a machine playback's goal (the run walks there over the next frames).
    pub fn goal(&mut self, playback: Handle, goal: i32) -> bool {
        let Some(p) = self.machines.get_mut(&playback.index()) else { return false };
        p.run.goal = Some(goal);
        true
    }

    /// Jumps a machine playback to a state (method 379): the route is walked at once, every state entered raising
    /// event 8.
    pub fn jump(&mut self, world: &mut World, playback: Handle, goal: i32) -> bool {
        let Some(mut p) = self.machines.remove(&playback.index()) else { return false };
        let mut ev = Vec::new();
        if let Some(def) = self.defs.get(&p.machine.index()) {
            let rng = &mut self.rng;
            p.run.walk(def, goal, &mut || next_random(rng), &mut ev);
        }
        let obj = p.object;
        self.machines.insert(playback.index(), p);
        for e in ev {
            if let RunEvent::Entered { from, to } = e {
                entered(world, &mut self.entered, obj, from, to);
            }
        }
        true
    }

    /// Applies a clip to a target: a new clip playback.
    pub fn apply_clip(
        &mut self,
        world: &mut World,
        clip: Handle,
        target: Handle,
        time: f32,
        speed: f32,
        flags: i32,
    ) -> Option<Handle> {
        let length = self.length(world, clip).unwrap_or(0.0);
        let module = world.module(clip);
        let object = new_object(world, CLIP_PLAYBACK_CLASS, module)?;
        let wrap = Wrap::of(flags);
        self.clips.insert(object.index(), ClipPlayback { object, clip, target, time, speed, length, wrap });
        world.raise(object, event::APPLIED, Vec::new());
        Some(object)
    }

    /// Seeks a clip playback (method 123): mode 0 sets the time, 1 adds to it, 2 sets it from the clip's end; the
    /// speed is set too.
    pub fn seek(&mut self, playback: Handle, mode: i32, time: f32, speed: Option<f32>) -> bool {
        let Some(c) = self.clips.get_mut(&playback.index()) else { return false };
        c.time = match mode {
            1 => c.time + time,
            2 => c.length - time,
            _ => time,
        }
        .clamp(0.0, c.length.max(0.0));
        if let Some(s) = speed {
            c.speed = s;
        }
        true
    }

    /// The playback of clip `id` on `target`, if one runs.
    pub fn find_clip(&self, world: &World, target: Handle, id: i32) -> Option<Handle> {
        self.clips.values().find(|c| c.target == target && world.sly2_id(c.clip) == Some(id)).map(|c| c.object)
    }

    /// Removes a playback (either kind): event 3 now, the object destroyed on the next step.
    pub fn remove(&mut self, world: &mut World, playback: Handle) {
        if self.machines.remove(&playback.index()).is_some() || self.clips.remove(&playback.index()).is_some() {
            world.raise(playback, event::REMOVED, Vec::new());
            self.doomed.push(playback);
        }
    }

    /// Advances every playback by `dt` game seconds.
    pub fn step(&mut self, world: &mut World, dt: f32) {
        for h in core::mem::take(&mut self.doomed) {
            world.destroy(h);
        }
        let keys: Vec<u32> = self.machines.keys().copied().collect();
        for k in keys {
            let Some(mut p) = self.machines.remove(&k) else { continue };
            if !world.contains(p.target) || !world.contains(p.machine) {
                world.destroy(p.object);
                continue;
            }
            let machine = p.machine;
            let Some(def) = self.defs.get(&machine.index()) else {
                self.machines.insert(k, p);
                continue;
            };
            let w: &World = world;
            let cache = &mut self.lengths;
            let mut lengths = |id: i32| -> Option<f32> { length_in(cache, w, Self::member(w, machine, id)?) };
            let mut ev = Vec::new();
            let rng = &mut self.rng;
            let mut random = || next_random(rng);
            p.run.update(def, dt, &mut lengths, &mut random, &mut ev);
            let obj = p.object;
            self.machines.insert(k, p);
            for e in ev {
                if let RunEvent::Entered { from, to } = e {
                    entered(world, &mut self.entered, obj, from, to);
                }
            }
        }
        if self.entered.len() > 256 {
            let n = self.entered.len() - 256;
            self.entered.drain(..n);
        }
        let mut done = Vec::new();
        for c in self.clips.values_mut() {
            c.time += dt * c.speed;
            let (end, start) = (c.time >= c.length, c.time < 0.0);
            match c.wrap {
                Wrap::Once if end || start => done.push(c.object),
                Wrap::Loop if c.length > 0.0 => c.time = wrap(c.time, c.length),
                Wrap::PingPong if end || start => {
                    // Turn around at the end it passed.
                    c.time = if end { 2.0 * c.length - c.time } else { -c.time }.clamp(0.0, c.length);
                    c.speed = -c.speed;
                }
                Wrap::Hold => c.time = c.time.clamp(0.0, c.length),
                _ => {}
            }
        }
        for o in done {
            self.remove(world, o);
        }
    }

    /// What plays on which target now (machine steps and states, clip playbacks).
    pub fn playing(&self, world: &World) -> Vec<Playing> {
        let mut out = Vec::new();
        for p in self.machines.values() {
            let Some(def) = self.defs.get(&p.machine.index()) else { continue };
            if let Some((id, t)) = p.run.playing(def)
                && let Some(c) = Self::member(world, p.machine, id)
            {
                let l = self.lengths.get(&c.index()).copied().unwrap_or(0.0);
                let t = if p.run.step.is_none() && l > 0.0 { wrap(t, l) } else { t };
                out.push((p.target, c, t));
            }
        }
        out.extend(self.clips.values().map(|c| (c.target, c.clip, c.time)));
        out
    }

    /// A playback option the store answers (module documentation), or `None`.
    pub fn get_option(&self, world: &World, playback: Handle, option: u16) -> Option<OptionValue> {
        let obj = |h: Handle| OptionValue::Int(world.sly2_id(h).unwrap_or(-1));
        if let Some(p) = self.machines.get(&playback.index()) {
            return match option {
                opt::STATE => Some(OptionValue::Int(p.run.state)),
                opt::GOAL | opt::GOAL_READ => Some(OptionValue::Int(p.run.goal.unwrap_or(p.run.state))),
                opt::SPEED => Some(OptionValue::Float(p.run.speed)),
                opt::TARGET => Some(obj(p.target)),
                _ => None,
            };
        }
        let c = self.clips.get(&playback.index())?;
        match option {
            opt::CLIP => Some(obj(c.clip)),
            opt::CLIP_TARGET => Some(obj(c.target)),
            opt::CLIP_TIME => Some(OptionValue::Float(c.time)),
            opt::CLIP_SPEED => Some(OptionValue::Float(c.speed)),
            _ => None,
        }
    }

    /// Sets a playback option the store answers; returns whether it did.
    pub fn set_option(&mut self, playback: Handle, option: u16, v: &OptionValue) -> bool {
        if let Some(p) = self.machines.get_mut(&playback.index()) {
            match option {
                opt::GOAL => {
                    let g = v.as_i32().unwrap_or(-1);
                    return self.goal(playback, g);
                }
                opt::SPEED => p.run.speed = v.as_f32().unwrap_or(1.0),
                _ => return false,
            }
            return true;
        }
        let Some(c) = self.clips.get_mut(&playback.index()) else { return false };
        match option {
            opt::CLIP_TIME => c.time = v.as_f32().unwrap_or(0.0),
            opt::CLIP_SPEED => c.speed = v.as_f32().unwrap_or(1.0),
            _ => return false,
        }
        true
    }

    /// Applies a voice line's timed record on `speaker`: a goal on one of its machines, or its eyes.
    pub fn cue(&mut self, world: &mut World, speaker: Handle, target: i16, goal: Option<i32>, eyes: Option<f32>) {
        if target == TARGET_EYES {
            if let (Some(e), Some(s)) = (eyes, world.slot(speaker)) {
                self.eyes.insert(s, e.clamp(0.0, 1.0));
            }
            return;
        }
        let Some(&machine) = SPEAKER_MACHINES.get(target.max(0) as usize).filter(|_| target >= 0) else { return };
        if let Some(g) = goal {
            self.set_goal(world, speaker, machine, g, false);
        }
    }
}

/// `t` wrapped into [0, `len`) (no_std has no `rem_euclid` for floats).
fn wrap(t: f32, len: f32) -> f32 {
    let r = t - len * ((t / len) as i64) as f32;
    if r < 0.0 { r + len } else { r }
}

/// A clip's duration, cached by slot.
fn length_in(cache: &mut BTreeMap<u32, f32>, world: &World, clip: Handle) -> Option<f32> {
    let slot = world.slot(clip)?;
    if let Some(&l) = cache.get(&slot) {
        return Some(l);
    }
    let table = &world.sly2.as_deref()?.table;
    let l = formats_sly2::anim::Clip::of_record(&world.level.bytes, world.sly2_record(clip)?, table).ok()?.duration;
    // Review hook: a negative or non-finite duration is read as 0, so the clamps on playback times cannot panic.
    let l = crate::anim::sly2_pose::sane_length(l);
    cache.insert(slot, l);
    Some(l)
}

/// Whether `h` is `root` or below it.
fn below(world: &World, mut h: Handle, root: Handle) -> bool {
    loop {
        if h == root {
            return true;
        }
        match world.parent(h) {
            Some(p) => h = p,
            None => return false,
        }
    }
}

/// Runs `f` on the world's playbacks.
pub fn with<R>(world: &mut World, f: impl FnOnce(&mut Sly2Anim, &mut World) -> R) -> Option<R> {
    let mut a = core::mem::take(&mut world.sly2.as_deref_mut()?.anim);
    let r = f(&mut a, world);
    if let Some(l) = world.sly2.as_deref_mut() {
        l.anim = a;
    }
    Some(r)
}

/// One frame (from `Game::frame`): the job system's goals, then every playback.
pub fn step(world: &mut World, dt: f32) {
    let goals = world
        .sly2
        .as_deref_mut()
        .and_then(|l| l.jobs.as_deref_mut())
        .map(|j| core::mem::take(&mut j.machine_goals))
        .unwrap_or_default();
    with(world, |a, w| {
        for (object, machine, goal, instant) in goals {
            // The goal's object is the target (−1: the machine's own owner).
            let m = w
                .slots_with_sly2_id(machine)
                .into_iter()
                .filter_map(|s| w.handle(s))
                .find(|&h| w.class(h) == Some(MACHINE_CLASS));
            let target = if object >= 0 { w.find_sly2_id(object, None) } else { m.and_then(|m| w.parent(m)) };
            if let (Some(t), Some(m)) = (target, m)
                && let Some(p) = a.machine_playback(w, m, t)
            {
                if instant {
                    a.jump(w, p, goal);
                } else {
                    a.goal(p, goal);
                }
            }
        }
        if dt > 0.0 {
            a.step(w, dt);
        }
    });
}

/// The speaker of a voice line: the world's first object of the speaker's class (dialogue-and-voice.md 4).
pub fn speaker(world: &World, class: i16) -> Option<Handle> {
    world.slots_of_class(class).iter().filter_map(|&s| world.handle(s)).next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_106_flags() {
        assert_eq!(Wrap::of(0), Wrap::Once);
        assert_eq!(Wrap::of(flag::EASE | flag::SNAP), Wrap::Once);
        assert_eq!(Wrap::of(flag::LOOP | flag::PING_PONG), Wrap::Loop, "8 wins over 16");
        assert_eq!(Wrap::of(flag::PING_PONG | flag::HOLD), Wrap::PingPong);
        assert_eq!(Wrap::of(flag::HOLD), Wrap::Hold);
    }
}
