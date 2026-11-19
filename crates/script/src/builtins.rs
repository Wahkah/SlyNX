//! The built-in operations (scripts.md section 7).

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::host::ScriptHost;
use crate::interp::{Interpreter, ScheduledCall};
use crate::math;
use crate::tree::Game;
use crate::value::{ObjectId, ScriptError, Value, equal, same_value};

/// Minimum argument count of a defined operation, and whether it takes more. `None`: not defined by the spec.
pub const fn arity(op: u32) -> Option<(u8, bool)> {
    Some(match op {
        0..=3 => (1, true),
        4 => (1, false),
        7..=13 => (2, false),
        25 | 27 => (1, false),
        30 => (3, false),
        31 => (1, false),
        53 => (2, false),
        55 | 56 => (1, false),
        63 | 64 => (2, false),
        65 => (1, false),
        66..=68 => (1, true),
        71 => (3, true),
        81 => (2, true),
        82 => (1, false),
        84 => (3, false),
        85 => (4, false),
        86 | 87 => (2, false),
        88 | 90 | 91 | 92 => (1, false),
        93 | 94 => (1, true),
        95 => (2, false),
        96 => (0, false),
        33 | 34 => (1, false),
        35 => (2, false),
        37 => (1, false),
        38 | 39 => (2, false),
        40 | 41 => (0, true),
        42..=44 => (2, false),
        46 | 48 => (3, false),
        49 => (2, false),
        51 => (1, true),
        97 => (2, true),
        98 => (2, false),
        99..=101 => (3, false),
        102..=106 => (2, false),
        107..=109 => (0, false),
        110 | 112 => (2, false),
        115 | 116 => (3, false),
        119 | 121 | 122 => (2, false),
        _ => return None,
    })
}

/// The IDs of every defined operation, in increasing order: the 74 that occur on the disc and 90 (round), which
/// scripts.md 11 defines although no script uses it.
pub fn defined() -> impl Iterator<Item = u32> {
    (0..125).filter(|&op| arity(op).is_some())
}

/// Most objects a list-returning find gives (operations 104 and 106).
pub const FIND_LIMIT: usize = 64;
/// Most objects operation 110 gives.
pub const CLASS_FIND_LIMIT: usize = 256;

enum Num {
    I(i32),
    F(f32),
}

fn num(v: &Value) -> Option<Num> {
    match *v {
        Value::Int(i) => Some(Num::I(i)),
        Value::Float(f) => Some(Num::F(f)),
        _ => None,
    }
}

/// A value read as a number (scripts.md 11, item 15): true and false are 1 and 0, anything else that is not a
/// number is 0.
fn read_num(v: &Value) -> Num {
    match *v {
        Value::Float(f) => Num::F(f),
        Value::Int(i) => Num::I(i),
        Value::Bool(b) => Num::I(b as i32),
        _ => Num::I(0),
    }
}

/// The identity transform, which a matrix read from a non-matrix gives.
pub const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// Coefficients read from a value; zero from anything else.
fn coeffs(v: &Value) -> [f32; 3] {
    match v {
        Value::Coeffs(g) => *g,
        _ => [0.0; 3],
    }
}

/// A range read from a value; zero to zero from anything else.
fn range(v: &Value) -> [f32; 2] {
    match v {
        Value::Range(r) => *r,
        _ => [0.0; 2],
    }
}

fn vec3(v: &Value) -> Option<[f32; 3]> {
    match v {
        Value::Vector(a) => Some(*a),
        _ => None,
    }
}

fn objects(list: Vec<ObjectId>, limit: usize) -> Value {
    Value::list(list.into_iter().take(limit).map(|o| Value::Object(Some(o))).collect::<Vec<_>>())
}

/// Row-major 4x4 from three axis rows and a translation (see `docs/script.md`).
fn matrix(x: [f32; 3], y: [f32; 3], z: [f32; 3], t: [f32; 3]) -> Value {
    Value::Matrix(Box::new([
        x[0], x[1], x[2], 0.0, y[0], y[1], y[2], 0.0, z[0], z[1], z[2], 0.0, t[0], t[1], t[2], 1.0,
    ]))
}

