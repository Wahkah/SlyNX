//! The Sly 2 side of [`WorldHost`] (specs/sly2/scripts.md 4 and 5, object-stream.md 1): what the script host does
//! differently when the world is a Sly 2 world (`World::sly2`).
//!
//! - **Classes.** The world object (operation 117) is class 1, the class whose option list holds the world options.
//! - **Options** are read and written by the executable's option table (object-stream.md 1.2): an option no list set
//!   reads as its value type's zero (false, 0, 0.0, a zero vector or range, an empty list). Writing stores the value
//!   with no Sly 1 side effect. Sly 1's run-time options (28 to 39 and the others of `live`) are not used.
//! - **Searches** (operations 112, 113 and 115, through the wide-ID hook) take the `s32` object ID. With a context
//!   object they look below it first, then in its copy (the package instance it belongs to, object-stream.md 4), then
//!   in the whole world; without one, in the whole world. "Nearest" searches use the same order. A placement's
//!   object ID names its copy's root (scripts.md 5): a search that matches a placement record finds the first
//!   object of the copy it placed instead, ranked by where that root is; a placement whose copy is empty is found
//!   itself.
//! - **Game operations** (`script::sly2::HOST_OPERATIONS`), with the meanings of scripts.md 4.2:
//!   - 97 the level row, 99 its episode (disc.md 6);
//!   - 100, 101, 102 the current job, the current task and the checkpoint task (missions.md), kept in
//!     `Sly2Level::job` by the job system; the null object until it sets them;
//!   - 103 the current difficulty help, 0 to 1 (`Sly2Level::difficulty_help`), 104 always 0;
//!   - 114 one entry per object ID of the list (or the one ID), in order, the null object where nothing is found;
//!     below the context object, or in the whole world when the context is the null object;
//!   - 118 (through the player hook, as Sly 1's operation 107) the sound manager, and 137 the HUD prompt object:
//!     objects of classes 152 and 154, whose options scripts use on them (266 and 273; 979); the game makes them,
//!     so the host makes one of each on first use;
//!   - 121 objects in a box, 123 and 124 objects hit by a segment: none (Sly 2 worlds have no collision bodies yet);
//!   - 131 a saved progress value by path: a quoted list of name symbols (the symbols are the save names' IDs),
//!     read from the job system's progress block (`game::sly2_jobs`); a missing path, or no job system, is "no
//!     value" (nil);
//!   - 138 whether item n of the item table is owned (false for −1), 139 sets item n's owned flag
//!     (`powerUps/mppukfAcquired` in the progress block) and, when it sets it, raises task event 59
//!     (`Sly2Level::task_events`).
//! - **The job system** (`game::sly2_jobs`, when attached) answers task options 1282 (state) and 1267 (play count)
//!   and runs the methods 1280 and 1281 (task requests), 1238 and 1239 (end, restart a job), 786 (fire an exit) and
//!   778 (warp). Options of value type `0x2000` + c hold the ID of an object of class c (1243, 1264, 1259, ...);
//!   reading one gives the object, found from the reading object, or the null object.
//!
//! Actions (typed-argument options) go to `World::run_action` as in Sly 1; with no Sly 2 behaviour yet they are
//! counted in `World::unhandled_actions`. Option 1095 on a noise template (class 117), called with a position and
//! an owner (or the null object), makes a noise from the template there (`behavior::sly2::NoiseTemplate`); the host
//! then returns a new noise object for it, a run-time object of class 147 (the noise class: the class map gives
//! script option 959, the investigation point 1098, only to class 147) with no record that lives as long as the
//! noise (destroyed on a later call once the noise has expired). Setting its option 1098 (the investigation point)
//! moves the noise there [working choice: the guards walk to the noise's position].

use alloc::vec::Vec;

use formats_sly2::exe::OptionEntry;
use script::{ObjectId, ScriptError, Value};

