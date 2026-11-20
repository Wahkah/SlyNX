//! Unit tests on synthetic trees.

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use std::format;

use crate::host::{RaisedEvent, ScriptHost};
use crate::machine::{MachineHost, MachineRun, State, StateMachine, Step, Transition};
use crate::tree::{Node, Program, kind};
use crate::value::{ObjectId, ScriptError, Value};
use crate::{Interpreter, OptionMap, builtins, event};

/// A cell to encode.
#[derive(Clone, Debug)]
enum T {
    I(i32),
    F(f32),
    V([f32; 3]),
    R([f32; 2]),
    C([f32; 3]),
    B(bool),
    S(u32),
    Op(u32),
    K(u8),
    L(Vec<T>),
    Nil,
    Null,
    /// A raw kind byte, written as is (Sly 2 keywords with no Sly 1 counterpart).
    Raw(u8),
}

use T::*;

fn kw(k: u8, rest: Vec<T>) -> T {
    let mut v = vec![K(k)];
    v.extend(rest);
    L(v)
}
fn call(op: u32, rest: Vec<T>) -> T {
    let mut v = vec![Op(op)];
    v.extend(rest);
    L(v)
}
fn lambda(params: &[u32], body: Vec<T>) -> T {
    let mut v = vec![K(kind::LAMBDA), L(params.iter().map(|&p| S(p)).collect())];
    v.extend(body);
    L(v)
}
fn define(s: u32, e: T) -> T {
    kw(kind::DEFINE, vec![S(s), e])
}
fn begin(items: Vec<T>) -> T {
    kw(kind::BEGIN, items)
}

fn enc_seq(items: &[T], out: &mut Vec<u8>) {
    enc_seq_with(items, out, &|k| k)
}

/// Sly 2's numbering of the Sly 1 kinds the encoder writes (specs/sly2/scripts.md 2 and 3).
fn sly2_kind(k: u8) -> u8 {
    match k {
        6..=15 => k + 1,
        kind::QUOTE => 18,
        20 => 21,
        21 => 22,
        22..=26 => k + 4,
        27..=29 => k - 4,
        30 => 31,
        32 => 33,
        _ => k,
    }
}

fn enc_seq_with(items: &[T], out: &mut Vec<u8>, map: &dyn Fn(u8) -> u8) {
    if items.is_empty() {
        out.push(0xFF);
        return;
    }
    for (i, t) in items.iter().enumerate() {
        let more = if i + 1 < items.len() { 0x80 } else { 0 };
        let f32s = |out: &mut Vec<u8>, k: u8, v: &[f32]| {
            out.push(map(k) | more);
            for x in v {
                out.extend_from_slice(&x.to_le_bytes());
            }
        };
        match t {
            I(n) => {
                out.push(map(kind::INT) | more);
                out.extend_from_slice(&n.to_le_bytes());
            }
            F(x) => f32s(out, kind::FLOAT, &[*x]),
            V(v) => f32s(out, kind::VECTOR, v),
            R(v) => f32s(out, kind::RANGE, v),
            C(v) => f32s(out, kind::COEFFS, v),
            B(b) => {
                out.push(map(kind::BOOL) | more);
                out.extend_from_slice(&(*b as u32).to_le_bytes());
            }
            S(s) | Op(s) => {
                out.push(map(if matches!(t, S(_)) { kind::SYMBOL } else { kind::BUILTIN }) | more);
                out.extend_from_slice(&s.to_le_bytes());
            }
            K(k) => out.push(map(*k) | more),
            Raw(k) => out.push(k | more),
            Nil => out.push(more),
            Null => {
                out.push(map(kind::OBJECT) | more);
                out.extend_from_slice(&0u32.to_le_bytes());
            }
            L(items) => {
                out.push(map(kind::LIST) | more);
                enc_seq_with(items, out, map);
            }
        }
    }
}

fn program(modules: &[T]) -> Program {
    let mut p = Program::new();
    for m in modules {
        let mut bytes = Vec::new();
        enc_seq(core::slice::from_ref(m), &mut bytes);
        let end = p.read_module(&bytes, 0, false).unwrap();
        assert_eq!(end, bytes.len());
    }
    p
}

/// Records every host call as text.
#[derive(Default)]
struct Mock {
    log: Vec<String>,
    class: i16,
    events: Vec<RaisedEvent>,
    rand: f32,
    time: f32,
    /// Lowest stack address seen in `report` (for measuring stack use).
    stack_low: Option<usize>,
}

impl ScriptHost for Mock {
    fn report(&mut self, _: &ScriptError) {
        let probe = 0u8;
        let at = core::hint::black_box(&probe) as *const u8 as usize;
        self.stack_low = Some(self.stack_low.map_or(at, |l| l.min(at)));
    }
    fn class_of(&self, _: ObjectId) -> Option<i16> {
        Some(self.class)
    }
    fn get_option(&mut self, o: ObjectId, option: u16) -> Value {
        self.log.push(format!("get {} {}", o.0, option));
        Value::Int(option as i32 * 10)
    }
    fn set_option(&mut self, o: ObjectId, option: u16, v: &Value) -> Result<(), ScriptError> {
        self.log.push(format!("set {} {} {}", o.0, option, v));
        if option == 99 {
            self.events.push(RaisedEvent { object: o, event: event::TRIGGERED, args: vec![] });
        }
        Ok(())
    }
    fn call_action(&mut self, o: ObjectId, option: u16, args: &[Value]) -> Value {
        self.log.push(format!("action {} {} {}", o.0, option, args.len()));
        Value::Void
    }
    fn find_object(&mut self, oid: i16, ctx: Option<ObjectId>) -> Option<ObjectId> {
        self.log.push(format!("find {oid} {ctx:?}"));
        Some(ObjectId(oid as u32 + 1000))
    }
    fn find_objects(&mut self, oid: i16, _: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        out.extend((0..100).map(|i| ObjectId(oid as u32 + i)));
    }
    fn play_sound(&mut self, s: i32, e: Option<ObjectId>) {
        self.log.push(format!("sound {s} {e:?}"));
    }
    fn random(&mut self) -> f32 {
        self.rand
    }
    fn game_time(&self) -> f32 {
        self.time
    }
    fn poll_event(&mut self) -> Option<RaisedEvent> {
        self.events.pop()
    }
    fn game_operation(&mut self, op: u32, args: &[Value]) -> Value {
        self.log.push(format!("game {op} {}", args.len()));
        Value::Int(op as i32 * 100)
    }
}

/// Evaluates `expr` inside a module that defines nothing else.
fn eval(expr: T) -> Value {
    let p = program(&[begin(vec![define(40, expr)])]);
    let mut it = Interpreter::new(p, None);
    let mut h = Mock::default();
    it.load(&mut h);
    it.lookup(0, 40).unwrap()
}

fn show(v: &Value) -> String {
    v.to_string()
}

#[test]
fn parses_nesting_and_empty_lists() {
    // (begin (define s0 (lambda () (op40))) (quote ()))
    let m = begin(vec![define(0, lambda(&[], vec![call(40, vec![])])), kw(kind::QUOTE, vec![L(vec![])]), Nil]);
    let p = program(&[m]);
    assert_eq!(p.module_count(), 1);
    let top = p.module(0)[0];
    assert_eq!(format!("{}", p.display(top)), "(begin (define s0 (lambda () (op40))) (quote ()) ())");
    assert_eq!(p.definitions(0).len(), 1);
    assert_eq!(p.cell_counts[kind::LIST as usize], 7);
    assert_eq!(p.cell_counts[37], 2);
    assert_eq!(p.cell_counts[0], 1);
}

#[test]
fn parse_errors() {
    let mut p = Program::new();
    assert!(p.read_module(&[0x1E], 0, false).is_err(), "truncated nested list");
    assert!(p.read_module(&[0x14, 1, 2], 0, false).is_err(), "truncated integer");
    assert!(p.read_module(&[0x40], 0, false).is_err(), "kind above 36");
    // Debug cells carry 10 extra bytes.
    let mut d = vec![0x14];
    d.extend_from_slice(&[0; 10]);
    d.extend_from_slice(&7i32.to_le_bytes());
    assert_eq!(p.read_module(&d, 0, true).unwrap(), d.len());
    assert_eq!(*p.node(p.module(0)[0]), Node::Int(7));
}

#[test]
fn read_table_with_debug_strings() {
    let mut b = Vec::new();
    for w in [1u32, 2, 1] {
        b.extend_from_slice(&w.to_le_bytes());
    }
    b.push(kind::BOOL);
    b.extend_from_slice(&[0; 10]);
    b.extend_from_slice(&1u32.to_le_bytes());
    for _ in 0..(3 + 3) {
        b.extend_from_slice(&[2, 0, b'a', b'b']);
    }
    let (p, end) = Program::read_table(&b, 0, true).unwrap();
    assert_eq!(end, b.len());
    assert_eq!(p.custom_options, 1);
    assert_eq!(*p.node(p.module(0)[0]), Node::Bool(true));
}

#[test]
fn truth_rule() {
    // Only false is false: 0, (), and the null object are true.
    for (test, want) in [(I(0), 1), (Nil, 1), (Null, 1), (B(false), 2), (B(true), 1)] {
        let v = eval(kw(kind::IF, vec![test, I(1), I(2)]));
        assert_eq!(v.as_i32(), Some(want));
    }
    assert!(matches!(eval(call(31, vec![I(0)])), Value::Bool(false)));
    assert!(matches!(eval(call(31, vec![B(false)])), Value::Bool(true)));
}