/// A transform at `pos` whose X axis points along `fwd`, with `up` as the up hint.
fn look(pos: [f32; 3], fwd: [f32; 3], up: [f32; 3]) -> Value {
    let x = math::normalize(fwd);
    let mut y = math::normalize(math::cross(up, x));
    if y == [0.0; 3] {
        // Forward parallel to up: any perpendicular will do.
        y = math::normalize(math::cross([1.0, 0.0, 0.0], x));
        if y == [0.0; 3] {
            y = math::normalize(math::cross([0.0, 1.0, 0.0], x));
        }
    }
    let z = math::cross(x, y);
    matrix(x, y, z, pos)
}

/// An operation's arguments, with the operation ID that errors name.
pub(crate) struct Call<'a> {
    pub(crate) op: u32,
    pub(crate) args: &'a [Value],
}

impl Call<'_> {
    pub(crate) fn arg(&self, i: usize) -> &Value {
        &self.args[i]
    }
    pub(crate) fn bad(&self, i: usize) -> ScriptError {
        ScriptError::BadArgument { op: self.op, index: i.min(255) as u8 }
    }
    // Arguments of the wrong kind are read as the nearest thing, never rejected (scripts.md 11, item 15): numbers
    // as 0 (booleans as 1 and 0), vectors as zero, matrices as the identity, objects as the null object.
    pub(crate) fn f(&self, i: usize) -> Result<f32, ScriptError> {
        Ok(match read_num(self.arg(i)) {
            Num::I(n) => n as f32,
            Num::F(f) => f,
        })
    }
    pub(crate) fn i(&self, i: usize) -> Result<i32, ScriptError> {
        Ok(match read_num(self.arg(i)) {
            Num::I(n) => n,
            Num::F(f) => math::trunc(f) as i32,
        })
    }
    pub(crate) fn v(&self, i: usize) -> Result<[f32; 3], ScriptError> {
        Ok(vec3(self.arg(i)).unwrap_or([0.0; 3]))
    }
    pub(crate) fn m(&self, i: usize) -> [f32; 16] {
        match self.arg(i) {
            Value::Matrix(m) => **m,
            _ => IDENTITY,
        }
    }
    fn obj(&self, i: usize) -> Result<Option<ObjectId>, ScriptError> {
        Ok(self.arg(i).as_object().flatten())
    }
    fn oid(&self, i: usize) -> Result<i16, ScriptError> {
        let n = self.i(i)?;
        i16::try_from(n).map_err(|_| self.bad(i))
    }
    /// Position of the first argument (from `from`) that satisfies `pred`.
    fn find(&self, pred: impl Fn(&Value) -> bool) -> Option<usize> {
        self.args.iter().position(pred)
    }
}

/// Runs operation `op` of the interpreter's game.
pub(crate) fn call(it: &mut Interpreter, host: &mut dyn ScriptHost, op: u32, args: Vec<Value>) -> Value {
    let given = args.len().min(255) as u8;
    let result = match it.game() {
        Game::Sly1 => match arity(op) {
            None => Err(ScriptError::UnknownOperation(op)),
            Some((min, _)) if given < min => Err(ScriptError::TooFewArguments { given, needed: min }),
            Some(_) => run(it, host, op, op, &args),
        },
        Game::Sly2 | Game::Sly3 => {
            let sly3 = it.game() == Game::Sly3;
            // The spec's rule (so the operation has the arguments it reads), then the executable's when loaded.
            let spec = if sly3 { crate::sly3::arity(op) } else { crate::sly2::arity(op) };
            let rules = [spec, it.sly2_builtins().and_then(|t| t.rule(op))];
            match rules {
                [None, _] => Err(ScriptError::UnknownOperation(op)),
                _ => {
                    let mut err = None;
                    for (min, more) in rules.into_iter().flatten() {
                        // Sly 3's `not` ignores a surplus argument (specs/sly3/scripts.md 4.1).
                        let more = more || (sly3 && op == 31);
                        if given < min {
                            err = Some(ScriptError::TooFewArguments { given, needed: min });
                        } else if !more && given > min {
                            err = Some(ScriptError::TooManyArguments { given, allowed: min });
                        }
                    }
                    match err {
                        Some(e) => Err(e),
                        None if sly3 => crate::sly3::run(it, host, op, &args),
                        None => crate::sly2::run(it, host, op, op, &args),
                    }
                }
            }
        }
    };
    match result {
        Ok(v) => v,
        Err(e) => it.fail(host, e),
    }
}

