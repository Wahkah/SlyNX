//! Sly 3 specifics (specs/sly3/scripts.md 4 to 6): the built-in table and event signatures read from the player's
//! executable, the operation numbering (each Sly 3 operation runs as its Sly 2 equivalent, 4.2), and the operations
//! Sly 3 added.

use alloc::vec::Vec;

use crate::builtins::{self, Call};
use crate::host::ScriptHost;
use crate::interp::Interpreter;
use crate::math;
use crate::option_map::MapError;
use crate::sly2::{BuiltinTable, EventSignatures};
use crate::tree::kind;
use crate::value::{ScriptError, Value};

/// The built-in table: address and entry count (scripts.md 4.1). A zero word follows it at [`BUILTIN_TABLE_END`].
pub const BUILTIN_TABLE: (u32, usize) = (0x0034_2030, 178);
/// Address of the zero word after the built-in table.
pub const BUILTIN_TABLE_END: u32 = 0x0034_2888;
/// Event signature index: 34 `s32` start positions, with no −1 after them (scripts.md 6).
pub const EVENT_INDEX: u32 = 0x002E_DDC0;
/// Event signature type codes, one `u32` per argument (55 words, up to the index).
pub const EVENT_TYPES: u32 = 0x002E_DCE4;
/// Number of events.
pub const EVENT_COUNT: usize = 34;

/// Reads the built-in table from the supported Sly 3 executable. Refuses any other executable.
pub fn builtin_table(exe: &formats_sly3::exe::Executable) -> Result<BuiltinTable, MapError> {
    if !formats_sly3::exe::is_supported(exe) {
        return Err(MapError::UnsupportedExecutable);
    }
    BuiltinTable::read_at(|va, len| exe.bytes_at(va, len).ok(), BUILTIN_TABLE)
}

/// Reads the event signatures from the supported Sly 3 executable: event 32's one type word is the index's first
/// word and event 33's signature is empty (scripts.md 6).
pub fn event_signatures(exe: &formats_sly3::exe::Executable) -> Result<EventSignatures, MapError> {
    if !formats_sly3::exe::is_supported(exe) {
        return Err(MapError::UnsupportedExecutable);
    }
    read_event_signatures(|va, len| exe.bytes_at(va, len).ok())
}

/// Reads the event signatures through `read(address, length)`. No hash check.
pub fn read_event_signatures<'a>(read: impl Fn(u32, usize) -> Option<&'a [u8]>) -> Result<EventSignatures, MapError> {
    EventSignatures::read_layout(read, EVENT_INDEX, EVENT_TYPES, EVENT_COUNT, false)
}

/// The argument rule (minimum, more allowed) of each operation that occurs on the disc, as scripts.md 4.3 gives it.
/// Other IDs are unknown operations. Operation 31 (not) is allowed a surplus argument, which is evaluated and
/// ignored (one call on the disc has one; scripts.md 4.1).
pub const fn arity(op: u32) -> Option<(u8, bool)> {
    Some(match op {
        0..=3 => (1, true),
        7..=12 => (2, false),
        19 | 25 | 27 => (1, false),
        30 => (3, false),
        31 => (1, true),
        32 => (2, false),
        33 | 34 => (1, false),
        35 | 36 => (2, false),
        37 => (1, false),
        38 | 39 => (2, false),
        40 => (0, true),
        41 => (1, false),
        42 => (0, true),
        43..=46 => (2, false),
        47 => (1, false),
        48 | 49 => (0, true),
        50 => (3, false),
        51 => (2, false),
        52 => (3, false),
        53 => (1, true),
        54 => (2, false),
        55 => (0, false),
        56..=59 | 61 | 63 => (1, false),
        64..=66 => (2, false),
        67 => (1, false),
        68..=70 => (1, true),
        71 => (2, false),
        73 => (3, true),
        74 => (2, false),
        78 => (3, false),
        81 => (1, false),
        83 => (2, true),
        84 => (1, false),
        85 => (2, false),
        86 => (3, false),
        87 => (4, false),
        88 | 89 => (2, false),
        90 | 91 => (4, false),
        92 | 94..=96 => (1, false),
        97 | 98 => (1, true),
        99 => (2, false),
        100 => (3, false),
        101..=105 | 107..=109 => (0, false),
        110 => (2, false),
        111 => (2, true),
        112 => (1, false),
        113 => (2, false),
        114..=116 => (3, false),
        117 => (2, false),
        118..=121 => (2, true),
        123..=127 => (0, false),
        128..=130 => (2, false),
        131..=134 => (3, false),
        137 => (2, false),
        138 => (2, true),
        139 | 141 => (1, false),
        140 => (2, false),
        143 | 144 => (0, true),
        145 => (0, false),
        146 => (1, false),
        147 => (2, false),
        _ => return None,
    })
}

