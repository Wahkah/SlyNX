//! Sly 2 specifics (specs/sly2/scripts.md 4 to 6): the built-in table and the event signatures read from the
//! player's executable, the operations that keep their Sly 1 meaning under a new number, and the ones Sly 2 added.

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::builtins::{self, Call};
use crate::host::ScriptHost;
use crate::interp::{Interpreter, ScheduledCall};
use crate::math;
use crate::option_map::MapError;
use crate::value::{ScriptError, Value};

/// The built-in table: address and entry count (scripts.md 4.1). A zero word follows it at [`BUILTIN_TABLE_END`].
pub const BUILTIN_TABLE: (u32, usize) = (0x002B_BB90, 169);
/// Address of the zero word after the built-in table.
pub const BUILTIN_TABLE_END: u32 = 0x002B_C37C;
/// Bytes per built-in table entry.
pub const BUILTIN_ENTRY_SIZE: usize = 12;
/// Event signature index: 30 `s32` start positions, then −1 (scripts.md 6).
pub const EVENT_INDEX: u32 = 0x0027_A1D8;
/// Event signature type codes, one `u32` per argument.
pub const EVENT_TYPES: u32 = 0x0027_A10C;
/// Number of events.
pub const EVENT_COUNT: usize = 30;

/// One entry of the built-in table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuiltinEntry {
    /// Address of the operation's code (not used by the port, which never runs the executable's code).
    pub code: u32,
    /// Minimum argument count.
    pub min_args: u8,
    /// Whether more arguments are allowed.
    pub more: bool,
}

/// The built-in table read from the executable: the argument rule of each of the 169 operation IDs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuiltinTable {
    pub entries: Vec<BuiltinEntry>,
}

fn word(b: &[u8], i: usize) -> u32 {
    b.get(i..i + 4).map_or(0, |w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
}

impl BuiltinTable {
    /// Reads the table from the supported Sly 2 executable. Refuses any other executable.
    pub fn from_executable(exe: &formats_sly2::exe::Executable) -> Result<BuiltinTable, MapError> {
        if !exe.is_supported() {
            return Err(MapError::UnsupportedExecutable);
        }
        Self::read(|va, len| exe.bytes_at(va, len).ok())
    }

    /// Reads the table through `read(address, length)`. No hash check.
    pub fn read<'a>(read: impl Fn(u32, usize) -> Option<&'a [u8]>) -> Result<BuiltinTable, MapError> {
        Self::read_at(read, BUILTIN_TABLE)
    }

    /// Reads a table of `n` entries at address `va` (Sly 2 or Sly 3 layout) through `read(address, length)`.
    pub fn read_at<'a>(
        read: impl Fn(u32, usize) -> Option<&'a [u8]>,
        (va, n): (u32, usize),
    ) -> Result<BuiltinTable, MapError> {
        let raw = read(va, n * BUILTIN_ENTRY_SIZE)
            .filter(|b| b.len() >= n * BUILTIN_ENTRY_SIZE)
            .ok_or(MapError::BadAddress(va))?;
        let mut entries = Vec::with_capacity(n);
        for i in 0..n {
            let o = i * BUILTIN_ENTRY_SIZE;
            let (min, more) = (word(raw, o + 4), word(raw, o + 8));
            if min > 16 || more > 1 {
                return Err(MapError::BadCount(min.max(more)));
            }
            entries.push(BuiltinEntry { code: word(raw, o), min_args: min as u8, more: more == 1 });
        }
        Ok(BuiltinTable { entries })
    }

    /// The argument rule of an operation: (minimum, more allowed).
    pub fn rule(&self, op: u32) -> Option<(u8, bool)> {
        self.entries.get(op as usize).map(|e| (e.min_args, e.more))
    }
}

/// The event argument signatures read from the executable: for each event, one type code per argument, the
/// object itself included. The codes are not decoded (scripts.md 6).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventSignatures {
    pub events: Vec<Vec<u32>>,
}

impl EventSignatures {
    /// Reads the signatures from the supported Sly 2 executable.
    pub fn from_executable(exe: &formats_sly2::exe::Executable) -> Result<EventSignatures, MapError> {
        if !exe.is_supported() {
            return Err(MapError::UnsupportedExecutable);
        }
        Self::read(|va, len| exe.bytes_at(va, len).ok())
    }