/// Runs Sly 1 operation `op`; errors name `report` (the operation as the script called it).
pub(crate) fn run(
    it: &mut Interpreter,
    host: &mut dyn ScriptHost,
    op: u32,
    report: u32,
    args: &[Value],
) -> Result<Value, ScriptError> {
    let c = Call { op: report, args };
    Ok(match op {
        0 => add(&c)?,
        1 => sub(&c)?,
        2 => mul(&c)?,
        3 => div(&c)?,
        4 => {
            host.print(c.arg(0));
            Value::Void
        }
        7 => Value::Bool(c.i(0)? == c.i(1)?),
        // False when either side is not a number.
        8..=11 => match (num(c.arg(0)), num(c.arg(1))) {
            (Some(Num::I(a)), Some(Num::I(b))) => Value::Bool([a < b, a <= b, a > b, a >= b][(op - 8) as usize]),
            (Some(_), Some(_)) => {
                let (a, b) = (c.f(0)?, c.f(1)?);
                Value::Bool([a < b, a <= b, a > b, a >= b][(op - 8) as usize])
            }
            _ => Value::Bool(false),
        },
        12 => Value::Bool(same_value(c.arg(0), c.arg(1))),
        13 => Value::Bool(equal(c.arg(0), c.arg(1))),
        25 => Value::Bool(matches!(c.arg(0), Value::Nil)),
        27 => Value::Bool(matches!(c.arg(0), Value::Object(None))),
        30 => {
            let tol = c.f(2)?;
            let d = match (c.arg(0), c.arg(1)) {
                (Value::Vector(a), Value::Vector(b)) => math::length(math::sub(*a, *b)),
                _ => math::abs(c.f(0)? - c.f(1)?),
            };
            Value::Bool(d <= tol)
        }
        31 => Value::Bool(!c.arg(0).is_true()),
        53 => {
            let (a, b) = (c.f(0)?, c.f(1)?);
            Value::Float(a + host.random() * (b - a))
        }
        55 => Value::Float(math::sin_cos(c.f(0)?).0),
        56 => Value::Float(math::sin_cos(c.f(0)?).1),
        63 => Value::Float(math::dot(c.v(0)?, c.v(1)?)),
        64 => Value::Vector(math::cross(c.v(0)?, c.v(1)?)),
        65 => Value::Float(math::length(c.v(0)?)),
        66 | 67 => {
            let a = c.v(0)?;
            let b = if args.len() > 1 { c.v(1)? } else { [0.0; 3] };
            let d = math::sub(a, b);
            Value::Float(if op == 66 { math::length(d) } else { math::dot(d, d) })
        }
        68 => {
            let v = c.v(0)?;
            if math::length(v) < 0.0001 {
                Value::Vector(if args.len() > 1 { c.v(1)? } else { [0.0; 3] })
            } else {
                Value::Vector(math::normalize(v))
            }
        }
        71 => {
            let (start, target, steepness) = (c.v(0)?, c.v(1)?, c.f(2)?);
            let launcher = if args.len() > 3 { c.obj(3)? } else { None };
            Value::Vector(launch(start, target, steepness, host.object_gravity(launcher)))
        }
        81 => {
            let (pos, target) = (c.v(0)?, c.v(1)?);
            let up = if args.len() > 2 { c.v(2)? } else { [0.0, 0.0, 1.0] };
            look(pos, math::sub(target, pos), up)
        }
        82 => Value::Matrix(Box::new(upright(&c.m(0)))),
        84 => {
            let (g, r, x) = (coeffs(c.arg(0)), range(c.arg(1)), c.f(2)?);
            Value::Float(clamp(g[0] + g[1] * x + g[2] * x * x, r))
        }
        85 => Value::Coeffs(fit(c.f(0)?, c.f(1)?, c.f(2)?, c.f(3)?)),
        86 | 87 => {
            let (r, x) = (range(c.arg(0)), c.f(1)?);
            if op == 87 { Value::Bool(x >= r[0] && x <= r[1]) } else { Value::Float(clamp(x, r)) }
        }
        88 => match read_num(c.arg(0)) {
            Num::I(i) => Value::Int(i),
            Num::F(f) => Value::Int(math::floor(f) as i32),
        },
        90 => match read_num(c.arg(0)) {
            Num::I(i) => Value::Int(i),
            Num::F(f) => Value::Int(math::floor(f + 0.5) as i32),
        },
        91 => Value::Int(c.i(0)?),
        92 => match read_num(c.arg(0)) {
            Num::I(i) => Value::Int(i.wrapping_abs()),
            Num::F(f) => Value::Float(math::abs(f)),
        },
        93 | 94 => {
            // The winning argument itself, so its kind is kept; the first one wins a tie.
            let mut best = 0;
            let mut best_v = c.f(0)?;
            for i in 1..args.len() {
                let v = c.f(i)?;
                if (op == 93 && v > best_v) || (op == 94 && v < best_v) {
                    best = i;
                    best_v = v;
                }
            }
            // A winner that is not a number is given as the number it was read as.
            match (num(c.arg(best)), read_num(c.arg(best))) {
                (Some(_), _) => c.arg(best).clone(),
                (None, Num::I(n)) => Value::Int(n),
                (None, Num::F(f)) => Value::Float(f),
            }
        }
        95 => {
            let (a, b) = (c.i(0)?, c.i(1)?);
            Value::Int(if b == 0 { 0 } else { a.wrapping_rem(b) })
        }
        96 => Value::Float(host.game_time()),

        // ---- lists and values ----
        // A list operation reads anything that is not a list as the empty list.
        33 => match c.arg(0) {
            Value::List(l) => l.car.borrow().clone(),
            _ => Value::Nil,
        },
        34 => match c.arg(0) {
            Value::List(l) => l.cdr.borrow().clone(),
            _ => Value::Nil,
        },
        35 => {
            if let Value::List(l) = c.arg(0) {
                *l.car.borrow_mut() = c.arg(1).clone();
            }
            Value::Void
        }
        37 => {
            if !c.arg(0).is_list() {
                return Ok(Value::Int(0));
            }
            let n = c.arg(0).iter().count();
            it.charge(n)?;
            Value::Int(n as i32)
        }
        38 => {
            if !c.arg(0).is_list() {
                return Ok(Value::Nil);
            }
            let n = usize::try_from(c.i(1)?).ok();
            it.charge(n.unwrap_or(0))?;
            n.and_then(|n| c.arg(0).iter().nth(n)).unwrap_or(Value::Nil)
        }
        39 => {
            if !c.arg(1).is_list() {
                return Ok(Value::Bool(false));
            }
            let v = c.arg(0);
            let pos = c.arg(1).iter().position(|e| same_value(&e, v));
            it.charge(pos.map_or_else(|| c.arg(1).iter().count(), |p| p + 1))?;
            Value::Bool(pos.is_some())
        }
        40 => {
            if args.len() > it.limits.max_list_len {
                return Err(ScriptError::TooLarge);
            }
            it.charge(args.len())?;
            Value::list(args.to_vec())
        }
        41 => {
            let Some((last, init)) = args.split_last() else { return Ok(Value::Nil) };
            let last = if last.is_list() { last } else { &Value::Nil };
            // The result's length counts the shared tail, so repeated appends cannot grow a list past the limit.
            let max = it.limits.max_list_len;
            let mut total = last.iter().take(max + 1).count();
            for l in init {
                total += l.iter().take(max + 1).count();
                if total > max {
                    return Err(ScriptError::TooLarge);
                }
            }
            if total > max {
                return Err(ScriptError::TooLarge);
            }
            it.charge(total)?;
            let mut items = Vec::new();
            for l in init.iter().filter(|l| l.is_list()) {
                items.extend(l.iter());
            }
            Value::list_with_tail(items, last.clone())
        }
        42..=44 => {
            // A procedure that is not callable gives an error value per element and does nothing (item 18).
            if !c.arg(1).is_list() {
                return Ok(if op == 44 { Value::Void } else { Value::Nil });
            }
            let f = c.arg(0).clone();
            it.charge(c.arg(1).iter().count())?;
            let mut out = Vec::new();
            // Error values are ordinary values: a mapped error is kept, and counts as true when filtering.
            for e in c.arg(1).iter() {
                let r = it.apply(host, f.clone(), alloc::vec![e.clone()]);
                match op {
                    42 => out.push(r),
                    43 if r.is_true() => out.push(e),
                    _ => {}
                }
            }
            if op == 44 { Value::Void } else { Value::list(out) }
        }
        46 => Value::Vector([c.f(0)?, c.f(1)?, c.f(2)?]),
        48 => Value::Coeffs([c.f(0)?, c.f(1)?, c.f(2)?]),
        49 => Value::Range([c.f(0)?, c.f(1)?]),
        51 => element(&c)?,

        // ---- world and objects ----
        97 => {
            let delay = c.f(0)?;
            if delay.is_nan() {
                return Err(c.bad(0));
            }
            if !c.arg(1).is_callable() {
                // The call would do nothing when it ran (item 18).
                return Ok(Value::Void);
            }
            it.schedule(ScheduledCall::new(delay, c.arg(1).clone(), args[2..].to_vec()))?;
            Value::Void
        }
        98 => {
            let t = c.f(1)?;
            if let Some(o) = c.obj(0)? {
                host.postpone_updates(o, t);
            }
            Value::Void
        }
        99..=101 => {
            let id = u32::try_from(c.i(1)?).map_err(|_| c.bad(1))?;
            let targets: Vec<ObjectId> = match c.arg(0) {
                Value::List(_) => {
                    it.charge(c.arg(0).iter().count())?;
                    c.arg(0).iter().filter_map(|v| v.as_object().flatten()).collect()
                }
                v => v.as_object().flatten().into_iter().collect(),
            };
            let mut result = Value::Void;
            for o in targets {
                let r = match op {
                    101 => it.set_option(host, o, id, c.arg(2)),
                    _ => it.add_custom(host, o, id, c.arg(2), op == 99),
                };
                if r.is_error() {
                    result = r;
                }
            }
            result
        }
        102 => {
            let id = u32::try_from(c.i(1)?).map_err(|_| c.bad(1))?;
            let target = match c.arg(0) {
                Value::List(_) => c.arg(0).iter().last().and_then(|v| v.as_object().flatten()),
                v => v.as_object().flatten(),
            };
            match target {
                Some(o) => it.get_option(host, o, id),
                None => Value::Void,
            }
        }
        // Sly 2 and 3 wide-ID hook: their object IDs are `s32` (object-stream.md 2.2), so the searches take the
        // full ID.
        103 | 105 if it.game() != Game::Sly1 => {
            let (id, ctx) = (c.i(0)?, c.obj(1)?);
            Value::Object(host.find_object_id(id, ctx, op == 105))
        }
        104 | 106 if it.game() != Game::Sly1 => {
            let (id, ctx) = (c.i(0)?, c.obj(1)?);
            let mut out = Vec::new();
            host.find_objects_id(id, ctx, op == 106, &mut out);
            objects(out, FIND_LIMIT)
        }
        103 | 105 => {
            let (oid, ctx) = (c.oid(0)?, c.obj(1)?);
            let r = if op == 103 { host.find_object(oid, ctx) } else { host.find_nearest(oid, ctx) };
            Value::Object(r)
        }
        104 | 106 => {
            let (oid, ctx) = (c.oid(0)?, c.obj(1)?);
            let mut out = Vec::new();
            if op == 104 {
                host.find_objects(oid, ctx, &mut out);
            } else {
                host.find_all_nearest(oid, ctx, &mut out);
            }
            objects(out, FIND_LIMIT)
        }
        107 => Value::Object(host.player()),
        108 => Value::Object(host.world()),
        109 => Value::Object(host.camera()),
        110 => {
            let (class, ctx) = (c.oid(0)?, c.obj(1)?);
            let mut out = Vec::new();
            host.find_by_class(class, ctx, &mut out);
            objects(out, CLASS_FIND_LIMIT)
        }
        112 => {
            let (centre, r) = (c.v(0)?, c.f(1)?);
            let mut out = Vec::new();
            host.objects_in_sphere(centre, r, &mut out);
            objects(out, usize::MAX)
        }
        115 | 116 => {
            let (from, to, v) = (c.obj(0)?, c.obj(1)?, c.v(2)?);
            let r = if op == 115 { host.convert_point(v, from, to) } else { host.convert_direction(v, from, to) };
            Value::Vector(r)
        }
        119 => {
            let (id, emitter) = (c.i(0)?, c.obj(1)?);
            host.play_sound(id, emitter);
            Value::Void
        }
        121 => {
            let (k, s) = (c.i(0)?, c.f(1)?);
            host.rumble(k, s);
            Value::Void
        }
        122 => {
            let (p, s) = (c.v(0)?, c.f(1)?);
            host.smoke(p, s);
            Value::Void
        }
        _ => return Err(ScriptError::UnknownOperation(op)),
    })
}