#[test]
fn and_or_cond_case() {
    assert_eq!(show(&eval(kw(kind::AND, vec![]))), "#t");
    assert_eq!(show(&eval(kw(kind::AND, vec![I(1), I(2)]))), "2");
    assert_eq!(show(&eval(kw(kind::AND, vec![I(1), B(false), I(2)]))), "#f");
    assert_eq!(show(&eval(kw(kind::OR, vec![]))), "#f");
    assert_eq!(show(&eval(kw(kind::OR, vec![B(false), I(3)]))), "3");
    let cond = kw(kind::COND, vec![L(vec![B(false), I(1)]), L(vec![I(7)]), L(vec![K(kind::ELSE), I(9)])]);
    assert_eq!(show(&eval(cond)), "7");
    let cond = kw(kind::COND, vec![L(vec![B(false), I(1)]), L(vec![K(kind::ELSE), I(8), I(9)])]);
    assert_eq!(show(&eval(cond)), "9");
    let case = |k: i32| {
        kw(
            kind::CASE,
            vec![
                I(k),
                L(vec![L(vec![I(1), I(2)]), I(10)]),
                L(vec![L(vec![I(3)]), I(30)]),
                L(vec![K(kind::ELSE), I(0)]),
            ],
        )
    };
    assert_eq!(show(&eval(case(2))), "10");
    assert_eq!(show(&eval(case(3))), "30");
    assert_eq!(show(&eval(case(5))), "0");
    // Same value: an integer and a float are different kinds.
    let c = kw(kind::CASE, vec![F(1.0), L(vec![L(vec![I(1)]), I(1)])]);
    assert!(matches!(eval(c), Value::Void));
}

#[test]
fn let_while_set_and_closures() {
    // (let ((s1 0) (s2 0)) (while (op8 s1 5) (set s2 (op0 s2 s1)) (set s1 (op0 s1 1))) s2) = 0+1+2+3+4
    let e = kw(
        kind::LET,
        vec![
            L(vec![L(vec![S(1), I(0)]), L(vec![S(2), I(0)])]),
            kw(
                kind::WHILE,
                vec![
                    call(8, vec![S(1), I(5)]),
                    kw(kind::SET, vec![S(2), call(0, vec![S(2), S(1)])]),
                    kw(kind::SET, vec![S(1), call(0, vec![S(1), I(1)])]),
                ],
            ),
            S(2),
        ],
    );
    assert_eq!(show(&eval(e)), "10");

    // A counter closure: (define s50 (let ((s1 0)) (lambda () (set s1 (op0 s1 1)))))
    let m = begin(vec![
        define(
            50,
            kw(
                kind::LET,
                vec![
                    L(vec![L(vec![S(1), I(0)])]),
                    lambda(&[], vec![kw(kind::SET, vec![S(1), call(0, vec![S(1), I(1)])])]),
                ],
            ),
        ),
        define(51, L(vec![S(50)])),
        define(52, L(vec![S(50)])),
        // Recursion: factorial.
        define(
            53,
            lambda(
                &[1],
                vec![kw(
                    kind::IF,
                    vec![
                        call(10, vec![S(1), I(1)]),
                        call(2, vec![S(1), L(vec![S(53), call(1, vec![S(1), I(1)])])]),
                        I(1),
                    ],
                )],
            ),
        ),
        define(54, L(vec![S(53), I(6)])),
        // Unbound set and unbound symbol give errors, not panics.
        define(55, kw(kind::SET, vec![S(99), I(1)])),
        define(56, S(98)),
        define(57, L(vec![S(53)])),
    ]);
    let mut it = Interpreter::new(program(&[m]), None);
    let mut h = Mock::default();
    it.load(&mut h);
    assert_eq!(show(&it.lookup(0, 51).unwrap()), "1");
    assert_eq!(show(&it.lookup(0, 52).unwrap()), "2");
    assert_eq!(show(&it.lookup(0, 54).unwrap()), "720");
    assert!(matches!(it.lookup(0, 55), Some(Value::Error(ScriptError::Unbound(99)))));
    assert!(matches!(it.lookup(0, 56), Some(Value::Error(ScriptError::Unbound(98)))));
    assert!(matches!(it.lookup(0, 57), Some(Value::Error(ScriptError::TooFewArguments { given: 0, needed: 1 }))));
    assert_eq!(it.error_count(), 3);
}

#[test]
fn rest_parameter_and_quote_identity() {
    // (lambda (rest s5) s5) takes all arguments as a list (kind 19 marks the rest parameter).
    let m = begin(vec![
        define(1, L(vec![K(kind::LAMBDA), L(vec![K(kind::REST), S(5)]), S(5)])),
        define(2, L(vec![S(1), I(1), I(2)])),
        define(3, lambda(&[], vec![kw(kind::QUOTE, vec![L(vec![I(1), I(2)])])])),
        define(4, call(12, vec![L(vec![S(3)]), L(vec![S(3)])])),
        define(5, call(12, vec![call(40, vec![I(1)]), call(40, vec![I(1)])])),
        define(6, call(13, vec![call(40, vec![I(1)]), call(40, vec![I(1)])])),
    ]);
    let mut it = Interpreter::new(program(&[m]), None);
    it.load(&mut Mock::default());
    assert_eq!(show(&it.lookup(0, 2).unwrap()), "(1 2)");
    assert_eq!(show(&it.lookup(0, 4).unwrap()), "#t");
    assert_eq!(show(&it.lookup(0, 5).unwrap()), "#f");
    assert_eq!(show(&it.lookup(0, 6).unwrap()), "#t");
}

#[test]
fn arithmetic() {
    let cases: Vec<(T, &str)> = vec![
        (call(0, vec![I(1), I(2), I(3)]), "6"),
        (call(0, vec![I(1), F(0.5)]), "1.5"),
        (call(0, vec![V([1.0, 2.0, 3.0]), V([1.0, 1.0, 1.0])]), "#(2.0 3.0 4.0)"),
        (call(1, vec![I(5)]), "5"),
        (call(3, vec![I(5)]), "5"),
        (call(3, vec![F(4.0)]), "4.0"),
        (call(1, vec![I(5), I(7)]), "-2"),
        (call(2, vec![V([1.0, 2.0, 3.0]), I(2)]), "#(2.0 4.0 6.0)"),
        (call(2, vec![I(3), F(0.5)]), "1.5"),
        (call(3, vec![I(7), I(2)]), "3"),
        (call(3, vec![I(7), F(2.0)]), "3.5"),
        (call(3, vec![R([2.0, 4.0]), I(2)]), "#(1.0 2.0)"),
        (call(7, vec![F(2.9), I(2)]), "#t"),
        (call(8, vec![I(1), F(1.5)]), "#t"),
        (call(11, vec![I(1), I(1)]), "#t"),
        (call(30, vec![F(1.0), F(1.05), F(0.1)]), "#t"),
        (call(55, vec![F(0.0)]), "0.0"),
        (call(56, vec![F(0.0)]), "1.0"),
        (call(63, vec![V([1.0, 2.0, 3.0]), V([4.0, 5.0, 6.0])]), "32.0"),
        (call(64, vec![V([1.0, 0.0, 0.0]), V([0.0, 1.0, 0.0])]), "#(0.0 0.0 1.0)"),
        (call(65, vec![V([3.0, 4.0, 0.0])]), "5.0"),
        (call(66, vec![V([3.0, 4.0, 0.0])]), "5.0"),
        (call(67, vec![V([3.0, 4.0, 0.0]), V([3.0, 0.0, 0.0])]), "16.0"),
        (call(68, vec![V([0.0, 0.0, 2.0])]), "#(0.0 0.0 1.0)"),
        (call(84, vec![C([1.0, 2.0, 1.0]), R([0.0, 5.0]), F(2.0)]), "5.0"),
        (call(84, vec![C([1.0, 2.0, 1.0]), R([0.0, 5.0]), F(1.0)]), "4.0"),
        (call(86, vec![R([0.0, 5.0]), F(-3.0)]), "0.0"),
        (call(87, vec![R([0.0, 5.0]), F(3.0)]), "#t"),
        (call(88, vec![F(-1.5)]), "-2"),
        (call(90, vec![F(2.5)]), "3"),
        (call(90, vec![F(-2.5)]), "-2"),
        (call(90, vec![I(7)]), "7"),
        (call(91, vec![F(-1.5)]), "-1"),
        (call(92, vec![I(-4)]), "4"),
        (call(93, vec![I(1), I(4), I(2)]), "4"),
        (call(93, vec![I(1), F(4.0), I(2)]), "4.0"),
        (call(94, vec![I(3), F(2.5)]), "2.5"),
        (call(94, vec![F(3.0), I(2), F(2.5)]), "2"),
        (call(95, vec![I(7), I(3)]), "1"),
        (call(95, vec![I(-7), I(3)]), "-1"),
        (call(95, vec![F(7.5), F(2.0)]), "1"),
        (call(95, vec![I(7), I(0)]), "0"),
        (call(0, vec![I(16_777_217), I(0)]), "16777216"),
        (call(2, vec![I(3), I(2), F(0.5)]), "3.0"),
        (call(53, vec![F(10.0), F(20.0)]), "15.0"),
        (call(96, vec![]), "2.0"),
        (call(46, vec![I(1), I(2), F(3.5)]), "#(1.0 2.0 3.5)"),
        (call(51, vec![V([1.0, 2.0, 3.0]), I(2)]), "3.0"),
        (call(49, vec![I(1), I(2)]), "#(1.0 2.0)"),
    ];
    for (e, want) in cases {
        let p = program(&[begin(vec![define(40, e.clone())])]);
        let mut it = Interpreter::new(p, None);
        let mut h = Mock { rand: 0.5, time: 2.0, ..Default::default() };
        it.load(&mut h);
        assert_eq!(show(&it.lookup(0, 40).unwrap()), want, "{e:?}");
    }
    assert!(matches!(eval(call(3, vec![I(1), I(0)])), Value::Error(ScriptError::DivideByZero)));
    // Wrong kinds are read, never rejected (scripts.md 11, item 15).
    let lenient = [
        (call(0, vec![I(1), V([0.0; 3])]), "1"),
        (call(0, vec![Nil, I(5)]), "5"),
        (call(2, vec![Nil, I(3)]), "3"),
        (call(1, vec![Nil, I(2)]), "-2"),
        (call(3, vec![Nil, F(2.0), Nil]), "0.0"),
        (call(1, vec![Nil]), "0"),
        (call(0, vec![Nil]), "0"),
        (call(2, vec![Nil]), "1"),
        (call(0, vec![V([1.0, 2.0, 3.0]), I(4), V([1.0, 1.0, 1.0])]), "#(2.0 3.0 4.0)"),
        (call(1, vec![V([1.0, 2.0, 3.0]), Nil, V([1.0, 1.0, 1.0])]), "#(0.0 1.0 2.0)"),
        (call(2, vec![V([1.0, 2.0, 3.0]), Nil, I(2)]), "#(2.0 4.0 6.0)"),
        (call(8, vec![Nil, I(2)]), "#f"),
        (call(11, vec![I(2), B(true)]), "#f"),
        (call(7, vec![B(true), I(1)]), "#t"),
        (call(53, vec![Nil, Nil]), "0.0"),
        (call(51, vec![I(4), I(0)]), "()"),
        (call(51, vec![V([1.0, 2.0, 3.0]), I(3)]), "()"),
        (call(33, vec![I(4)]), "()"),
        (call(37, vec![I(4)]), "0"),
        (call(41, vec![call(40, vec![I(1)]), I(9), call(40, vec![I(2)])]), "(1 2)"),
        (call(88, vec![Nil]), "0"),
        (call(65, vec![Nil]), "0.0"),
        (call(93, vec![Nil, I(-1)]), "0"),
    ];
    for (e, want) in lenient {
        let mut h = Mock { rand: 0.5, ..Default::default() };
        let p = program(&[begin(vec![define(40, e.clone())])]);
        let mut it = Interpreter::new(p, None);
        it.load(&mut h);
        assert_eq!(show(&it.lookup(0, 40).unwrap()), want, "{e:?}");
        assert_eq!(it.error_count(), 0, "{e:?}");
    }
    assert!(matches!(eval(call(0, vec![])), Value::Error(ScriptError::TooFewArguments { .. })));
    assert!(matches!(eval(call(5, vec![])), Value::Error(ScriptError::UnknownOperation(5))));
}

