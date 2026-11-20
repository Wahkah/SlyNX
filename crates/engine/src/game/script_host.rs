//! The world as the scripts see it: [`WorldHost`] implements the script crate's `ScriptHost` (scripts.md 7 and 8).
//!
//! Script objects are world handles packed into the script crate's `ObjectId`; the world object (operation 108) has
//! its own ID and reads and writes the world option list. Option values are converted by the option's encoding
//! (option-encodings.md): bytes become booleans, `s16` and `s32` integers, pairs ranges, four floats a list.

use alloc::vec::Vec;

use formats::options::OptionEncoding;
use script::{ObjectId, RaisedEvent, ScriptError, ScriptHost, Value};

mod sly2; // Sly 2 hook: the Sly 2 host (script_host/sly2.rs).
mod sly3; // Sly 3 hook: the Sly 3 host (script_host/sly3.rs).

use super::rng::Rng;
use super::systems::CollisionQueries;
use crate::object::{Arg, Handle, OptionValue, WORLD_SCRIPT_ID, class::id};
use crate::world::{Request, World};

/// Stream options no option list stores but scripts use as actions (scripts.md 11.5).
pub const ACTIONS: &[u16] = &[
    4, 9, 27, 86, 87, 90, 94, 107, 108, 117, 118, 124, 126, 162, 174, 197, 198, 199, 200, 207, 228, 282, 296, 311, 312,
    315, 316, 362, 363, 364, 439, 481, 482, 489, 490, 522, 546, 553, 554, 562, 568, 569, 570, 571, 574, 575, 577, 578,
    579, 581, 595, 598, 599, 600, 606, 607, 613, 622, 647, 657, 663, 667, 677, 721, 722, 732, 733, 734, 737, 738, 739,
    740, 743, 744, 745, 751, 821, 822, 827, 840, 888, 889, 911, 912, 913, 914, 917, 928, 939, 974, 975, 1001, 1004,
    1016, 1020, 1026, 1087, 1088, 1091, 1113, 1115, 1116, 1117, 1132, 1143, 1144, 1147, 1152, 1153, 1156, 1157, 1158,
    1168, 1171, 1174, 1175, 1178,
];

/// Run-time options of placed objects (options-by-class.md 1.1).
pub mod live {
    /// Frozen (read only).
    pub const FROZEN: u16 = 28;
    /// Local position (writing moves the object).
    pub const POSITION: u16 = 29;
    /// Original (loaded) position.
    pub const ORIGINAL_POSITION: u16 = 30;
    /// World position (read only).
    pub const WORLD_POSITION: u16 = 31;
    /// Local rotation (writing rotates the object).
    pub const ROTATION: u16 = 32;
    /// Original (loaded) rotation.
    pub const ORIGINAL_ROTATION: u16 = 33;
    /// World matrix (read only).
    pub const WORLD_MATRIX: u16 = 34;
    /// Euler angles of the local rotation.
    pub const EULER: u16 = 35;
    /// Velocity in local axes.
    pub const VELOCITY: u16 = 36;
    /// Angular velocity.
    pub const ANGULAR: u16 = 37;
    /// Linear and angular acceleration (read only).
    pub const ACCEL: u16 = 38;
    /// See [`ACCEL`].
    pub const ANGULAR_ACCEL: u16 = 39;
    /// Dropped pickups' state (0 normal .. 7 attracted).
    pub const PICKUP_STATE: u16 = 981;
    /// The world's adaptive difficulty, 0 to 1 (world option; script option 634).
    pub const DIFFICULTY: u16 = 547;
    /// A segment's duration (class 131, read only).
    pub const SEGMENT_DURATION: u16 = 188;
    /// Object references: stored as the object's OID, read as the object, and the null object while unset.
    pub const REFERENCES: [u16; 5] = [205, 844, 848, 880, 1084];
    /// Mass, a float that scripts also write as an integer.
    pub const MASS: u16 = 127;
}

fn matrix(a: &formats::geometry::Affine) -> Value {
    Value::Matrix(alloc::boxed::Box::new(a.to_mat4()))
}

fn affine_of(v: &Value) -> Option<formats::geometry::Affine> {
    let Value::Matrix(m) = v else { return None };
    Some(formats::geometry::Affine {
        cols: [[m[0], m[1], m[2]], [m[4], m[5], m[6]], [m[8], m[9], m[10]]],
        t: [m[12], m[13], m[14]],
    })
}

