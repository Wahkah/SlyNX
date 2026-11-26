//! The Sly 3 side of [`WorldHost`] (specs/sly3/scripts.md 4 to 6): what the script host does when the world is a Sly 3
//! world (`World::sly3`). It mirrors the Sly 2 host ([`super::sly2`]) and shares its option code; the Sly 3
//! interpreter (the script crate's Sly 3 evaluation) calls it once that lands. See `docs/script.md`, "Sly 3".
//!
//! - **Classes and options** as Sly 2: the world object (operation 123) is class 1; options are read and written by
//!   the executable's option table, an unset option reads as its value type's zero, and an unset procedure option
//!   (value type `0x12`, or flag `0x1` with arguments) reads as a method.
//! - **Searches** (118, 119, 121 with the full `s32` ID) look below the context first, then in its copy, then in the
//!   whole world.
//! - **Fixed objects.** The objects that 124, 125, 126 and 145 give are recognised by the options scripts read from
//!   them (scripts.md 4.3): classes 194, 198, 155 and 158, which no world stores; the host makes one of each on first
//!   use. 124 is the player-controlled object when the world has one (`World::player`), as Sly 2's 118.
//! - **Game operations**, Sly 3 numbering (scripts.md 4.3), with our working choices where the spec has none:
//!   - 103 the world slot; 104 and 105 the first in-world Sly (class 52) and Bentley (class 54), as Sly 2's 100
//!     and 101 [working assumption];
//!   - 107 the adaptive-difficulty value (0.0), 108 the game-state value (0, as in the release build);
//!   - 109 the world's current condition (the load context's condition);
//!   - 110 and 140 (meanings not established): no value;
//!   - 120 the objects found for a list of IDs (or one ID) in a context, as Sly 2's 114;
//!   - 127 the first object of a world list: no value (the world lists are not read yet);
//!   - 129 to 132 objects in a box or sphere and objects hit by a segment: none (no Sly 3 collision bodies yet);
//!   - 139 stop a sound: nothing to stop;
//!   - 146 and 147 the pickpocket items (`Sly3Level::pickpocket`).
//! - **Task methods** (class 169; specs/sly3/missions.md 3.2): scripts name script IDs 1309, 1310, 1311 and 1314
//!   to 1318, which the class option map turns into stream options 1409 (request a state: state, requester; −1 or 4
//!   makes the task current), 1410 (the same, forced), 1413 (raise script event 23 on the task with an integer),
//!   1418 to 1421 (force state 0 to 3) and 1422 (make current). The session's job system lives outside the world, so
//!   the requests are queued in `World::shared.task_calls`; event 23 is raised here. Script IDs 1280 and 1281 on Sly
//!   3 tasks are stored fields (stream 1411, 1412), not methods. The job methods (class 168, script IDs 1259 to 1263)
//!   are not run yet: their meanings are only inferred.

use alloc::vec::Vec;

use script::{ObjectId, ScriptError, Value};

use super::WorldHost;
use crate::object::{Handle, OptionValue, flags};
use crate::world::NONE;

/// Class of the object operation 124 gives (when the world has no player object).
pub const PLAYER_CLASS: i16 = 194;
/// Class of the camera object (operation 125).
pub const CAMERA_CLASS: i16 = 198;
/// Class of the object operation 126 gives.
pub const OP126_CLASS: i16 = 155;
/// Class of the object operation 145 gives.
pub const GAME_CLASS: i16 = 158;
/// The placement class (specs/sly3/object-stream.md 3).
pub const PLACEMENT_CLASS: i16 = 8;
/// Classes of the characters for operations 104 and 105: Sly, Bentley.
pub const CHARACTER_CLASSES: [i16; 2] = [52, 54];
/// The task class.
pub const TASK_CLASS: i16 = 169;
/// The script event task method 1413 raises on the task (missions.md 3.2).
pub const TASK_SIGNAL_EVENT: u32 = 23;
/// The world object's player methods (stream options; script IDs 984 and 1000).
mod world_method {
    /// The object player n controls.
    pub const CONTROLLED: u16 = 986;
    /// Whether player n's control is held.
    pub const HELD: u16 = 1001;
    /// Hand player 0's control to an object (script ID 983; characters.md 6): the safe house, a crew member, a
    /// machine.
    pub const GIVE_CONTROL: u16 = 985;
}

