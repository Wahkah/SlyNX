//! Tests against the player's own Sly 2 disc image (specs/sly2/scripts.md test vectors). They skip, and pass, when
//! `SLYNX_DISC_SLY2` is not set.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use formats_sly2::exe::Executable;
use formats_sly2::stream::{Record, visit};
use formats_sly2::{Disc, World};
use harness::golden::{Game as DiscGame, require_disc};
use script::sly2::{self, BUILTIN_TABLE_END, BuiltinTable, EventSignatures};
use script::tree::{Node, NodeId, kind};
use script::{Game, Interpreter, ObjectId, OptionMap, Program, RaisedEvent, ScriptError, ScriptHost, Value};

/// One world's scripts and the facts the tests need about its objects.
struct WorldScripts {
    row: usize,
    /// Package names, in order (the level package last).
    packages: Vec<String>,
    program: Program,
    /// Top-level records of every package: (package, class, script index).
    records: Vec<(usize, i16, i16)>,
    /// Records at every depth that name a module: (package, script index).
    nested_named: Vec<(usize, i16)>,
}

struct DiscData {
    exe: Executable,
    options: formats_sly2::exe::OptionTable,
    worlds: Vec<WorldScripts>,
}

fn disc(test: &str) -> Option<&'static DiscData> {
    let path = require_disc(DiscGame::Sly2, test)?;
    static CELL: OnceLock<DiscData> = OnceLock::new();
    Some(CELL.get_or_init(|| load(&path)))
}