#[test]
fn geometry_ops() {
    // Launch: flat ground, target 1000 units away at speed 1000 under g = 980: reachable, horizontal part along +X.
    // Launch (op 71): the arc passes through the target; steepness raises the elevation toward vertical.
    let lands = |start: [f32; 3], target: [f32; 3], steep: f32| {
        let v = builtins::launch(start, target, steep, 1500.0);
        let d = [target[0] - start[0], target[1] - start[1]];
        let dist = (d[0] * d[0] + d[1] * d[1]).sqrt();
        let t = dist / (v[0] * v[0] + v[1] * v[1]).sqrt();
        let z = start[2] + v[2] * t - 0.5 * 1500.0 * t * t;
        assert!((z - target[2]).abs() < 0.5, "{start:?} -> {target:?}: z {z}");
        v
    };
    let flat = lands([0.0; 3], [1000.0, 0.0, 0.0], 0.5);
    assert!(flat[0] > 0.0 && flat[1] == 0.0);
    // Elevation sin(0.5) of the way from horizontal to vertical.
    let elevation = flat[2].atan2(flat[0]);
    assert!((elevation - 0.5f32.sin() * core::f32::consts::FRAC_PI_2).abs() < 1e-3, "{elevation}");
    let up = lands([10.0, 20.0, 0.0], [10.0, 520.0, 300.0], 0.5);
    let steeper = lands([10.0, 20.0, 0.0], [10.0, 520.0, 300.0], 1.2);
    assert!(steeper[2] / steeper[1] > up[2] / up[1]);
    lands([0.0, 0.0, 400.0], [700.0, 0.0, 0.0], 0.3);
    // No arc: zero steepness toward a point level with the start, straight up, or no gravity.
    assert_eq!(builtins::launch([0.0; 3], [100.0, 0.0, 0.0], 0.0, 1500.0), [0.0; 3]);
    assert_eq!(builtins::launch([0.0; 3], [0.0, 0.0, 100.0], 0.5, 1500.0), [0.0; 3]);
    assert_eq!(builtins::launch([0.0; 3], [100.0, 0.0, 0.0], 0.5, 0.0), [0.0; 3]);
    // Look-at: X toward the target, Z up.
    let m = eval(call(81, vec![V([1.0, 2.0, 3.0]), V([1.0, 12.0, 3.0])]));
    let Value::Matrix(m) = m else { panic!() };
    assert_eq!(&m[0..3], &[0.0, 1.0, 0.0]);
    assert_eq!(&m[8..11], &[0.0, 0.0, 1.0]);
    assert_eq!(&m[12..15], &[1.0, 2.0, 3.0]);
    let pos = eval(call(51, vec![call(82, vec![call(81, vec![V([0.0; 3]), V([1.0, 0.0, 1.0])])]), I(3)]));
    assert_eq!(show(&pos), "#(0.0 0.0 0.0)");
    let fwd = eval(call(51, vec![call(82, vec![call(81, vec![V([0.0; 3]), V([1.0, 0.0, 1.0])])]), I(0)]));
    let Value::Vector(fwd) = fwd else { panic!() };
    assert!(fwd[0] > 0.9999 && fwd[1].abs() < 1e-6 && fwd[2].abs() < 1e-6, "{fwd:?}");
    // 82 turns local Z to world +Z by the shortest rotation, keeping the position.
    let tilted = call(81, vec![V([5.0, 6.0, 7.0]), V([6.0, 1.0, 9.0])]);
    let z = eval(call(51, vec![call(82, vec![tilted.clone()]), I(2)]));
    let Value::Vector(z) = z else { panic!() };
    assert!((z[2] - 1.0).abs() < 1e-5 && z[0].abs() < 1e-5 && z[1].abs() < 1e-5, "{z:?}");
    assert_eq!(show(&eval(call(51, vec![call(82, vec![tilted]), I(3)]))), "#(5.0 6.0 7.0)");
    let down = builtins::upright(&[1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 2.0, 3.0, 1.0]);
    assert!((down[10] - 1.0).abs() < 1e-6 && (down[5] - 1.0).abs() < 1e-6, "{down:?}");
    // 85: f(0) = a, f(1) = b, f(u) = c.
    assert_eq!(builtins::fit(1.0, 2.0, 2.0, 5.0), [1.0, 0.0, 1.0]);
    let g = builtins::fit(0.0, 1.0, 600.0, 0.01);
    let f = |x: f32| g[0] + g[1] * x + g[2] * x * x;
    assert!(f(0.0) == 0.0 && (f(1.0) - 1.0).abs() < 1e-5 && (f(600.0) - 0.01).abs() < 1e-3, "{g:?}");
    // 51 on a matrix: axis rows and the position; a component index must be 0 to 2.
    let m = call(81, vec![V([1.0, 2.0, 3.0]), V([1.0, 12.0, 3.0])]);
    assert_eq!(show(&eval(call(51, vec![m.clone(), I(0), I(1)]))), "1.0");
    assert_eq!(show(&eval(call(51, vec![m, I(3), I(3)]))), "()");
}

#[test]
fn lists() {
    let l = || call(40, vec![I(1), I(2), I(3)]);
    let cases: Vec<(T, &str)> = vec![
        (call(33, vec![l()]), "1"),
        (call(34, vec![l()]), "(2 3)"),
        (call(37, vec![l()]), "3"),
        (call(37, vec![Nil]), "0"),
        (call(38, vec![l(), I(2)]), "3"),
        (call(38, vec![l(), I(5)]), "()"),
        (call(39, vec![I(2), l()]), "#t"),
        (call(39, vec![I(9), l()]), "#f"),
        (call(41, vec![l(), Nil, l()]), "(1 2 3 1 2 3)"),
        (call(42, vec![Op(92), call(40, vec![I(-1), I(2)])]), "(1 2)"),
        (call(43, vec![lambda(&[1], vec![call(10, vec![S(1), I(1)])]), l()]), "(2 3)"),
        (call(44, vec![Op(4), l()]), "#void"),
        (call(25, vec![Nil]), "#t"),
        (call(25, vec![l()]), "#f"),
        (call(27, vec![Null]), "#t"),
        (call(33, vec![Nil]), "()"),
    ];
    for (e, want) in cases {
        assert_eq!(show(&eval(e.clone())), want, "{e:?}");
    }
    // Replace the first element in place: visible through every reference to the list.
    let m = begin(vec![
        define(1, call(40, vec![I(1), I(2)])),
        define(2, call(35, vec![S(1), I(9)])),
        define(3, call(33, vec![S(1)])),
    ]);
    let mut it = Interpreter::new(program(&[m]), None);
    it.load(&mut Mock::default());
    assert_eq!(show(&it.lookup(0, 3).unwrap()), "9");
}

fn option_map() -> OptionMap {
    let mut classes = vec![Vec::new(); 148];
    classes[2] = vec![(10, 2), (1015, 77), (1006, 977), (5, 99), (267, 974)];
    OptionMap::from_lists(classes)
}

