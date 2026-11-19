//! Tests against the player's own Sly 3 disc image (specs/sly3/scripts.md). They skip, and pass, when
//! `SLYNX_DISC_SLY3` is not set.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use formats_sly3::exe::Executable;
use formats_sly3::stream::{Record, visit};
use formats_sly3::{Disc, World};
use harness::golden::{Game as DiscGame, require_disc};
use script::sly3::{self, BUILTIN_TABLE_END};
use script::tree::{Node, NodeId, kind};
use script::{
    BuiltinTable, EventSignatures, Game, Interpreter, ObjectId, OptionMap, Program, RaisedEvent, ScriptError,
    ScriptHost, Value,
};

/// One world's scripts and the facts the tests need about its objects.
struct WorldScripts {
    slot: usize,
    groups: Vec<String>,
    program: Program,
    /// Top-level records of every group: (group, class, script module).
    records: Vec<(usize, i16, i16)>,
    /// Records at every depth that name a module: (group, script module).
    nested_named: Vec<(usize, i16)>,
}

struct DiscData {
    exe: Executable,
    options: formats_sly2::exe::OptionTable,
    worlds: Vec<WorldScripts>,
}

fn disc(test: &str) -> Option<&'static DiscData> {
    let path = require_disc(DiscGame::Sly3, test)?;
    static CELL: OnceLock<DiscData> = OnceLock::new();
    Some(CELL.get_or_init(|| load(&path)))
}