/// The IDs of every operation [`arity`] defines (the 116 that occur on the disc), in increasing order.
pub fn defined() -> impl Iterator<Item = u32> {
    (0..BUILTIN_TABLE.1 as u32).filter(|&op| arity(op).is_some())
}

/// The Sly 2 operation a Sly 3 operation corresponds to (scripts.md 4.2). `None` for the 11 operations Sly 3 added
/// (46, 55, 90, 91, 100, 102, 109, 110, 126, 127, 148) and for IDs past the table.
pub const fn sly2_equivalent(op: u32) -> Option<u32> {
    Some(match op {
        0..=45 => op,
        47..=54 => op - 1,
        56..=89 => op - 2,
        92..=99 => op - 4,
        101 => 96,
        103 => 97,
        104..=108 => op - 4,
        111..=125 => op - 6,
        128..=147 => op - 8,
        149..=177 => op - 9,
        _ => return None,
    })
}

/// Sly 3 operations whose Sly 2 equivalent occurs nowhere on the Sly 2 disc, so Sly 2 does not implement it, but
/// whose meaning (scripts.md 4.3) is a Sly 1 operation's: cross product (66), launch velocity (73), tilt a matrix
/// upright (84), coefficients within a range (86) and "is within a range" (89). Gives the Sly 1 operation.
pub const fn sly1_direct(op: u32) -> Option<u32> {
    Some(match op {
        66 => 64,
        73 => 71,
        84 => 82,
        86 => 84,
        89 => 87,
        _ => return None,
    })
}

/// Operations whose meaning belongs to the game, passed to [`ScriptHost::game_operation`] with their Sly 3 numbers
/// and arguments: the current world (103), fixed game objects (104, 105, 126, 145), the adaptive-difficulty value
/// (107), the game-state value (108), the world's condition (109), the scaled count (110), another object search
/// (120), the first object of a world list (127), objects in a box or sphere (129, 130), segment hits (131, 132),
/// stop a sound (139), set a progress variable (140) and the pickpocket items (146, 147).
pub const HOST_OPERATIONS: [u32; 19] =
    [103, 104, 105, 107, 108, 109, 110, 120, 126, 127, 129, 130, 131, 132, 139, 140, 145, 146, 147];

/// Deepest nesting operation 47 evaluates (quoted data can loop back on itself).
const EVAL_DEPTH: u32 = 64;

