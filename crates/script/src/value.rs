//! Run-time values (scripts.md section 2).

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::fmt;

use crate::env::EnvRef;
use crate::tree::NodeId;

/// Where a nesting limit was reached, for tracing: the event handler running then (object, event, the handler's
/// module), or for an event dropped because raising it would nest too deeply, that event. All `None` outside any
/// handler.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Site {
    /// The object the event was raised on.
    pub object: Option<ObjectId>,
    /// The event number.
    pub event: Option<u32>,
    /// The module whose handler runs (or would run).
    pub module: Option<u32>,
}

/// An engine object, as the host names it. The interpreter never looks inside; the null object is
/// `Value::Object(None)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u32);

/// Why an operation failed. Failures are values, never panics (scripts.md section 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptError {
    /// A symbol with no binding.
    Unbound(u32),
    /// The head of a call is not a built-in, a procedure or a method.
    NotCallable,
    /// A call with fewer arguments than the callee needs.
    TooFewArguments {
        /// Arguments given.
        given: u8,
        /// Arguments needed.
        needed: u8,
    },
    /// A call with more arguments than a Sly 2 built-in allows (its table entry allows no more than its minimum).
    TooManyArguments {
        /// Arguments given.
        given: u8,
        /// Arguments allowed.
        allowed: u8,
    },
    /// A built-in got an argument of the wrong kind (argument index from 0).
    BadArgument {
        /// Operation ID.
        op: u32,
        /// Argument index.
        index: u8,
    },
    /// A built-in operation ID the spec does not define.
    UnknownOperation(u32),
    /// A keyword kind that never occurs on the disc (3, 4, 16, 18; 19 outside a formals list) or `else` outside
    /// `cond`/`case`.
    UnknownKeyword(u8),
    /// A special form with the wrong shape (the keyword kind).
    BadForm(u8),
    /// A cell kind that is not a value (the kind).
    BadCell(u8),
    /// Integer division or modulo by zero.
    DivideByZero,
    /// No script option mapping was given to the interpreter.
    NoOptionMap,
    /// The host does not know the object's class.
    NoClass,
    /// The object's class has no option with this script option ID.
    NoOption {
        /// Class ID.
        class: i16,
        /// Script option ID.
        option: u32,
    },
    /// Custom properties need IDs from 1173 up.
    NotCustom(u32),
    /// The host refused or does not implement an operation (operation ID or option ID, host's choice).
    Host(u32),
    /// Nesting deeper than the interpreter's limit, with where it happened.
    TooDeep(Site),
    /// The step budget of one entry into the interpreter ran out (an endless loop, for instance).
    OutOfSteps,
    /// A list would exceed [`crate::Limits::max_list_len`], or the scheduled-call queue is full.
    TooLarge,
    /// A procedure whose environment was collected (a value kept outside the interpreter too long).
    StaleEnvironment,
}

/// A list cell. Lists are chains of cells ending in [`Value::Nil`]; identity is the cell's address.
///
/// Both halves can be replaced in place (Sly 1 operation 35; Sly 2 operations 35 and 36), so a list can contain
/// itself or loop back on itself. Every walk therefore carries its own limit: [`ListIter`] stops after
/// [`MAX_LIST_ITEMS`] elements, [`equal`] and `Display` after [`MAX_WALK_ITEMS`], and the collector keeps a visited
/// set.
#[derive(Debug)]
pub struct Cons {
    /// The element.
    pub car: RefCell<Value>,
    /// The rest of the list.
    pub cdr: RefCell<Value>,
}

/// Most elements one walk of a list yields; a longer chain is a list that loops back on itself.
pub const MAX_LIST_ITEMS: usize = 1 << 20;

impl Drop for Cons {
    /// Frees long and deeply nested lists without recursion: cells this one owns alone are taken apart on an
    /// explicit stack.
    fn drop(&mut self) {
        let mut stack: Vec<Rc<Cons>> = Vec::new();
        let take = |v: Value, stack: &mut Vec<Rc<Cons>>| {
            if let Value::List(c) = v {
                stack.push(c);
            }
        };
        take(core::mem::replace(self.car.get_mut(), Value::Nil), &mut stack);
        take(core::mem::replace(self.cdr.get_mut(), Value::Nil), &mut stack);
        while let Some(rc) = stack.pop() {
            // Only cells nobody else holds are taken apart; dropping a shared handle just lowers its count.
            if let Ok(mut cell) = Rc::try_unwrap(rc) {
                take(core::mem::replace(cell.car.get_mut(), Value::Nil), &mut stack);
                take(core::mem::replace(cell.cdr.get_mut(), Value::Nil), &mut stack);
            }
        }
    }
}

/// A procedure made by `lambda`.
#[derive(Debug)]
pub struct Procedure {
    /// The `lambda` list node (its body is elements 2 onwards).
    pub node: NodeId,
    /// Formal parameters.
    pub params: Vec<u32>,
    /// Rest parameter, if any.
    pub rest: Option<u32>,
    /// The defining environment.
    pub env: EnvRef,
}