fn load(path: &Path) -> DiscData {
    let mut d = Disc::open_path(path).expect("open disc");
    let slots: Vec<usize> = d.world_slots().collect();
    let compressed: Vec<(usize, Vec<u8>)> = slots.iter().map(|&s| (s, d.read_world_compressed(s).unwrap())).collect();
    let options = d.options.clone();
    let multipliers = d.tangent_multipliers;
    let worlds = std::thread::scope(|s| {
        let hs: Vec<_> = compressed
            .iter()
            .map(|(slot, c)| {
                let options = &options;
                s.spawn(move || {
                    let data = formats::compress::decompress(c).unwrap();
                    let world =
                        World::parse_with(&data, options, multipliers).unwrap_or_else(|e| panic!("slot {slot}: {e}"));
                    let program = Program::from_sly3_world(&world).unwrap_or_else(|e| panic!("slot {slot}: {e}"));
                    let groups = world.groups.iter().map(|g| g.name.clone()).collect();
                    let mut records = Vec::new();
                    let mut nested_named = Vec::new();
                    for (g, body) in world.bodies.iter().enumerate() {
                        for r in &body.objects {
                            records.push((g, r.class, r.script));
                        }
                        visit(&body.objects, &mut |r: &Record| {
                            if r.script >= 0 {
                                nested_named.push((g, r.script));
                            }
                        });
                    }
                    WorldScripts { slot: *slot, groups, program, records, nested_named }
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    DiscData { exe: d.executable.clone(), options, worlds }
}

fn builtins(d: &DiscData) -> &'static BuiltinTable {
    static CELL: OnceLock<BuiltinTable> = OnceLock::new();
    CELL.get_or_init(|| sly3::builtin_table(&d.exe).expect("built-in table"))
}

fn option_map(d: &DiscData) -> &'static OptionMap {
    static CELL: OnceLock<OptionMap> = OnceLock::new();
    CELL.get_or_init(|| OptionMap::from_sly3_executable(&d.exe).expect("option map"))
}

/// Visits every non-empty list outside quoted data: `f(list)`.
fn unquoted_lists(p: &Program, f: &mut impl FnMut(NodeId)) {
    fn walk(p: &Program, n: NodeId, f: &mut impl FnMut(NodeId)) {
        if !matches!(p.node(n), Node::List { len, .. } if *len > 0) {
            return;
        }
        f(n);
        if p.head_keyword(n) == Some(kind::QUOTE) {
            return;
        }
        for &c in p.list(n) {
            walk(p, c, f);
        }
    }
    for m in 0..p.module_count() {
        for &top in p.module(m) {
            walk(p, top, f);
        }
    }
}

#[test]
fn script_blocks_parse_in_every_world() {
    let Some(d) = disc("script_blocks_parse_in_every_world") else { return };
    // scripts.md 1: 1,524 of 7,151 groups carry modules, 11,007 modules in all.
    let groups: usize = d.worlds.iter().map(|w| w.groups.len()).sum();
    let with: usize = d.worlds.iter().map(|w| w.program.packages.iter().filter(|p| p.1 > 0).count()).sum();
    let modules: usize = d.worlds.iter().map(|w| w.program.module_count()).sum();
    assert_eq!((d.worlds.len(), groups, with, modules), (39, 7_151, 1_524, 11_007));
    // Every record's script module is within its own group's block: 6,698 top-level records and 10,631 at every
    // depth name one.
    let mut named = (0, 0);
    for w in &d.worlds {
        named.0 += w.records.iter().filter(|r| r.2 >= 0).count();
        named.1 += w.nested_named.len();
        for &(g, s) in &w.nested_named {
            assert!(w.program.package_module(g, s).is_some(), "slot {}: group {g} module {s}", w.slot);
        }
    }
    assert_eq!(named, (6_698, 10_631));
    // Every module is one `begin` holding only `define`s (3).
    for w in &d.worlds {
        let p = &w.program;
        for m in 0..p.module_count() {
            let tops = p.module(m);
            assert_eq!(tops.len(), 1, "slot {} module {m}", w.slot);
            assert_eq!(p.head_keyword(tops[0]), Some(kind::BEGIN), "slot {} module {m}", w.slot);
            for &def in &p.list(tops[0])[1..] {
                assert_eq!(p.head_keyword(def), Some(kind::DEFINE), "slot {} module {m}", w.slot);
                assert_eq!(p.list(def).len(), 3);
            }
        }
    }
}

#[test]
fn keywords_and_cells_match_the_spec() {
    let Some(d) = disc("keywords_and_cells_match_the_spec") else { return };
    let mut cells = [0u64; 38];
    let mut heads: BTreeMap<u8, u64> = BTreeMap::new();
    for w in &d.worlds {
        let p = &w.program;
        for (c, n) in cells.iter_mut().zip(p.cell_counts) {
            *c += u64::from(n);
        }
        unquoted_lists(p, &mut |l| {
            if let Some(k) = p.head_keyword(l) {
                *heads.entry(k).or_default() += 1;
            }
        });
    }
    // Cell kinds (2).
    for (k, n) in [(19, 228_405), (20, 17_141), (21, 8_496), (22, 258_696), (23, 180_961), (24, 2), (25, 0)] {
        assert_eq!(cells[k], n, "kind {k}");
    }
    for (k, n) in [(26, 31), (27, 60), (28, 1), (29, 463_142), (31, 25_553), (0, 6_556)] {
        assert_eq!(cells[k], n, "kind {k}");
    }
    // Keyword heads outside quoted data (3), as the interpreter's kinds.
    let expected = BTreeMap::from([
        (kind::SET, 20_132),
        (kind::DEFINE, 54_209),
        (kind::ASSERT, 585),
        (kind::IF, 12_018),
        (kind::OR, 492),
        (kind::AND, 3_358),
        (kind::COND, 579),
        (kind::ELSE, 699),
        (kind::CASE, 7_386),
        (kind::LET, 26),
        (kind::WHILE, 153),
        (kind::LAMBDA, 31_321),
        (kind::BEGIN, 17_146),
        (kind::QUOTE, 6_924),
    ]);
    assert_eq!(heads, expected);
}

#[test]
fn builtin_table_and_every_call() {
    let Some(d) = disc("builtin_table_and_every_call") else { return };
    let t = builtins(d);
    assert_eq!(t.entries.len(), 178);
    assert_eq!(d.exe.bytes_at(BUILTIN_TABLE_END, 4).unwrap(), [0, 0, 0, 0]);
    let mut calls: BTreeMap<u32, u64> = BTreeMap::new();
    let mut surplus = Vec::new();
    for w in &d.worlds {
        let p = &w.program;
        unquoted_lists(p, &mut |l| {
            let items = p.list(l);
            let &Node::Builtin(op) = p.node(items[0]) else { return };
            *calls.entry(op).or_default() += 1;
            let args = items.len() - 1;
            let (min, more) = t.rule(op).unwrap_or_else(|| panic!("op {op} outside the table"));
            if !(args >= min as usize && (more || args == min as usize)) {
                surplus.push((op, args));
            }
        });
    }
    // 4.1: 178,038 calls of 116 IDs; every call obeys its rule except one call of 31 with two arguments.
    assert_eq!((calls.values().sum::<u64>(), calls.len()), (178_038, 116));
    assert_eq!(surplus, [(31, 2)]);
    // The operations used are exactly those the crate defines, with the executable's rules (31 aside, which the
    // crate lets take its surplus argument).
    assert_eq!(calls.keys().copied().collect::<Vec<_>>(), sly3::defined().collect::<Vec<_>>());
    for op in sly3::defined().filter(|&op| op != 31) {
        assert_eq!(sly3::arity(op), t.rule(op), "op {op}");
    }
    // 4.2: corresponding entries share their argument rule with Sly 2's (6 game operations differ, 138 the only
    // one on the disc).
    for (op, n) in [(117, 68_435), (118, 18_869), (116, 17_281), (31, 7_676), (40, 6_895), (7, 5_505)] {
        assert_eq!(calls[&op], n, "op {op}");
    }
}

#[test]
fn option_map_from_the_executable() {
    let Some(d) = disc("option_map_from_the_executable") else { return };
    let m = option_map(d);
    assert_eq!((m.class_count(), m.records), (249, 16_803));
    let mut ids = BTreeSet::new();
    let mut targets: BTreeMap<u16, BTreeSet<u16>> = BTreeMap::new();
    let mut max_stream = 0;
    for c in 0..249 {
        for &(s, t) in m.class(c) {
            ids.insert(s);
            targets.entry(s).or_default().insert(t);
            max_stream = max_stream.max(t);
        }
    }
    assert_eq!(ids.len(), 1_587);
    assert_eq!((ids.first(), ids.last()), (Some(&1), Some(&1_815)));
    assert_eq!(max_stream, 2_043);
    assert_eq!(targets.values().filter(|t| t.len() > 1).count(), 237);
    // Duplicate entries (5): the game's binary search finds these.
    for (class, id, found) in [
        (74, 216, 92),
        (88, 939, 1968),
        (88, 940, 1965),
        (100, 145, 883),
        (111, 111, 44),
        (111, 112, 45),
        (111, 114, 1139),
        (111, 115, 1142),
        (115, 39, 25),
        (136, 39, 25),
        (137, 39, 25),
    ] {
        assert_eq!(m.lookup(class, id), Some(found), "class {class} ID {id}");
    }
    // Custom properties from 1,816; the literals scripts pass to 114 to 117 start at 1,817.
    assert_eq!(script::option_map::SLY3_CUSTOM_BASE, 1_816);
    // Counted over every list, quoted data included, as the spec counts them.
    let mut literal = BTreeSet::new();
    for w in &d.worlds {
        let p = &w.program;
        for l in (0..p.nodes.len() as NodeId).filter(|&n| matches!(p.node(n), Node::List { len, .. } if *len > 0)) {
            let items = p.list(l);
            if let (&Node::Builtin(114..=117), Some(&Node::Int(id))) =
                (p.node(items[0]), items.get(2).map(|&n| p.node(n)))
            {
                literal.insert(id);
            }
        }
    }
    let custom: BTreeSet<i32> = literal.iter().copied().filter(|&id| !ids.contains(&(id as u16))).collect();
    assert_eq!((literal.len(), custom.len()), (1_531, 960));
    assert_eq!((custom.first(), custom.last()), (Some(&1_817), Some(&2_786)));
}

#[test]
fn event_signatures_and_handlers() {
    let Some(d) = disc("event_signatures_and_handlers") else { return };
    let sig: EventSignatures = sly3::event_signatures(&d.exe).expect("signatures");
    // 6: lengths from the start positions; event 32's one word is the index's first, event 33 is empty.
    let starts = [
        0, 1, 2, 3, 4, 6, 7, 8, 11, 14, 15, 16, 17, 19, 22, 25, 28, 29, 31, 33, 34, 37, 39, 40, 41, 42, 43, 45, 47, 49,
        50, 51, 55, 56,
    ];
    assert_eq!(sig.events.len(), 34);
    for e in 0..34 {
        let want = if e + 1 < 34 { starts[e + 1] - starts[e] } else { 0 };
        assert_eq!(sig.events[e].len(), want, "event {e}");
    }
    assert_eq!(sig.events[32], [u32::from_le_bytes(d.exe.bytes_at(sly3::EVENT_INDEX, 4).unwrap().try_into().unwrap())]);
    // Handlers: parameter counts equal signature lengths except event 28's five (one fewer) and five of event 32's
    // (four more).
    let mut off: BTreeMap<(u32, i64), u64> = BTreeMap::new();
    for w in &d.worlds {
        let p = &w.program;
        for m in 0..p.module_count() {
            for (sym, value) in p.definitions(m) {
                if sym >= 34 || p.head_keyword(value) != Some(kind::LAMBDA) {
                    continue;
                }
                let params = p.list(p.list(value)[1]).len() as i64;
                let want = sig.len(sym).unwrap() as i64;
                if params != want {
                    *off.entry((sym, params - want)).or_default() += 1;
                }
            }
        }
    }
    assert_eq!(off, BTreeMap::from([((28, -1), 5), ((32, 4), 5)]));
}

/// The test vectors of section 8 (v_apt, group `v_macro`).
#[test]
fn decoded_modules() {
    let Some(d) = disc("decoded_modules") else { return };
    let w = d.worlds.iter().find(|w| w.groups.iter().any(|g| g == "v_macro")).expect("v_apt");
    let g = w.groups.iter().position(|g| g == "v_macro").unwrap();
    assert_eq!(w.program.packages[g].1, 144);
    let text = |m: i16| {
        let id = w.program.package_module(g, m).unwrap();
        w.program.module(id as usize).iter().map(|&n| w.program.display(n).to_string()).collect::<String>()
    };
    assert_eq!(text(1), "(begin (define s0 (lambda (s1240) ((op117 (op124) 290) 46))))");
    assert_eq!(
        text(117),
        "(begin (define s24 (lambda (s1240) ((op117 (op117 s1240 1295) 2717) s1240 140196 (quote (140197))))))"
    );
}

/// Answers every query with something plausible (Sly 3 game operations by their Sly 3 numbers).
#[derive(Default)]
struct Mock {
    classes: BTreeMap<ObjectId, i16>,
    events: Vec<RaisedEvent>,
}

impl ScriptHost for Mock {
    fn class_of(&self, o: ObjectId) -> Option<i16> {
        self.classes.get(&o).copied()
    }
    fn get_option(&mut self, _: ObjectId, _: u16) -> Value {
        Value::Int(0)
    }
    fn set_option(&mut self, _: ObjectId, _: u16, _: &Value) -> Result<(), ScriptError> {
        Ok(())
    }
    fn call_action(&mut self, _: ObjectId, _: u16, _: &[Value]) -> Value {
        Value::Void
    }
    fn find_object(&mut self, _: i16, _: Option<ObjectId>) -> Option<ObjectId> {
        self.classes.keys().next().copied()
    }
    fn game_operation(&mut self, op: u32, _: &[Value]) -> Value {
        match op {
            103 | 107..=110 | 146 => Value::Int(0),
            120 | 129..=132 => Value::Nil,
            139 | 140 | 147 => Value::Void,
            _ => Value::Object(self.classes.keys().next().copied()),
        }
    }
    fn poll_event(&mut self) -> Option<RaisedEvent> {
        self.events.pop()
    }
}

/// Errors that mean the interpreter does not understand the program (see the Sly 2 disc tests).
fn structural(e: &ScriptError) -> bool {
    matches!(
        e,
        ScriptError::UnknownKeyword(_)
            | ScriptError::BadForm(_)
            | ScriptError::BadCell(_)
            | ScriptError::UnknownOperation(_)
            | ScriptError::TooFewArguments { .. }
            | ScriptError::TooManyArguments { .. }
            | ScriptError::StaleEnvironment
    )
}

struct Counting(Mock, Vec<ScriptError>, u64);

impl ScriptHost for Counting {
    fn class_of(&self, o: ObjectId) -> Option<i16> {
        self.0.class_of(o)
    }
    fn get_option(&mut self, o: ObjectId, op: u16) -> Value {
        self.0.get_option(o, op)
    }
    fn set_option(&mut self, o: ObjectId, op: u16, v: &Value) -> Result<(), ScriptError> {
        self.0.set_option(o, op, v)
    }
    fn call_action(&mut self, o: ObjectId, op: u16, a: &[Value]) -> Value {
        self.0.call_action(o, op, a)
    }
    fn find_object(&mut self, oid: i16, c: Option<ObjectId>) -> Option<ObjectId> {
        self.0.find_object(oid, c)
    }
    fn game_operation(&mut self, op: u32, a: &[Value]) -> Value {
        self.0.game_operation(op, a)
    }
    fn poll_event(&mut self) -> Option<RaisedEvent> {
        self.0.poll_event()
    }
    fn report(&mut self, e: &ScriptError) {
        self.2 += 1;
        if structural(e) {
            self.1.push(*e);
        }
    }
}

#[test]
fn every_world_loads_and_runs_its_load_events() {
    let Some(d) = disc("every_world_loads_and_runs_its_load_events") else { return };
    let table = builtins(d);
    let map = option_map(d);
    let results: Vec<(Vec<String>, u64)> = std::thread::scope(|s| {
        let hs: Vec<_> = d
            .worlds
            .iter()
            .map(|w| {
                s.spawn(move || {
                    let mut it = Interpreter::new(w.program.clone(), Some(map.clone()));
                    assert_eq!(it.game(), Game::Sly3);
                    it.set_sly3_tables(Some(table.clone()), &d.options);
                    let mut host = Counting(Mock::default(), Vec::new(), 0);
                    let mut attached = Vec::new();
                    for (i, &(g, class, script)) in w.records.iter().enumerate() {
                        let o = ObjectId(i as u32);
                        host.0.classes.insert(o, class);
                        if let Some(m) = w.program.package_module(g, script) {
                            it.attach(o, m);
                            attached.push(o);
                        }
                    }
                    it.load(&mut host);
                    it.post_load(&mut host, attached.iter().copied());
                    for _ in 0..600 {
                        it.tick(&mut host, 1.0 / 60.0);
                    }
                    (host.1.iter().map(|e| format!("slot {}: {e:?}", w.slot)).collect::<Vec<_>>(), host.2)
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let errors: u64 = results.iter().map(|r| r.1).sum();
    let failures: Vec<&String> = results.iter().flat_map(|r| &r.0).collect();
    eprintln!("{} error values in all (mock answers included)", errors);
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for f in &failures {
        *by_kind.entry(f.split(": ").nth(1).unwrap_or("").to_string()).or_default() += 1;
    }
    assert!(failures.is_empty(), "{} structural errors: {by_kind:?}", failures.len());
}