use super::{WorldHost, to_value};
use crate::game::sly2_anim;
use crate::game::sly2_hud;
use crate::game::sly2_jobs::{self, shop};
use crate::object::{Handle, OptionValue, WORLD_SCRIPT_ID};
use crate::world::NONE;

/// Class of the world object.
pub const WORLD_CLASS: i16 = 1;
/// Class of the object operation 118 gives (the sound manager).
pub const SOUND_MANAGER_CLASS: i16 = 152;
/// Class of the object operation 137 gives (the HUD prompt).
pub const PROMPT_CLASS: i16 = 154;
/// Task event operation 139 raises when it sets an owned-item flag, as a purchase does.
pub const ITEM_OWNED_EVENT: u32 = 59;
/// The noise-template method (scripts.md 5): makes a noise and returns a noise object.
pub const MAKE_NOISE: u16 = 1095;
/// A noise object's investigation point.
pub const NOISE_POINT: u16 = 1098;
/// Class of the run-time noise objects option 1095 returns (the class whose map holds 1098).
pub const NOISE_CLASS: i16 = 147;

/// Value type `0x12`: never stored in an option list (object-stream.md 1.2); the option is a procedure of the game
/// that scripts call.
pub const VALUE_TYPE_PROCEDURE: u32 = 0x12;

/// Whether scripts call option `entry` as a method although the option table does not mark it as a typed argument
/// list: value type `0x12`, or flag bit `0x1` with arguments [inference from the disc, see `docs/engine.md`].
pub fn is_procedure(entry: &OptionEntry) -> bool {
    !entry.is_typed() && (entry.value_type == VALUE_TYPE_PROCEDURE || (entry.flags & 0x1 != 0 && entry.arg_count > 0))
}

/// The value a never-set option of `entry` reads as.
pub fn zero_of(entry: Option<&OptionEntry>) -> Value {
    let Some(e) = entry else { return Value::Void };
    if e.is_typed() {
        return Value::Void;
    }
    match e.value_type {
        0 => Value::Bool(false),
        1 => Value::Float(0.0),
        2 | 9 => Value::Vector([0.0; 3]),
        10 => Value::Range([0.0; 2]),
        formats_sly2::options::VALUE_TYPE_U16_LIST => Value::Nil,
        _ => Value::Int(0),
    }
}

fn num(v: &Value) -> Option<f32> {
    match v {
        Value::Bool(b) => Some(*b as i32 as f32),
        _ => v.as_f32(),
    }
}