fn load(path: &Path) -> DiscData {
    let mut d = Disc::open_path(path).expect("open disc");
    let rows: Vec<usize> = d.world_rows().collect();
    let compressed: Vec<(usize, Vec<u8>)> = rows.iter().map(|&r| (r, d.read_world_compressed(r).unwrap())).collect();
    let options = d.options.clone();
    let worlds = std::thread::scope(|s| {
        let hs: Vec<_> = compressed
            .iter()
            .map(|(row, c)| {
                let options = &options;
                s.spawn(move || {
                    let data = formats::compress::decompress(c).unwrap();
                    let world = World::parse(&data, options).unwrap_or_else(|e| panic!("row {row}: {e}"));
                    let program = Program::from_sly2_world(&world).unwrap_or_else(|e| panic!("row {row}: {e}"));
                    let packages = world.header.packages.iter().map(|p| p.name.clone()).collect();
                    let mut records = Vec::new();
                    let mut nested_named = Vec::new();
                    for (p, body) in world.bodies.iter().enumerate() {
                        for r in &body.objects {
                            records.push((p, r.class, r.script));
                        }
                        visit(&body.objects, &mut |r: &Record| {
                            if r.script >= 0 {
                                nested_named.push((p, r.script));
                            }
                        });
                    }
                    WorldScripts { row: *row, packages, program, records, nested_named }
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    DiscData { exe: d.executable.clone(), options, worlds }
}

fn builtins(d: &DiscData) -> &'static BuiltinTable {
    static CELL: OnceLock<BuiltinTable> = OnceLock::new();
    CELL.get_or_init(|| BuiltinTable::from_executable(&d.exe).expect("built-in table"))
}

fn option_map(d: &DiscData) -> &'static OptionMap {
    static CELL: OnceLock<OptionMap> = OnceLock::new();
    CELL.get_or_init(|| OptionMap::from_sly2_executable(&d.exe).expect("option map"))
}

/// Calls every node of a program: (list node, head node).
fn lists(p: &Program) -> impl Iterator<Item = NodeId> + '_ {
    (0..p.nodes.len() as NodeId).filter(|&n| matches!(p.node(n), Node::List { len, .. } if *len > 0))
}

#[test]
fn script_blocks_parse_in_every_world() {
    let Some(d) = disc("script_blocks_parse_in_every_world") else { return };
    assert_eq!(d.worlds.len(), 43);
    let packages: usize = d.worlds.iter().map(|w| w.packages.len()).sum();
    let with: usize = d.worlds.iter().map(|w| w.program.packages.iter().filter(|p| p.1 > 0).count()).sum();
    let modules: usize = d.worlds.iter().map(|w| w.program.module_count()).sum();
    assert_eq!((packages, with, modules), (7_753, 1_558, 10_930));
    // Cell totals (7.2) and keyword counts (3), by raw Sly 2 kind.
    let mut cells = [0u64; 38];
    for w in &d.worlds {
        for (c, n) in cells.iter_mut().zip(w.program.cell_counts) {
            *c += u64::from(n);
        }
    }
    let by_kind: BTreeMap<u8, u64> = [
        (31, 302_646),
        (24, 154_946),
        (21, 146_907),
        (25, 117_345),
        (33, 19_874),
        (22, 6_850),
        (23, 5_672),
        (0, 4_134),
        (28, 7),
        (26, 0),
        (27, 0),
        (29, 0),
        (30, 0),
    ]
    .into();
    for (k, want) in by_kind {
        assert_eq!(cells[k as usize], want, "kind {k}");
    }
    // Keywords heading lists (3), by the kind the interpreter gives them (Sly 2 kind in the comment).
    let mut heads: BTreeMap<u8, u64> = BTreeMap::new();
    for w in &d.worlds {
        for l in lists(&w.program) {
            if let Some(k) = w.program.head_keyword(l) {
                *heads.entry(k).or_default() += 1;
            }
        }
    }
    let want: BTreeMap<u8, u64> = [
        (kind::SET, 8_474),     // 1
        (kind::DEFINE, 34_763), // 2
        (kind::ASSERT, 262),    // 5
        (kind::IF, 8_257),      // 7
        (kind::OR, 273),        // 8
        (kind::AND, 1_423),     // 9
        (kind::COND, 130),      // 10
        (kind::ELSE, 190),      // 11
        (kind::CASE, 4_772),    // 12
        (kind::WHILE, 30),      // 14
        (kind::LAMBDA, 24_194), // 15
        (kind::BEGIN, 14_644),  // 16
        (kind::QUOTE, 3_883),   // 18
    ]
    .into();
    assert_eq!(heads, want, "kinds 3, 4, 6, 13 (let) and 17 never head a list");
    // One `or` cell is not a list head (quoted data).
    assert_eq!(cells[8], 274);
    // Every module is one begin of defines (one is empty).
    let mut empty = 0;
    for w in &d.worlds {
        let p = &w.program;
        for m in 0..p.module_count() {
            let tops = p.module(m);
            if tops.is_empty() || matches!(p.node(tops[0]), Node::Nil) {
                empty += 1;
                continue;
            }
            assert_eq!(tops.len(), 1, "row {} module {m}", w.row);
            assert_eq!(p.head_keyword(tops[0]), Some(kind::BEGIN), "row {} module {m}", w.row);
            for &def in &p.list(tops[0])[1..] {
                assert_eq!(p.head_keyword(def), Some(kind::DEFINE), "row {} module {m}", w.row);
            }
        }
    }
    assert_eq!(empty, 1);
}

#[test]
fn module_counts_per_world() {
    let Some(d) = disc("module_counts_per_world") else { return };
    // 7.1: (level package, modules, packages with scripts, packages).
    let table = [
        ("jb_intro", 154, 40, 239),
        ("dvd_menu", 1, 1, 3),
        ("f_nightclub_exterior", 378, 66, 287),
        ("f_nightclub_cellar", 237, 28, 172),
        ("i_palace_ext", 322, 57, 243),
        ("p_prison_ext", 442, 70, 284),
        ("p_castle_ext", 462, 68, 251),
        ("c_sawmill_ext", 414, 75, 281),
        ("a_blimp_boss_final", 226, 36, 263),
    ];
    for (name, modules, with, packages) in table {
        let w = d
            .worlds
            .iter()
            .find(|w| w.packages.last().map(String::as_str) == Some(name))
            .unwrap_or_else(|| panic!("no world {name}"));
        let p = &w.program;
        assert_eq!(p.module_count(), modules, "{name}");
        assert_eq!(p.packages.iter().filter(|x| x.1 > 0).count(), with, "{name}");
        assert_eq!(w.packages.len(), packages, "{name}");
    }
}

#[test]
fn records_name_modules_of_their_own_package() {
    let Some(d) = disc("records_name_modules_of_their_own_package") else { return };
    let mut named = 0;
    let mut top = 0;
    for w in &d.worlds {
        for &(p, s) in &w.nested_named {
            named += 1;
            assert!(w.program.package_module(p, s).is_some(), "row {}: package {p} script {s}", w.row);
        }
        top += w.records.iter().filter(|r| r.2 >= 0).count();
    }
    // 7,138 top-level records name a module (scripts.md 1); nested records do too, 10,554 at every depth.
    assert_eq!((top, named), (7_138, 10_554));
}

#[test]
fn builtin_table_and_every_call() {
    let Some(d) = disc("builtin_table_and_every_call") else { return };
    let t = builtins(d);
    assert_eq!(t.entries.len(), 169);
    assert_eq!(d.exe.bytes_at(BUILTIN_TABLE_END, 4).unwrap(), [0, 0, 0, 0]);
    let mut calls: BTreeMap<u32, u64> = BTreeMap::new();
    let mut total = 0u64;
    for w in &d.worlds {
        let p = &w.program;
        for l in lists(p) {
            let items = p.list(l);
            let &Node::Builtin(op) = p.node(items[0]) else { continue };
            total += 1;
            *calls.entry(op).or_default() += 1;
            let args = items.len() - 1;
            let (min, more) = t.rule(op).unwrap_or_else(|| panic!("op {op} outside the table"));
            assert!(args >= min as usize && (more || args == min as usize), "row {}: op {op} with {args}", w.row);
        }
    }
    assert_eq!(total, 117_333);
    // The 97 operations used are exactly those the crate implements, with the executable's argument rules.
    assert_eq!(calls.keys().copied().collect::<Vec<_>>(), sly2::defined().collect::<Vec<_>>());
    for op in sly2::defined() {
        assert_eq!(sly2::arity(op), t.rule(op), "op {op}");
    }
    // A few call counts from 4.2.
    for (op, n) in [(111, 52_334), (112, 17_969), (110, 11_789), (7, 3_385), (105, 1_824), (13, 1), (121, 1)] {
        assert_eq!(calls[&op], n, "op {op}");
    }
}

#[test]
fn option_map_from_the_executable() {
    let Some(d) = disc("option_map_from_the_executable") else { return };
    let m = option_map(d);
    assert_eq!(m.class_count(), 199);
    assert_eq!(m.records, 12_010);
    let mut ids = BTreeSet::new();
    let mut targets: BTreeMap<u16, BTreeSet<u16>> = BTreeMap::new();
    let mut max_stream = 0;
    let mut kept = 0;
    for c in 0..199 {
        for &(s, t) in m.class(c) {
            ids.insert(s);
            targets.entry(s).or_default().insert(t);
            max_stream = max_stream.max(t);
            kept += 1;
        }
    }
    assert_eq!(ids.len(), 1_091);
    assert_eq!((ids.first(), ids.last()), (Some(&1), Some(&1_232)));
    assert_eq!(max_stream, 1_448);
    assert_eq!(targets.values().filter(|t| t.len() > 1).count(), 210);
    // Five classes list an ID twice in a row; both entries are kept and looked up by the game's binary search.
    let word = |va: u32| u32::from_le_bytes(d.exe.bytes_at(va, 4).unwrap().try_into().unwrap());
    let mut dup_classes = Vec::new();
    for c in 0..199u32 {
        let (n, addr) = (word(0x0027_9AD0 + c * 8), word(0x0027_9AD0 + c * 8 + 4));
        let list: Vec<(u32, u32)> = (0..n).map(|i| (word(addr + i * 8), word(addr + i * 8 + 4))).collect();
        let dups: Vec<_> = list.windows(2).filter(|w| w[0].0 == w[1].0).map(|w| (w[0].0, w[0].1 == w[1].1)).collect();
        if !dups.is_empty() {
            dup_classes.push((c, dups));
        }
    }
    // Five classes, nine repeated pairs, and each pair maps the ID to two different stream options; the record the
    // search reaches (scripts.md 5) is the second for class 98, IDs 112 and 113.
    assert_eq!(dup_classes.iter().map(|c| c.0).collect::<Vec<_>>(), [88, 98, 100, 101, 102]);
    assert_eq!(m.records, kept);
    assert_eq!(dup_classes.iter().map(|c| c.1.len()).sum::<usize>(), 9);
    assert!(dup_classes.iter().flat_map(|c| &c.1).all(|&(_, same)| !same));
    for (class, id, found) in [
        (88, 141, 833),
        (98, 110, 1040),
        (98, 111, 1043),
        (98, 112, 46),
        (98, 113, 47),
        (100, 183, 1103),
        (101, 183, 1103),
        (102, 37, 1113),
        (102, 183, 1103),
    ] {
        assert_eq!(m.lookup(class, id), Some(found), "class {class} ID {id}");
    }
    // Literal option IDs passed to 108 to 111: custom properties are the ones above the map (from 1,233, the game's
    // fixed threshold), 1,234 to 1,904 on the disc.
    assert_eq!(script::option_map::SLY2_CUSTOM_BASE, ids.last().map_or(0, |&l| u32::from(l) + 1));
    let mut literal = BTreeSet::new();
    for w in &d.worlds {
        let p = &w.program;
        for l in lists(p) {
            let items = p.list(l);
            if let (&Node::Builtin(108..=111), Some(&Node::Int(id))) =
                (p.node(items[0]), items.get(2).map(|&n| p.node(n)))
            {
                literal.insert(id);
            }
        }
    }
    let custom: BTreeSet<i32> = literal.iter().copied().filter(|&id| !ids.contains(&(id as u16))).collect();
    assert_eq!((literal.len(), custom.len()), (1_016, 664));
    assert_eq!((custom.first(), custom.last()), (Some(&1_234), Some(&1_904)));
}

#[test]
fn event_handlers_match_their_signatures() {
    let Some(d) = disc("event_handlers_match_their_signatures") else { return };
    let sig = EventSignatures::from_executable(&d.exe).expect("signatures");
    assert_eq!(sig.events.len(), 30);
    // Signature lengths and module-level definitions per event (6).
    let table: [(u32, usize, u64); 26] = [
        (0, 1, 2_100),
        (1, 1, 158),
        (2, 1, 1_113),
        (3, 1, 759),
        (4, 2, 3_036),
        (5, 1, 1_418),
        (6, 1, 851),
        (7, 3, 54),
        (8, 3, 79),
        (9, 1, 33),
        (10, 3, 6),
        (11, 3, 57),
        (12, 3, 1),
        (13, 1, 1),
        (14, 2, 501),
        (15, 2, 309),
        (16, 1, 63),
        (17, 3, 639),
        (18, 2, 27),
        (19, 1, 3_358),
        (20, 1, 263),
        (21, 1, 907),
        (22, 1, 68),
        (24, 2, 64),
        (25, 2, 3),
        (28, 4, 22),
    ];
    let mut defs: BTreeMap<u32, u64> = BTreeMap::new();
    let mut fewer: BTreeMap<u32, u64> = BTreeMap::new();
    let mut more: BTreeMap<u32, u64> = BTreeMap::new();
    for w in &d.worlds {
        let p = &w.program;
        for m in 0..p.module_count() {
            for (sym, value) in p.definitions(m) {
                *defs.entry(sym).or_default() += 1;
                if sym >= 30 || p.head_keyword(value) != Some(kind::LAMBDA) {
                    continue;
                }
                let params = p.list(p.list(value)[1]).len();
                let want = sig.len(sym).unwrap();
                if params + 1 == want {
                    *fewer.entry(sym).or_default() += 1;
                } else if params == want + 1 {
                    *more.entry(sym).or_default() += 1;
                } else {
                    assert_eq!(params, want, "row {} module {m} event {sym}", w.row);
                }
            }
        }
    }
    for (e, len, handlers) in table {
        assert_eq!(sig.len(e), Some(len), "event {e}");
        assert_eq!(defs.get(&e).copied().unwrap_or(0), handlers, "event {e}");
    }
    // Event 29 starts at the end of the 51 type words: its signature is empty (6).
    assert_eq!(sig.len(29), Some(0));
    for e in [23, 26, 27, 29] {
        assert_eq!(defs.get(&e), None, "event {e}");
    }
    assert_eq!((defs[&34], defs[&57], defs[&58]), (6, 69, 3));
    assert_eq!(fewer.values().sum::<u64>(), 8);
    assert!(fewer.keys().all(|e| [14, 15, 25].contains(e)), "{fewer:?}");
    assert_eq!(more, BTreeMap::from([(9, 3)]));
}

fn find_module<'a>(d: &'a DiscData, package: &str, module: i16) -> (&'a WorldScripts, usize, u32) {
    for w in &d.worlds {
        if let Some(p) = w.packages.iter().position(|n| n == package) {
            return (w, p, w.program.package_module(p, module).expect("module"));
        }
    }
    panic!("no package {package}")
}

fn text(w: &WorldScripts, m: u32) -> String {
    let p = &w.program;
    p.module(m as usize).iter().map(|&n| format!("{}", p.display(n))).collect()
}

#[test]
fn decoded_modules() {
    let Some(d) = disc("decoded_modules") else { return };
    let (w, p, m) = find_module(d, "f_nightclub_cellar_rig", 12);
    assert_eq!(text(w, m), "(begin (define s6 (lambda (s1002) ((op111 s1002 40)))))");
    let classes: Vec<i16> = w.records.iter().filter(|r| r.0 == p && r.2 == 12).map(|r| r.1).collect();
    assert_eq!(classes, [15]);
    let (w, _, m) = find_module(d, "f_nightclub_macro", 10);
    assert_eq!(text(w, m), "(begin (define s0 (lambda (s1002) ((op111 (op118) 273) 28))))");
    let (w, p, m0) = find_module(d, "f_nightclub_cellar_rig", 0);
    let want = "(begin (define s5 (lambda (s1002) ((op111 (op137) 979) 762 5))))";
    assert_eq!(text(w, m0), want);
    assert_eq!(text(w, w.program.package_module(p, 1).unwrap()), want);
    let (w, _, m) = find_module(d, "bentley_mine", 0);
    let t = text(w, m);
    assert!(
        t.starts_with(
            "(begin (define s1020 (lambda (s1002) (define s1021 -1) (case (op111 s1002 960) ((5 7) (set s1021 3968)) \
             ((9) (set s1021 3965))"
        ),
        "{}",
        &t[..200.min(t.len())]
    );
    assert!(t.contains("(op108 s1002 1238 (lambda () ((op111 s1002 110) (s1020 s1002))))"));
}

/// Answers every query with something plausible and records calls into the game.
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
            97 | 99 | 103 | 104 | 138 => Value::Int(0),
            _ => Value::Object(self.classes.keys().next().copied()),
        }
    }
    fn poll_event(&mut self) -> Option<RaisedEvent> {
        self.events.pop()
    }
}

