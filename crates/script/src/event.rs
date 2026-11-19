//! The fixed event symbols 0 to 29 (scripts.md section 6). Names are descriptive names of our own.

/// After the world has loaded, once per object.
pub const LOADED: u32 = 0;
/// Every frame while a placed object updates (argument: frame time).
pub const UPDATE: u32 = 1;
/// Triggered (button pressed, alarm sounded, pickup collected, ...).
pub const TRIGGERED: u32 = 2;
/// Untriggered (released, switched off, left).
pub const UNTRIGGERED: u32 = 3;
/// Added to the world.
pub const ADDED: u32 = 4;
/// Removed from the world.
pub const REMOVED: u32 = 5;
/// Started touching another solid (argument: the other object).
pub const TOUCH_BEGIN: u32 = 6;
/// Stopped touching (argument: the other object).
pub const TOUCH_END: u32 = 7;
/// Never raised.
pub const UNUSED_8: u32 = 8;
/// An animation segment was applied (argument: the playback).
pub const SEGMENT_APPLIED: u32 = 9;
/// An animation event of type 40 was crossed (arguments: the playback, the 16-bit value).
pub const SEGMENT_MARKER: u32 = 10;
/// An animation playback was removed (argument: its root object).
pub const SEGMENT_REMOVED: u32 = 11;
/// A trigger-volume button updated.
pub const VOLUME_UPDATE: u32 = 12;
/// Fired a projectile (argument: the projectile).
pub const FIRED: u32 = 13;
/// A state machine entered a state (arguments: the run, from-state OID, to-state OID).
pub const STATE_ENTERED: u32 = 14;
/// A guard changed behaviour state (arguments: old state, new state).
pub const GUARD_STATE: u32 = 15;
/// A dialogue reached a "call script" step (argument: label OID).
pub const DIALOGUE_CALL: u32 = 16;
/// A state machine started (argument: the run).
pub const MACHINE_STARTED: u32 = 17;
/// A state machine stopped.
pub const MACHINE_STOPPED: u32 = 18;
/// The player hiding in a hiding place took damage (argument: the damage).
pub const HIDING_DAMAGE: u32 = 19;
/// A breakable took a hit (argument: hit count so far).
pub const HIT: u32 = 20;
/// A dialogue finished.
pub const DIALOGUE_DONE: u32 = 21;
/// A trigger checked against the save state.
pub const SAVED_TRIGGER: u32 = 22;
/// A jump guide started.
pub const JUMP_START: u32 = 23;
/// A jump guide reached a label (argument: label value).
pub const JUMP_LABEL: u32 = 24;
/// A jump guide ended.
pub const JUMP_END: u32 = 25;
/// A rhythm sequence started.
pub const RHYTHM_START: u32 = 26;
/// A rhythm sequence reached a command (argument: value).
pub const RHYTHM_COMMAND: u32 = 27;
/// A rhythm sequence stopped.
pub const RHYTHM_STOP: u32 = 28;
/// A barrier was hit by an object (argument: the object).
pub const BARRIER_HIT: u32 = 29;
/// Number of fixed event symbols.
pub const COUNT: u32 = 30;