/// A script value.
#[derive(Clone, Debug)]
pub enum Value {
    /// The empty list, also "none".
    Nil,
    /// The result of a statement.
    Void,
    /// Integer.
    Int(i32),
    /// Float.
    Float(f32),
    /// Vector.
    Vector([f32; 3]),
    /// 4x4 matrix in stored order (see `docs/script.md` for the layout the built-ins assume).
    Matrix(Box<[f32; 16]>),
    /// Quadratic coefficients g0, g1, g2.
    Coeffs([f32; 3]),
    /// (min, max).
    Range([f32; 2]),
    /// Smoothing parameters.
    Smoothing([f32; 3]),
    /// Boolean.
    Bool(bool),
    /// A symbol as data (inside `quote`).
    Symbol(u32),
    /// A keyword as data (inside `quote`).
    Keyword(u8),
    /// Built-in operation.
    Builtin(u32),
    /// A non-empty list.
    List(Rc<Cons>),
    /// An object; `None` is the null object.
    Object(Option<ObjectId>),
    /// A procedure.
    Procedure(Rc<Procedure>),
    /// An object's action option bound to the object (scripts.md section 8). `option` is the stream option ID.
    Method {
        /// The object.
        object: ObjectId,
        /// Stream option ID of the action.
        option: u16,
    },
    /// A failed operation.
    Error(ScriptError),
}

impl Value {
    /// The truth rule: only the boolean false is false.
    pub fn is_true(&self) -> bool {
        !matches!(self, Value::Bool(false))
    }

    /// Whether this is an error value.
    pub fn is_error(&self) -> bool {
        matches!(self, Value::Error(_))
    }

    /// The number as `f32` (integers converted).
    pub fn as_f32(&self) -> Option<f32> {
        match *self {
            Value::Int(i) => Some(i as f32),
            Value::Float(f) => Some(f),
            _ => None,
        }
    }

    /// The number as an integer (floats truncated, booleans 0 or 1).
    pub fn as_i32(&self) -> Option<i32> {
        match *self {
            Value::Int(i) => Some(i),
            Value::Float(f) => Some(crate::math::trunc(f) as i32),
            Value::Bool(b) => Some(b as i32),
            _ => None,
        }
    }

    /// The object, or `Some(None)` for the null object and the empty list; `None` for anything else.
    pub fn as_object(&self) -> Option<Option<ObjectId>> {
        match *self {
            Value::Object(o) => Some(o),
            Value::Nil => Some(None),
            _ => None,
        }
    }

    /// Whether this is a list (empty or not).
    pub fn is_list(&self) -> bool {
        matches!(self, Value::Nil | Value::List(_))
    }

    /// Whether this can be called.
    pub fn is_callable(&self) -> bool {
        matches!(self, Value::Builtin(_) | Value::Procedure(_) | Value::Method { .. })
    }

    /// A list of the given values.
    pub fn list<I>(items: I) -> Value
    where
        I: IntoIterator<Item = Value>,
        I::IntoIter: DoubleEndedIterator,
    {
        Self::list_with_tail(items, Value::Nil)
    }

    /// The given values followed by `tail`.
    pub fn list_with_tail<I>(items: I, tail: Value) -> Value
    where
        I: IntoIterator<Item = Value>,
        I::IntoIter: DoubleEndedIterator,
    {
        items
            .into_iter()
            .rev()
            .fold(tail, |cdr, car| Value::List(Rc::new(Cons { car: RefCell::new(car), cdr: RefCell::new(cdr) })))
    }

    /// The elements of a list (empty for anything that is not a list).
    pub fn iter(&self) -> ListIter {
        ListIter { at: self.clone(), left: MAX_LIST_ITEMS }
    }

    /// The elements of a list collected into a vector.
    pub fn to_vec(&self) -> Vec<Value> {
        self.iter().collect()
    }
}

/// Iterator over list elements; it stops after [`MAX_LIST_ITEMS`] (a list that loops back on itself).
pub struct ListIter {
    at: Value,
    left: usize,
}

impl Iterator for ListIter {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        let Value::List(c) = core::mem::replace(&mut self.at, Value::Nil) else { return None };
        self.at = c.cdr.borrow().clone();
        let v = c.car.borrow().clone();
        Some(v)
    }
}