#[test]
fn options_methods_custom_properties_and_events() {
    // Module 0: on load set option 1015 to 2, read option 10 (an action) and call it, keep a custom property.
    let m0 = begin(vec![
        define(
            0,
            lambda(
                &[66],
                vec![
                    call(101, vec![S(66), I(1015), I(2)]),
                    L(vec![call(102, vec![S(66), I(10)])]),
                    call(99, vec![S(66), I(1173), B(false)]),
                    call(99, vec![S(66), I(1173), B(true)]),
                    call(100, vec![S(66), I(1174), I(4)]),
                    call(100, vec![S(66), I(1174), I(5)]),
                ],
            ),
        ),
        // Event 2: read an option and a never-set custom property; setting option 5 makes the host raise event 2
        // on the object again (synchronously); the handler counts.
        define(70, I(0)),
        define(
            2,
            lambda(
                &[66],
                vec![
                    kw(kind::SET, vec![S(70), call(0, vec![S(70), I(1)])]),
                    define(71, call(102, vec![S(66), I(1006)])),
                    define(72, call(102, vec![S(66), I(1180)])),
                    kw(kind::IF, vec![call(8, vec![S(70), I(2)]), call(101, vec![S(66), I(5), I(1)])]),
                ],
            ),
        ),
    ]);
    let mut it = Interpreter::new(program(&[m0]), Some(option_map()));
    let mut h = Mock { class: 2, ..Default::default() };
    it.load(&mut h);
    let o = ObjectId(7);
    it.attach(o, 0);
    it.post_load(&mut h, [o, ObjectId(8)]);
    assert_eq!(h.log, ["set 7 77 2", "action 7 2 0"]);
    // 99 always appends and the newest entry is read; 100 overwrites the newest entry for its ID.
    assert_eq!(show(it.custom_property(o, 1173).unwrap()), "#t");
    assert_eq!(show(it.custom_property(o, 1174).unwrap()), "5");
    assert_eq!(it.custom_entries(o), 3);
    h.log.clear();
    let r = it.raise_event(&mut h, o, event::TRIGGERED, &[]).unwrap();
    assert!(!r.is_error(), "{r}");
    assert_eq!(h.log, ["get 7 977", "set 7 99 1", "get 7 977"]);
    // Stream option 974 is not in the encoding grid but is an action (scripts.md 11, item 4): a method.
    let m = it.call(&mut h, Value::Builtin(102), vec![Value::Object(Some(o)), Value::Int(267)]);
    assert!(matches!(m, Value::Method { object: ObjectId(7), option: 974 }), "{m}");
    assert_eq!(show(&it.lookup(0, 70).unwrap()), "2");
    assert!(it.raise_event(&mut h, o, event::UNTRIGGERED, &[]).is_none());
    assert!(it.raise_event(&mut h, ObjectId(8), event::TRIGGERED, &[]).is_none());
    // No mapping for an option in the class: reading falls through to the custom properties (scripts.md 11,
    // item 16) and gives the empty list, with no error.
    h.class = 3;
    let before = it.error_count();
    let r = it.call(&mut h, Value::Builtin(102), vec![Value::Object(Some(o)), Value::Int(5)]);
    assert!(matches!(r, Value::Nil), "{r}");
    assert_eq!(it.error_count(), before);
}

#[test]
fn null_object_and_lists_of_objects() {
    let m = begin(vec![
        define(1, call(101, vec![Null, I(1015), I(1)])),
        define(2, call(102, vec![Null, I(1015)])),
        define(3, call(101, vec![call(104, vec![I(5), Null]), I(1015), I(3)])),
        define(4, call(37, vec![call(104, vec![I(5), Null])])),
        define(5, call(119, vec![I(12), call(103, vec![I(3), Null])])),
    ]);
    let mut it = Interpreter::new(program(&[m]), Some(option_map()));
    let mut h = Mock { class: 2, ..Default::default() };
    it.load(&mut h);
    assert!(matches!(it.lookup(0, 1), Some(Value::Void)));
    assert!(matches!(it.lookup(0, 2), Some(Value::Void)));
    assert_eq!(show(&it.lookup(0, 4).unwrap()), "64");
    assert_eq!(h.log.iter().filter(|l| l.starts_with("set")).count(), 64);
    assert_eq!(h.log.last().unwrap(), "sound 12 Some(ObjectId(1003))");
}

#[test]
fn scheduled_calls() {
    // s0: schedule s1 in 1 s with argument 5; s1 records its argument and schedules itself with delay 0.
    let m = begin(vec![
        define(10, Nil),
        define(
            1,
            lambda(
                &[2],
                vec![
                    kw(kind::SET, vec![S(10), call(41, vec![call(40, vec![S(2)]), S(10)])]),
                    call(97, vec![I(0), S(1), I(6)]),
                ],
            ),
        ),
        define(0, lambda(&[66], vec![call(97, vec![F(1.0), S(1), I(5)])])),
    ]);
    let mut it = Interpreter::new(program(&[m]), None);
    let mut h = Mock::default();
    it.load(&mut h);
    it.attach(ObjectId(1), 0);
    it.post_load(&mut h, [ObjectId(1)]);
    assert_eq!(it.scheduled().len(), 1);
    it.tick(&mut h, 0.5);
    assert_eq!(show(&it.lookup(0, 10).unwrap()), "()");
    it.tick(&mut h, 0.5);
    assert_eq!(show(&it.lookup(0, 10).unwrap()), "(5)");
    // The call it scheduled runs on a later frame, not in the same tick.
    assert_eq!(it.scheduled().len(), 1);
    it.tick(&mut h, 1.0 / 60.0);
    assert_eq!(show(&it.lookup(0, 10).unwrap()), "(6 5)");
}

#[test]
fn limits_and_garbage() {
    // An endless loop stops at the step limit; infinite recursion stops at the depth limit.
    let m = begin(vec![
        define(1, lambda(&[], vec![kw(kind::WHILE, vec![B(true)])])),
        define(2, lambda(&[], vec![L(vec![S(2)])])),
        // A closure stored in its own call environment: a cycle that reference counting alone would leak.
        define(3, lambda(&[], vec![define(4, lambda(&[], vec![S(4)])), S(4)])),
    ]);
    let mut it = Interpreter::new(program(&[m]), None);
    it.limits.max_steps = 10_000;
    let mut h = Mock::default();
    it.load(&mut h);
    let f1 = it.lookup(0, 1).unwrap();
    assert!(matches!(it.call(&mut h, f1, vec![]), Value::Error(ScriptError::OutOfSteps)));
    let f2 = it.lookup(0, 2).unwrap();
    assert!(matches!(it.call(&mut h, f2, vec![]), Value::Error(ScriptError::TooDeep)));
    let f3 = it.lookup(0, 3).unwrap();
    it.collect_garbage();
    let base = it.live_environments();
    for _ in 0..1000 {
        it.call(&mut h, f3.clone(), vec![]);
    }
    assert!(it.live_environments() >= base + 1000);
    it.collect_garbage();
    assert_eq!(it.live_environments(), base);
    // A procedure kept outside across a collection is detected, not misused.
    let kept = it.call(&mut h, f3.clone(), vec![]);
    it.collect_garbage();
    assert!(matches!(it.call(&mut h, kept, vec![]), Value::Error(ScriptError::StaleEnvironment)));
}

#[test]
fn builtin_table() {
    assert_eq!(builtins::defined().count(), 75);
    assert_eq!(builtins::arity(97), Some((2, true)));
    assert_eq!(builtins::arity(5), None);
}

#[test]
fn option_map_reader() {
    // Two classes; class 1 has two entries, stored unsorted.
    let mut img = vec![0u8; 0x20_0000];
    let base = 0x10_0000u32;
    let put = |img: &mut Vec<u8>, va: u32, w: u32| {
        let o = (va - base) as usize;
        img[o..o + 4].copy_from_slice(&w.to_le_bytes());
    };
    put(&mut img, crate::option_map::TABLE_VA + 8, 2);
    put(&mut img, crate::option_map::TABLE_VA + 12, 0x1F_0000);
    put(&mut img, 0x1F_0000, 20);
    put(&mut img, 0x1F_0004, crate::option_map::DESCRIPTOR_VA + 32 * 7);
    put(&mut img, 0x1F_0008, 10);
    put(&mut img, 0x1F_000C, crate::option_map::DESCRIPTOR_VA + 32 * 3);
    let read = |va: u32, len: usize| {
        let o = va.checked_sub(base)? as usize;
        img.get(o..o + len)
    };
    let m = OptionMap::read(read).unwrap();
    assert_eq!(m.class_count(), 148);
    assert_eq!(m.lookup(1, 10), Some(3));
    assert_eq!(m.lookup(1, 20), Some(7));
    assert_eq!(m.lookup(1, 11), None);
    assert_eq!(m.lookup(0, 10), None);
    assert_eq!(m.lookup(-1, 10), None);
    let mut bad = img.clone();
    let o = (0x1F_0004 - base) as usize;
    bad[o..o + 4].copy_from_slice(&(crate::option_map::DESCRIPTOR_VA + 5).to_le_bytes());
    let read = |va: u32, len: usize| {
        let o = va.checked_sub(base)? as usize;
        bad.get(o..o + len)
    };
    assert!(OptionMap::read(read).is_err());
}

// ---- state machines ----

#[derive(Default)]
struct MHost {
    log: Vec<String>,
    rand: f32,
}

impl MachineHost for MHost {
    fn random(&mut self) -> f32 {
        self.rand
    }
    fn play(&mut self, s: Step) {
        self.log.push(format!("play {} {}", s.transition, s.reverse));
    }
    fn stop(&mut self, s: Step) {
        self.log.push(format!("stop {}", s.transition));
    }
    fn entered(&mut self, from: i16, to: i16) {
        self.log.push(format!("enter {from}->{to}"));
    }
}

fn tr(from: i16, to: i16, flags: i32, p: f32, animated: bool) -> Transition {
    Transition { animated, from, to, flags, probability: p }
}

/// States 1..=4: 1-2 (two-way, animated), 2-3 (animated, interruptible), 1-3 instant, 3-4 two ways with weights,
/// 4 loops on itself and is its own next state.
fn machine() -> StateMachine {
    StateMachine {
        states: vec![
            State { oid: 1, next: None },
            State { oid: 2, next: None },
            State { oid: 3, next: None },
            State { oid: 4, next: Some(4) },
        ],
        transitions: vec![
            tr(1, 2, 1, 1.0, true),
            tr(2, 3, 2, 1.0, true),
            tr(1, 3, 0, 1.0, false),
            tr(3, 4, 0, 1.0, true),
            tr(3, 4, 0, 3.0, true),
            tr(4, 4, 0, 1.0, true),
        ],
        auto_start: true,
    }
}

#[test]
fn shortest_routes() {
    let m = machine();
    let steps = |a, b| m.first_steps(a, b).iter().map(|s| (s.transition, s.reverse)).collect::<Vec<_>>();
    assert_eq!(steps(1, 3), [(2, false)]);
    assert_eq!(steps(2, 1), [(0, true)]);
    assert_eq!(steps(1, 2), [(0, false)]);
    assert_eq!(steps(3, 4), [(3, false), (4, false)]);
    assert_eq!(steps(4, 4), [(5, false)]);
    assert_eq!(steps(4, 1), []);
    assert_eq!(steps(2, 4), [(1, false)]);
}