/// Euler angles (x, y, z) of `R = Rz · Ry · Rx` (animation.md 4.1).
pub fn euler_of(c: &[[f32; 3]; 3]) -> [f32; 3] {
    let y = crate::math::atan2(-c[0][2], crate::math::sqrt(c[0][0] * c[0][0] + c[0][1] * c[0][1]));
    let z = crate::math::atan2(c[0][1], c[0][0]);
    let x = crate::math::atan2(c[1][2], c[2][2]);
    [x, y, z]
}

/// Downward gravity of the world, units per second squared (physics.md 2), for operation 71.
pub const WORLD_GRAVITY: f32 = 1500.0;

/// The world behind the script interpreter, for one call into it.
pub struct WorldHost<'a> {
    /// The world.
    pub world: &'a mut World,
    /// The game's random numbers.
    pub rng: &'a mut Rng,
    /// Game time, seconds.
    pub time: f32,
    /// Collision queries (operation 112).
    pub collision: &'a dyn CollisionQueries,
    /// Error values reported by operations during this call.
    pub errors: u32,
    /// Each kind of error reported, with its count (for diagnostics).
    pub kinds: Vec<(ScriptError, u32)>,
}

/// The world handle of a script object ID (`None` for the null object, the world object or a stale object).
pub fn handle_of(world: &World, id: ObjectId) -> Option<Handle> {
    let (slot, gen_bits) = Handle::split_script(id)?;
    let h = world.handle(slot)?;
    (h.generation() & 0xFFF == gen_bits).then_some(h)
}

/// An option value as a script value.
pub fn to_value(v: &OptionValue) -> Value {
    match v {
        OptionValue::Byte(b) => Value::Bool(*b != 0),
        OptionValue::Short(s) => Value::Int(*s as i32),
        OptionValue::Float(f) => Value::Float(*f),
        OptionValue::Int(i) => Value::Int(*i),
        OptionValue::Color(c) => Value::Int(i32::from_le_bytes(*c)),
        OptionValue::Pair(p) => Value::Range(*p),
        OptionValue::Vector(v) => Value::Vector(*v),
        OptionValue::Quad(q) => Value::list(q.iter().map(|&x| Value::Float(x))),
        OptionValue::Args(_) => Value::Void,
    }
}

/// The "unset" value of an encoding: false, 0, 0.0, zero vectors.
pub fn zero_value(enc: OptionEncoding) -> Value {
    match enc {
        OptionEncoding::Byte => Value::Bool(false),
        OptionEncoding::Short | OptionEncoding::Int | OptionEncoding::Color => Value::Int(0),
        OptionEncoding::Float => Value::Float(0.0),
        OptionEncoding::Pair => Value::Range([0.0; 2]),
        OptionEncoding::Vector => Value::Vector([0.0; 3]),
        OptionEncoding::Quad => Value::list([0.0f32; 4].iter().map(|&x| Value::Float(x))),
        OptionEncoding::Args(_) => Value::Void,
    }
}

fn num(v: &Value) -> Option<f32> {
    match v {
        Value::Bool(b) => Some(*b as i32 as f32),
        _ => v.as_f32(),
    }
}

impl WorldHost<'_> {
    fn obj(&self, id: ObjectId) -> Option<Handle> {
        handle_of(self.world, id)
    }

    fn ctx(&self, context: Option<ObjectId>) -> Option<Handle> {
        context.filter(|&c| c != WORLD_SCRIPT_ID).and_then(|c| self.obj(c))
    }

    /// A script value converted to an option of encoding `enc`.
    fn to_option(&self, enc: OptionEncoding, v: &Value) -> Option<OptionValue> {
        Some(match enc {
            OptionEncoding::Byte => OptionValue::Byte(match v {
                Value::Bool(b) => *b as u8,
                _ => num(v)? as i32 as u8,
            }),
            OptionEncoding::Short => OptionValue::Short(match v {
                // An OID option set from an object takes the object's OID.
                Value::Object(Some(o)) => self.world.oid(self.obj(*o)?)?,
                _ => num(v)? as i32 as i16,
            }),
            OptionEncoding::Float => OptionValue::Float(num(v)?),
            OptionEncoding::Int => OptionValue::Int(match v {
                Value::Float(f) => *f as i32,
                _ => num(v)? as i32,
            }),
            OptionEncoding::Color => OptionValue::Color((num(v)? as i32).to_le_bytes()),
            OptionEncoding::Pair => match v {
                Value::Range(r) => OptionValue::Pair(*r),
                _ => return None,
            },
            OptionEncoding::Vector => match v {
                Value::Vector(x) | Value::Coeffs(x) | Value::Smoothing(x) => OptionValue::Vector(*x),
                _ => return None,
            },
            OptionEncoding::Quad => {
                let l = v.to_vec();
                if l.len() != 4 {
                    return None;
                }
                OptionValue::Quad([num(&l[0])?, num(&l[1])?, num(&l[2])?, num(&l[3])?])
            }
            OptionEncoding::Args(_) => return None,
        })
    }

    fn to_arg(&self, v: &Value) -> Option<Arg> {
        Some(match v {
            Value::Bool(b) => Arg::Bool(*b),
            Value::Int(i) => Arg::Int(*i),
            Value::Float(f) => Arg::Float(*f),
            Value::Vector(x) => Arg::Vector(2, *x),
            Value::Range(r) => Arg::Range(*r),
            // Sly 2 objects are named by their `s32` IDs; Sly 1 by their OIDs.
            Value::Object(Some(o)) => match self.world.sly2_id(self.obj(*o)?) {
                Some(id) => Arg::Int(id),
                None => Arg::Short(6, self.world.oid(self.obj(*o)?)?),
            },
            _ => return None,
        })
    }
}