/// Errors that mean the interpreter does not understand the program (as opposed to a mock answer the script did
/// not expect). Depth and step limits are left out: some handlers recurse until the game's state changes, which a
/// mock never does (36 calls reach the depth limit over the 43 worlds).
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

struct Counting(Mock, Vec<ScriptError>);

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
    let failures: Vec<String> = std::thread::scope(|s| {
        let hs: Vec<_> = d
            .worlds
            .iter()
            .map(|w| {
                s.spawn(move || {
                    let mut it = Interpreter::new(w.program.clone(), Some(map.clone()));
                    assert_eq!(it.game(), Game::Sly2);
                    it.set_sly2_tables(Some(table.clone()), &d.options);
                    let mut host = Counting(Mock::default(), Vec::new());
                    let mut attached = Vec::new();
                    for (i, &(p, class, script)) in w.records.iter().enumerate() {
                        let o = ObjectId(i as u32);
                        host.0.classes.insert(o, class);
                        if let Some(m) = w.program.package_module(p, script) {
                            it.attach(o, m);
                            attached.push(o);
                        }
                    }
                    it.load(&mut host);
                    it.post_load(&mut host, attached.iter().copied());
                    for _ in 0..600 {
                        it.tick(&mut host, 1.0 / 60.0);
                    }
                    host.1.iter().map(|e| format!("row {}: {e:?}", w.row)).collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    assert!(failures.is_empty(), "{} structural errors, first: {:?}", failures.len(), failures.first());
}