fn clamp(x: f32, r: [f32; 2]) -> f32 {
    if x < r[0] {
        r[0]
    } else if x > r[1] {
        r[1]
    } else {
        x
    }
}

/// Operation 71 (scripts.md 11, item 10): the launch velocity from `start` that lands on `target` under downward
/// gravity `g` (units/s², positive; Z up). The elevation starts at the angle of the straight line to the target
/// (at least 0) and moves toward vertical by `sin(steepness)` of the remaining angle; the speed is the one whose
/// arc passes through the target. Zero vector when there is no such arc.
pub fn launch(start: [f32; 3], target: [f32; 3], steepness: f32, g: f32) -> [f32; 3] {
    let d = math::sub(target, start);
    let horiz = [d[0], d[1], 0.0];
    let dist = math::length(horiz);
    // NaN inputs fall through to the zero vector too.
    if g.is_nan() || g <= 0.0 || dist.is_nan() || dist <= 0.0 {
        return [0.0; 3];
    }
    let base = math::atan2(d[2], dist).max(0.0);
    let theta = base + math::sin_cos(steepness).0 * (core::f32::consts::FRAC_PI_2 - base);
    let (sin, cos) = math::sin_cos(theta);
    // z(x) = x tan(theta) - g x^2 / (2 v^2 cos^2(theta)) passes through (dist, d.z) when
    // v^2 = g dist^2 / (2 cos(theta) (dist sin(theta) - d.z cos(theta))).
    let denom = 2.0 * cos * (dist * sin - d[2] * cos);
    if cos.is_nan() || cos <= 1e-6 || denom.is_nan() || denom <= 0.0 {
        return [0.0; 3];
    }
    let speed = math::sqrt(g * dist * dist / denom);
    if !speed.is_finite() {
        return [0.0; 3];
    }
    let h = math::scale(horiz, speed * cos / dist);
    [h[0], h[1], speed * sin]
}