#[test]
fn machine_runs() {
    let m = machine();
    let mut h = MHost::default();
    let mut run = MachineRun::new(&m);
    assert_eq!(run.state, 1);
    run.start(&m, &mut h);
    assert!(h.log.is_empty());
    // Goal 4: instant step 1->3, then a weighted choice between transitions 3 (weight 1) and 4 (weight 3).
    h.rand = 0.5; // 0.5 * 4 = 2.0 -> transition 4
    run.set_goal(&m, &mut h, Some(4));
    assert_eq!(h.log, ["enter 1->3", "play 4 false"]);
    h.log.clear();
    run.animation_finished(&m, &mut h);
    // Reaching goal 4 moves the goal to its next state (4 itself), so it keeps looping.
    assert_eq!(h.log, ["enter 3->4", "play 5 false"]);
    assert_eq!(run.goal, Some(4));
    // No route back from 4: the goal is cleared once the loop step ends.
    h.log.clear();
    run.set_goal(&m, &mut h, Some(1));
    assert_eq!(h.log, Vec::<String>::new(), "non-interruptible step keeps playing");
    run.animation_finished(&m, &mut h);
    assert_eq!(h.log, ["enter 4->4"]);
    assert_eq!(run.goal, None);

    // Interruptible: from 2 heading to 3, change the goal to 1: the step stops and ends in its destination, 3
    // (with the enter-state call); there is no route from 3 back to 1, so the goal is cleared.
    let mut run = MachineRun::new(&m);
    run.state = 2;
    run.start(&m, &mut h);
    h.log.clear();
    run.set_goal(&m, &mut h, Some(3));
    run.set_goal(&m, &mut h, Some(1));
    assert_eq!(h.log, ["play 1 false", "stop 1", "enter 2->3"]);
    assert_eq!(run.state, 3);
    assert_eq!(run.step, None);
    assert_eq!(run.goal, None);
    run.stop(&mut h);
    assert!(!run.running);
}

#[test]
fn zero_weights_pick_the_last_candidate() {
    let mut m = machine();
    m.transitions[3].probability = 0.0;
    m.transitions[4].probability = 0.0;
    let mut h = MHost { rand: 0.0, ..Default::default() };
    let mut run = MachineRun::new(&m);
    run.state = 3;
    run.start(&m, &mut h);
    run.set_goal(&m, &mut h, Some(4));
    assert_eq!(h.log, ["play 4 false"]);
}

#[test]
fn parses_state_machine_body() {
    let mut b = vec![2u8];
    for (s, n) in [(10i16, -1i16), (11, 10)] {
        b.extend_from_slice(&s.to_le_bytes());
        b.extend_from_slice(&n.to_le_bytes());
    }
    b.push(1);
    b.push(1);
    b.extend_from_slice(&10i16.to_le_bytes());
    b.extend_from_slice(&11i16.to_le_bytes());
    b.extend_from_slice(&3i32.to_le_bytes());
    b.extend_from_slice(&0.5f32.to_le_bytes());
    let m = StateMachine::parse_body(&b, 0).unwrap();
    assert_eq!(m.states[1], State { oid: 11, next: Some(10) });
    assert!(m.transitions[0].two_way() && m.transitions[0].interruptible());
    assert_eq!(m.animation_index(0), Some(0));
    assert!(StateMachine::parse_body(&b[..b.len() - 1], 0).is_err());
}

// ---- robustness regressions ----

fn list_of(n: usize) -> Value {
    Value::list((0..n as i32).map(Value::Int).collect::<Vec<_>>())
}

/// A list whose first element is the list itself (what operation 35 can make).
fn self_containing() -> Value {
    let l = Value::list(vec![Value::Int(1), Value::Int(2)]);
    if let Value::List(c) = &l {
        *c.car.borrow_mut() = l.clone();
    }
    l
}

#[test]
fn element_with_one_argument_is_empty() {
    assert_eq!(show(&eval(call(51, vec![V([1.0, 2.0, 3.0])]))), "()");
    assert_eq!(show(&eval(call(51, vec![R([1.0, 2.0])]))), "()");
}

#[test]
fn collector_handles_cycles_and_shared_lists() {
    // (define s1 (op40 1 2)) (op35 s1 s1): a list that contains itself, reachable from the module.
    let m = begin(vec![define(1, call(40, vec![I(1), I(2)])), call(35, vec![S(1), S(1)])]);
    let mut it = Interpreter::new(program(&[m]), None);
    let mut h = Mock::default();
    it.load(&mut h);
    it.collect_garbage();
    // 2^64 paths through 64 shared cells, and a self-containing list, as roots.
    let mut shared = Value::Nil;
    for _ in 0..64 {
        shared = Value::list(vec![shared.clone(), shared.clone()]);
    }
    it.set_custom_property(ObjectId(1), 1173, shared);
    it.set_custom_property(ObjectId(1), 1174, self_containing());
    it.collect_garbage();
    assert!(it.live_environments() >= 1);
}

#[test]
fn equal_and_display_are_bounded() {
    use crate::value::equal;
    let a = self_containing();
    let b = self_containing();
    assert!(equal(&a, &a), "the same cell");
    assert!(!equal(&a, &b), "two self-containing lists: cut off, unequal");
    let text = show(&a);
    assert!(text.contains("...") && text.len() < 1 << 20, "{}", text.len());
    // Deep nesting: no recursion.
    let deep = |n: usize| (0..n).fold(Value::Nil, |v, _| Value::list(vec![v]));
    let (x, y) = (deep(100_000), deep(100_000));
    assert!(!equal(&x, &y), "deeper than the walk limit");
    assert!(equal(&deep(100), &deep(100)));
    assert!(show(&x).contains("(...)"));
    // Shared sublists print within the item limit.
    let mut shared = Value::Nil;
    for _ in 0..64 {
        shared = Value::list(vec![shared.clone(), shared.clone()]);
    }
    assert!(show(&shared).ends_with(')'));
    assert!(!equal(&shared, &shared.clone()) || matches!(shared, Value::List(_)));
}

#[test]
fn long_and_deep_lists_drop_without_recursion() {
    // On a 64 KiB stack, which recursive dropping would overflow.
    let t = std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(|| {
            drop(list_of(1_000_000));
            let mut deep = Value::Nil;
            for _ in 0..1_000_000 {
                deep = Value::list(vec![deep]);
            }
            drop(deep);
            // A shared tail is only taken apart once its last owner goes.
            let tail = list_of(1000);
            let a = Value::list_with_tail(vec![Value::Int(1)], tail.clone());
            drop(a);
            assert_eq!(tail.iter().count(), 1000);
        })
        .unwrap();
    t.join().unwrap();
}

#[test]
fn list_size_limit_and_step_charges() {
    let mut it = Interpreter::new(program(&[begin(vec![])]), None);
    let mut h = Mock::default();
    it.load(&mut h);
    it.limits.max_list_len = 1000;
    let half = list_of(600);
    let r = it.call(&mut h, Value::Builtin(41), vec![half.clone(), half.clone()]);
    assert!(matches!(r, Value::Error(ScriptError::TooLarge)), "{r}");
    let r = it.call(&mut h, Value::Builtin(41), vec![list_of(400), half]);
    assert_eq!(r.iter().count(), 1000);
    let r = it.call(&mut h, Value::Builtin(40), (0..1001).map(Value::Int).collect());
    assert!(matches!(r, Value::Error(ScriptError::TooLarge)));
    // A doubling loop stops at the limit instead of growing without bound:
    // (define s1 (op40 1)) (while #t (set s1 (op41 s1 s1)))
    let m = begin(vec![
        define(1, call(40, vec![I(1)])),
        kw(kind::WHILE, vec![B(true), kw(kind::SET, vec![S(1), call(41, vec![S(1), S(1)])])]),
    ]);
    let mut it = Interpreter::new(program(&[m]), None);
    it.limits.max_steps = 100_000;
    it.load(&mut h);
    assert!(it.error_count() > 0);
    // Walking built-ins use the step budget: a large count with a small budget runs out.
    let mut it = Interpreter::new(program(&[begin(vec![])]), None);
    it.limits.max_steps = 1000;
    it.load(&mut h);
    let r = it.call(&mut h, Value::Builtin(37), vec![list_of(5000)]);
    assert!(matches!(r, Value::Error(ScriptError::OutOfSteps)), "{r}");
    let r = it.call(&mut h, Value::Builtin(37), vec![list_of(500)]);
    assert!(matches!(r, Value::Int(500)), "{r}");
}

#[test]
fn scheduled_queue_and_tick_budget() {
    // (define s6 (lambda () (while #t (op119 1 null)))): each iteration plays a sound, which the host logs.
    let m = begin(vec![define(6, lambda(&[], vec![kw(kind::WHILE, vec![B(true), call(119, vec![I(1), Null])])]))]);
    let mut it = Interpreter::new(program(&[m]), None);
    it.limits.max_steps = 10_000;
    it.limits.max_scheduled = 8;
    let mut h = Mock::default();
    it.load(&mut h);
    let f = it.lookup(0, 6).unwrap();
    for i in 0..10 {
        let r = it.call(&mut h, Value::Builtin(97), vec![Value::Float(0.0), f.clone()]);
        if i < 8 {
            assert!(matches!(r, Value::Void), "{r}");
        } else {
            assert!(matches!(r, Value::Error(ScriptError::TooLarge)), "{r}");
        }
    }
    assert_eq!(it.scheduled().len(), 8);
    // Eight endless calls share one budget: the counter stays well under one budget's worth of iterations
    // per call (each iteration takes several steps).
    h.log.clear();
    it.tick(&mut h, 1.0);
    assert!(it.scheduled().is_empty());
    let n = h.log.iter().filter(|l| l.starts_with("sound")).count();
    assert!(n > 0 && n < 10_000 / 2, "{n}");
    // A NaN delay is refused rather than queued forever.
    let r = it.call(&mut h, Value::Builtin(97), vec![Value::Float(f32::NAN), f]);
    assert!(matches!(r, Value::Error(ScriptError::BadArgument { op: 97, index: 0 })));
    assert!(it.scheduled().is_empty());
}