impl WorldHost<'_> {
    /// Values of the run-time options (options-by-class.md 1.1).
    fn live_option(&self, h: Handle, option: u16) -> Option<Value> {
        let w = &*self.world;
        if !w.sly1_meanings() {
            return None; // Sly 2 and Sly 3 hook: the live options are Sly 1 option IDs.
        }
        let rec = w.record(h).map(|r| w.level.record(r).local_transform());
        let mover = w.movers().iter().find(|m| m.slot == h.index()).copied();
        Some(match option {
            live::FROZEN => Value::Bool(w.is_postponed(h.index(), self.time)),
            live::POSITION => Value::Vector(w.local(h)?.t),
            live::ORIGINAL_POSITION => Value::Vector(rec.map_or([0.0; 3], |r| r.t)),
            live::WORLD_POSITION => Value::Vector(w.position(h)?),
            live::ROTATION => matrix(&w.local(h)?),
            live::ORIGINAL_ROTATION => matrix(&rec.unwrap_or(formats::geometry::Affine::IDENTITY)),
            live::WORLD_MATRIX => matrix(&w.world_transform(h)?),
            live::EULER => Value::Vector(euler_of(&w.local(h)?.cols)),
            live::VELOCITY => Value::Vector(mover.map_or([0.0; 3], |m| m.velocity)),
            live::ANGULAR => Value::Vector(mover.map_or([0.0; 3], |m| m.angular)),
            live::ACCEL | live::ANGULAR_ACCEL => Value::Vector([0.0; 3]),
            // The enumeration reads 0 (normal) on anything that is not a dropped pickup.
            live::PICKUP_STATE => match w.behavior(h) {
                Some(crate::behavior::Behavior::Pickup(p)) => Value::Int(p.state_code()),
                _ => Value::Int(0),
            },
            // A segment's duration in seconds, from its record (animation.md 5.1).
            live::SEGMENT_DURATION if w.class(h) == Some(id::SEGMENT) => {
                let offset = w.level.record(w.record(h)?).offset + 6;
                let seg = formats::anim::Segment::parse(&w.level.bytes, offset, &w.level.table).ok();
                Value::Float(seg.map_or(0.0, |s| s.duration()))
            }
            _ => return None,
        })
    }

    /// Writes of the run-time options; true when handled.
    fn set_live_option(&mut self, h: Handle, option: u16, value: &Value) -> bool {
        if !self.world.sly1_meanings() {
            return false; // Sly 2 and Sly 3 hook: the live options are Sly 1 option IDs.
        }
        match option {
            live::POSITION => {
                if let Value::Vector(p) = value {
                    self.world.set_position(h, *p);
                }
                true
            }
            live::ROTATION => {
                if let Some(a) = affine_of(value) {
                    self.world.set_rotation(h, a.cols);
                }
                true
            }
            live::EULER => {
                if let Value::Vector(e) = value {
                    self.world.set_rotation(h, crate::anim::euler_zyx(*e));
                }
                true
            }
            live::VELOCITY | live::ANGULAR => {
                if let Value::Vector(v) = value {
                    let id = if option == live::VELOCITY {
                        crate::world::opt::VELOCITY
                    } else {
                        crate::world::opt::ANGULAR_VELOCITY
                    };
                    self.world.set_option(h, id, OptionValue::Vector(*v));
                }
                true
            }
            live::FROZEN | live::WORLD_POSITION | live::WORLD_MATRIX | live::ACCEL | live::ANGULAR_ACCEL => true,
            _ => false,
        }
    }

    /// A value stored as given, for options without an encoding.
    fn untyped_option(&self, v: &Value) -> Option<OptionValue> {
        Some(match v {
            Value::Bool(b) => OptionValue::Byte(*b as u8),
            Value::Int(i) => OptionValue::Int(*i),
            Value::Float(f) => OptionValue::Float(*f),
            Value::Vector(x) | Value::Coeffs(x) | Value::Smoothing(x) => OptionValue::Vector(*x),
            Value::Range(r) => OptionValue::Pair(*r),
            Value::Object(Some(o)) => OptionValue::Short(self.world.oid(self.obj(*o)?)?),
            Value::Object(None) | Value::Nil => OptionValue::Short(-1),
            _ => return None,
        })
    }
}