    /// Reads the signatures through `read(address, length)`. The type codes fill the words from [`EVENT_TYPES`] up
    /// to the index; the last event (29) starts there, so its signature is empty (scripts.md 6; the index's final
    /// word is −1, not an end position).
    pub fn read<'a>(read: impl Fn(u32, usize) -> Option<&'a [u8]>) -> Result<EventSignatures, MapError> {
        Self::read_layout(read, EVENT_INDEX, EVENT_TYPES, EVENT_COUNT, true)
    }

    /// Reads `count` signatures whose start positions are at `index` and type codes at `types` (just before the
    /// index). Event n runs from start[n] to start[n + 1] − 1, and the last event to the end of the type words.
    /// `terminated`: a −1 follows the index (Sly 2); without one (Sly 3, specs/sly3/scripts.md 6) the index's first
    /// word also counts as a type word, as the game reads it.
    pub fn read_layout<'a>(
        read: impl Fn(u32, usize) -> Option<&'a [u8]>,
        index_va: u32,
        types_va: u32,
        count: usize,
        terminated: bool,
    ) -> Result<EventSignatures, MapError> {
        let index_words = count + usize::from(terminated);
        let index = read(index_va, index_words * 4)
            .filter(|b| b.len() >= index_words * 4)
            .ok_or(MapError::BadAddress(index_va))?;
        let words = (index_va.checked_sub(types_va).ok_or(MapError::BadAddress(types_va))? / 4) as usize
            + usize::from(!terminated);
        let types = read(types_va, words * 4).filter(|b| b.len() >= words * 4).ok_or(MapError::BadAddress(types_va))?;
        if terminated && word(index, count * 4) != u32::MAX {
            return Err(MapError::BadCount(word(index, count * 4)));
        }
        let mut events = Vec::with_capacity(count);
        for e in 0..count {
            let start = word(index, e * 4) as usize;
            let end = if e + 1 < count { word(index, (e + 1) * 4) as usize } else { words.max(start) };
            if start > end || end > words.max(start) {
                return Err(MapError::BadCount(start as u32));
            }
            events.push((start..end).map(|i| word(types, i * 4)).collect());
        }
        Ok(EventSignatures { events })
    }

    /// Number of arguments (the object included) of an event.
    pub fn len(&self, event: u32) -> Option<usize> {
        self.events.get(event as usize).map(Vec::len)
    }

    /// Whether no event was read.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

/// The argument rule (minimum, more allowed) of each operation that occurs on the disc, as scripts.md 4.2 gives
/// it. The interpreter checks it on every call (so an operation always has the arguments it reads) and, when the
/// executable's table is loaded, that table's rule too. Other IDs are unknown operations.
pub const fn arity(op: u32) -> Option<(u8, bool)> {
    Some(match op {
        0..=3 => (1, true),
        7..=13 => (2, false),
        25 | 27 | 31 => (1, false),
        30 => (3, false),
        53 => (2, false),
        54..=56 | 59 | 61 | 65 => (1, false),
        62 | 63 => (2, false),
        66..=68 => (1, true),
        72 => (2, false),
        76 => (3, false),
        79 => (1, false),
        81 => (2, true),
        83 => (2, false),
        85 => (4, false),
        86 => (2, false),
        88 | 90..=92 => (1, false),
        93 | 94 => (1, true),
        95 => (2, false),
        96 => (0, false),
        135 | 136 => (0, true),
        32 => (2, false),
        33 | 34 | 37 | 41 => (1, false),
        35 | 36 | 38 | 39 => (2, false),
        40 | 42 | 47 | 48 => (0, true),
        43..=45 => (2, false),
        49 => (3, false),
        50 => (2, false),
        52 => (1, true),
        97 | 99..=104 => (0, false),
        105 => (2, true),
        106 => (1, false),
        107 => (2, false),
        108..=110 => (3, false),
        111 => (2, false),
        112..=115 => (2, true),
        117..=119 => (0, false),
        120 | 121 => (2, false),
        123..=126 => (3, false),
        129 | 130 => (2, false),
        131 | 133 | 138 => (1, false),
        137 => (0, false),
        139 => (2, false),
        _ => return None,
    })
}

/// The IDs of every operation [`arity`] defines (the 97 that occur on the disc), in increasing order.
pub fn defined() -> impl Iterator<Item = u32> {
    (0..169).filter(|&op| arity(op).is_some())
}

