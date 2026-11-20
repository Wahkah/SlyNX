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
use crate::object::{Handle, flags};
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
}

/// Task methods: request, forced request, signal, force state 0 to 3 (1418 to 1421), make current.
pub mod task_method {
    pub const REQUEST: u16 = 1409;
    pub const FORCE: u16 = 1410;
    pub const SIGNAL: u16 = 1413;
    pub const FORCE_LOCKED: u16 = 1418;
    pub const FORCE_CLOSED: u16 = 1421;
    pub const MAKE_CURRENT: u16 = 1422;
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
                _ => None,
            };
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