/// Operation 12, "same value": same kind and equal; numbers by value, objects, lists and procedures by identity.
pub fn same_value(a: &Value, b: &Value) -> bool {
    use Value::*;
    match (a, b) {
        (Nil, Nil) | (Void, Void) => true,
        (Int(x), Int(y)) => x == y,
        (Float(x), Float(y)) => x == y,
        (Vector(x), Vector(y)) | (Coeffs(x), Coeffs(y)) | (Smoothing(x), Smoothing(y)) => x == y,
        (Range(x), Range(y)) => x == y,
        (Matrix(x), Matrix(y)) => x == y,
        (Bool(x), Bool(y)) => x == y,
        (Symbol(x), Symbol(y)) | (Builtin(x), Builtin(y)) => x == y,
        (Keyword(x), Keyword(y)) => x == y,
        (List(x), List(y)) => Rc::ptr_eq(x, y),
        (Object(x), Object(y)) => x == y,
        (Procedure(x), Procedure(y)) => Rc::ptr_eq(x, y),
        (Method { object: o1, option: p1 }, Method { object: o2, option: p2 }) => o1 == o2 && p1 == p2,
        (Error(x), Error(y)) => x == y,
        _ => false,
    }
}

/// Deepest list nesting [`equal`] and `Display` follow.
pub const MAX_WALK_DEPTH: usize = 256;
/// Most elements [`equal`] compares and `Display` prints in one call.
pub const MAX_WALK_ITEMS: usize = 1 << 16;

/// Operation 13, "equal": as [`same_value`] but lists element by element. Two handles to the same cell are equal
/// without a walk. Lists nested deeper than [`MAX_WALK_DEPTH`], or needing more than [`MAX_WALK_ITEMS`]
/// comparisons (a list that contains itself, for instance), compare unequal.
pub fn equal(a: &Value, b: &Value) -> bool {
    let mut stack: Vec<(ListIter, ListIter)> = Vec::new();
    let mut next = Some((a.clone(), b.clone()));
    let mut work = 0usize;
    loop {
        if let Some((x, y)) = next.take() {
            match (&x, &y) {
                (Value::List(p), Value::List(q)) => {
                    if !Rc::ptr_eq(p, q) {
                        if stack.len() >= MAX_WALK_DEPTH {
                            return false;
                        }
                        stack.push((x.iter(), y.iter()));
                    }
                }
                _ => {
                    if !same_value(&x, &y) {
                        return false;
                    }
                }
            }
        }
        let Some((xs, ys)) = stack.last_mut() else { return true };
        work += 1;
        if work > MAX_WALK_ITEMS {
            return false;
        }
        match (xs.next(), ys.next()) {
            (None, None) => {
                stack.pop();
            }
            (Some(p), Some(q)) => next = Some((p, q)),
            _ => return false,
        }
    }
}

/// Writes a value; lists are walked with an explicit stack and cut short with `...` past [`MAX_WALK_DEPTH`]
/// levels or [`MAX_WALK_ITEMS`] elements.
fn write_value(f: &mut fmt::Formatter<'_>, v: &Value) -> fmt::Result {
    let Value::List(_) = v else { return write_atom(f, v) };
    let mut stack: Vec<ListIter> = alloc::vec![v.iter()];
    let mut first = true;
    let mut items = 0usize;
    f.write_str("(")?;
    while let Some(top) = stack.last_mut() {
        let Some(item) = top.next() else {
            stack.pop();
            f.write_str(")")?;
            first = false;
            continue;
        };
        if !first {
            f.write_str(" ")?;
        }
        first = false;
        items += 1;
        if items > MAX_WALK_ITEMS {
            f.write_str("...")?;
            for _ in 0..stack.len() {
                f.write_str(")")?;
            }
            return Ok(());
        }
        match item {
            Value::List(_) if stack.len() >= MAX_WALK_DEPTH => f.write_str("(...)")?,
            Value::List(_) => {
                f.write_str("(")?;
                stack.push(item.iter());
                first = true;
            }
            other => write_atom(f, &other)?,
        }
    }
    Ok(())
}

fn write_atom(f: &mut fmt::Formatter<'_>, v: &Value) -> fmt::Result {
    match v {
        Value::Nil => f.write_str("()"),
        Value::Void => f.write_str("#void"),
        Value::Int(i) => write!(f, "{i}"),
        Value::Float(x) => write!(f, "{x:?}"),
        Value::Vector(v) | Value::Coeffs(v) | Value::Smoothing(v) => {
            write!(f, "#({:?} {:?} {:?})", v[0], v[1], v[2])
        }
        Value::Matrix(_) => f.write_str("#matrix"),
        Value::Range(r) => write!(f, "#({:?} {:?})", r[0], r[1]),
        Value::Bool(b) => f.write_str(if *b { "#t" } else { "#f" }),
        Value::Symbol(s) => write!(f, "s{s}"),
        Value::Keyword(k) => f.write_str(crate::tree::keyword_name(*k)),
        Value::Builtin(o) => write!(f, "op{o}"),
        Value::List(_) => f.write_str("(...)"),
        Value::Object(None) => f.write_str("null-object"),
        Value::Object(Some(o)) => write!(f, "#object{}", o.0),
        Value::Procedure(_) => f.write_str("#procedure"),
        Value::Method { object, option } => write!(f, "#method{}:{}", object.0, option),
        Value::Error(e) => write!(f, "#error({e:?})"),
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_value(f, self)
    }
}