/// The Sly 1 operation that does the same as a Sly 2 operation (scripts.md 4.2, column "Sly 1").
pub const fn sly1_equivalent(op: u32) -> Option<u32> {
    Some(match op {
        0..=3 | 7..=13 | 25 | 27 | 30 | 31 | 33..=35 | 37..=40 | 53 | 55 | 56 | 63 | 65..=68 | 81 | 85 | 86 | 90 => op,
        91..=96 => op,
        // "Float to integer, discarding the fraction".
        88 => 91,
        42 => 41,
        43 => 42,
        44 => 43,
        45 => 44,
        49 => 48,
        50 => 49,
        52 => 51,
        107 => 98,
        108 => 99,
        109 => 100,
        110 => 101,
        111 => 102,
        112 => 103,
        113 => 104,
        115 => 105,
        117 => 108,
        118 => 107,
        119 => 109,
        120 => 110,
        125 => 115,
        126 => 116,
        129 => 119,
        130 => 121,
        _ => return None,
    })
}

/// Operations whose meaning belongs to the game, passed to [`ScriptHost::game_operation`] with their arguments
/// (scripts.md 4.2 and 4.3): the level row (97) and its episode (99); the current job, the current task and the
/// checkpoint task (100 to 102); the difficulty help, 0 to 1 (103); a game-state integer that is always 0 (104);
/// several objects found by a list of IDs (114); objects in a box (121); objects hit by a segment, all (123) or the
/// first (124); a saved progress value read by its path of name symbols (131); the HUD prompt object (137); and the
/// owned-item flags, read (138) and set (139).
pub const HOST_OPERATIONS: [u32; 15] = [97, 99, 100, 101, 102, 103, 104, 114, 121, 123, 124, 131, 137, 138, 139];

/// Runs Sly 2 operation `op` (the interpreter has checked its argument rule); errors name `report` (the operation as
/// the script called it: Sly 3 runs its operations through their Sly 2 equivalents).
pub(crate) fn run(
    it: &mut Interpreter,
    host: &mut dyn ScriptHost,
    op: u32,
    report: u32,
    args: &[Value],
) -> Result<Value, ScriptError> {
    if let Some(sly1) = sly1_equivalent(op) {
        return builtins::run(it, host, sly1, report, args);
    }
    let c = Call { op: report, args };
    Ok(match op {
        54 => Value::Float(math::sqrt(c.f(0)?)),
        59 => Value::Float(acos(c.f(0)?)),
        61 => Value::Float(wrap_angle(c.f(0)?)),
        62 => Value::Float(math::atan2(c.f(0)?, c.f(1)?)),
        72 => {
            let (angle, axis) = angle_axis(c.v(0)?, c.v(1)?);
            Value::list([Value::Float(angle), Value::Vector(axis)])
        }
        76 => {
            let (Value::Matrix(a), Value::Matrix(b)) = (c.arg(0), c.arg(1)) else {
                return Err(c.bad(if matches!(c.arg(0), Value::Matrix(_)) { 1 } else { 0 }));
            };
            Value::Matrix(Box::new(interpolate(a, b, c.f(2)?)))
        }
        79 => {
            let Value::Matrix(m) = c.arg(0) else { return Err(c.bad(0)) };
            Value::Vector(euler(m))
        }
        83 => {
            let Value::Coeffs(g) = c.arg(0) else { return Err(c.bad(0)) };
            let x = c.f(1)?;
            Value::Float(g[0] + g[1] * x + g[2] * x * x)
        }
        135 | 136 => {
            let mut acc: i32 = if op == 135 { -1 } else { 0 };
            for i in 0..args.len() {
                let v = c.i(i)?;
                acc = if op == 135 { acc & v } else { acc | v };
            }
            Value::Int(acc)
        }
        32 => {
            if !c.arg(1).is_list() {
                return Err(c.bad(1));
            }
            Value::list_with_tail([c.arg(0).clone()], c.arg(1).clone())
        }
        36 => match c.arg(0) {
            Value::List(l) if c.arg(1).is_list() => {
                *l.cdr.borrow_mut() = c.arg(1).clone();
                Value::Void
            }
            Value::List(_) => return Err(c.bad(1)),
            _ => return Err(c.bad(0)),
        },
        41 => {
            if !c.arg(0).is_list() {
                return Err(c.bad(0));
            }
            let mut items = c.arg(0).to_vec();
            it.charge(items.len())?;
            // Fisher-Yates with the game's random numbers.
            for i in (1..items.len()).rev() {
                let j = ((host.random() * (i + 1) as f32) as usize).min(i);
                items.swap(i, j);
            }
            Value::list(items)
        }
        47 => Value::Vector(make_vector(&c)?),
        48 => Value::Matrix(Box::new(make_matrix(&c)?)),
        105 => {
            let delay = c.f(0)?;
            if delay.is_nan() {
                return Err(c.bad(0));
            }
            if !c.arg(1).is_callable() {
                return Err(c.bad(1));
            }
            let id = it.schedule(ScheduledCall::new(delay, c.arg(1).clone(), args[2..].to_vec()))?;
            Value::Int(id as i32)
        }
        106 => {
            let id = c.i(0)?;
            if let Ok(id) = u32::try_from(id) {
                it.cancel(id);
            }
            Value::Void
        }
        // Looked up in the calling environment by the interpreter; called indirectly (through 43 to 45, say), the
        // world environment is searched.
        133 => match *c.arg(0) {
            Value::Symbol(s) => it.global_lookup(s).unwrap_or(Value::Builtin(134)),
            // The payload is used as a symbol ID whatever the value's kind (scripts.md 4.3).
            Value::Int(i) => it.global_lookup(i as u32).unwrap_or(Value::Builtin(134)),
            _ => return Err(c.bad(0)),
        },
        _ if HOST_OPERATIONS.contains(&op) => match host.game_operation(op, args) {
            Value::Error(e) => return Err(e),
            v => v,
        },
        _ => return Err(ScriptError::UnknownOperation(op)),
    })
}