/// Operation 85 (scripts.md 11, item 11): the coefficients (g0, g1, g2) of f(x) = g0 + g1 x + g2 x^2 with
/// f(0) = a, f(1) = b and f(u) = c. Float rules apply when `u` is 0 or 1.
pub fn fit(a: f32, b: f32, u: f32, c: f32) -> [f32; 3] {
    let g2 = ((c - a) / u - (b - a)) / (u - 1.0);
    [a, (b - a) - g2, g2]
}

/// Operation 82: the transform with its rotation turned by the shortest rotation that takes its local Z axis
/// (row 2) to world +Z; the position is kept.
pub fn upright(m: &[f32; 16]) -> [f32; 16] {
    let z = math::normalize([m[8], m[9], m[10]]);
    let up = [0.0, 0.0, 1.0];
    let axis = math::cross(z, up);
    let sin = math::length(axis);
    let cos = math::dot(z, up);
    // Rotation axis and angle; when Z points straight down, turn half a turn about the local X axis.
    let (k, sin, cos) = if sin > 1e-6 {
        (math::scale(axis, 1.0 / sin), sin, cos)
    } else if cos > 0.0 {
        return *m;
    } else {
        (math::normalize([m[0], m[1], m[2]]), 0.0, -1.0)
    };
    // Rodrigues: v' = v cos + (k x v) sin + k (k . v)(1 - cos)
    let rot = |v: [f32; 3]| {
        let kxv = math::cross(k, v);
        let kd = math::dot(k, v) * (1.0 - cos);
        [
            v[0] * cos + kxv[0] * sin + k[0] * kd,
            v[1] * cos + kxv[1] * sin + k[1] * kd,
            v[2] * cos + kxv[2] * sin + k[2] * kd,
        ]
    };
    let mut out = *m;
    for row in 0..3 {
        let r = rot([m[row * 4], m[row * 4 + 1], m[row * 4 + 2]]);
        out[row * 4..row * 4 + 3].copy_from_slice(&r);
    }
    out
}