/// Task methods: request, forced request, signal, force state 0 to 3 (1418 to 1421), make current.
pub mod task_method {
    pub const REQUEST: u16 = 1409;
    pub const FORCE: u16 = 1410;
    pub const SIGNAL: u16 = 1413;
    pub const FORCE_LOCKED: u16 = 1418;
    pub const FORCE_CLOSED: u16 = 1421;
    pub const MAKE_CURRENT: u16 = 1422;
    /// A task's state (script ID 525; missions.md 4.1a), read from the session's mirror.
    pub const STATE: u16 = 1415;
    /// A task's mode (script ID 1280; missions.md 4.1a): 0 live play, 1 the restore or world-start step 2.
    pub const MODE: u16 = 1411;
}

/// Command-script method 747 (script ID 247; missions.md 4.1a): starts the script, with one argument (the object).
const START_SCRIPT: u16 = 747;
/// Command-script method 749 (script ID 583; interactables.md 5a, task 2241 stopping script 2233): stops the script.
const STOP_SCRIPT: u16 = 749;
/// Command-script method 748 (script ID 733; missions.md 4.1a): starts the script with its default context, player
/// 0's controlled object.
const START_SCRIPT_DEFAULT: u16 = 748;
/// A hit record (class 196; missions.md 4.1a): the object hit (script ID 556) and the hitter (276).
const HIT_CLASS: i16 = 196;
const HIT_TARGET: u16 = 1646;
const HIT_HITTER: u16 = 1644;
/// A trigger shape's member list (script ID 586, stream 479; missions.md 4.1a) and flag (script ID 43, stream 28).
const TRIGGER_CLASS: i16 = 21;
const TRIGGER_MEMBERS: u16 = 479;
const TRIGGER_FLAG: u16 = 28;
/// "Derived from class n" (script ID 3, stream 2 on any class; missions.md 4.1a), read as a method.
const DERIVED_FROM: u16 = 2;
/// Command-script method 751 (script ID 735; missions.md 4.1a, the gauntlet's buttons): sets an alarm on the script.
const SET_ALARM: u16 = 751;
/// The context object of the runner calling the module (script ID 741, stream 757; missions.md 4.1a), written.
pub const RUNNER_CONTEXT: u16 = 757;
/// A command script's context object (script ID 740, stream 756; missions.md 4.1a), the one its start wrote.
const SCRIPT_OBJECT: u16 = 756;
/// Methods 41 and 42 (stream 26 and 27, on any object; interactables.md 5a, task 2245 adding and removing photo target
/// 139578): add the object to the world, take it out.
const ADD_TO_WORLD: u16 = 26;
const REMOVE_FROM_WORLD: u16 = 27;
/// Command scripts.
const COMMAND_SCRIPT_CLASS: i16 = 135;
/// Use points, and their user (script ID 555; interactables.md 5).
const USE_POINT_CLASS: i16 = 33;
const USE_USER: u16 = 1162;
/// Tumble locks, and their start method (script ID 247; missions.md 4.1a).
const LOCK_CLASS: i16 = 79;
const LOCK_START: u16 = 1600;
/// The cell lock's part (class 47, nested in the lock; interactables.md 5) reads its owner, the lock, through script
/// ID 34 (stream 20), and its event 5 calls script ID 12 (stream 11) on it, which raises event 5 on the lock.
const PART_CLASS: i16 = 47;
const OWNER_REF: u16 = 20;
const RAISE_FIVE: u16 = 11;
/// Script ID 32 (stream 18), on any object: whether the object is the one with the given ID or a copy of it, as task
/// 2115's handler tests the lock (missions.md 4.1a).
const IS_ID: u16 = 18;
/// Script ID 825 (stream 1282), on placements: takes a new instance of the placement (missions.md 4.1a, the brawl's
/// spawners; `World::sly3_acquire`).
const TAKE_INSTANCE: u16 = 1282;
/// Script ID 35 (stream 21, two arguments), on any object: re-parents it (the new parent, flags; missions.md 4.1a).
const REPARENT: u16 = 21;
/// The screen object class (`jt`, missions.md 4.1a) and its script IDs 1149, 1150 (hide) and 1153 (a two-argument
/// setter), streams 1215, 1216, 1220.
const SCREEN_CLASS: i16 = 163;
const SCREEN_METHODS: [u16; 4] = [1215, 1216, 1217, 1220];
/// Script ID 424 (stream 297) on a guard: an integer from a vector field (missions.md 4.1a, meaning open).
const GUARD_VALUE: u16 = 297;
/// The guard class.
const GUARD_CLASS: i16 = 58;
/// Crew option 938 (stream 936): an enum on the playable characters (missions.md 4.1a, meaning open).
const CREW_STATE: u16 = 936;
/// The canal chase (missions.md 4.1a). A player record (class 49) hands out its pad, script ID 943 (stream 942): a
/// pad object (class 187) whose script IDs 1072 (stick x) and 1074 (stick size) are values, and 1078 (a button mask
/// pressed) and 1079 (a button mask held) tests; streams 1081, 1083, 1087 and 1088.
const PLAYER_RECORD_CLASS: i16 = 49;
const PAD: u16 = 942;
const PAD_CLASS: i16 = 187;
const PAD_STICK_X: u16 = 1081;
const PAD_STICK: u16 = 1083;
const PAD_PRESSED: u16 = 1087;
const PAD_HELD: u16 = 1088;
/// The key of the pad among the game's objects.
const PAD_OBJECT: u32 = 1_000_187;
/// A state machine (class 132) run on an object: script ID 226 (stream 412) gives the run (class 173), whose script
/// IDs 529 (stream 416, the state) and 532 (stream 419, the state before) are kept as options, and 248 (stream 423)
/// sets the state.
const MACHINE_CLASS: i16 = 132;
const MACHINE_RUN: u16 = 412;
const RUN_CLASS: i16 = 173;
const RUN_STATE: u16 = 416;
const RUN_PREVIOUS: u16 = 419;
const RUN_SET: u16 = 423;
/// An animation segment (class 130) applied by script ID 226 (stream 112) gives a segment run (class 171) whose
/// 248 (stream 129) is not modelled.
const SEGMENT_CLASS: i16 = 130;
const SEGMENT_APPLY: u16 = 112;
const SEGMENT_RUN_CLASS: i16 = 171;
const SEGMENT_RUN_SET: u16 = 129;
/// A reactive object's (class 111) last hitter, script ID 1105 (stream 1126).
const REACTIVE_CLASS: i16 = 111;
const LAST_HIT: u16 = 1126;