/// Arc cosine, clamped to the domain.
pub fn acos(x: f32) -> f32 {
    let x = x.clamp(-1.0, 1.0);
    math::atan2(math::sqrt(1.0 - x * x), x)
}

/// Operation 61 (scripts.md 4.3): an angle within −π..π (both ends included) is unchanged; any other angle a
/// becomes ((a + π) mod 2π) − π with a non-negative modulo.
pub fn wrap_angle(a: f32) -> f32 {
    use core::f32::consts::{PI, TAU};
    if !a.is_finite() || (-PI..=PI).contains(&a) {
        return a;
    }
    let x = a + PI;
    let mut m = x - TAU * math::floor(x / TAU);
    if m >= TAU {
        m -= TAU;
    }
    m - PI
}

/// Shortest vector length treated as a direction by operation 72.
const ANGLE_AXIS_EPSILON: f32 = 1e-6;

/// Operation 72 (scripts.md 4.3): the angle between `a` and `b` (0..π, from the clamped cosine of their normalised
/// dot product) and the unit axis along a × b. A vector shorter than a small epsilon gives (0, (1, 0, 0)); parallel
/// vectors (cross product shorter than about 1e-8) give a unit vector perpendicular to `a`.
pub fn angle_axis(a: [f32; 3], b: [f32; 3]) -> (f32, [f32; 3]) {
    let (la, lb) = (math::length(a), math::length(b));
    if la < ANGLE_AXIS_EPSILON || lb < ANGLE_AXIS_EPSILON {
        return (0.0, [1.0, 0.0, 0.0]);
    }
    let (na, nb) = (a.map(|x| x / la), b.map(|x| x / lb));
    let angle = acos(math::dot(na, nb));
    let c = math::cross(na, nb);
    let lc = math::length(c);
    if lc >= 1e-8 {
        return (angle, c.map(|x| x / lc));
    }
    // Parallel: cross `a` with the world axis it is least aligned with.
    let (x, y, z) = (math::abs(na[0]), math::abs(na[1]), math::abs(na[2]));
    let pick = if x <= y && x <= z {
        [1.0, 0.0, 0.0]
    } else if y <= z {
        [0.0, 1.0, 0.0]
    } else {
        [0.0, 0.0, 1.0]
    };
    (angle, math::normalize(math::cross(na, pick)))
}