/// Operation 51. Something that is not a container, or an index out of range, gives the empty list (item 15).
fn element(c: &Call<'_>) -> Result<Value, ScriptError> {
    let i = if c.args.len() > 1 { usize::try_from(c.i(1)?).ok() } else { None };
    let idx = |n: usize| i.filter(|&i| i < n);
    Ok(match c.arg(0) {
        Value::Vector(v) | Value::Coeffs(v) | Value::Smoothing(v) => idx(3).map_or(Value::Nil, |i| Value::Float(v[i])),
        Value::Range(r) => idx(2).map_or(Value::Nil, |i| Value::Float(r[i])),
        Value::Matrix(m) => {
            let Some(row) = idx(4) else { return Ok(Value::Nil) };
            if c.args.len() > 2 {
                match usize::try_from(c.i(2)?).ok().filter(|&j| j < 3) {
                    Some(col) => Value::Float(m[row * 4 + col]),
                    None => Value::Nil,
                }
            } else {
                Value::Vector([m[row * 4], m[row * 4 + 1], m[row * 4 + 2]])
            }
        }
        _ => Value::Nil,
    })
}

// Arithmetic on arguments of mixed kinds (scripts.md 11, item 15): add and multiply skip arguments that are not
// numbers wherever they are; subtract and divide read their first argument as a number and skip later
// non-numbers. With a vector or matrix first argument, arguments of another kind are skipped.