impl WorldHost<'_> {
    /// The option-table entry of a stream option (the Sly 2 or Sly 3 executable's table; the option code is shared
    /// with `script_host::sly3`).
    pub(super) fn entry(&self, option: u16) -> Option<&OptionEntry> {
        match (self.world.sly2.as_deref(), self.world.sly3.as_deref()) {
            (Some(l), _) => l.table.get(option as i16),
            (_, Some(l)) => l.table.get(option as i16),
            _ => None,
        }
    }

    /// The `s32` ID an object value stands for in an option, or the number.
    fn id_value(&self, v: &Value) -> Option<i32> {
        match v {
            Value::Object(Some(o)) => {
                let h = self.obj(*o)?;
                self.world.sly2_id(h).or_else(|| self.world.sly3_id(h))
            }
            Value::Object(None) => Some(-1),
            Value::Bool(b) => Some(*b as i32),
            Value::Float(f) => Some(*f as i32),
            _ => v.as_i32(),
        }
    }

    /// A script value converted to a Sly 2 option value of `entry`'s value type.
    fn sly2_to_option(&self, entry: Option<&OptionEntry>, v: &Value) -> Option<OptionValue> {
        let Some(e) = entry else { return self.untyped_option(v) };
        // Nil (the empty list) stored in an object option is no object (Sly 3's option 541, value type 0x2002, is set
        // to nil by m_ext's scripts).
        if matches!(v, Value::Nil) && (e.value_type == 6 || (0x2000..0x3000).contains(&e.value_type)) {
            return Some(OptionValue::Int(-1));
        }
        Some(match e.value_type {
            0 => OptionValue::Byte(match v {
                Value::Bool(b) => *b as u8,
                _ => num(v)? as i32 as u8,
            }),
            1 => OptionValue::Float(num(v)?),
            2 | 9 => match v {
                Value::Vector(x) | Value::Coeffs(x) | Value::Smoothing(x) => OptionValue::Vector(*x),
                _ => return None,
            },
            10 => match v {
                Value::Range(r) => OptionValue::Pair(*r),
                _ => return None,
            },
            13 => OptionValue::Color(self.id_value(v)?.to_le_bytes()),
            7 | 14 | 15 | 16 => OptionValue::Short(self.id_value(v)? as i16),
            _ => OptionValue::Int(self.id_value(v)?),
        })
    }

    /// Also the Sly 3 class lookup: both games' world object is class 1.
    pub(super) fn sly2_class_of(&self, object: ObjectId) -> Option<i16> {
        if object == WORLD_SCRIPT_ID {
            return Some(WORLD_CLASS);
        }
        self.world.class(self.obj(object)?)
    }

    /// Also Sly 3's option read (same value types, same store).
    pub(super) fn sly2_get_option(&mut self, object: ObjectId, option: u16) -> Value {
        let stored = if object == WORLD_SCRIPT_ID {
            self.world.world_options.get(option).cloned()
        } else {
            let Some(h) = self.obj(object) else { return Value::Error(ScriptError::Host(option as u32)) };
            self.world.options(h).and_then(|o| o.get(option)).cloned()
        };
        if stored.is_none() && object != WORLD_SCRIPT_ID && self.entry(option).is_some_and(is_procedure) {
            return Value::Method { object, option };
        }
        if self.world.sly2.is_some() && object != WORLD_SCRIPT_ID {
            if let Some(v) = self.job_option(object, option) {
                return v;
            }
            if let Some(h) = self.obj(object)
                && let Some(v) = self.world.sly2.as_deref().and_then(|l| l.anim.get_option(self.world, h, option))
            {
                return match (option, v) {
                    // Object options give the object.
                    (
                        sly2_anim::opt::TARGET | sly2_anim::opt::CLIP | sly2_anim::opt::CLIP_TARGET,
                        OptionValue::Int(id),
                    ) => Value::Object(self.sly2_find(id, Some(object))),
                    (_, v) => to_value(&v),
                };
            }
            // Value types 0x2000 + c hold the ID of an object of class c: scripts get the object.
            if let Some(e) = self.entry(option)
                && (0x2000..0x3000).contains(&e.value_type)
                && !e.is_typed()
            {
                let id = stored.as_ref().and_then(OptionValue::as_i32).unwrap_or(-1);
                return Value::Object(if id < 0 { None } else { self.sly2_find(id, Some(object)) });
            }
        }
        match stored {
            Some(OptionValue::Args(a)) => Value::list(a.iter().map(|x| match x {
                crate::object::Arg::Short(_, s) => Value::Int(*s as i32),
                _ => Value::Void,
            })),
            Some(v) => to_value(&v),
            None => zero_of(self.entry(option)),
        }
    }

    /// Also Sly 3's option write.
    pub(super) fn sly2_set_option(&mut self, object: ObjectId, option: u16, value: &Value) -> Result<(), ScriptError> {
        if matches!(value, Value::Error(_) | Value::Void) {
            // An error value was reported where it was made; storing it would only repeat it. "No value" (for
            // example a typed option read back, as scripts do with option 34 before setting a noise's 1098) leaves
            // the option as it is.
            return Ok(());
        }
        let v = self.sly2_to_option(self.entry(option), value).ok_or(ScriptError::Host(option as u32))?;
        if object == WORLD_SCRIPT_ID {
            self.world.world_options.set(option, v);
            return Ok(());
        }
        let h = self.obj(object).ok_or(ScriptError::Host(option as u32))?;
        if self.world.sly2.as_deref_mut().is_some_and(|l| l.anim.set_option(h, option, &v)) {
            return Ok(());
        }
        if option == NOISE_POINT
            && let Some(to) = v.as_vec3()
        {
            self.move_noise(h, to);
        }
        self.world.set_option(h, option, v);
        Ok(())
    }

    /// Whether `slot` is `root` or below it.
    pub(super) fn below(&self, slot: u32, root: Handle) -> bool {
        let Some(mut h) = self.world.handle(slot) else { return false };
        loop {
            if h == root {
                return true;
            }
            match self.world.parent(h) {
                Some(p) => h = p,
                None => return false,
            }
        }
    }

    /// The root of the copy placement `slot` made: the first object of that copy, below the placement.
    fn copy_root(&self, slot: u32) -> Option<u32> {
        let l = self.world.sly2.as_deref()?;
        let h = self.world.handle(slot)?;
        if !matches!(self.world.sly2_record(h)?.body, formats_sly2::stream::Body::Placement(_)) {
            return None;
        }
        let own = l.object(slot)?.copy;
        self.world
            .children(h)
            .filter_map(|c| self.world.slot(c))
            .filter(|&c| l.object(c).is_some_and(|o| o.copy != own))
            .min()
    }

    /// Every object with Sly 2 ID `id`, in the search order of the module documentation (placements stand for
    /// their copies' roots).
    fn sly2_search(&self, id: i32, context: Option<ObjectId>) -> Vec<Handle> {
        let mut slots: Vec<u32> =
            self.world.slots_with_sly2_id(id).into_iter().map(|s| self.copy_root(s).unwrap_or(s)).collect();
        slots.sort_unstable();
        slots.dedup();
        let ctx = self.ctx(context);
        let copy = ctx.and_then(|c| self.world.sly2_object(c)).map(|o| o.copy);
        let rank = |s: u32| -> u8 {
            match ctx {
                Some(c) if self.below(s, c) => 0,
                Some(_)
                    if copy.is_some()
                        && self.world.sly2.as_deref().and_then(|l| l.object(s)).map(|o| o.copy) == copy =>
                {
                    1
                }
                _ => 2,
            }
        };
        let mut ranked: Vec<(u8, u32)> = slots.into_iter().map(|s| (rank(s), s)).collect();
        ranked.sort_unstable();
        ranked.into_iter().filter_map(|(_, s)| self.world.handle(s)).collect()
    }

    pub(super) fn sly2_find(&mut self, id: i32, context: Option<ObjectId>) -> Option<ObjectId> {
        self.sly2_search(id, context).first().map(|h| h.to_script())
    }

    pub(super) fn sly2_find_all(&mut self, id: i32, context: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        out.extend(self.sly2_search(id, context).into_iter().map(Handle::to_script));
    }

    /// The object the game provides for operation `op` (made on first use, out of every package).
    fn game_object(&mut self, op: u32, class: i16) -> Option<ObjectId> {
        let existing = self.world.sly2.as_deref()?.game_objects.get(&op).copied();
        let slot = match existing.and_then(|s| self.world.handle(s)) {
            Some(h) => h.index(),
            None => {
                let s = self.world.create(
                    class,
                    -1,
                    u32::MAX,
                    -1,
                    formats::geometry::Affine::IDENTITY,
                    NONE,
                    u64::MAX - op as u64,
                    0,
                );
                let l = self.world.sly2.as_deref_mut()?;
                l.game_objects.insert(op, s);
                s
            }
        };
        self.world.handle(slot).map(Handle::to_script)
    }

    /// Operation 118 (run as Sly 1's player operation, 107): the sound manager, not the player.
    pub(super) fn sly2_player(&mut self) -> Option<ObjectId> {
        self.game_object(118, SOUND_MANAGER_CLASS)
    }

    /// After option 1095 ran on a noise template (its behaviour, `behavior::sly2::NoiseTemplate`, made the noise, the
    /// latest of `World::shared.noises`): the new noise object the method gives back.
    pub(super) fn sly2_noise_object(&mut self) -> Option<Handle> {
        let now = self.world.shared.time;
        self.expire_noises(now);
        let n = *self.world.shared.noises.last()?;
        let (at, lifetime) = (n.position, n.lifetime);
        let s =
            self.world.create(NOISE_CLASS, -1, u32::MAX, -1, formats::geometry::Affine::IDENTITY, NONE, u64::MAX, 0);
        let h = self.world.handle(s)?;
        self.world.set_option(h, NOISE_POINT, OptionValue::Vector(at));
        let until = if lifetime < 0.0 { -1.0 } else { now + lifetime };
        self.world.sly2.as_deref_mut()?.noise_objects.push((s, until));
        Some(h)
    }

    /// Destroys the noise objects whose noise has expired by `now`.
    fn expire_noises(&mut self, now: f32) {
        let Some(l) = self.world.sly2.as_deref_mut() else { return };
        let (gone, kept): (Vec<_>, Vec<_>) = l.noise_objects.drain(..).partition(|&(_, t)| t >= 0.0 && now > t);
        l.noise_objects = kept;
        for (s, _) in gone {
            if let Some(h) = self.world.handle(s) {
                self.world.destroy(h);
            }
        }
    }

    /// The job system's methods (module documentation): task requests (1280, 1281), event 23 (1284), forced states
    /// (1290 to 1293), becoming current (1294); a job's failure (1238), restart at the checkpoint (1239), end (1240)
    /// and play-count reset (1250); firing an exit (786) and warping to a warp point (778). `None` for any other
    /// option.
    pub(super) fn sly2_call_action(&mut self, object: ObjectId, option: u16, args: &[Value]) -> Option<Value> {
        use crate::game::sly2_jobs::{flow, opt};
        if let Some(v) = self.anim_action(object, option, args) {
            return Some(v);
        }
        // The HUD prompt object (operation 137, class 154): show (979 = 1300) or end (980 = 1301) a prompt.
        if (option == sly2_hud::SHOW_PROMPT || option == sly2_hud::END_PROMPT)
            && self.obj(object).and_then(|h| self.world.class(h)) == Some(PROMPT_CLASS)
        {
            let id = args.first().and_then(|a| a.as_i32()).unwrap_or(-1);
            let time = args.get(1).and_then(|a| a.as_f32()).unwrap_or(-1.0);
            let entry = if option == sly2_hud::SHOW_PROMPT { (id, time) } else { (-1, 0.0) };
            if let Some(l) = self.world.sly2.as_deref_mut() {
                l.hud.prompts.push(entry);
                if l.hud.prompts.len() > 16 {
                    l.hud.prompts.remove(0);
                }
            }
            return Some(Value::Void);
        }
        let job_method = [opt::FAILURE, opt::RESTART, opt::END_JOB, opt::RESET_COUNTS].contains(&option);
        let task_method = [opt::REQUEST, opt::FORCE, opt::SIGNAL, opt::MAKE_CURRENT].contains(&option)
            || (opt::FORCE_LOCKED..=opt::FORCE_CLOSED).contains(&option);
        if !job_method && !task_method && option != flow::opt::FIRE && option != flow::opt::WARP {
            return None;
        }
        let h = self.obj(object)?;
        let id = self.world.sly2_id(h)?;
        let class = self.world.class(h)?;
        let want = args.first().and_then(|a| a.as_i32()).unwrap_or(-1);
        let done = sly2_jobs::with(self.world, |j, w| match (option, class) {
            (opt::REQUEST | opt::FORCE, 133) => {
                if let Some(&t) = j.graph.task_ids.get(&id) {
                    j.request(t, want as i8, option == opt::FORCE);
                }
                true
            }
            (opt::SIGNAL, 133) => {
                w.raise(h, sly2_jobs::script_event::TASK_SIGNAL, alloc::vec::Vec::new());
                true
            }
            (o, 133) if (opt::FORCE_LOCKED..=opt::FORCE_CLOSED).contains(&o) => {
                if let Some(&t) = j.graph.task_ids.get(&id) {
                    j.force_state(t, (o - opt::FORCE_LOCKED) as i8);
                }
                true
            }
            (opt::MAKE_CURRENT, 133) => {
                if let Some(&t) = j.graph.task_ids.get(&id) {
                    j.request_current(t, false);
                }
                true
            }
            (opt::FAILURE, 132) => {
                j.count_failure();
                true
            }
            (opt::END_JOB, 132) => {
                if let Some(&jb) = j.graph.job_ids.get(&id) {
                    j.end_job(jb);
                }
                true
            }
            (opt::RESET_COUNTS, 132) => {
                if let Some(&jb) = j.graph.job_ids.get(&id) {
                    j.reset_play_counts(jb);
                }
                true
            }
            (opt::RESTART, 132) => {
                if j.job.is_some_and(|jb| j.graph.jobs[jb].id == id) {
                    j.restart_at_checkpoint();
                }
                true
            }
            (flow::opt::FIRE, flow::EXIT_CLASS) => {
                j.fire_exit_id(w, id);
                true
            }
            (flow::opt::WARP, flow::WARP_CLASS) => {
                j.warp(w, h);
                true
            }
            _ => false,
        });
        done.filter(|&d| d).map(|_| Value::Void)
    }

    /// The playback methods (`game::sly2_anim`): find (73, 74) and apply (106, 368) playbacks, jump a machine to a
    /// state (379), seek a clip playback (123), remove a playback (380, 124). `None` for any other option or class.
    fn anim_action(&mut self, object: ObjectId, option: u16, args: &[Value]) -> Option<Value> {
        use sly2_anim::opt;
        if ![
            opt::FIND_CLIP,
            opt::FIND_MACHINE,
            opt::APPLY_CLIP,
            opt::APPLY_MACHINE,
            opt::JUMP,
            opt::REMOVE,
            opt::CLIP_SEEK,
            opt::CLIP_REMOVE,
        ]
        .contains(&option)
        {
            return None;
        }
        let h = self.obj(object)?;
        let class = self.world.class(h)?;
        let int = |i: usize| args.get(i).and_then(|a| a.as_i32());
        let float = |i: usize| args.get(i).and_then(|a| a.as_f32());
        // The target: an object, or an object ID searched from the clip or machine; none: the clip's or machine's
        // owner (its parent) [working choice].
        let target = match args.first() {
            Some(Value::Object(Some(o))) => self.obj(*o),
            Some(v @ (Value::Int(_) | Value::Float(_))) => {
                v.as_i32().and_then(|id| self.sly2_find(id, Some(object))).and_then(|o| self.obj(o))
            }
            _ => None,
        }
        .or_else(|| self.world.parent(h));
        let out = sly2_anim::with(self.world, |a, w| -> Option<Option<Handle>> {
            Some(match (option, class) {
                (opt::FIND_MACHINE, _) => a.find_machine(w, h, int(0)?),
                (opt::FIND_CLIP, _) => a.find_clip(w, h, int(0)?),
                (opt::APPLY_MACHINE, sly2_anim::MACHINE_CLASS) => {
                    let p = a.machine_playback(w, h, target?)?;
                    if let Some(g) = int(1).filter(|&g| g >= 0) {
                        a.goal(p, g);
                    }
                    Some(p)
                }
                (opt::APPLY_CLIP, sly2_anim::CLIP_CLASS | sly2_anim::CLIP_CLASS_2) => {
                    a.apply_clip(w, h, target?, float(1).unwrap_or(0.0), float(2).unwrap_or(1.0), int(3).unwrap_or(0))
                }
                (opt::JUMP, sly2_anim::MACHINE_PLAYBACK_CLASS) => {
                    a.jump(w, h, int(0)?);
                    None
                }
                (opt::CLIP_SEEK, sly2_anim::CLIP_PLAYBACK_CLASS) => {
                    a.seek(h, int(0).unwrap_or(0), float(1).unwrap_or(0.0), float(2));
                    None
                }
                (opt::REMOVE, sly2_anim::MACHINE_PLAYBACK_CLASS)
                | (opt::CLIP_REMOVE, sly2_anim::CLIP_PLAYBACK_CLASS) => {
                    a.remove(w, h);
                    None
                }
                _ => return None,
            })
        })??;
        Some(match option {
            opt::JUMP | opt::REMOVE | opt::CLIP_SEEK | opt::CLIP_REMOVE => Value::Void,
            _ => Value::Object(out.map(Handle::to_script)),
        })
    }

    /// The job and task options the job system answers (module documentation): a task's state (1282) and play
    /// count (1267).
    fn job_option(&self, object: ObjectId, option: u16) -> Option<Value> {
        use crate::game::sly2_jobs::opt;
        // Object-model hook: option 1286 (script ID 342) also reads the task's state (level-flow.md 4.1: the episode
        // macros' intro handlers test it).
        let option = if option == TASK_STATE_342 { opt::STATE } else { option };
        if option != opt::STATE && option != opt::PLAYED {
            return None;
        }
        let h = self.obj(object)?;
        let jobs = self.world.sly2.as_deref()?.jobs.as_deref()?;
        let t = *jobs.graph.task_ids.get(&self.world.sly2_id(h)?)?;
        Some(Value::Int(match option {
            opt::STATE => jobs.states[t] as i32,
            _ => jobs.graph.tasks[t]
                .keys
                .focus
                .as_ref()
                .and_then(|k| jobs.progress.get(k, None))
                .map_or(0, |v| v.as_i32()),
        }))
    }

    /// Setting a noise object's investigation point moves its noise (the latest noise at the old point).
    fn move_noise(&mut self, h: Handle, to: [f32; 3]) {
        let Some(s) = self.world.slot(h) else { return };
        if !self.world.sly2.as_deref().is_some_and(|l| l.noise_objects.iter().any(|&(n, _)| n == s)) {
            return;
        }
        let from = self.world.options(h).and_then(|o| o.get(NOISE_POINT)).and_then(|v| v.as_vec3());
        if let Some(from) = from
            && let Some(n) = self.world.shared.noises.iter_mut().rev().find(|n| n.position == from)
        {
            n.position = to;
        }
    }

    pub(super) fn sly2_game_operation(&mut self, op: u32, args: &[Value]) -> Value {
        let row = self.world.sly2.as_deref().and_then(|l| l.row).map_or(self.world.info.index as i32, |r| r as i32);
        match op {
            97 => Value::Int(row),
            99 => {
                let ep = self.world.sly2.as_deref().and_then(|l| l.header.level_rows.get(row.max(0) as usize));
                Value::Int(ep.map_or(0, |r| r.episode as i32))
            }
            100..=102 => {
                let slot = self.world.sly2.as_deref().and_then(|l| l.job[(op - 100) as usize]);
                Value::Object(slot.and_then(|s| self.world.handle(s)).map(Handle::to_script))
            }
            103 => Value::Float(self.world.sly2.as_deref().map_or(0.0, |l| l.difficulty_help.clamp(0.0, 1.0))),
            104 => Value::Int(0),
            114 => {
                let ctx = args.get(1).and_then(|a| a.as_object()).flatten();
                let ids: Vec<i32> = match args.first() {
                    Some(v @ Value::List(_)) => v.to_vec().iter().map(|x| x.as_i32().unwrap_or(-1)).collect(),
                    Some(v) => v.as_i32().into_iter().collect(),
                    None => Vec::new(),
                };
                let found: Vec<Value> = ids.into_iter().map(|id| Value::Object(self.sly2_find(id, ctx))).collect();
                Value::list(found)
            }
            121 | 123 => Value::Nil,
            124 => Value::Object(None),
            131 => {
                // A path of name symbols (the symbols are the save names' IDs), then an index for an array.
                let (mut path, mut index) = (Vec::new(), None);
                for x in args.first().map(|p| p.to_vec()).unwrap_or_default() {
                    match x {
                        Value::Symbol(n) => path.push(n as u16),
                        v => index = v.as_i32().map(|i| i.max(0) as usize),
                    }
                }
                let progress = self.world.sly2.as_deref().and_then(|l| l.jobs.as_deref()).map(|j| &j.progress);
                match progress.and_then(|p| p.get(&path, index)) {
                    Some(save::sly2::SaveValue::Int(v)) => Value::Int(v),
                    Some(save::sly2::SaveValue::Float(f)) => Value::Float(f),
                    Some(save::sly2::SaveValue::Flag(b)) => Value::Bool(b),
                    None => Value::Nil,
                }
            }
            137 => Value::Object(self.game_object(137, PROMPT_CLASS)),
            138 => {
                let id = args.first().and_then(|a| a.as_i32()).unwrap_or(-1);
                let jobs = self.world.sly2.as_deref().and_then(|l| l.jobs.as_deref());
                Value::Bool(jobs.is_some_and(|j| shop::owned(&j.progress, id)))
            }
            139 => {
                let id = args.first().and_then(|a| a.as_i32()).unwrap_or(-1);
                let on = args.get(1).is_none_or(|v| !matches!(v, Value::Bool(false) | Value::Int(0) | Value::Nil));
                if let Some(l) = self.world.sly2.as_deref_mut() {
                    if let Some(j) = l.jobs.as_deref_mut() {
                        shop::set_owned(&mut j.progress, id, on);
                    }
                    if on {
                        l.task_events.push(ITEM_OWNED_EVENT);
                    }
                }
                Value::Void
            }
            _ => Value::Error(ScriptError::Host(op)),
        }
    }
}