/// Runs Sly 3 operation `op` (the interpreter has checked its argument rule).
pub(crate) fn run(
    it: &mut Interpreter,
    host: &mut dyn ScriptHost,
    op: u32,
    args: &[Value],
) -> Result<Value, ScriptError> {
    let c = Call { op, args };
    if HOST_OPERATIONS.contains(&op) {
        crate::sly2::charge_host_args(it, args)?;
        return match host.game_operation(op, args) {
            Value::Error(e) => Err(e),
            v => Ok(v),
        };
    }
    if let Some(sly1) = sly1_direct(op) {
        return builtins::run(it, host, sly1, op, args);
    }
    Ok(match op {
        19 => Value::Bool(matches!(c.arg(0), Value::Vector(_))),
        // The surplus argument of the one two-argument call is ignored.
        31 => Value::Bool(!c.arg(0).is_true()),
        46 => sort(it, host, &c)?,
        47 => eval_value(it, host, c.arg(0), 0)?,
        52 => Value::Smoothing([c.f(0)?, c.f(1)?, c.f(2)?]),
        55 => Value::Float(f32::MAX),
        59 => {
            let (s, co) = math::sin_cos(c.f(0)?);
            Value::Float(s / co)
        }
        // Working assumption: the vector's component along the normal, (v · n) n.
        71 => {
            let (v, n) = (c.v(0)?, c.v(1)?);
            Value::Vector(math::scale(n, math::dot(v, n)))
        }
        90 => {
            let (v, g, k) = (c.f(0)?, c.f(1)?, step(c.f(2)?, c.f(3)?));
            Value::Float(v + (g - v) * k)
        }
        91 => {
            let (v, g, k) = (c.v(0)?, c.v(1)?, step(c.f(2)?, c.f(3)?));
            Value::Vector([0, 1, 2].map(|i| v[i] + (g[i] - v[i]) * k))
        }
        100 => {
            let (a, b, t) = (c.f(0)?, c.f(1)?, c.f(2)?);
            Value::Float(a + t * (b - a))
        }
        // Working assumption: the real-time clock reads as the game clock.
        102 => Value::Float(host.game_time()),
        // Called indirectly (directly, the interpreter looks the symbol up in the calling environment); unbound,
        // operation 142 as a value (Sly 2's 134).
        141 => match *c.arg(0) {
            Value::Symbol(s) => it.global_lookup(s).unwrap_or(Value::Builtin(142)),
            Value::Int(i) => it.global_lookup(i as u32).unwrap_or(Value::Builtin(142)),
            _ => return Err(c.bad(0)),
        },
        _ => match sly2_equivalent(op) {
            Some(two) => return crate::sly2::run(it, host, two, op, args),
            None => return Err(ScriptError::UnknownOperation(op)),
        },
    })
}

/// A list's elements, refusing one longer than the interpreter's list limit (a list that loops back on itself
/// would otherwise be walked to `MAX_LIST_ITEMS` on every call), and charging them to the step budget.
fn bounded_items(it: &mut Interpreter, v: &Value) -> Result<Vec<Value>, ScriptError> {
    let max = it.limits.max_list_len;
    let items: Vec<Value> = v.iter().take(max + 1).collect();
    if items.len() > max {
        return Err(ScriptError::TooLarge);
    }
    it.charge(items.len())?;
    Ok(items)
}

/// Working assumption for operations 90 and 91: the fraction of the way to the goal covered in one step, the rate
/// times the time step, limited to 0..1.
fn step(rate: f32, dt: f32) -> f32 {
    let k = rate * dt;
    if k.is_nan() { 0.0 } else { k.clamp(0.0, 1.0) }
}

/// Operation 46: a sorted copy of a list, by a comparison procedure that says whether its first argument goes before
/// its second (working assumption on the procedure's sense; the sort is stable).
fn sort(it: &mut Interpreter, host: &mut dyn ScriptHost, c: &Call<'_>) -> Result<Value, ScriptError> {
    if !c.arg(0).is_list() {
        return Err(c.bad(0));
    }
    if !c.arg(1).is_callable() {
        return Err(c.bad(1));
    }
    let f = c.arg(1).clone();
    let items = bounded_items(it, c.arg(0))?;
    it.charge(items.len().saturating_mul(items.len()))?;
    let mut out: Vec<Value> = Vec::with_capacity(items.len());
    for e in items {
        // Insert after every element `e` does not go before, so equal elements keep their order.
        let mut at = out.len();
        while at > 0 && it.apply(host, f.clone(), alloc::vec![e.clone(), out[at - 1].clone()]).is_true() {
            at -= 1;
        }
        out.insert(at, e);
    }
    Ok(Value::list(out))
}