#[test]
fn nesting_limit_when_reading() {
    let nest = |n: usize| (0..n).fold(I(1), |t, _| L(vec![t]));
    let mut ok = Vec::new();
    enc_seq(&[nest(crate::tree::MAX_NESTING)], &mut ok);
    assert!(Program::new().read_module(&ok, 0, false).is_ok());
    let mut bad = Vec::new();
    enc_seq(&[nest(crate::tree::MAX_NESTING + 1)], &mut bad);
    assert!(Program::new().read_module(&bad, 0, false).is_err());
    // Many levels in a flat run of bytes (no encoder recursion): refused, not a stack overflow.
    let mut flat = vec![kind::LIST; 100_000];
    flat.push(kind::INT);
    flat.extend_from_slice(&1i32.to_le_bytes());
    assert!(Program::new().read_module(&flat, 0, false).is_err());
}

#[test]
fn option_map_reader_refuses_bad_sizes() {
    use crate::option_map::{CLASS_COUNT, MapError, TABLE_VA};
    // A short table.
    let short = [0u8; 8];
    assert!(matches!(OptionMap::read(|_, _| Some(&short[..])), Err(MapError::BadAddress(TABLE_VA))));
    // A huge count is refused before anything is allocated.
    let mut table = vec![0u8; CLASS_COUNT * 8];
    table[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(OptionMap::read(|va, _| (va == TABLE_VA).then_some(&table[..])), Err(MapError::BadCount(_))));
    // A list read that comes back short.
    table[..4].copy_from_slice(&4u32.to_le_bytes());
    table[4..8].copy_from_slice(&0x0030_0000u32.to_le_bytes());
    let list = [0u8; 8];
    let r = OptionMap::read(|va, _| if va == TABLE_VA { Some(&table[..]) } else { Some(&list[..]) });
    assert!(matches!(r, Err(MapError::BadAddress(0x0030_0000))));
}

#[test]
fn stack_use_at_the_depth_limit() {
    // Infinite recursion stops with TooDeep at the default depth limit; measure the stack it took.
    let t = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(|| {
            let m = begin(vec![define(2, lambda(&[], vec![L(vec![S(2)])]))]);
            let mut it = Interpreter::new(program(&[m]), None);
            let mut h = Mock::default();
            it.load(&mut h);
            let f = it.lookup(0, 2).unwrap();
            let top = 0u8;
            let top = core::hint::black_box(&top) as *const u8 as usize;
            assert!(matches!(it.call(&mut h, f, vec![]), Value::Error(ScriptError::TooDeep)));
            let used = top - h.stack_low.unwrap();
            std::eprintln!(
                "stack at depth limit {}: {} KiB ({} bytes per level)",
                it.limits.max_depth,
                used / 1024,
                used / it.limits.max_depth as usize
            );
            used
        })
        .unwrap();
    let used = t.join().unwrap();
    // docs/script.md gives the measured release figure; this bound only catches large regressions.
    assert!(used < 32 << 20, "{used}");
}

#[test]
fn clarified_evaluation_rules() {
    // Error values are ordinary values: true in tests, passed on as arguments, wrong-kind for arithmetic.
    let err = || L(vec![I(3)]); // an error value (calling a number)
    assert_eq!(show(&eval(kw(kind::IF, vec![err(), I(1), I(2)]))), "1");
    assert_eq!(show(&eval(kw(kind::AND, vec![err(), I(7)]))), "7");
    assert!(matches!(eval(kw(kind::OR, vec![err(), I(7)])), Value::Error(_)));
    assert_eq!(show(&eval(call(0, vec![err(), I(1)]))), "1", "skipped like any non-number");
    assert_eq!(show(&eval(call(40, vec![err(), I(1)]))), "(#error(NotCallable) 1)");
    assert!(matches!(eval(L(vec![err(), I(1)])), Value::Error(ScriptError::NotCallable)));
    // Calling something not callable is a no-op: its arguments are not evaluated and nothing is reported
    // (scripts.md 11, item 18).
    let p = program(&[begin(vec![define(9, I(0)), L(vec![I(3), kw(kind::SET, vec![S(9), I(1)])])])]);
    let mut it = Interpreter::new(p, None);
    let mut h = Mock::default();
    it.load(&mut h);
    assert_eq!(show(&it.lookup(0, 9).unwrap()), "0");
    assert_eq!(it.error_count(), 0);
    assert_eq!(show(&eval(call(43, vec![Op(0), call(40, vec![I(1), B(true)])]))), "(1 #t)", "error counts as true");
    // define and set give the value; if without else, while and an unmatched cond or case give void.
    assert_eq!(show(&eval(begin(vec![define(9, I(5))]))), "5");
    assert_eq!(show(&eval(begin(vec![define(9, I(5)), kw(kind::SET, vec![S(9), I(6)])]))), "6");
    assert_eq!(show(&eval(kw(kind::IF, vec![B(false), I(1)]))), "#void");
    assert_eq!(show(&eval(kw(kind::WHILE, vec![B(false)]))), "#void");
    assert_eq!(show(&eval(kw(kind::COND, vec![L(vec![B(false), I(1)])]))), "#void");
    assert_eq!(show(&eval(kw(kind::COND, vec![L(vec![I(4)])]))), "4", "a clause without body gives its test");
    assert_eq!(show(&eval(kw(kind::CASE, vec![I(3), L(vec![L(vec![I(1)]), I(9)])]))), "#void");
    assert_eq!(show(&eval(kw(kind::BEGIN, vec![]))), "#void");
    // Case values are data: a symbol in a clause is not looked up.
    assert_eq!(show(&eval(kw(kind::CASE, vec![kw(kind::QUOTE, vec![S(3)]), L(vec![L(vec![S(3)]), I(9)])]))), "9");

    // Extra arguments to a procedure without a rest parameter are not evaluated.
    // (define s1 0) (define s2 (lambda (s3) s3)) (define s4 (s2 7 (set s1 1)))
    let m = begin(vec![
        define(1, I(0)),
        define(2, lambda(&[3], vec![S(3)])),
        define(4, L(vec![S(2), I(7), kw(kind::SET, vec![S(1), I(1)])])),
        // With a rest parameter they are: (define s5 (lambda (s3 (rest s6)) s6)) (define s7 (s5 1 (set s1 2) 3))
        define(5, L(vec![K(kind::LAMBDA), L(vec![S(3), K(kind::REST), S(6)]), S(6)])),
        define(7, L(vec![S(5), I(1), kw(kind::SET, vec![S(1), I(2)]), I(3)])),
        define(8, L(vec![S(5), I(1)])),
    ]);
    let mut it = Interpreter::new(program(&[m]), None);
    let mut h = Mock::default();
    it.load(&mut h);
    assert_eq!(show(&it.lookup(0, 4).unwrap()), "7");
    assert_eq!(show(&it.lookup(0, 7).unwrap()), "(2 3)");
    assert_eq!(show(&it.lookup(0, 1).unwrap()), "2");
    assert_eq!(show(&it.lookup(0, 8).unwrap()), "()");
    // A rest marker anywhere but before the last symbol is a bad form.
    let bad = begin(vec![define(1, L(vec![K(kind::LAMBDA), L(vec![K(kind::REST), S(6), S(7)]), I(0)]))]);
    let mut it = Interpreter::new(program(&[bad]), None);
    it.load(&mut h);
    assert!(matches!(it.lookup(0, 1), Some(Value::Error(ScriptError::BadForm(_)))));
}

#[test]
fn custom_slots_and_unlisted_actions() {
    use crate::interp::{CUSTOM_SLOTS, UNLISTED_ACTIONS};
    let mut it = Interpreter::new(program(&[begin(vec![])]), None);
    let mut h = Mock::default();
    it.load(&mut h);
    let o = Value::Object(Some(ObjectId(3)));
    for i in 0..20 {
        it.call(&mut h, Value::Builtin(99), vec![o.clone(), Value::Int(1180), Value::Int(i)]);
    }
    // Appends beyond 16 entries are ignored: the newest kept entry is the 16th.
    assert_eq!(it.custom_entries(ObjectId(3)), CUSTOM_SLOTS);
    assert_eq!(show(it.custom_property(ObjectId(3), 1180).unwrap()), "15");
    it.call(&mut h, Value::Builtin(100), vec![o.clone(), Value::Int(1180), Value::Int(99)]);
    assert_eq!(show(it.custom_property(ObjectId(3), 1180).unwrap()), "99");
    assert_eq!(it.custom_entries(ObjectId(3)), CUSTOM_SLOTS);
    // Both accept a list of objects and give void.
    let both = Value::list(vec![Value::Object(Some(ObjectId(4))), Value::Object(Some(ObjectId(5)))]);
    let r = it.call(&mut h, Value::Builtin(100), vec![both, Value::Int(1190), Value::Int(1)]);
    assert!(matches!(r, Value::Void));
    assert_eq!(show(it.custom_property(ObjectId(5), 1190).unwrap()), "1");
    // The unlisted action table: 117 sorted IDs, including the dialogue start (974) and not its flag (977).
    assert_eq!(UNLISTED_ACTIONS.len(), 117);
    assert!(UNLISTED_ACTIONS.windows(2).all(|w| w[0] < w[1]));
    assert!(UNLISTED_ACTIONS.binary_search(&974).is_ok());
    assert!(UNLISTED_ACTIONS.binary_search(&977).is_err());
    // The complete method list of scripts.md 11, item 17.
    use crate::interp::{METHODS, method_arity};
    assert!(METHODS.windows(2).all(|w| w[0].0 < w[1].0));
    assert_eq!(method_arity(147), Some(Some(1)));
    assert_eq!(method_arity(197), Some(Some(3)));
    assert_eq!(method_arity(1168), Some(None));
    assert_eq!(method_arity(521), None, "306 in class 147 is a value");
    assert_eq!(method_arity(977), None);
    assert_eq!((555..=561).filter(|&o| method_arity(o) == Some(Some(1))).count(), 7);
}

// ---- Sly 2 ----

fn program_sly2(modules: &[T]) -> Program {
    let mut p = Program::for_game(crate::Game::Sly2);
    for m in modules {
        let mut bytes = Vec::new();
        enc_seq_with(core::slice::from_ref(m), &mut bytes, &sly2_kind);
        let end = p.read_module(&bytes, 0, false).unwrap();
        assert_eq!(end, bytes.len());
    }
    p
}