impl ScriptHost for WorldHost<'_> {
    fn class_of(&self, object: ObjectId) -> Option<i16> {
        if !self.world.sly1_meanings() {
            return self.sly2_class_of(object); // Sly 2 and Sly 3 hook
        }
        if object == WORLD_SCRIPT_ID {
            return Some(id::WORLD);
        }
        self.world.class(self.obj(object)?)
    }

    fn get_option(&mut self, object: ObjectId, option: u16) -> Value {
        if !self.world.sly1_meanings() {
            return self.sly2_get_option(object, option); // Sly 2 and Sly 3 hook
        }
        let enc = self.world.level.table.get(option as i16);
        if object == WORLD_SCRIPT_ID {
            // World option 547 (script option 634): the level's adaptive difficulty, 0 to 1.
            if option == live::DIFFICULTY && self.world.world_options.get(option).is_none() {
                return Value::Float(self.world.shared.difficulty);
            }
            return match self.world.world_options.get(option) {
                Some(v) => to_value(v),
                None => enc.map_or(Value::Nil, zero_value),
            };
        }
        let Some(h) = self.obj(object) else { return Value::Nil };
        if let Some(v) = self.live_option(h, option) {
            return v;
        }
        if let Some(v) = self.world.behavior(h).and_then(|b| b.get_option(self.world, h, option)) {
            return to_value(&v);
        }
        let stored = self.world.options(h).and_then(|o| o.get(option));
        if live::REFERENCES.contains(&option) {
            let oid = stored.and_then(OptionValue::as_i32).map(|v| v as i16).filter(|&v| v >= 0);
            return Value::Object(oid.and_then(|v| self.world.find_nearest(v, Some(h))).map(Handle::to_script));
        }
        if option == live::MASS {
            return Value::Float(stored.and_then(OptionValue::as_f32).unwrap_or(0.0));
        }
        match stored {
            Some(v) => to_value(v),
            None => enc.map_or(Value::Nil, zero_value),
        }
    }

    fn set_option(&mut self, object: ObjectId, option: u16, value: &Value) -> Result<(), ScriptError> {
        if !self.world.sly1_meanings() {
            return self.sly2_set_option(object, option, value); // Sly 2 and Sly 3 hook
        }
        // An error value written to an option keeps its own kind (scripts.md 11, item 7): it is the script's error,
        // not the host's.
        if let Value::Error(e) = value {
            return Err(*e);
        }
        if object != WORLD_SCRIPT_ID
            && let Some(h) = self.obj(object)
            && self.set_live_option(h, option, value)
        {
            return Ok(());
        }
        // Options no stream stores have no encoding (options-by-class.md 1.1): keep the value as given. References
        // keep the object's OID (-1 for the null object); mass is a float whatever number is written.
        let v = if live::REFERENCES.contains(&option) {
            match value {
                Value::Object(None) | Value::Nil | Value::Void => Some(OptionValue::Short(-1)),
                Value::Object(Some(o)) => self.obj(*o).and_then(|h| self.world.oid(h)).map(OptionValue::Short),
                _ => None,
            }
        } else if option == live::MASS {
            num(value).map(OptionValue::Float)
        } else {
            match self.world.level.table.get(option as i16) {
                Some(enc) => self.to_option(enc, value),
                None => self.untyped_option(value),
            }
        }
        .ok_or(ScriptError::Host(option as u32))?;
        if object == WORLD_SCRIPT_ID {
            // World darkening (effects.md 8): 540 sets it at once, 541 sets the target it eases toward.
            if let Some(x) = v.as_f32() {
                match option {
                    540 => {
                        self.world.shared.screen.darkening = x;
                        self.world.shared.screen.darkening_target = x;
                    }
                    541 => self.world.shared.screen.darkening_target = x,
                    _ => {}
                }
            }
            self.world.world_options.set(option, v);
            return Ok(());
        }
        let Some(h) = self.obj(object) else { return Ok(()) };
        self.world.set_option(h, option, v);
        Ok(())
    }

    fn call_action(&mut self, object: ObjectId, option: u16, args: &[Value]) -> Value {
        if self.world.sly2.is_some()
            && let Some(v) = self.sly2_call_action(object, option, args)
        {
            return v; // Sly 2 hook: the job system's methods.
        }
        if self.world.sly3.is_some()
            && let Some(v) = self.sly3_call_action(object, option, args)
        {
            return v; // Sly 3 hook: the task methods, for the session's job system.
        }
        let Some(h) = self.obj(object) else { return Value::Void };
        let args: Vec<Arg> = args.iter().filter_map(|a| self.to_arg(a)).collect();
        self.world.action_result = None;
        if !self.world.run_action(h, option, &args) {
            // An action no behaviour implements yet: no effect, counted (not an error of the script).
            *self.world.unhandled_actions.entry(option).or_insert(0) += 1;
        }
        // A method gives back an object (Sly 2 option 1095: the new noise).
        let result = self.world.action_result.take();
        if self.world.sly2.is_some() && option == sly2::MAKE_NOISE && result.is_some() {
            return Value::Object(self.sly2_noise_object().map(Handle::to_script)); // Sly 2 hook
        }
        match result {
            Some(r) => Value::Object(Some(r.to_script())),
            None => Value::Void,
        }
    }

    fn is_action(&self, option: u16) -> bool {
        ACTIONS.contains(&option)
    }

    fn find_object(&mut self, oid: i16, context: Option<ObjectId>) -> Option<ObjectId> {
        let ctx = self.ctx(context);
        self.world.find_oid(oid, ctx).map(Handle::to_script)
    }

    fn find_objects(&mut self, oid: i16, context: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        let ctx = self.ctx(context);
        let mut v = Vec::new();
        self.world.find_all_oid(oid, ctx, &mut v);
        out.extend(v.into_iter().map(Handle::to_script));
    }

    fn find_nearest(&mut self, oid: i16, context: Option<ObjectId>) -> Option<ObjectId> {
        let ctx = self.ctx(context);
        self.world.find_nearest(oid, ctx).map(Handle::to_script)
    }

    fn find_all_nearest(&mut self, oid: i16, context: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        let ctx = self.ctx(context);
        let mut v = Vec::new();
        self.world.find_all_nearest(oid, ctx, &mut v);
        out.extend(v.into_iter().map(Handle::to_script));
    }

    fn find_by_class(&mut self, class: i16, context: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        let ctx = self.ctx(context);
        let mut v = Vec::new();
        self.world.find_class(class, ctx, &mut v);
        out.extend(v.into_iter().map(Handle::to_script));
    }

    fn objects_in_sphere(&mut self, centre: [f32; 3], radius: f32, out: &mut Vec<ObjectId>) {
        let mut v = Vec::new();
        if !self.collision.overlap_sphere(self.world, centre, radius, &mut v) {
            // Without collision queries: solid objects whose origin is inside the sphere.
            let r2 = radius * radius;
            for h in self.world.handles() {
                let solid = self.world.class(h).and_then(crate::object::class_info).is_some_and(|c| c.is_solid());
                if solid
                    && self.world.is_active(h)
                    && self.world.position(h).is_some_and(|p| crate::behavior::dist_sq(p, centre) <= r2)
                {
                    v.push(h);
                }
            }
        }
        out.extend(v.into_iter().map(Handle::to_script));
    }

    fn find_object_id(&mut self, id: i32, context: Option<ObjectId>, nearest: bool) -> Option<ObjectId> {
        if self.world.sly2.is_some() {
            return self.sly2_find(id, context); // Sly 2 hook
        }
        if self.world.sly3.is_some() {
            return self.sly3_find(id, context); // Sly 3 hook
        }
        let oid = i16::try_from(id).ok()?;
        if nearest { self.find_nearest(oid, context) } else { self.find_object(oid, context) }
    }

    fn find_objects_id(&mut self, id: i32, context: Option<ObjectId>, nearest: bool, out: &mut Vec<ObjectId>) {
        if self.world.sly2.is_some() {
            return self.sly2_find_all(id, context, out); // Sly 2 hook
        }
        if self.world.sly3.is_some() {
            return self.sly3_find_all(id, context, out); // Sly 3 hook
        }
        let Ok(oid) = i16::try_from(id) else { return };
        if nearest { self.find_all_nearest(oid, context, out) } else { self.find_objects(oid, context, out) }
    }

    fn game_operation(&mut self, op: u32, args: &[Value]) -> Value {
        if self.world.sly2.is_some() {
            return self.sly2_game_operation(op, args); // Sly 2 hook
        }
        if self.world.sly3.is_some() {
            return self.sly3_game_operation(op, args); // Sly 3 hook
        }
        Value::Error(ScriptError::Host(op))
    }

    fn player(&mut self) -> Option<ObjectId> {
        if self.world.sly2.is_some() {
            return self.sly2_player(); // Sly 2 hook
        }
        if self.world.sly3.is_some() {
            return self.sly3_player(); // Sly 3 hook
        }
        self.world.player.filter(|&p| self.world.contains(p)).map(Handle::to_script)
    }

    fn camera(&mut self) -> Option<ObjectId> {
        if self.world.sly3.is_some() {
            return self.sly3_camera(); // Sly 3 hook
        }
        None
    }

    fn world(&mut self) -> Option<ObjectId> {
        Some(WORLD_SCRIPT_ID)
    }

    fn convert_point(&mut self, point: [f32; 3], from: Option<ObjectId>, to: Option<ObjectId>) -> [f32; 3] {
        let from = self.ctx(from).and_then(|h| self.world.world_transform(h));
        let to = self.ctx(to).and_then(|h| self.world.world_transform(h));
        let w = from.map_or(point, |m| m.transform_point(point));
        match to.and_then(|m| crate::math::affine_inverse(&m)) {
            Some(inv) => inv.transform_point(w),
            None => w,
        }
    }

    fn convert_direction(&mut self, dir: [f32; 3], from: Option<ObjectId>, to: Option<ObjectId>) -> [f32; 3] {
        let from = self.ctx(from).and_then(|h| self.world.world_transform(h));
        let to = self.ctx(to).and_then(|h| self.world.world_transform(h));
        let w = from.map_or(dir, |m| m.transform_vector(dir));
        match to.and_then(|m| crate::math::inverse(&m.cols)) {
            Some(inv) => crate::math::mat_mul_vec(&inv, w),
            None => w,
        }
    }

    fn postpone_updates(&mut self, object: ObjectId, seconds: f32) {
        if let Some(h) = self.obj(object) {
            self.world.postpone(h, self.time + seconds);
        }
    }

    fn play_sound(&mut self, sound: i32, emitter: Option<ObjectId>) {
        let e = emitter.and_then(|e| self.obj(e));
        self.world.play_sound(sound, e);
    }

    fn rumble(&mut self, kind: i32, strength: f32) {
        self.world.request(Request::Rumble { kind, strength });
    }

    fn smoke(&mut self, position: [f32; 3], size: f32) {
        self.world.request(Request::Smoke { position, size });
    }

    fn game_time(&self) -> f32 {
        self.time
    }

    fn random(&mut self) -> f32 {
        self.rng.next_f32()
    }

    fn gravity(&self) -> f32 {
        WORLD_GRAVITY
    }

    fn report(&mut self, error: &ScriptError) {
        self.errors += 1;
        match self.kinds.iter_mut().find(|(e, _)| e == error) {
            Some((_, n)) => *n += 1,
            None => self.kinds.push((*error, 1)),
        }
    }

    fn poll_event(&mut self) -> Option<RaisedEvent> {
        self.world.events.pop_front()
    }

    fn module_of(&mut self, object: ObjectId) -> Option<u32> {
        // Sly 2 hook: run-time playbacks (classes 134 and 136) run their clip's or machine's module.
        let h = self.obj(object)?;
        let class = self.world.class(h)?;
        let m = self.world.module(h);
        let playback = [super::sly2_anim::CLIP_PLAYBACK_CLASS, super::sly2_anim::MACHINE_PLAYBACK_CLASS];
        (self.world.sly2.is_some() && playback.contains(&class) && m >= 0).then_some(m as u32)
    }
}
