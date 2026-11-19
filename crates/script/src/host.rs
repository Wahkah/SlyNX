//! The engine boundary: everything a built-in operation does to the world goes through [`ScriptHost`].

use alloc::vec::Vec;

use crate::value::{ObjectId, ScriptError, Value};

/// An event the host raised while the interpreter was calling it (for example, setting an option made an object
/// trigger). The interpreter polls these after each call into the host and raises them at once, so events stay
/// synchronous (scripts.md section 6).
#[derive(Clone, Debug)]
pub struct RaisedEvent {
    /// The object raising the event.
    pub object: ObjectId,
    /// Event number (0 to 29).
    pub event: u32,
    /// The event's extra arguments (the object itself is added as the first argument by the interpreter).
    pub args: Vec<Value>,
}

/// What the game engine provides to scripts.
///
/// Objects are named by [`ObjectId`]s of the host's choosing; `None` stands for the null object (or, for
/// coordinate conversions, the world). Option IDs passed to the host are **stream** option IDs (option-encodings.md):
/// the interpreter has already mapped the script option ID through the object's class. Custom properties (script
/// option IDs from 1173 up) are kept by the interpreter and never reach the host.
///
/// Every method has a default that does nothing (finders find nothing, getters give an error value), so a host can
/// start small.
pub trait ScriptHost {
    /// The class ID of an object (object-stream.md section 6), used to map script option IDs.
    fn class_of(&self, object: ObjectId) -> Option<i16> {
        let _ = object;
        None
    }

    /// Reads a stream option of an object. Action options never come here (they become methods).
    fn get_option(&mut self, object: ObjectId, option: u16) -> Value {
        let _ = object;
        Value::Error(ScriptError::Host(option as u32))
    }

    /// Writes a stream option, with the same effect as the option appearing in the object's option list.
    fn set_option(&mut self, object: ObjectId, option: u16, value: &Value) -> Result<(), ScriptError> {
        let _ = (object, value);
        Err(ScriptError::Host(option as u32))
    }

    /// Performs an action option (an option whose encoding is a typed argument list) with arguments.
    fn call_action(&mut self, object: ObjectId, option: u16, args: &[Value]) -> Value {
        let _ = (object, args);
        Value::Error(ScriptError::Host(option as u32))
    }

    /// Whether a stream option is an action. No longer consulted: the interpreter decides from the `formats`
    /// encoding table and [`crate::interp::UNLISTED_ACTIONS`] (scripts.md 11, item 4). Kept so existing hosts
    /// still build.
    fn is_action(&self, option: u16) -> bool {
        let _ = option;
        false
    }

    /// Operation 103: the object with an OID, in the whole world (`context` null) or below `context`. For an OID
    /// that belongs to a proxy, the first object the proxy placed (its first source or clone, in placement order);
    /// a source whose own proxy has the asked OID is found only through that proxy (scripts.md 11, item 3).
    fn find_object(&mut self, oid: i16, context: Option<ObjectId>) -> Option<ObjectId> {
        let _ = (oid, context);
        None
    }

    /// Operation 104: every object with an OID (the interpreter keeps the first 64).
    fn find_objects(&mut self, oid: i16, context: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        let _ = (oid, context, out);
    }

    /// Operation 105: the object with an OID nearest to `context` in the hierarchy (object-stream.md 7.4).
    fn find_nearest(&mut self, oid: i16, context: Option<ObjectId>) -> Option<ObjectId> {
        self.find_object(oid, context)
    }

    /// Operation 106: every nearest object with an OID (the interpreter keeps the first 64).
    fn find_all_nearest(&mut self, oid: i16, context: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        self.find_objects(oid, context, out)
    }

    /// Sly 2 wide-ID hook: operations 112 and 115 (`nearest`) with an `s32` object ID. The default uses the 16-bit
    /// search when the ID fits.
    fn find_object_id(&mut self, id: i32, context: Option<ObjectId>, nearest: bool) -> Option<ObjectId> {
        let oid = i16::try_from(id).ok()?;
        if nearest { self.find_nearest(oid, context) } else { self.find_object(oid, context) }
    }