/// Rotation of a script matrix (rows 0 to 2 the images of the local axes) as a unit quaternion (w, x, y, z).
fn quaternion(m: &[f32; 16]) -> [f32; 4] {
    // r(i, j): row i, column j of the rotation whose columns are the axis images.
    let r = |i: usize, j: usize| m[j * 4 + i];
    let trace = r(0, 0) + r(1, 1) + r(2, 2);
    let q = if trace > 0.0 {
        let s = math::sqrt(trace + 1.0) * 2.0;
        [0.25 * s, (r(2, 1) - r(1, 2)) / s, (r(0, 2) - r(2, 0)) / s, (r(1, 0) - r(0, 1)) / s]
    } else if r(0, 0) > r(1, 1) && r(0, 0) > r(2, 2) {
        let s = math::sqrt(1.0 + r(0, 0) - r(1, 1) - r(2, 2)) * 2.0;
        [(r(2, 1) - r(1, 2)) / s, 0.25 * s, (r(0, 1) + r(1, 0)) / s, (r(0, 2) + r(2, 0)) / s]
    } else if r(1, 1) > r(2, 2) {
        let s = math::sqrt(1.0 + r(1, 1) - r(0, 0) - r(2, 2)) * 2.0;
        [(r(0, 2) - r(2, 0)) / s, (r(0, 1) + r(1, 0)) / s, 0.25 * s, (r(1, 2) + r(2, 1)) / s]
    } else {
        let s = math::sqrt(1.0 + r(2, 2) - r(0, 0) - r(1, 1)) * 2.0;
        [(r(1, 0) - r(0, 1)) / s, (r(0, 2) + r(2, 0)) / s, (r(1, 2) + r(2, 1)) / s, 0.25 * s]
    };
    let n = math::sqrt(q.iter().map(|v| v * v).sum());
    if n > 0.0 { q.map(|v| v / n) } else { [1.0, 0.0, 0.0, 0.0] }
}

/// Operation 76 (scripts.md 4.3): `a`'s rotation turned by the fraction `t` of the rotation (angle 0..π, about its
/// axis) that takes it to `b`'s; t outside 0..1 extrapolates. The position row is (0, 0, 0).
pub fn interpolate(a: &[f32; 16], b: &[f32; 16], t: f32) -> [f32; 16] {
    let qa = quaternion(a);
    let mut qb = quaternion(b);
    let mut d: f32 = (0..4).map(|i| qa[i] * qb[i]).sum();
    if d < 0.0 {
        qb = qb.map(|v| -v);
        d = -d;
    }
    let (wa, wb) = if d > 0.9995 {
        (1.0 - t, t)
    } else {
        let theta = acos(d);
        let s = math::sin_cos(theta).0;
        (math::sin_cos((1.0 - t) * theta).0 / s, math::sin_cos(t * theta).0 / s)
    };
    let q = [0, 1, 2, 3].map(|i| wa * qa[i] + wb * qb[i]);
    let n = math::sqrt(q.iter().map(|v| v * v).sum());
    let [w, x, y, z] = q.map(|v| v / n);
    // Columns of the rotation are the axis images; the script matrix stores them as rows.
    let cols = [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y + w * z), 2.0 * (x * z - w * y)],
        [2.0 * (x * y - w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z + w * x)],
        [2.0 * (x * z + w * y), 2.0 * (y * z - w * x), 1.0 - 2.0 * (x * x + y * y)],
    ];
    let mut out = [0.0; 16];
    for (row, axis) in cols.iter().enumerate() {
        out[row * 4..row * 4 + 3].copy_from_slice(axis);
    }
    out[15] = 1.0;
    out
}

/// Operation 79 (scripts.md 4.3): angles (x, y, z) in radians with rotation = Rz(z) Ry(y) Rx(x); mij is row i,
/// component j. y = asin(−m02); x = atan2(m12, m22), z = atan2(m01, m00); near the poles (|y| ≥ 1.5707) x is 0 and
/// z = atan2(−m10, m20) when y > 0, atan2(−m10, −m20) when y < 0.
pub fn euler(m: &[f32; 16]) -> [f32; 3] {
    // The game's pole threshold, slightly below pi / 2.
    #[allow(clippy::approx_constant)]
    const POLE: f32 = 1.5707;
    let at = |i: usize, j: usize| m[i * 4 + j];
    let sy = (-at(0, 2)).clamp(-1.0, 1.0);
    let y = math::atan2(sy, math::sqrt(1.0 - sy * sy));
    if math::abs(y) < POLE {
        [math::atan2(at(1, 2), at(2, 2)), y, math::atan2(at(0, 1), at(0, 0))]
    } else if y > 0.0 {
        [0.0, y, math::atan2(-at(1, 0), at(2, 0))]
    } else {
        [0.0, y, math::atan2(-at(1, 0), -at(2, 0))]
    }
}