/// Operation 47 (working assumption on the environment): evaluates a value as an expression in the world
/// environment. A symbol gives its value; a list headed by `quote` gives its datum; any other list is a call of its
/// evaluated head with its evaluated elements; anything else is itself.
fn eval_value(it: &mut Interpreter, host: &mut dyn ScriptHost, v: &Value, depth: u32) -> Result<Value, ScriptError> {
    if depth >= EVAL_DEPTH {
        return Err(ScriptError::TooDeep);
    }
    it.charge(1)?;
    Ok(match v {
        &Value::Symbol(s) => it.global_lookup(s).ok_or(ScriptError::Unbound(s))?,
        Value::List(_) => {
            let items = bounded_items(it, v)?;
            if let Some(Value::Keyword(k)) = items.first() {
                return match (*k, items.get(1)) {
                    (kind::QUOTE, Some(d)) => Ok(d.clone()),
                    (k, _) => Err(ScriptError::UnknownKeyword(k)),
                };
            }
            let mut evaluated = Vec::with_capacity(items.len());
            for e in &items {
                evaluated.push(eval_value(it, host, e, depth + 1)?);
            }
            let f = evaluated.remove(0);
            it.apply(host, f, evaluated)
        }
        other => other.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_and_sort_refuse_a_looping_list() {
        use crate::{NullHost, Program};
        let mut it = Interpreter::new(Program::for_game(crate::Game::Sly3), None);
        let mut h = NullHost;
        // (1 2 1 2 ...): the rest of the second cell is the first cell.
        let l = Value::list([Value::Int(1), Value::Int(2)]);
        if let Value::List(c) = &l
            && let Value::List(second) = &*c.cdr.borrow()
        {
            *second.cdr.borrow_mut() = l.clone();
        }
        let r = it.call(&mut h, Value::Builtin(47), alloc::vec![l.clone()]);
        assert!(matches!(r, Value::Error(ScriptError::TooLarge)), "{r:?}");
        let r = it.call(&mut h, Value::Builtin(46), alloc::vec![l, Value::Builtin(8)]);
        assert!(matches!(r, Value::Error(ScriptError::TooLarge)), "{r:?}");
        // An ordinary list still evaluates: (op0 1 2) is 3.
        let call = Value::list([Value::Builtin(0), Value::Int(1), Value::Int(2)]);
        assert!(matches!(it.call(&mut h, Value::Builtin(47), alloc::vec![call]), Value::Int(3)));
    }

    #[test]
    fn numbering() {
        assert_eq!(defined().count(), 116);
        let new = [46, 55, 90, 91, 100, 102, 109, 110, 126, 127, 148];
        for op in 0..BUILTIN_TABLE.1 as u32 {
            assert_eq!(sly2_equivalent(op).is_none(), new.contains(&op), "op {op}");
        }
        // Sly 2's 98 and 99 have no Sly 3 counterpart; every other Sly 2 ID has exactly one.
        let mut seen: Vec<u32> = (0..178).filter_map(sly2_equivalent).collect();
        seen.sort_unstable();
        let expected: Vec<u32> = (0..169).filter(|op| *op != 98 && *op != 99).collect();
        assert_eq!(seen, expected);
        assert_eq!(sly2_equivalent(117), Some(111));
        assert_eq!(sly2_equivalent(123), Some(117));
        assert_eq!(sly2_equivalent(141), Some(133));
        assert_eq!(sly2_equivalent(104), Some(100));
        assert_eq!(sly2_equivalent(108), Some(104));
    }

    #[test]
    fn every_operation_runs_somewhere() {
        for op in defined() {
            let two = sly2_equivalent(op);
            assert!(
                HOST_OPERATIONS.contains(&op)
                    || sly1_direct(op).is_some()
                    || [19, 31, 46, 47, 52, 55, 59, 71, 90, 91, 100, 102, 141].contains(&op)
                    || two.is_some_and(|t| crate::sly2::sly1_equivalent(t).is_some()
                        || [54, 59, 61, 62, 72, 76, 79, 83, 135, 136, 32, 36, 41, 47, 48, 105, 106].contains(&t)),
                "op {op} has no implementation"
            );
        }
        for op in HOST_OPERATIONS {
            assert!(arity(op).is_some(), "{op}");
        }
    }
}