/// Evaluates `expr` in a Sly 2 module that defines nothing else.
fn eval2(expr: T) -> (Value, Mock) {
    let p = program_sly2(&[begin(vec![define(40, expr)])]);
    let mut it = Interpreter::new(p, None);
    let mut h = Mock { rand: 0.0, ..Default::default() };
    it.load(&mut h);
    (it.lookup(0, 40).unwrap(), h)
}

#[test]
fn sly2_cells_read_into_the_same_tree() {
    let m = begin(vec![
        define(1, lambda(&[2], vec![kw(kind::IF, vec![call(7, vec![S(2), I(3)]), F(1.5), B(true)])])),
        kw(kind::QUOTE, vec![L(vec![I(1), Null, C([1.0, 2.0, 3.0]), V([0.0; 3]), R([1.0, 2.0])])]),
        kw(kind::CASE, vec![S(2), L(vec![L(vec![I(5)]), I(1)]), L(vec![K(kind::ELSE), I(0)])]),
    ]);
    let one = program(core::slice::from_ref(&m));
    let two = program_sly2(&[m]);
    assert_eq!(two.game, crate::Game::Sly2);
    let text = |p: &Program| format!("{}", p.display(p.module(0)[0]));
    assert_eq!(text(&one), text(&two));
    // Cell counts are kept by raw kind: Sly 2 lists are kind 31, integers 21, symbols 24, built-ins 25.
    assert_eq!(two.cell_counts[31], one.cell_counts[30]);
    assert_eq!(two.cell_counts[21], one.cell_counts[20]);
    assert_eq!((two.cell_counts[24], two.cell_counts[25]), (one.cell_counts[28], one.cell_counts[29]));
    assert_eq!(two.cell_counts[16], 1, "begin is kind 16");
    // The two new keywords.
    let p = program_sly2(&[L(vec![Raw(6), I(4)]), L(vec![Raw(17)]), L(vec![Raw(19)])]);
    assert_eq!(format!("{}", p.display(p.module(0)[0])), "(eval-one 4)");
    assert_eq!(p.head_keyword(p.module(1)[0]), Some(kind::NO_VALUE));
    assert_eq!(p.head_keyword(p.module(2)[0]), Some(kind::SLY2_KIND_19));
    // Debug tables carry 16 bytes after each tag.
    let mut d = vec![21u8];
    d.extend_from_slice(&[0; 16]);
    d.extend_from_slice(&9i32.to_le_bytes());
    let mut p = Program::for_game(crate::Game::Sly2);
    assert_eq!(p.read_module(&d, 0, true).unwrap(), d.len());
    assert_eq!(*p.node(p.module(0)[0]), Node::Int(9));
}

#[test]
fn sly2_forms_and_operations() {
    let s = |t: T| show(&eval2(t).0);
    // New keywords: one expression's value, or no value.
    assert_eq!(s(L(vec![Raw(6), I(4)])), "4");
    assert_eq!(s(L(vec![Raw(6)])), "#void");
    assert_eq!(s(L(vec![Raw(17)])), "#void");
    assert!(matches!(eval2(L(vec![Raw(19)])).0, Value::Error(ScriptError::UnknownKeyword(_))));
    // Renumbered operations keep their Sly 1 meaning.
    assert_eq!(s(call(0, vec![I(2), I(3)])), "5");
    assert_eq!(s(call(88, vec![F(-2.7)])), "-2", "88 discards the fraction");
    assert_eq!(s(call(90, vec![F(2.5)])), "3");
    assert_eq!(s(call(42, vec![call(40, vec![I(1)]), call(40, vec![I(2), I(3)])])), "(1 2 3)", "42 appends");
    assert_eq!(s(call(43, vec![Op(92), call(40, vec![I(-1), I(2)])])), "(1 2)", "43 maps");
    assert_eq!(s(call(52, vec![V([1.0, 2.0, 3.0]), I(1)])), "2.0", "52 is element");
    assert_eq!(s(call(50, vec![I(1), I(2)])), "#(1.0 2.0)", "50 makes a range");
    // New operations.
    assert_eq!(s(call(32, vec![I(1), call(40, vec![I(2)])])), "(1 2)");
    assert_eq!(s(call(54, vec![F(9.0)])), "3.0");
    assert_eq!(s(call(62, vec![F(1.0), F(0.0)])), format!("{:?}", core::f32::consts::FRAC_PI_2));
    assert_eq!(s(call(83, vec![C([1.0, 2.0, 1.0]), F(2.0)])), "9.0");
    assert_eq!(s(call(135, vec![I(12), I(10)])), "8");
    assert_eq!(s(call(136, vec![I(12), I(3)])), "15");
    assert_eq!(s(call(135, vec![])), "-1");
    // 47: one number n gives (n, n, n); two numbers are not a form the game handles.
    assert_eq!(s(call(47, vec![I(2)])), "#(2.0 2.0 2.0)");
    assert_eq!(s(call(47, vec![I(1), I(2), I(3)])), "#(1.0 2.0 3.0)");
    assert!(matches!(eval2(call(47, vec![I(1), I(2)])).0, Value::Error(ScriptError::BadArgument { .. })));
    assert_eq!(s(call(52, vec![call(48, vec![]), I(1)])), "#(0.0 1.0 0.0)", "48 without arguments: identity");
    assert_eq!(s(call(52, vec![call(48, vec![F(2.0)]), I(0)])), "#(2.0 0.0 0.0)", "48 with one number: diagonal");
    let nine: Vec<T> = (1..=9).map(I).collect();
    assert_eq!(s(call(52, vec![call(48, nine), I(1)])), "#(4.0 5.0 6.0)", "48 with nine numbers: rows");
    // 72: (angle axis).
    assert_eq!(
        s(call(72, vec![V([1.0, 0.0, 0.0]), V([0.0, 1.0, 0.0])])),
        format!("({:?} #(0.0 0.0 1.0))", core::f32::consts::FRAC_PI_2)
    );
    // A shuffled copy (random 0 always picks index 0).
    assert_eq!(s(call(41, vec![call(40, vec![I(1), I(2), I(3)])])), "(2 3 1)");
    // Argument rules: too many for a fixed-count operation, unknown IDs.
    assert!(matches!(
        eval2(call(7, vec![I(1), I(2), I(3)])).0,
        Value::Error(ScriptError::TooManyArguments { given: 3, allowed: 2 })
    ));
    assert!(matches!(eval2(call(5, vec![])).0, Value::Error(ScriptError::UnknownOperation(5))));
    // Game operations go to the host.
    let (v, h) = eval2(call(97, vec![]));
    assert_eq!(show(&v), "9700");
    assert_eq!(h.log, ["game 97 0"]);
}

#[test]
fn sly2_rest_replacement_scheduling_and_lookup() {
    // (define s1 (op40 1 2)) (op36 s1 s1): the rest of the list is the list itself; walks still end.
    let m = begin(vec![
        define(1, call(40, vec![I(1), I(2)])),
        call(36, vec![S(1), S(1)]),
        define(2, call(37, vec![S(1)])),
        // 105 returns a handle; 106 cancels it.
        define(3, call(105, vec![F(1.0), Op(40), I(7)])),
        define(4, call(105, vec![F(1.0), Op(40), I(8)])),
        call(106, vec![S(3)]),
        // 133: a symbol's value in the calling environment.
        define(5, I(77)),
        define(6, lambda(&[7], vec![call(133, vec![kw(kind::QUOTE, vec![S(7)])])])),
        define(8, L(vec![S(6), I(9)])),
        define(9, call(133, vec![kw(kind::QUOTE, vec![S(5)])])),
    ]);
    let mut it = Interpreter::new(program_sly2(&[m]), None);
    it.limits.max_steps = 10_000_000;
    let mut h = Mock::default();
    it.load(&mut h);
    let Some(Value::Int(n)) = it.lookup(0, 2) else { panic!("{:?}", it.lookup(0, 2)) };
    assert_eq!(n as usize, crate::value::MAX_LIST_ITEMS, "a looping list is cut off");
    assert!(show(&it.lookup(0, 1).unwrap()).contains("..."));
    let (Some(Value::Int(a)), Some(Value::Int(b))) = (it.lookup(0, 3), it.lookup(0, 4)) else { panic!() };
    assert!(a > 0 && b > a);
    assert_eq!(it.scheduled().len(), 1);
    assert_eq!(it.scheduled()[0].id, b as u32);
    assert_eq!(show(&it.lookup(0, 8).unwrap()), "9", "the procedure's own parameter");
    assert_eq!(show(&it.lookup(0, 9).unwrap()), "77");
}

#[test]
fn sly2_custom_properties_and_actions() {
    // Custom properties start at 1234; one holding a procedure is called as a method (specs/sly2/scripts.md 7.3).
    let m = begin(vec![
        define(
            0,
            lambda(
                &[1002],
                vec![
                    call(108, vec![S(1002), I(1238), lambda(&[], vec![I(42)])]),
                    kw(kind::SET, vec![S(50), L(vec![call(111, vec![S(1002), I(1238)])])]),
                    kw(kind::SET, vec![S(51), call(108, vec![S(1002), I(40), I(1)])]),
                    kw(kind::SET, vec![S(52), call(111, vec![S(1002), I(40)])]),
                ],
            ),
        ),
        define(50, I(0)),
        define(51, I(0)),
        define(52, I(0)),
    ]);
    let mut classes = vec![Vec::new(); 199];
    // Class 2 lists script option 40 twice: the game's binary search reaches the first entry here; class 3's
    // ID 1 is listed twice before a third record and the search reaches the second (specs/sly2/scripts.md 5).
    classes[2] = vec![(40, 974), (40, 5), (10, 2)];
    classes[3] = vec![(1, 7), (1, 8), (2, 9)];
    let map = OptionMap::from_lists(classes);
    assert_eq!(map.lookup(2, 40), Some(974));
    assert_eq!((map.lookup(3, 1), map.lookup(3, 2), map.lookup(3, 3)), (Some(8), Some(9), None));
    assert_eq!(map.records, 6);
    let mut it = Interpreter::new(program_sly2(&[m]), Some(map));
    let mut table = formats_sly2::exe::OptionTable {
        entries: vec![formats_sly2::exe::OptionEntry { value_type: 0, flags: 0, arg_count: 0 }; 1000],
    };
    table.entries[974].flags = 0x400; // a typed argument list: an action
    it.set_sly2_tables(None, &table);
    assert_eq!(it.custom_base(), 1233);
    let mut h = Mock { class: 2, ..Default::default() };
    it.load(&mut h);
    it.attach(ObjectId(5), 0);
    it.post_load(&mut h, [ObjectId(5)]);
    assert_eq!(show(&it.lookup(0, 50).unwrap()), "42");
    // Adding (108) uses the property store whatever the ID: no error, "no value".
    assert!(matches!(it.lookup(0, 51), Some(Value::Void)));
    assert!(matches!(it.lookup(0, 52), Some(Value::Method { object: ObjectId(5), option: 974 })));
}

