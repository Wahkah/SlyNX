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
        if self.world.sly3.is_some()
            && let Some(v) = self.sly3_live_option(object, option)
        {
            return v; // Sly 3 hook: options the game computes.
        }
        // Object-model hook (missions.md 4.3: 1826's end test asks volume 123886 about the current character's
        // position): the world's option 926 (script ID 844, a getter) is the character the player controls, and
        // option 21 (script ID 34) is an object's position.
        if self.world.sly3.is_none() {
            if object == WORLD_SCRIPT_ID && option == WORLD_CURRENT_CHARACTER {
                return Value::Object(self.world.player.map(Handle::to_script));
            }
            if option == OBJECT_POSITION
                && let Some(p) = self.obj(object).and_then(|h| self.world.position(h))
            {
                return Value::Vector(p);
            }
            // Script ID 41 (option 27, a getter) is the object's in-world flag (missions.md 4.3: the siege
            // generator counts only its spawns in the world as alive).
            if option == IN_WORLD
                && let Some(h) = self.obj(object)
            {
                return Value::Bool(self.world.in_world(h));
            }
            // Script ID 643 (stream option 620, a getter) on a sequence is the starter it runs with
            // (dialogue-and-voice.md 2.1) [I: from its uses; row 2's safe-house sequence 27254, started by 141710
            // with Sly, takes every character but that one out of the world].
            if option == SEQUENCE_STARTER
                && let Some(h) = self.obj(object).filter(|&h| self.world.class(h) == Some(114))
            {
                let starter = crate::behavior::sly2_commands::starter_of(self.world, h);
                return Value::Object(starter.map(Handle::to_script));
            }
            // The parent field (script ID 32, option 19) and the placement leasing the object's copy (script ID 31,
            // option 18), as missions.md 4.3 reads them for the water main: a copy root's parent is the placement's
            // parent (the tank root's parent is the rig 123922, not placement 124106) [I], and the placement is
            // given as a search for its ID finds it, its copy's root (scripts.md 5).
            if option == OBJECT_PARENT
                && let Some(h) = self.obj(object)
            {
                let is_placement = |p: Handle| self.world.class(p).is_some_and(|c| crate::world::sly2::is_a(c, 6));
                let parent =
                    self.world.parent(h).and_then(|p| if is_placement(p) { self.world.parent(p) } else { Some(p) });
                return Value::Object(parent.map(Handle::to_script));
            }
            if option == OBJECT_PLACEMENT
                && let Some(h) = self.obj(object)
            {
                let mut at = self.world.parent(h);
                while let Some(p) = at {
                    if self.world.class(p).is_some_and(|c| crate::world::sly2::is_a(c, 6)) {
                        break;
                    }
                    at = self.world.parent(p);
                }
                let given = at.map(|p| {
                    self.world.slot(p).and_then(|s| self.copy_root(s)).and_then(|r| self.world.handle(r)).unwrap_or(p)
                });
                return Value::Object(given.map(Handle::to_script));
            }
            // The object's own ID (script ID 28, option 15; scripts.md 5), and its alias (script ID 29, option 16): the
            // ID of the placement whose copy's root it is, when that differs, else −1.
            if option == OBJECT_ALIAS
                && let Some(h) = self.obj(object)
            {
                let alias = self.world.parent(h).filter(|&p| {
                    self.world.class(p).is_some_and(|c| crate::world::sly2::is_a(c, 6))
                        && self.world.slot(p).and_then(|ps| self.copy_root(ps)) == Some(h.index())
                });
                let id = alias.and_then(|p| self.world.sly2_id(p)).filter(|&id| Some(id) != self.world.sly2_id(h));
                return Value::Int(id.unwrap_or(-1));
            }
            if option == OBJECT_ID
                && let Some(h) = self.obj(object)
                && self.world.options(h).and_then(|o| o.get(option)).is_none()
                && let Some(id) = self.world.sly2_id(h)
            {
                return Value::Int(id);
            }
            // A trigger volume's first counting object found inside (script ID 534, option 447; interactables.md 4).
            if option == VOLUME_FIRST
                && let Some(h) = self.obj(object)
                && let Some(crate::behavior::Behavior::Sly2Volume(t)) = self.world.behavior(h)
            {
                return Value::Object(t.first.map(Handle::to_script));
            }
        }
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
        // Sly 3 hook (missions.md 4.1a): a module writing its runner's context (script ID 741 on a command script).
        if self.world.sly3.is_some()
            && option == super::sly3::RUNNER_CONTEXT
            && let Some(h) = self.obj(object)
            && self.world.class(h) == Some(135)
            && let Some(script) = self.world.sly3_id(h)
        {
            let with = match value {
                Value::Object(Some(o)) => self.obj(*o).and_then(|o| self.world.sly3_id(o)).unwrap_or(-1),
                _ => -1,
            };
            self.world.shared.sly3_context_writes.push((script, with));
            return Ok(());
        }
        // Sly 3 hook: options the game applies rather than stores (a crew member's velocity, `script_host::sly3`).
        if self.world.sly3.is_some() && self.sly3_set_live_option(object, option, value) {
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
        // Object-model hook: writing an object's position (script ID 34, option 21) moves it there in the world
        // [I: the getter's counterpart; m1_recon's 1771 puts photo target 122764 150 above Dimitri, then parents it to
        // him]. Stored only, the target stayed where it was loaded and rode on him from there.
        if option == OBJECT_POSITION
            && self.world.sly3.is_none()
            && let Some(p) = v.as_vec3()
            && let Some(mut w) = self.world.world_transform(h)
        {
            w.t = p;
            self.world.set_world_transform(h, w);
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
        // As loaded, even once a spawn has detached it from the placement.
        if let Some(&(r, ..)) = l.placed.get(&slot) {
            return Some(r);
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

    /// Operation 120 (scripts.md 4.3): every object of class `class` or a class derived from it, in load order; with
    /// a context, the context itself and what lies below it, else every object of the world.
    pub(super) fn sly2_find_by_class(&mut self, class: i16, context: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        let ctx = self.ctx(context);
        if context.is_some() && ctx.is_none() {
            return;
        }
        let hits: Vec<Handle> = self
            .world
            .handles()
            .filter(|&h| self.world.class(h).is_some_and(|c| crate::world::sly2::is_a(c, i32::from(class))))
            .filter(|&h| ctx.is_none_or(|c| self.below(h.index(), c)))
            // Only fresh placements are class-6 objects in the game; a copy placement makes none (classes/placement.md
            // 2.1: the big tank 123921 is not among the generator's candidates).
            .filter(|&h| {
                !matches!(self.world.sly2_record(h).map(|r| &r.body),
                    Some(formats_sly2::stream::Body::Placement(p)) if !p.fresh)
            })
            .collect();
        out.extend(hits.into_iter().map(Handle::to_script));
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
        // Object-model hook: a placement's lease of its shared package's copy (classes/placement.md 2.1): 1019
        // (1175) leases one, 1021 (1177) gives the lease (taken first if none). The world holds one copy per placement,
        // so the lease is that copy's root. Taking it puts the copy in the world, which the placement's own option 26
        // does not hold back, and raises script event 14 on the placement and the copy; the placement's blocks and
        // matrix were applied to the copy at load. Leases go by the copy's grade (classes/placement.md 2.1): 0 locked
        // (1023 / 1024, 1179 / 1180, lock and unlock), 4 held by no placement, 1 held with an object in the world, 2
        // flagged in the world but not active (its placement is out), 3 all removed. 1019 needs at least 2, and a
        // non-fixed placement already holding its copy re-takes it only at 3 or more; 1021 needs at least 1. A copy
        // is held from load by a non-fixed placement in the world, and by any placement once leased.
        // Simplifications: one copy per placement (the pool is that copy); 1017 (1174, can one be leased) answers
        // whether the placement has a copy. 1020 (the current lease, 1176) is a getter, not a method.
        // A search for the placement's ID finds the copy's root (scripts.md 5), so the call can arrive there.
        let is_placement = |h: Handle| self.world.class(h).is_some_and(|c| crate::world::sly2::is_a(c, 6));
        let placement = self.obj(object).and_then(|h| {
            if is_placement(h) {
                Some(h)
            } else {
                // A copy's root (what a search for the placement's ID gives), named by its own ID or not.
                self.world.parent(h).filter(|&p| {
                    is_placement(p)
                        && (self.world.sly2_id(p) == self.world.sly2_id(h)
                            || self.world.slot(p).and_then(|ps| self.copy_root(ps)) == Some(h.index()))
                })
            }
        });
        if (LEASE_CAN..=LEASE_UNLOCK).contains(&option)
            && let Some(h) = placement
        {
            // The copy's root as loaded (a spawn detaches it from the placement), else the first child.
            let placed = self.world.slot(h).and_then(|s| self.world.sly2.as_deref()?.placed.get(&s).copied());
            let root = placed.and_then(|(r, ..)| self.world.handle(r)).or_else(|| self.world.children(h).next());
            let slot = h.index();
            match option {
                LEASE_LOCK => {
                    self.world.shared.lease_locked.insert(slot);
                    return Some(Value::Void);
                }
                LEASE_UNLOCK => {
                    self.world.shared.lease_locked.remove(&slot);
                    return Some(Value::Void);
                }
                _ => {}
            }
            let fixed = self.world.options(h).and_then(|o| o.i32(FIXED_PLACEMENT)) == Some(1);
            // Held from load by a non-fixed placement in the world (its copy is in use there), and by any placement
            // once leased; a placement out of the world (m1_neela's 8436) holds none.
            let held = self.world.shared.leased.contains(&slot) || (!fixed && self.world.in_world(h));
            let grade = match root {
                _ if self.world.shared.lease_locked.contains(&slot) => 0,
                None => 0,
                Some(_) if !held => 4,
                Some(r) if self.world.is_active(r) => 1,
                Some(r) if self.world.in_world(r) => 2,
                Some(_) => 3,
            };
            // 1019 (1175) also needs a single-root prefab: one top-level object (the rat pool 125315).
            let single = placed.is_some_and(|(_, tops, _)| tops == 1);
            let can_take = single && if held && !fixed { grade >= 3 } else { grade >= 2 };
            if option == LEASE_TAKE && !can_take {
                return Some(Value::Object(None));
            }
            if option == LEASE_OF && grade < 1 {
                return Some(Value::Object(None));
            }
            let take = option == LEASE_TAKE || option == LEASE_OF && !self.world.shared.leased.contains(&slot);
            if take && let Some(r) = root {
                // The copy goes back under its placement at its loaded matrix (classes/placement.md 2.1: at the
                // placement's matrix, beside the generator), wherever a spawn had left it.
                if let Some((_, _, local)) = placed
                    && self.world.parent(r) != Some(h)
                {
                    self.world.reparent(r, Some(h), false);
                    self.world.set_local(r, local);
                    if let Some(crate::behavior::Behavior::Sly2Guard(g)) = self.world.behavior_mut(r) {
                        g.held = false;
                        g.flight = None;
                    }
                }
                // A knocked-out creature's copy leased again comes back up (the siege's rats).
                {
                    let mut b = self.world.take_behavior(r.index());
                    if let crate::behavior::Behavior::Sly2Guard(g) = &mut b
                        && g.knocked_out()
                    {
                        g.revive(self.world, r);
                    }
                    self.world.put_behavior(r.index(), b);
                }
                self.world.shared.leased.insert(h.index());
                self.world.set_in_world(h, true);
                self.world.set_in_world(r, true);
                // The copy's objects take the placement's zone mask, passed down to every child that does not keep
                // its own (option 32), clock-and-loop.md 3.1 and 3.2 (m1_neela: 8436's 0x2 over Neyla's 0x1). The
                // placement's radius (option 13) is not copied: activation by radius is not modelled.
                let mask = self.world.sly2_zone_mask(h);
                let mut stack = alloc::vec![r];
                while let Some(o) = stack.pop() {
                    if self.world.options(o).and_then(|x| x.get(KEEPS_MASK)).is_some_and(|v| v.as_bool() != Some(false))
                    {
                        continue;
                    }
                    if let Some(l) = self.world.sly2.as_deref_mut() {
                        l.masks.insert(o.index(), mask);
                    }
                    stack.extend(self.world.children(o));
                }
                self.world.raise(h, LEASE_EVENT, Vec::new());
                self.world.raise(r, LEASE_EVENT, Vec::new());
            }
            return Some(match option {
                LEASE_CAN => Value::Bool(root.is_some()),
                _ => Value::Object(root.map(Handle::to_script)),
            });
        }
        // Object-model hook: script ID 235 (option 1071) on an interaction spot (class 35) runs its "used by" routine
        // with the character given (dialogue-and-voice.md 2.1; row 0: 1674 has Sly use 80398, whose sequence 1675,
        // set by 150360's copy patch, signals 1674).
        if option == SPOT_USE
            && let Some(h) =
                self.obj(object).filter(|&h| self.world.class(h).is_some_and(|c| crate::world::sly2::is_a(c, 35)))
        {
            let user = match args.first() {
                Some(Value::Object(Some(o))) => self.obj(*o),
                _ => None,
            };
            crate::behavior::sly2_commands::use_spot(self.world, h, user);
            return Some(Value::Void);
        }
        // Object-model hook: a guard or character (class 56 and derived) told to run a command list (script ID 441,
        // stream option 317; missions.md 4.3: Dimitri's lists), and a volume (class 22 and derived) asked whether a
        // position is inside it (script ID 108, stream option 366).
        // Script ID 970 (stream 1117): an escort job on the list given (guards.md 4.4; m1_neela's chase, 123907).
        if option == ESCORT
            && let Some(h) =
                self.obj(object).filter(|&h| self.world.class(h).is_some_and(|c| crate::world::sly2::is_a(c, 56)))
        {
            if let Some(Value::Object(Some(l))) = args.first()
                && let Some(list) = self.obj(*l)
            {
                crate::behavior::sly2_commands::start_escort(self.world, h, list);
            }
            return Some(Value::Void);
        }
        if option == RUN_LIST
            && let Some(h) =
                self.obj(object).filter(|&h| self.world.class(h).is_some_and(|c| crate::world::sly2::is_a(c, 56)))
        {
            if let Some(Value::Object(Some(l))) = args.first()
                && let Some(list) = self.obj(*l)
            {
                crate::behavior::sly2_commands::run_commands(self.world, h, list);
            }
            return Some(Value::Void);
        }
        if option == VOLUME_HOLDS
            && let Some(h) =
                self.obj(object).filter(|&h| self.world.class(h).is_some_and(|c| crate::world::sly2::is_a(c, 22)))
        {
            let p = match args.first() {
                Some(Value::Vector(v)) => Some(*v),
                _ => None,
            };
            let inside = p
                .zip(crate::behavior::sly2_props::volume_box(self.world, h))
                .is_some_and(|(p, b)| crate::behavior::sly2_props::in_volume(&b, p));
            return Some(Value::Bool(inside));
        }
        // Object-model hook: script ID 939 (stream option 1051) on Murray (class 54) has him take Bentley up ready to
        // throw (missions.md 4.3: 1895's state 1), done by the gang.
        if option == HOLD_BENTLEY
            && self.obj(object).is_some_and(|h| self.world.class(h) == Some(54))
            && let Some(l) = self.world.sly2.as_deref_mut()
        {
            l.hold_bentley = true;
            return Some(Value::Void);
        }
        // Object-model hook: script ID 125 (stream option 53) on a reactive prop sets the response byte of each
        // contact-kind bit of its first argument to its second (characters.md 2.3; missions.md 4.3: 1895's module
        // makes 123934 take only thrown objects).
        if option == crate::behavior::sly2_props::CONTACT_RESPONSE
            && let [mask, byte] = args
            && let (Some(mask), Some(byte)) = (mask.as_i32(), byte.as_i32())
            && let Some(h) = self.obj(object)
            && let Some(crate::behavior::Behavior::Sly2Prop(p)) = self.world.behavior_mut(h)
        {
            p.set_response(mask as u32, byte as u8);
            return Some(Value::Void);
        }
        // Object-model hook: a sequence's signal set (dialogue-and-voice.md 2.1): script ID 640 (617) sends a value,
        // 641 (618) takes it away (the volumes' leave handlers [I]) and 642 (619) asks whether it is there.
        if (SIGNAL_SEND..=SIGNAL_HAS).contains(&option)
            && let Some(h) = self.obj(object).filter(|&h| self.world.class(h) == Some(114))
            && let Some(v) = args.first().and_then(|a| a.as_i32())
            && let Some(s) = self.world.slot(h)
        {
            let shared = &mut self.world.shared;
            return Some(match option {
                SIGNAL_SEND => {
                    shared.send_signal(s, v);
                    Value::Void
                }
                SIGNAL_REMOVE => {
                    shared.take_signal(s, v, true);
                    Value::Void
                }
                _ => Value::Bool(shared.sequence_signals.get(&s).is_some_and(|set| set.contains(&v))),
            });
        }
        // Object-model hook: script IDs 39 and 40 (stream options 25 and 26, any object) put the object in the world
        // and take it out (classes/placement.md 2.1; missions.md 4.3: the water main removes the rat that fell in, the
        // rat generator 17346 adds its root when it starts), and script ID 12 (option 11) raises script event 5 on it
        // (missions.md 4.3: the volume tells the tank root).
        if (option == ADD_TO_WORLD || option == REMOVE_FROM_WORLD)
            && args.is_empty()
            && let Some(h) = self.obj(object)
        {
            self.world.set_in_world(h, option == ADD_TO_WORLD);
            // A copy's root answers to its placement (scripts.md 5): adding it adds the placement it hangs under
            // too, which a placement with option 26 left out (m1_boss: 1918's 39 on Dimitri, 122332, whose
            // placement starts out of the world, kept him inactive).
            if option == ADD_TO_WORLD
                && let Some(p) = self.world.parent(h)
                && self.world.slot(p).and_then(|ps| self.copy_root(ps)) == Some(h.index())
            {
                self.world.set_in_world(p, true);
            }
            // A creature taken out mid-throw starts afresh when it is leased again.
            if let Some(crate::behavior::Behavior::Sly2Guard(g)) = self.world.behavior_mut(h) {
                g.held = false;
                g.flight = None;
            }
            return Some(Value::Void);
        }
        if option == RAISE_FIVE
            && args.is_empty()
            && let Some(h) = self.obj(object)
        {
            self.world.raise(h, 5, alloc::vec::Vec::new());
            return Some(Value::Void);
        }
        // Object-model hook: script ID 3 (stream option 2 on every class) is the class test "is-a"
        // (object-stream.md 3.1): `((op111 x 3) 26)` asks whether x is a door. No object is of no class.
        if option == 2
            && let [arg] = args
            && let Some(want) = arg.as_i32()
        {
            let class = self.obj(object).and_then(|h| self.world.class(h));
            let is = class.is_some_and(|c| crate::world::sly2::is_a(c, want));
            return Some(Value::Bool(is));
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
        let h = self.obj(object)?;
        let jobs = self.world.sly2.as_deref()?.jobs.as_deref()?;
        let id = self.world.sly2_id(h)?;
        // Object-model hook: the read-only job and task options of missions.md 3.1: task 1268 (done) and 1269
        // (current), job 1251 (last task done) and 1252 (current). The arrival point of a job world asks 1252 to make
        // its first task current (397 in the nightclub cellar).
        let done = |t: usize| jobs.states[t] >= crate::game::sly2_jobs::state::DONE;
        // A door's option 789 (script ID 771) is a getter: its destination's level-table row (missions.md 4.3).
        if option == DOOR_ROW && self.world.class(h) == Some(crate::game::sly2_jobs::flow::EXIT_CLASS) {
            let row = jobs.exits.iter().find(|e| e.handle == h).and_then(|e| e.row);
            return Some(Value::Int(row.map_or(-1, i32::from)));
        }
        match option {
            JOB_LAST_DONE | JOB_CURRENT => {
                let j = *jobs.graph.job_ids.get(&id)?;
                return Some(Value::Bool(match option {
                    JOB_CURRENT => jobs.job == Some(j),
                    _ => jobs.graph.jobs[j].last.is_some_and(done),
                }));
            }
            TASK_DONE | TASK_CURRENT => {
                let t = *jobs.graph.task_ids.get(&id)?;
                return Some(Value::Bool(if option == TASK_CURRENT { jobs.current == Some(t) } else { done(t) }));
            }
            _ => {}
        }
        if option != opt::STATE && option != opt::PLAYED {
            return None;
        }
        let t = *jobs.graph.task_ids.get(&id)?;
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
                        l.task_events.push((ITEM_OWNED_EVENT, None));
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
/// A placement's lease methods (script IDs 1017 to 1024).
const LEASE_CAN: u16 = 1174;
/// See [`LEASE_CAN`].
const LEASE_TAKE: u16 = 1175;
/// See [`LEASE_CAN`].
const LEASE_OF: u16 = 1177;
/// The script event a lease raises on the placement and the copy.
const LEASE_EVENT: u32 = 14;
/// A fixed placement (its copy kept out of the world as a pool; classes/placement.md 2.1).
const FIXED_PLACEMENT: u16 = 1178;
/// Murray takes Bentley up to throw (script ID 939).
const HOLD_BENTLEY: u16 = 1051;
/// The in-world flag (script ID 41).
const IN_WORLD: u16 = 27;
/// A sequence's starter (script ID 643).
const SEQUENCE_STARTER: u16 = 620;
/// An object that keeps its own zone mask when one is set on it or an ancestor (clock-and-loop.md 3.1).
const KEEPS_MASK: u16 = 32;
/// See [`LEASE_CAN`].
const LEASE_LOCK: u16 = 1179;
/// See [`LEASE_CAN`].
const LEASE_UNLOCK: u16 = 1180;
/// A character's "run this command list" (script ID 441).
const RUN_LIST: u16 = 317;
/// An escort job on a list (script ID 970; guards.md 4.4).
const ESCORT: u16 = 1117;
/// A volume's "is this position inside" (script ID 108).
const VOLUME_HOLDS: u16 = 366;
/// An interaction spot's "used by" (script ID 235).
const SPOT_USE: u16 = 1071;
/// The world's current character (script ID 844).
const WORLD_CURRENT_CHARACTER: u16 = 926;
/// An object's position (script ID 34).
const OBJECT_POSITION: u16 = 21;
/// The object's own ID (script ID 28), and its alias through a copy (script ID 29).
const OBJECT_ID: u16 = 15;
const OBJECT_ALIAS: u16 = 16;
/// The parent field (script ID 32).
const OBJECT_PARENT: u16 = 19;
/// The placement leasing the object's copy (script ID 31).
const OBJECT_PLACEMENT: u16 = 18;
/// A trigger volume's first counting object inside (script ID 534).
const VOLUME_FIRST: u16 = 447;
/// Put the object in the world (script ID 39).
const ADD_TO_WORLD: u16 = 25;
/// Take the object out of the world (script ID 40).
const REMOVE_FROM_WORLD: u16 = 26;
/// Raise script event 5 on the object (script ID 12).
const RAISE_FIVE: u16 = 11;
/// A sequence's signal options (dialogue-and-voice.md 2.1): send, remove, test.
const SIGNAL_SEND: u16 = 617;
/// See [`SIGNAL_SEND`].
const SIGNAL_REMOVE: u16 = 618;
/// See [`SIGNAL_SEND`].
const SIGNAL_HAS: u16 = 619;
/// A door's destination row (missions.md 4.3).
const DOOR_ROW: u16 = 789;
/// Read-only job and task options (missions.md 3.1).
const TASK_DONE: u16 = 1268;
const TASK_CURRENT: u16 = 1269;
const JOB_LAST_DONE: u16 = 1251;
const JOB_CURRENT: u16 = 1252;

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
