//! Guard command lists (specs/sly3/guards.md 4a): command `0x62` sends a guard a list, a class-134 object whose
//! typed options each append one command, run in order. Sly 3 hook: the guard AI job itself (kind 200) is not
//! modelled; the session runs the commands this module knows directly on the guard.
//!
//! | Option | Command | Here |
//! |---:|---|---|
//! | 735 | follow a path (path, forwards, …) | walked point to point ([`SPEED`], [`ARRIVE`]: Sly 2's behaviour, working values) |
//! | 743 | play an animation and wait for its end | waits [`CLIP_TIME`] (no Sly 3 guard clips yet; working value) |
//! | 746 | set a numbered alarm on a script (script, alarm) | sets the alarm for the command scripts |
//! | 740 | use an object (object, …) | walked to, then waits [`CLIP_TIME`] for the use |
//! | 729 to 734, 736 to 742, 744, 745 | context, ranges, look at, message, wait, go to, clip, label, jump, use, lock, unlock, unknown | skipped, counted |
//!
//! The guard object is the guard's placement when it is one (its copy follows), else the object itself.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use engine::math::sqrt;
use engine::object::{Arg, Handle};
use engine::world::World;

/// A guard command list's class.
pub const LIST_CLASS: i16 = 134;
/// Commands.
pub mod cmd {
    pub const PATH: u16 = 735;
    pub const ANIMATE: u16 = 743;
    pub const ALARM: u16 = 746;
    /// Use an object: go to it, use it, hold the list until the use ends (Sly 2's kind 13).
    pub const USE: u16 = 740;
}
/// Working values (module documentation): walking speed, units per second.
pub const SPEED: f32 = 400.0;
/// See [`SPEED`]: a path point is reached within this distance.
pub const ARRIVE: f32 = 25.0;
/// See [`SPEED`]: how long an animation command waits, seconds.
pub const CLIP_TIME: f32 = 1.0;

fn int(a: &Arg) -> Option<i32> {
    engine::behavior::sly3::interactables::arg_i32(a)
}

/// A guard running a command list.
#[derive(Clone, Debug, PartialEq)]
pub struct GuardRun {
    /// The moved object.
    pub guard: Handle,
    /// The list's object ID.
    pub list: i32,
    /// The commands: (option, arguments).
    commands: Vec<(u16, Vec<Arg>)>,
    /// The next command.
    pub pc: usize,
    /// The command in progress: the points left to walk, or the time left waiting.
    walk: Vec<[f32; 3]>,
    wait: f32,
    /// Commands skipped, by option.
    pub skipped: BTreeMap<u16, usize>,
}

impl GuardRun {
    /// Guard `guard` (an object ID) given list `list` (an object ID): None when either is missing.
    pub fn new(world: &World, guard: i32, list: i32) -> Option<GuardRun> {
        let g = world.find_sly3_id(guard, None)?;
        let l = world.find_sly3_id(list, None).filter(|&h| world.class(h) == Some(LIST_CLASS))?;
        let slot = world.slot(l)?;
        let level = world.sly3.as_deref()?;
        let commands = level.actions.iter().filter(|a| a.0 == slot).map(|a| (a.1, a.2.to_vec())).collect();
        Some(GuardRun { guard: g, list, commands, pc: 0, walk: Vec::new(), wait: 0.0, skipped: BTreeMap::new() })
    }

    /// Whether the list has run to its end.
    pub fn done(&self) -> bool {
        self.pc >= self.commands.len() && self.walk.is_empty() && self.wait <= 0.0
    }

    /// The path's points in world space, in the order walked.
    fn path(world: &World, id: i32, forwards: bool) -> Vec<[f32; 3]> {
        let Some(h) = world.find_sly3_id(id, None) else { return Vec::new() };
        let (Some(r), Some(t)) = (world.sly3_record(h), world.world_transform(h)) else { return Vec::new() };
        let mut p: Vec<[f32; 3]> = formats_sly3::stream::path_points(&world.level.bytes, r.body_offset)
            .unwrap_or_default()
            .into_iter()
            .map(|q| t.transform_point(q))
            .collect();
        if !forwards {
            p.reverse();
        }
        p
    }

    /// One frame. `alarms` are the command scripts' alarms.
    pub fn update(&mut self, dt: f32, world: &mut World, alarms: &mut alloc::collections::BTreeSet<i32>) {
        // A knocked-out guard (the object, or its placement's copy) stops where it lies (guards.md 7: Sly 2's rule).
        let out =
            |h: Handle| matches!(world.behavior(h), Some(engine::behavior::Behavior::Sly2Guard(g)) if g.knocked_out());
        if out(self.guard) || world.children(self.guard).any(out) {
            return;
        }
        let mut budget = dt;
        loop {
            // The walk first, then the wait (a use walks to its object, then waits for the use).
            if let Some(&to) = self.walk.first() {
                let Some(at) = world.position(self.guard) else { return };
                let d = [to[0] - at[0], to[1] - at[1], to[2] - at[2]];
                let len = sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
                let step = SPEED * budget;
                if len <= ARRIVE || step >= len {
                    world.set_position(self.guard, to);
                    self.walk.remove(0);
                    budget -= if SPEED > 0.0 { len / SPEED } else { 0.0 };
                    if budget <= 0.0 {
                        return;
                    }
                    continue;
                }
                let k = step / len;
                world.set_position(self.guard, [at[0] + d[0] * k, at[1] + d[1] * k, at[2] + d[2] * k]);
                return;
            }
            if self.wait > 0.0 {
                let t = self.wait.min(budget);
                self.wait -= t;
                budget -= t;
                if self.wait > 0.0 {
                    return;
                }
            }
            let Some((id, args)) = self.commands.get(self.pc).cloned() else { return };
            self.pc += 1;
            match id {
                cmd::PATH => {
                    let forwards = !matches!(args.get(1), Some(Arg::Bool(false)));
                    if let Some(p) = args.first().and_then(int) {
                        self.walk = GuardRun::path(world, p, forwards);
                    }
                }
                cmd::ANIMATE => self.wait = CLIP_TIME,
                cmd::USE => {
                    // Walked straight to the object; the use itself waits as an animation does (working value).
                    if let Some(at) = args
                        .first()
                        .and_then(int)
                        .and_then(|id| world.find_sly3_id(id, None))
                        .and_then(|h| world.position(h))
                    {
                        self.walk = alloc::vec![at];
                    }
                    self.wait = CLIP_TIME;
                }
                cmd::ALARM => {
                    if let Some(a) = args.get(1).and_then(int) {
                        alarms.insert(a);
                    }
                }
                other => *self.skipped.entry(other).or_default() += 1,
            }
        }
    }
}