/// Operation 47 (scripts.md 4.3): no argument (0, 0, 0); one number n (n, n, n); three numbers (x, y, z). Integers
/// are converted to float; the game handles no other count.
pub(crate) fn make_vector(c: &Call<'_>) -> Result<[f32; 3], ScriptError> {
    match c.args.len() {
        0 => Ok([0.0; 3]),
        1 => Ok([c.f(0)?; 3]),
        3 => Ok([c.f(0)?, c.f(1)?, c.f(2)?]),
        n => Err(c.bad(n.min(255))),
    }
}

/// Operation 48 (scripts.md 4.3): from the identity. One number n: n on the rotation's three diagonal entries;
/// three or four vectors: rows 0 to 2 (and 3, the position); nine or sixteen numbers: a 3 x 3 or 4 x 4 block filled
/// row by row. Other counts give the identity.
pub(crate) fn make_matrix(c: &Call<'_>) -> Result<[f32; 16], ScriptError> {
    let mut m = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    match c.args.len() {
        1 => {
            let n = c.f(0)?;
            for i in 0..3 {
                m[i * 5] = n;
            }
        }
        3 | 4 => {
            for row in 0..c.args.len() {
                let v = c.v(row)?;
                m[row * 4..row * 4 + 3].copy_from_slice(&v);
            }
        }
        9 => {
            for i in 0..9 {
                m[(i / 3) * 4 + i % 3] = c.f(i)?;
            }
        }
        16 => {
            for (i, x) in m.iter_mut().enumerate() {
                *x = c.f(i)?;
            }
        }
        _ => {}
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ninety_seven_operations() {
        assert_eq!(defined().count(), 97);
        for op in defined() {
            assert!(
                sly1_equivalent(op).is_some()
                    || HOST_OPERATIONS.contains(&op)
                    || [54, 59, 61, 62, 72, 76, 79, 83, 135, 136, 32, 36, 41, 47, 48, 105, 106, 133].contains(&op),
                "op {op} has no implementation"
            );
        }
        for op in HOST_OPERATIONS {
            assert!(arity(op).is_some() && sly1_equivalent(op).is_none());
        }
    }

    #[test]
    fn angles_and_rotations() {
        use core::f32::consts::{FRAC_PI_2, PI};
        assert!((acos(0.0) - FRAC_PI_2).abs() < 1e-6 && (acos(-1.0) - PI).abs() < 1e-6 && acos(2.0) == 0.0);
        // Outside -pi..pi: ((a + pi) mod 2pi) - pi; the ends themselves are kept.
        assert!((wrap_angle(3.0 * PI) + PI).abs() < 1e-5);
        assert_eq!(wrap_angle(PI), PI);
        assert!((wrap_angle(-0.5) + 0.5).abs() < 1e-6);
        // Angle and axis: 90 degrees about +Z from X to Y; a short vector gives (0, X); parallel ones an axis
        // perpendicular to the first.
        let (a, axis) = angle_axis([2.0, 0.0, 0.0], [0.0, 3.0, 0.0]);
        assert!((a - FRAC_PI_2).abs() < 1e-5 && (axis[2] - 1.0).abs() < 1e-5, "{a} {axis:?}");
        assert_eq!(angle_axis([0.0; 3], [1.0, 0.0, 0.0]), (0.0, [1.0, 0.0, 0.0]));
        let (a, axis) = angle_axis([0.0, 0.0, 1.0], [0.0, 0.0, -2.0]);
        assert!((a - PI).abs() < 1e-5 && axis[2].abs() < 1e-6 && (math::length(axis) - 1.0).abs() < 1e-5);
        // A 90-degree turn about Z, halfway: 45 degrees; the position row is zero.
        let id = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        let rz = [0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 0.0, 0.0, 1.0];
        let half = interpolate(&id, &rz, 0.5);
        let s = core::f32::consts::FRAC_1_SQRT_2;
        assert!((half[0] - s).abs() < 1e-5 && (half[1] - s).abs() < 1e-5 && half[12] == 0.0, "{half:?}");
        let mut rot = rz;
        rot[12] = 0.0;
        assert_eq!(interpolate(&id, &rz, 1.0).map(|v| (v * 1e4).round()), rot.map(|v| (v * 1e4).round()));
        let e = euler(&rz);
        assert!(e[0].abs() < 1e-6 && e[1].abs() < 1e-6 && (e[2] - FRAC_PI_2).abs() < 1e-5, "{e:?}");
        assert_eq!(euler(&id), [0.0, 0.0, 0.0]);
    }
}