fn add(c: &Call<'_>) -> Result<Value, ScriptError> {
    match c.arg(0) {
        Value::Vector(_) => {
            let mut acc = [0.0; 3];
            for v in c.args.iter().filter_map(vec3) {
                acc = [acc[0] + v[0], acc[1] + v[1], acc[2] + v[2]];
            }
            Ok(Value::Vector(acc))
        }
        Value::Matrix(_) => {
            let mut acc = [0.0f32; 16];
            for a in c.args {
                if let Value::Matrix(m) = a {
                    for (x, y) in acc.iter_mut().zip(m.iter()) {
                        *x += *y;
                    }
                }
            }
            Ok(Value::Matrix(Box::new(acc)))
        }
        _ => fold_numbers(Num::I(0), c.args.iter().filter_map(num), |a, b| a + b),
    }
}

fn sub(c: &Call<'_>) -> Result<Value, ScriptError> {
    match c.arg(0) {
        // With one argument, the argument unchanged (no negation).
        Value::Vector(_) | Value::Matrix(_) | Value::Int(_) | Value::Float(_) if c.args.len() == 1 => {
            Ok(c.arg(0).clone())
        }
        Value::Vector(v) => {
            let mut acc = *v;
            for b in c.args[1..].iter().filter_map(vec3) {
                acc = math::sub(acc, b);
            }
            Ok(Value::Vector(acc))
        }
        Value::Matrix(m) => {
            let mut acc = **m;
            for a in &c.args[1..] {
                if let Value::Matrix(n) = a {
                    for (x, y) in acc.iter_mut().zip(n.iter()) {
                        *x -= *y;
                    }
                }
            }
            Ok(Value::Matrix(Box::new(acc)))
        }
        first => fold_numbers(read_num(first), c.args[1..].iter().filter_map(num), |a, b| a - b),
    }
}