    /// Sly 2 wide-ID hook: operation 113 (and the all-nearest search) with an `s32` object ID.
    fn find_objects_id(&mut self, id: i32, context: Option<ObjectId>, nearest: bool, out: &mut Vec<ObjectId>) {
        let Ok(oid) = i16::try_from(id) else { return };
        if nearest { self.find_all_nearest(oid, context, out) } else { self.find_objects(oid, context, out) }
    }

    /// Operation 110: every object of a class (the interpreter keeps the first 256).
    fn find_by_class(&mut self, class: i16, context: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        let _ = (class, context, out);
    }

    /// Operation 112: the solid objects overlapping a sphere.
    fn objects_in_sphere(&mut self, centre: [f32; 3], radius: f32, out: &mut Vec<ObjectId>) {
        let _ = (centre, radius, out);
    }

    /// Operation 107: the player-controlled object.
    fn player(&mut self) -> Option<ObjectId> {
        None
    }

    /// Operation 108: the world object.
    fn world(&mut self) -> Option<ObjectId> {
        None
    }

    /// Operation 109: the camera object.
    fn camera(&mut self) -> Option<ObjectId> {
        None
    }

    /// Operation 115: a position in `from`'s space converted to `to`'s space (`None`: the world).
    fn convert_point(&mut self, point: [f32; 3], from: Option<ObjectId>, to: Option<ObjectId>) -> [f32; 3] {
        let _ = (from, to);
        point
    }

    /// Operation 116: a direction converted the same way (rotation only).
    fn convert_direction(&mut self, dir: [f32; 3], from: Option<ObjectId>, to: Option<ObjectId>) -> [f32; 3] {
        let _ = (from, to);
        dir
    }

    /// Operation 98: postpone an object's updates by `seconds`.
    fn postpone_updates(&mut self, object: ObjectId, seconds: f32) {
        let _ = (object, seconds);
    }

    /// Operation 119: play a sound effect, from an emitting object or none.
    fn play_sound(&mut self, sound: i32, emitter: Option<ObjectId>) {
        let _ = (sound, emitter);
    }

    /// Operation 121: controller rumble.
    fn rumble(&mut self, kind: i32, strength: f32) {
        let _ = (kind, strength);
    }

    /// Operation 122: a puff of smoke.
    fn smoke(&mut self, position: [f32; 3], size: f32) {
        let _ = (position, size);
    }

    /// Operation 96: the game clock in seconds.
    fn game_time(&self) -> f32 {
        0.0
    }

    /// A random number in [0, 1), from the game's deterministic generator (operation 53 and state machines).
    fn random(&mut self) -> f32 {
        0.0
    }

    /// The world's downward acceleration in game units per second squared (physics.md 2: 1500).
    fn gravity(&self) -> f32 {
        1500.0
    }

    /// Downward acceleration of a launching object (operation 71's fourth argument), or the world's when there is
    /// none (scripts.md 11, item 10).
    fn object_gravity(&self, object: Option<ObjectId>) -> f32 {
        let _ = object;
        self.gravity()
    }

    /// Operations whose meaning belongs to the game, with their arguments and the game's own numbering: Sly 2's
    /// [`crate::sly2::HOST_OPERATIONS`] (level row and episode, current job and tasks, difficulty help, the
    /// game-state integer, searches by a list of IDs, box and segment queries, saved progress, the HUD prompt,
    /// owned items) and Sly 3's [`crate::sly3::HOST_OPERATIONS`]. The default refuses them.
    fn game_operation(&mut self, op: u32, args: &[Value]) -> Value {
        let _ = args;
        Value::Error(ScriptError::Host(op))
    }

    /// Operation 4 (no effect in play; a debug host may log the value).
    fn print(&mut self, value: &Value) {
        let _ = value;
    }

    /// Called once for every error value an operation creates (not again when the value is passed on).
    fn report(&mut self, error: &ScriptError) {
        let _ = error;
    }

    /// The next event the host raised during the last call into it, if any (see [`RaisedEvent`]).
    fn poll_event(&mut self) -> Option<RaisedEvent> {
        None
    }
}

/// A host that does nothing (every default).
#[derive(Clone, Copy, Debug, Default)]
pub struct NullHost;

impl ScriptHost for NullHost {}