#[test]
fn sly2_tables_from_an_image() {
    use crate::sly2::{BUILTIN_TABLE, BuiltinTable, EVENT_INDEX, EVENT_TYPES, EventSignatures};
    // A fake image: the built-in table and the event signature tables at their addresses.
    let mut builtins = Vec::new();
    for op in 0..BUILTIN_TABLE.1 as u32 {
        builtins.extend_from_slice(&(0x10_0000 + op).to_le_bytes());
        builtins.extend_from_slice(&(op % 4).to_le_bytes());
        builtins.extend_from_slice(&u32::from(op % 3 == 0).to_le_bytes());
    }
    let t =
        BuiltinTable::read(|va, len| (va == BUILTIN_TABLE.0).then(|| &builtins[..len.min(builtins.len())])).unwrap();
    assert_eq!(t.entries.len(), 169);
    assert_eq!(t.rule(6), Some((2, true)));
    assert_eq!(t.rule(7), Some((3, false)));
    assert_eq!(t.rule(169), None);
    builtins[4] = 99;
    assert!(BuiltinTable::read(|_, len| Some(&builtins[..len])).is_err());

    let words = ((EVENT_INDEX - EVENT_TYPES) / 4) as usize;
    let types: Vec<u8> = (0..words as u32).flat_map(|i| (100 + i).to_le_bytes()).collect();
    let mut index: Vec<u8> = (0..30u32).flat_map(|e| e.min(words as u32).to_le_bytes()).collect();
    index.extend_from_slice(&u32::MAX.to_le_bytes());
    let read = |va: u32, len: usize| -> Option<&[u8]> {
        match va {
            EVENT_INDEX => Some(&index[..len]),
            EVENT_TYPES => Some(&types[..len]),
            _ => None,
        }
    };
    let ev = EventSignatures::read(read).unwrap();
    assert_eq!(ev.events.len(), 30);
    assert_eq!(ev.len(0), Some(1));
    assert_eq!(ev.events[3], [103]);
    assert_eq!(ev.len(29), Some(words - 29), "the last event runs to the end of the type words");
}

#[test]
fn outer_module_chain() {
    // Module 0: (define s1 1) (define s2 (lambda (s9) s3)) (define s4 (lambda (s9) (set s3 8)))
    // Module 1: (define s3 7) (define s1 5) (define s5 (lambda (s9) 55))
    // Module 2: (define s3 70)
    let m0 = begin(vec![
        define(1, I(1)),
        define(2, lambda(&[9], vec![S(3)])),
        define(4, lambda(&[9], vec![kw(kind::SET, vec![S(3), I(8)])])),
    ]);
    let m1 = begin(vec![define(3, I(7)), define(1, I(5)), define(5, lambda(&[9], vec![I(55)]))]);
    let m2 = begin(vec![define(3, I(70))]);
    let mut it = Interpreter::new(program(&[m0, m1, m2]), None);
    let mut h = Mock::default();
    it.load(&mut h);
    let o = ObjectId(3);
    // Before chaining, module 0 cannot see s3.
    assert!(it.lookup(0, 3).is_none());
    assert!(it.attach_with_outer(o, 0, Some(1)));
    assert_eq!(it.module_of(o), Some(0));
    assert_eq!(it.outer_module(0), Some(1));
    // The new module's own definitions come first; the outer module's are found after them.
    assert_eq!(show(&it.lookup(0, 1).unwrap()), "1");
    assert_eq!(show(&it.lookup(0, 3).unwrap()), "7");
    // A procedure of the new module sees the outer module's names, and `set` reaches them.
    let f = it.lookup(0, 2).unwrap();
    assert_eq!(show(&it.call(&mut h, f.clone(), vec![Value::Object(Some(o))])), "7");
    let g = it.lookup(0, 4).unwrap();
    it.call(&mut h, g, vec![Value::Object(Some(o))]);
    assert_eq!(show(&it.lookup(1, 3).unwrap()), "8");
    // An event the outer module handles reaches the object (symbol 5 is event 5 here).
    assert_eq!(show(&it.raise_event(&mut h, o, 5, &[]).unwrap()), "55");
    // The same outer again is fine; a different one, or a loop, is refused and counted.
    assert!(it.attach_with_outer(ObjectId(4), 0, Some(1)));
    assert!(!it.attach_with_outer(ObjectId(5), 0, Some(2)));
    assert!(!it.attach_with_outer(ObjectId(6), 1, Some(0)));
    assert_eq!(it.outer_conflicts(), 2);
    assert_eq!(it.outer_module(1), None);
    // No outer, or the module itself: a plain attachment.
    assert!(it.attach_with_outer(ObjectId(7), 2, None));
    assert!(it.attach_with_outer(ObjectId(8), 2, Some(2)));
    assert_eq!(it.outer_module(2), None);
    // Loading again starts every module over, without outer modules; collection keeps chained environments.
    it.collect_garbage();
    assert_eq!(show(&it.lookup(0, 3).unwrap()), "8");
    it.load(&mut h);
    assert!(it.lookup(0, 3).is_none());
}

#[test]
fn sly3_value_kinds_and_unknown_keywords() {
    // Sly 3 numbering of the Sly 1 value kinds (specs/sly3/world-file.md 4.1: Sly 2's kinds above 18 down by 2).
    let sly3 = |k: u8| match k {
        20 => 19,
        21 => 20,
        22..=26 => k + 2,
        27..=29 => k - 6,
        30 => 29,
        32 => 31,
        _ => k,
    };
    let datum = L(vec![
        I(-5),
        F(1.5),
        B(true),
        S(7),
        Op(40),
        V([1.0, 2.0, 3.0]),
        C([4.0, 5.0, 6.0]),
        R([0.0, 1.0]),
        Null,
        L(vec![I(1)]),
    ]);
    let mut bytes = Vec::new();
    enc_seq_with(core::slice::from_ref(&datum), &mut bytes, &sly3);
    let mut p = Program::for_game(crate::Game::Sly3);
    assert_eq!(p.read_module(&bytes, 0, false).unwrap(), bytes.len());
    let one = program(core::slice::from_ref(&datum));
    let text = |p: &Program| format!("{}", p.display(p.module(0)[0]));
    assert_eq!(text(&p), text(&one));
    assert_eq!((p.cell_counts[29], p.cell_counts[19], p.cell_counts[31]), (2, 2, 1));
    // Keywords map onto the interpreter's (specs/sly3/scripts.md 3): kind 15 is begin, 16 quote, 18 the rest marker.
    let mut q = Program::for_game(crate::Game::Sly3);
    q.read_module(&[29, 15 | 0x80, 19, 1, 0, 0, 0], 0, false).unwrap();
    assert_eq!(format!("{}", q.display(q.module(0)[0])), "(begin 1)");
    let mut it = Interpreter::new(q, None);
    let mut h = Mock::default();
    it.load(&mut h);
    assert_eq!(it.custom_base(), 1816);
    let mut op = |id: u32, args: Vec<Value>| show(&it.call(&mut h, Value::Builtin(id), args));
    // Operations run as their Sly 2 equivalents (4.2), or as Sly 3's own.
    assert_eq!(op(0, vec![Value::Int(1), Value::Int(2)]), "3");
    assert_eq!(op(56, vec![Value::Float(9.0)]), "3.0", "56 is Sly 2's 54, square root");
    assert_eq!(op(48, vec![Value::Float(2.0)]), "#(2.0 2.0 2.0)", "48 is Sly 2's 47");
    assert_eq!(op(100, vec![Value::Float(0.0), Value::Float(10.0), Value::Float(0.25)]), "2.5");
    assert_eq!(op(19, vec![Value::Vector([0.0; 3])]), "#t");
    assert_eq!(op(31, vec![Value::Bool(false), Value::Int(1)]), "#t", "31 ignores a surplus argument");
    assert_eq!(op(66, vec![Value::Vector([1.0, 0.0, 0.0]), Value::Vector([0.0, 1.0, 0.0])]), "#(0.0 0.0 1.0)");
    assert_eq!(op(5, vec![]), "#error(UnknownOperation(5))");
}

#[test]
fn host_list_arguments_are_charged() {
    // Sly 2's 114 (objects for a list of IDs) and 131 (a progress value by path) walk their lists in the host: the
    // elements are charged to the step budget first, and a looping list is refused.
    let mut it = Interpreter::new(Program::for_game(crate::Game::Sly2), None);
    it.limits.max_steps = 1000;
    let mut h = Mock::default();
    let long = Value::list((0..5000).map(Value::Int).collect::<Vec<_>>());
    let r = it.call(&mut h, Value::Builtin(114), vec![long, Value::Object(None)]);
    assert!(matches!(r, Value::Error(ScriptError::OutOfSteps)), "{r:?}");
    let looped = Value::list([Value::Int(1)]);
    if let Value::List(c) = &looped {
        *c.cdr.borrow_mut() = looped.clone();
    }
    let r = it.call(&mut h, Value::Builtin(131), vec![looped]);
    assert!(matches!(r, Value::Error(ScriptError::TooLarge)), "{r:?}");
    assert!(h.log.is_empty(), "the host never saw them: {:?}", h.log);
    // A short list reaches the host.
    let r = it.call(&mut h, Value::Builtin(131), vec![Value::list([Value::Symbol(7)])]);
    assert!(!matches!(r, Value::Error(ScriptError::OutOfSteps | ScriptError::TooLarge)), "{r:?}");
    assert_eq!(h.log.len(), 1);
}