fn mul(c: &Call<'_>) -> Result<Value, ScriptError> {
    // A vector or matrix is scaled by the product of the numbers.
    let shaped = c.find(|v| matches!(v, Value::Vector(_) | Value::Matrix(_)));
    if let Some(si) = shaped {
        let mut s = 1.0;
        for x in c.args.iter().filter_map(Value::as_f32) {
            s *= x;
        }
        return Ok(match c.arg(si) {
            Value::Vector(v) => Value::Vector(math::scale(*v, s)),
            Value::Matrix(m) => Value::Matrix(Box::new(m.map(|x| x * s))),
            _ => unreachable!(),
        });
    }
    fold_numbers(Num::I(1), c.args.iter().filter_map(num), |a, b| a * b)
}

fn div(c: &Call<'_>) -> Result<Value, ScriptError> {
    let first = c.arg(0);
    let shaped =
        matches!(first, Value::Vector(_) | Value::Coeffs(_) | Value::Smoothing(_) | Value::Range(_) | Value::Matrix(_));
    // With one argument, the argument unchanged (no reciprocal).
    if c.args.len() == 1 && (shaped || num(first).is_some()) {
        return Ok(first.clone());
    }
    if shaped {
        let mut d = 1.0;
        for x in c.args[1..].iter().filter_map(Value::as_f32) {
            d *= x;
        }
        let inv = |x: f32| x / d;
        return Ok(match first {
            Value::Vector(v) => Value::Vector(v.map(inv)),
            Value::Coeffs(v) => Value::Coeffs(v.map(inv)),
            Value::Smoothing(v) => Value::Smoothing(v.map(inv)),
            Value::Range(r) => Value::Range(r.map(inv)),
            Value::Matrix(m) => Value::Matrix(Box::new(m.map(inv))),
            _ => unreachable!(),
        });
    }
    fold_numbers(read_num(first), c.args[1..].iter().filter_map(num), |a, b| a / b)
}

/// Folds numbers left to right from `first` in single-precision floats (scripts.md 11, item 11). The result is an
/// integer, truncated toward zero, when every number is an integer; an integer result that is not finite (a
/// division by zero) is an error.
fn fold_numbers(
    first: Num,
    rest: impl Iterator<Item = Num>,
    op: impl Fn(f32, f32) -> f32,
) -> Result<Value, ScriptError> {
    let mut all_int = true;
    let mut as_f32 = |n: Num| match n {
        Num::I(n) => n as f32,
        Num::F(f) => {
            all_int = false;
            f
        }
    };
    let mut acc = as_f32(first);
    for x in rest {
        acc = op(acc, as_f32(x));
    }
    if !all_int {
        return Ok(Value::Float(acc));
    }
    if !acc.is_finite() {
        return Err(ScriptError::DivideByZero);
    }
    Ok(Value::Int(math::trunc(acc) as i32))
}