/// Object-model hook: option 1286 (script ID 342), a task's state as level-flow.md 4.1 reads it.
const TASK_STATE_342: u16 = 1286;

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(value_type: u32, flags: u32, arg_count: i32) -> OptionEntry {
        OptionEntry { value_type, flags, arg_count }
    }

    #[test]
    fn zeros_by_value_type() {
        assert!(matches!(zero_of(Some(&entry(0, 0, 0))), Value::Bool(false)));
        assert!(matches!(zero_of(Some(&entry(1, 0, 0))), Value::Float(0.0)));
        assert!(matches!(zero_of(Some(&entry(9, 0, 0))), Value::Vector(v) if v == [0.0; 3]));
        assert!(matches!(zero_of(Some(&entry(10, 0, 0))), Value::Range(r) if r == [0.0; 2]));
        assert!(matches!(zero_of(Some(&entry(0x2030, 0, 0))), Value::Int(0)));
        assert!(matches!(zero_of(Some(&entry(15, 0, 0))), Value::Int(0)));
        assert!(matches!(zero_of(Some(&entry(0x16, 0x80, 0))), Value::Nil));
        assert!(matches!(zero_of(Some(&entry(0x12, 0x401, 1))), Value::Void));
        assert!(matches!(zero_of(None), Value::Void));
    }

    #[test]
    fn procedures() {
        // Value type 0x12 with or without arguments, and flag 0x1 with arguments, are called as methods.
        assert!(is_procedure(&entry(0x12, 0x4001, 0)));
        assert!(is_procedure(&entry(0x12, 0x1, 2)));
        assert!(is_procedure(&entry(0x2093, 0x1, 2)));
        // Typed argument lists are actions already; plain values are not procedures.
        assert!(!is_procedure(&entry(0x12, 0x401, 1)));
        assert!(!is_procedure(&entry(0x2030, 0x2, 0)));
        assert!(!is_procedure(&entry(0, 0x1, 0)));
    }
}