/// The PS2 pad's masks of the Switch pad's buttons (pad layout [I]): Cross 0x40, Square 0x80, R1 0x08, Triangle
/// 0x10, Circle 0x20, L1 0x04, L2 0x01, R2 0x02.
fn ps2_mask(bits: u32) -> i32 {
    use platform_api::Buttons;
    let map = [
        (Buttons::B, 0x40),
        (Buttons::Y, 0x80),
        (Buttons::ZR, 0x08),
        (Buttons::X, 0x10),
        (Buttons::A, 0x20),
        (Buttons::L, 0x04),
        (Buttons::ZL, 0x01),
        (Buttons::R, 0x02),
    ];
    map.iter().filter(|(b, _)| bits & b.0 != 0).fold(0, |m, (_, v)| m | v)
}

impl WorldHost<'_> {
    /// The root of the copy placement `slot` (class 8) made: the first object of that copy, below the placement.
    /// As in Sly 2 (specs/sly2/scripts.md 5), a placement's ID names its copy's root for scripts; Sly 3 keeps Sly 2's
    /// evaluator and options (specs/sly3/scripts.md), so the rule is assumed unchanged (the scripts of three worlds
    /// set an option class 8 does not list on objects found by placement IDs).
    fn sly3_copy_root(&self, slot: u32) -> Option<u32> {
        let l = self.world.sly3.as_deref()?;
        let h = self.world.handle(slot)?;
        if self.world.class(h)? != PLACEMENT_CLASS {
            return None;
        }
        let own = l.object(slot)?.copy;
        self.world
            .children(h)
            .filter_map(|c| self.world.slot(c))
            .filter(|&c| l.object(c).is_some_and(|o| o.copy != own))
            .min()
    }

    /// Every object with Sly 3 ID `id` (placements standing for their copies' roots): below the context, then in its
    /// copy, then the rest, each in load order.
    fn sly3_search(&self, id: i32, context: Option<ObjectId>) -> Vec<Handle> {
        let mut slots: Vec<u32> =
            self.world.slots_with_sly3_id(id).into_iter().map(|s| self.sly3_copy_root(s).unwrap_or(s)).collect();
        slots.sort_unstable();
        slots.dedup();
        let ctx = self.ctx(context);
        let copy = ctx.and_then(|c| self.world.sly3_object(c)).map(|o| o.copy);
        let level = self.world.sly3.as_deref();
        let rank = |s: u32| -> u8 {
            match ctx {
                Some(c) if self.below(s, c) => 0,
                Some(_) if copy.is_some() && level.and_then(|l| l.object(s)).map(|o| o.copy) == copy => 1,
                _ => 2,
            }
        };
        let mut ranked: Vec<(u8, u32)> = slots.into_iter().map(|s| (rank(s), s)).collect();
        ranked.sort_unstable();
        ranked.into_iter().filter_map(|(_, s)| self.world.handle(s)).collect()
    }

    pub(super) fn sly3_find(&mut self, id: i32, context: Option<ObjectId>) -> Option<ObjectId> {
        self.sly3_search(id, context).first().map(|h| h.to_script())
    }

    pub(super) fn sly3_find_all(&mut self, id: i32, context: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        out.extend(self.sly3_search(id, context).into_iter().map(Handle::to_script));
    }

    /// The object the game provides for operation `op` (made on first use, outside every group).
    fn sly3_game_object(&mut self, op: u32, class: i16) -> Option<ObjectId> {
        let existing = self.world.sly3.as_deref()?.game_objects.get(&op).copied();
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
                self.world.sly3.as_deref_mut()?.game_objects.insert(op, s);
                s
            }
        };
        self.world.handle(slot).map(Handle::to_script)
    }

    fn sly3_character(&self, class: i16) -> Option<ObjectId> {
        let s = *self
            .world
            .slots_of_class(class)
            .iter()
            .find(|&&s| self.world.handle(s).is_some_and(|h| self.world.flags(h) & flags::IN_WORLD != 0))?;
        self.world.handle(s).map(Handle::to_script)
    }

    /// Options the game computes rather than stores: the world object's methods 984 and 1000, and option 30 (stream
    /// 16): a crew member's owner code, any other object's ID
    /// (Sly 24, Bentley 25, Murray 26; the item table's owner codes, characters.md 3.1), which mission waypoints
    /// compare with their custom property 1991 (missions.md 4.2).
    pub(super) fn sly3_live_option(&mut self, object: ObjectId, option: u16) -> Option<Value> {
        const OWNER_CODE: u16 = 16;
        // The world object's player methods are methods to call (the shared read hands out methods only for
        // objects other than the world).
        if object == crate::object::WORLD_SCRIPT_ID {
            return matches!(option, world_method::CONTROLLED | world_method::HELD | world_method::GIVE_CONTROL)
                .then_some(Value::Method { object, option });
        }
        let h = self.obj(object)?;
        // A task's state, from the session's job system (missions.md 4.1a).
        if option == task_method::STATE && self.world.class(h) == Some(TASK_CLASS) {
            let id = self.world.sly3_id(h)?;
            return self.world.shared.sly3_task_states.get(&id).map(|&s| Value::Int(s as i32));
        }
        // A task's mode (mode 3, a silent change, raises no events, so scripts never see it).
        if option == task_method::MODE && self.world.class(h) == Some(TASK_CLASS) {
            return Some(Value::Int(i32::from(self.world.shared.sly3_restore_mode)));
        }
        // Whether the object is the one with an ID (or a copy of it): same object ID. Adding to and removing from the
        // world. Whether its class derives from another. Re-parenting.
        if matches!(option, IS_ID | ADD_TO_WORLD | REMOVE_FROM_WORLD | DERIVED_FROM | REPARENT) {
            return Some(Value::Method { object, option });
        }
        // A placement's new instance; the screen object's methods; the guard's value.
        let class = self.world.class(h);
        if (option == TAKE_INSTANCE && class == Some(PLACEMENT_CLASS))
            || (SCREEN_METHODS.contains(&option) && class == Some(SCREEN_CLASS))
            || (option == GUARD_VALUE && class == Some(GUARD_CLASS))
        {
            return Some(Value::Method { object, option });
        }
        // Crew option 938. Working choice (missions.md 4.1a: the enum's meaning and the guards' damage to the crew are
        // open): 0, never 2 ("down"), as the guards' strikes do not hurt the crew here.
        if option == CREW_STATE && matches!(class, Some(52..=54)) {
            return Some(Value::Int(0));
        }
        // The canal chase: the player record's pad; the pad's, the machines', runs' and segments' methods; a reactive
        // object's last hitter.
        if option == PAD && class == Some(PLAYER_RECORD_CLASS) {
            return Some(Value::Object(self.sly3_game_object(PAD_OBJECT, PAD_CLASS)));
        }
        // The stick's values are read; the buttons' tests are called.
        if class == Some(PAD_CLASS) && matches!(option, PAD_STICK_X | PAD_STICK) {
            let input = if self.world.shared.driving.is_some() { self.world.shared.input } else { Default::default() };
            let [x, y] = input.left;
            return Some(Value::Float(if option == PAD_STICK_X { x } else { crate::math::sqrt(x * x + y * y) }));
        }
        if (class == Some(PAD_CLASS) && matches!(option, PAD_PRESSED | PAD_HELD))
            || (class == Some(MACHINE_CLASS) && option == MACHINE_RUN)
            || (class == Some(RUN_CLASS) && option == RUN_SET)
            || (class == Some(SEGMENT_CLASS) && option == SEGMENT_APPLY)
            || (class == Some(SEGMENT_RUN_CLASS) && option == SEGMENT_RUN_SET)
        {
            return Some(Value::Method { object, option });
        }
        if option == LAST_HIT && class == Some(REACTIVE_CLASS) {
            let slot = self.world.slot(h)?;
            let by = self.world.shared.sly3_last_hit.get(&slot).copied();
            return Some(Value::Object(by.map(Handle::to_script)));
        }
        // A trigger shape's members (the tracked objects inside) and its flag. Working reading: the flag is whether
        // the shape is in the world (its meaning is not given).
        if self.world.class(h) == Some(TRIGGER_CLASS) && matches!(option, TRIGGER_MEMBERS | TRIGGER_FLAG) {
            if option == TRIGGER_FLAG {
                return Some(Value::Bool(self.world.in_world(h)));
            }
            let members = match self.world.behavior(h) {
                Some(crate::behavior::Behavior::Sly2Volume(t)) => t.members.clone(),
                _ => Vec::new(),
            };
            return Some(Value::list(members.into_iter().map(|m| Value::Object(Some(m.to_script())))));
        }
        // A hit record's object hit and hitter.
        if matches!(option, HIT_TARGET | HIT_HITTER) && self.world.class(h) == Some(HIT_CLASS) {
            let slot = self.world.slot(h)?;
            let &(target, hitter) = self.world.shared.sly3_hits.get(&slot)?;
            let o = if option == HIT_TARGET { target } else { hitter };
            return Some(Value::Object(Some(o.to_script())));
        }
        // A command script's start method, a lock's start and event-5 methods.
        if (matches!(option, START_SCRIPT | START_SCRIPT_DEFAULT | STOP_SCRIPT | SET_ALARM)
            && self.world.class(h) == Some(COMMAND_SCRIPT_CLASS))
            || (matches!(option, LOCK_START | RAISE_FIVE) && self.world.class(h) == Some(LOCK_CLASS))
        {
            return Some(Value::Method { object, option });
        }
        // The lock part's owner (interactables.md 5, the cell lock): the first enclosing object that is not a
        // placement (working choice).
        if option == OWNER_REF
            && matches!(self.world.class(h), Some(PART_CLASS | COMMAND_SCRIPT_CLASS))
            && self.world.options(h).and_then(|o| o.get(OWNER_REF)).is_none()
        {
            let owner = core::iter::successors(self.world.parent(h), |&q| self.world.parent(q))
                .find(|&q| self.world.class(q) != Some(PLACEMENT_CLASS));
            return Some(Value::Object(owner.map(Handle::to_script)));
        }
        // A command script's object, from the session's mirror of the running scripts.
        if option == SCRIPT_OBJECT && self.world.class(h) == Some(COMMAND_SCRIPT_CLASS) {
            let id = self.world.sly3_id(h)?;
            let with = self.world.shared.sly3_script_objects.get(&id).copied().filter(|&o| o >= 0);
            return Some(Value::Object(with.and_then(|o| self.sly3_find(o, None))));
        }
        // A use point's user, stored when it was used.
        if option == USE_USER && self.world.class(h) == Some(USE_POINT_CLASS) {
            let slot = self.world.slot(h)?;
            let user = self.world.shared.sly3_use_users.get(&slot).copied();
            return Some(Value::Object(user.map(Handle::to_script)));
        }
        // Value types 0x2000 + c hold the ID of an object of class c: scripts get the object (as Sly 2's read
        // does), found by Sly 3 ID from the object's copy.
        if option != OWNER_CODE {
            let e = self.entry(option)?;
            if !(0x2000..0x3000).contains(&e.value_type) || e.is_typed() {
                return None;
            }
            let id = self.world.options(h).and_then(|o| o.get(option)).and_then(OptionValue::as_i32)?;
            return Some(Value::Object(if id < 0 { None } else { self.sly3_find(id, Some(object)) }));
        }
        let code = match self.world.class(h)? {
            52 => 24,
            54 => 25,
            53 => 26,
            // Working reading [I]: on any other object it is the object's ID, as the crew's codes are their IDs and
            // 2291's module compares a task's with task IDs; v_interpol's 88261 finds its caller script by it.
            _ => self.world.sly3_id(h)?,
        };
        Some(Value::Int(code))
    }

    pub(super) fn sly3_player(&mut self) -> Option<ObjectId> {
        match self.world.player.filter(|&p| self.world.contains(p)) {
            Some(p) => Some(p.to_script()),
            None => self.sly3_game_object(124, PLAYER_CLASS),
        }
    }

    pub(super) fn sly3_camera(&mut self) -> Option<ObjectId> {
        self.sly3_game_object(125, CAMERA_CLASS)
    }

    /// The task methods (module documentation), queued for the session's job system. `None` for any other option
    /// or class.
    pub(super) fn sly3_call_action(&mut self, object: ObjectId, option: u16, args: &[Value]) -> Option<Value> {
        use crate::world::shared::TaskCall;
        use task_method::*;
        // The world object's player methods (missions.md 4.2), for player 0: 984 (stream 986) the object it
        // controls, 1000 (stream 1001) whether its control is held.
        if object == crate::object::WORLD_SCRIPT_ID {
            return match option {
                world_method::CONTROLLED => Some(Value::Object(self.sly3_player())),
                world_method::HELD => Some(Value::Bool(self.world.shared.sly3_holds > 0)),
                world_method::GIVE_CONTROL => {
                    // The object among the arguments (the player index, when given, is 0 here).
                    if let Some(h) = args.iter().find_map(|a| match a {
                        Value::Object(Some(o)) => self.obj(*o),
                        _ => None,
                    }) {
                        self.world.shared.sly3_control_to.push(h);
                    }
                    Some(Value::Void)
                }
                _ => None,
            };
        }
        if option == IS_ID {
            let h = self.obj(object)?;
            let id = args.first().and_then(|a| a.as_i32());
            return Some(Value::Bool(id.is_some() && self.world.sly3_id(h) == id));
        }
        if option == DERIVED_FROM {
            let h = self.obj(object)?;
            let class = self.world.class(h)?;
            let base = args.first().and_then(|a| a.as_i32()).unwrap_or(-1) as i16;
            let derived =
                class == base || self.world.shared.sly3_derived.get(&base).is_some_and(|d| d.contains(&class));
            return Some(Value::Bool(derived));
        }
        if matches!(option, ADD_TO_WORLD | REMOVE_FROM_WORLD) {
            let h = self.obj(object)?;
            self.world.set_in_world(h, option == ADD_TO_WORLD);
            // A placement and its copy are one object in the game (a search for the placement's ID finds the copy's
            // root, scripts.md 5), so adding the root adds its placement too when the placement was taken out at the
            // start (option 27 on the placement record; v_canal's enemy boats, 139842 and on, missions.md 4.1a).
            if option == ADD_TO_WORLD
                && let Some(p) = self.world.parent(h)
                && self.world.class(p) == Some(PLACEMENT_CLASS)
                && self.world.slot(p).and_then(|s| self.sly3_copy_root(s)) == self.world.slot(h)
            {
                self.world.set_in_world(p, true);
            }
            return Some(Value::Void);
        }
        // A placement's new instance (none when its pool has none free).
        if option == TAKE_INSTANCE {
            let h = self.obj(object)?;
            return Some(Value::Object(self.world.sly3_acquire(h).map(Handle::to_script)));
        }
        // Re-parenting: the new parent (none: the top level); flags bits 0 and 1 keep the world transform, as Sly 2's
        // set-parent action does.
        if option == REPARENT {
            let h = self.obj(object)?;
            let parent = match args.first() {
                Some(Value::Object(Some(o))) => self.obj(*o),
                _ => None,
            };
            let keep = args.get(1).and_then(|a| a.as_i32()).unwrap_or(0) & 3 != 0;
            self.world.reparent(h, parent, keep);
            return Some(Value::Void);
        }
        // Not modelled (missions.md 4.1a): the screen object's methods (the Sly 3 2D layer does not draw it), and the
        // guard's value 424 (meaning open; working choice 0).
        if SCREEN_METHODS.contains(&option) {
            return Some(Value::Void);
        }
        if option == GUARD_VALUE {
            return Some(Value::Int(0));
        }
        // The pad (missions.md 4.1a): player 0's, while the player drives a machine (control handed to the player
        // record); idle otherwise. "Pressed within a time" reads as pressed this frame [I].
        if matches!(option, PAD_PRESSED | PAD_HELD) {
            let driving = self.world.shared.driving.is_some();
            let input = if driving { self.world.shared.input } else { Default::default() };
            let mask = args.first().and_then(|a| a.as_i32()).unwrap_or(0);
            let bits = if option == PAD_PRESSED { input.pressed } else { input.buttons };
            return Some(Value::Bool(ps2_mask(bits) & mask != 0));
        }
        // A machine run on an object: one per machine (made on first use, out of the world), its state set from
        // the call (the object, the state, a flag).
        if option == MACHINE_RUN {
            let machine = self.obj(object)?;
            let key = 2_000_000_000 + self.world.slot(machine)?;
            let run = self.sly3_game_object(key, RUN_CLASS)?;
            let rh = self.obj(run)?;
            if let Some(state) = args.get(1).and_then(|a| a.as_i32())
                && let Some(o) = self.world.options_mut(rh)
            {
                o.set(RUN_STATE, OptionValue::Int(state));
            }
            return Some(Value::Object(Some(run)));
        }
        if option == RUN_SET {
            let rh = self.obj(object)?;
            let state = args.first().and_then(|a| a.as_i32()).unwrap_or(-1);
            if let Some(o) = self.world.options_mut(rh) {
                let before = o.get(RUN_STATE).and_then(OptionValue::as_i32).unwrap_or(-1);
                o.set(RUN_PREVIOUS, OptionValue::Int(before));
                o.set(RUN_STATE, OptionValue::Int(state));
            }
            return Some(Value::Void);
        }
        // An animation segment applied by a script: its run, whose controls are not modelled (the segment itself
        // is not played this way; `0x16` plays segments).
        if option == SEGMENT_APPLY {
            let seg = self.obj(object)?;
            let key = 2_100_000_000 + self.world.slot(seg)?;
            return Some(Value::Object(self.sly3_game_object(key, SEGMENT_RUN_CLASS)));
        }
        if option == SEGMENT_RUN_SET {
            return Some(Value::Void);
        }
        // Event 5 raised on the lock (its part forwards its own).
        if option == RAISE_FIVE {
            let h = self.obj(object)?;
            if self.world.class(h)? != LOCK_CLASS {
                return None;
            }
            self.world.raise(h, 5, Vec::new());
            return Some(Value::Void);
        }
        // A tumble lock started with its user, queued for the session (missions.md 4.1a).
        if option == LOCK_START {
            let h = self.obj(object)?;
            if self.world.class(h)? != LOCK_CLASS {
                return None;
            }
            let user = match args.first() {
                Some(Value::Object(Some(o))) => self.obj(*o),
                _ => None,
            };
            self.world.shared.sly3_lock_starts.push((h, user));
            return Some(Value::Void);
        }
        // An alarm set on a command script (the gauntlet's buttons on script 2291).
        if option == SET_ALARM {
            let h = self.obj(object)?;
            if self.world.class(h)? != COMMAND_SCRIPT_CLASS {
                return None;
            }
            let script = self.world.sly3_id(h)?;
            if let Some(alarm) = args.first().and_then(|a| a.as_i32()) {
                self.world.shared.sly3_alarms.push((script, alarm));
            }
            return Some(Value::Void);
        }
        // A command script stopped by a script, queued for the session's runner.
        if option == STOP_SCRIPT {
            let h = self.obj(object)?;
            if self.world.class(h)? != COMMAND_SCRIPT_CLASS {
                return None;
            }
            let script = self.world.sly3_id(h)?;
            self.world.shared.sly3_script_stops.push(script);
            return Some(Value::Void);
        }
        // A command script started with its default context.
        if option == START_SCRIPT_DEFAULT {
            let h = self.obj(object)?;
            if self.world.class(h)? != COMMAND_SCRIPT_CLASS {
                return None;
            }
            let script = self.world.sly3_id(h)?;
            let with = self.world.player.and_then(|p| self.world.sly3_id(p)).unwrap_or(-1);
            let starts = &mut self.world.shared.sly3_script_starts;
            if !starts.iter().any(|&(_, _, o)| o == h) {
                starts.push((script, with, h));
            }
            return Some(Value::Void);
        }
        // A command script started by a script, queued for the session's runner (missions.md 4.1a).
        if option == START_SCRIPT {
            let h = self.obj(object)?;
            if self.world.class(h)? != COMMAND_SCRIPT_CLASS {
                return None;
            }
            let script = self.world.sly3_id(h)?;
            let with = match args.first() {
                Some(Value::Object(Some(o))) => self.obj(*o).and_then(|o| self.world.sly3_id(o)).unwrap_or(-1),
                _ => -1,
            };
            // One start per script object per frame, however often a script loops over the call.
            let starts = &mut self.world.shared.sly3_script_starts;
            if !starts.iter().any(|&(_, _, o)| o == h) {
                starts.push((script, with, h));
            }
            return Some(Value::Void);
        }
        if !matches!(option, REQUEST | FORCE | SIGNAL | MAKE_CURRENT)
            && !(FORCE_LOCKED..=FORCE_CLOSED).contains(&option)
        {
            return None;
        }
        let h = self.obj(object)?;
        if self.world.class(h)? != TASK_CLASS {
            return None;
        }
        let task = self.world.sly3_id(h)?;
        let state = args.first().and_then(|a| a.as_i32()).unwrap_or(-1);
        let call = match option {
            REQUEST | FORCE => Some(TaskCall::Request { task, state, forced: option == FORCE }),
            MAKE_CURRENT => Some(TaskCall::MakeCurrent(task)),
            SIGNAL => {
                // Raises script event 23 on the task with the integer (missions.md 3.2).
                let n = args.first().and_then(|a| a.as_i32()).unwrap_or(0);
                self.world.raise(h, TASK_SIGNAL_EVENT, alloc::vec![Value::Int(n)]);
                None
            }
            o => Some(TaskCall::Force { task, state: (o - FORCE_LOCKED) as i8 }),
        };
        if let Some(c) = call {
            self.world.shared.task_calls.push(c);
        }
        Some(Value::Void)
    }

    pub(super) fn sly3_game_operation(&mut self, op: u32, args: &[Value]) -> Value {
        match op {
            103 => Value::Int(self.world.info.index as i32),
            104 | 105 => Value::Object(self.sly3_character(CHARACTER_CLASSES[(op - 104) as usize])),
            107 => Value::Float(0.0),
            108 => Value::Int(0),
            109 => Value::Int(self.world.sly3.as_deref().map_or(-1, |l| l.context.condition)),
            110 | 140 | 127 | 139 => Value::Void,
            120 => {
                let ctx = args.get(1).and_then(|a| a.as_object()).flatten();
                let ids: Vec<i32> = match args.first() {
                    Some(v @ Value::List(_)) => v.to_vec().iter().filter_map(|x| x.as_i32()).collect(),
                    Some(v) => v.as_i32().into_iter().collect(),
                    None => Vec::new(),
                };
                let found: Vec<Value> =
                    ids.into_iter().filter_map(|id| self.sly3_find(id, ctx)).map(|o| Value::Object(Some(o))).collect();
                Value::list(found)
            }
            124 => Value::Object(self.sly3_player()),
            125 => Value::Object(self.sly3_camera()),
            126 => Value::Object(self.sly3_game_object(126, OP126_CLASS)),
            145 => Value::Object(self.sly3_game_object(145, GAME_CLASS)),
            129..=131 => Value::Nil,
            132 => Value::Object(None),
            146 => {
                let id = args.first().and_then(|a| a.as_i32()).unwrap_or(-1);
                Value::Bool(self.world.sly3.as_deref().is_some_and(|l| l.pickpocket.contains(&id)))
            }
            147 => {
                let id = args.first().and_then(|a| a.as_i32()).unwrap_or(-1);
                let on = args.get(1).is_none_or(|v| !matches!(v, Value::Bool(false) | Value::Int(0) | Value::Nil));
                if let Some(l) = self.world.sly3.as_deref_mut() {
                    if on {
                        l.pickpocket.insert(id);
                    } else {
                        l.pickpocket.remove(&id);
                    }
                }
                Value::Void
            }
            _ => Value::Error(ScriptError::Host(op)),
        }
    }
}
