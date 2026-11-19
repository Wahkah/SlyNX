//! Tests against the player's own Sly 1 disc image (scripts.md test vectors). They skip, and pass, when
//! `SLYNX_DISC_SLY1` is not set.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use formats::disc::{Disc, Executable};
use formats::options::OptionTable;
use formats::stream::{Context, ObjectStream};
use formats::world::WorldLayout;
use harness::golden::{Game, require_disc};
use script::machine::StateMachine;
use script::option_map::{self, OptionMap};
use script::tree::{Node, NodeId, kind};
use script::{Interpreter, ObjectId, Program, RaisedEvent, ScriptError, ScriptHost, Value, builtins, event};

struct WorldData {
    name: String,
    bytes: Vec<u8>,
    layout: WorldLayout,
    stream: ObjectStream,
    program: Program,
}

struct DiscData {
    exe: Executable,
    worlds: Vec<WorldData>,
}

fn disc(test: &str) -> Option<&'static DiscData> {
    let path = require_disc(Game::Sly1, test)?;
    static CELL: OnceLock<DiscData> = OnceLock::new();
    Some(CELL.get_or_init(|| {
        let mut d = Disc::open_path(&path).expect("open disc");
        let compressed: Vec<(String, Vec<u8>)> =
            (0..d.worlds.len()).map(|i| (d.worlds[i].name.clone(), d.read_world_compressed(i).unwrap())).collect();
        let worlds = std::thread::scope(|s| {
            let hs: Vec<_> = compressed
                .iter()
                .map(|(name, c)| {
                    s.spawn(move || {
                        let bytes = formats::compress::decompress(c).unwrap();
                        let table = OptionTable::sly1();
                        let layout = WorldLayout::parse(&bytes, &table).unwrap();
                        let stream = ObjectStream::walk(&bytes, layout.object_stream, &table).unwrap();
                        let program = Program::from_world(&bytes, &layout).unwrap_or_else(|e| panic!("{name}: {e}"));
                        WorldData { name: name.clone(), bytes, layout, stream, program }
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        DiscData { exe: d.executable.clone(), worlds }
    }))
}

fn option_map(d: &DiscData) -> &'static OptionMap {
    static CELL: OnceLock<OptionMap> = OnceLock::new();
    CELL.get_or_init(|| OptionMap::from_executable(&d.exe).expect("option map"))
}

fn world<'a>(d: &'a DiscData, name: &str) -> &'a WorldData {
    d.worlds.iter().find(|w| w.name == name).unwrap_or_else(|| panic!("no world {name}"))
}

#[test]
fn cell_and_form_counts() {
    let Some(d) = disc("cell_and_form_counts") else { return };
    assert_eq!(d.worlds.len(), 46);
    let mut cells = [0u64; 38];
    let mut modules = 0;
    let mut defs = 0;
    let mut event_defs = BTreeMap::<u32, u32>::new();
    let mut builtin_ids = BTreeSet::new();
    let mut set_ok = true;
    let mut shape_ok = true;
    for w in &d.worlds {
        let p = &w.program;
        assert_eq!(p.module_count(), w.layout.scripts.count as usize, "{}", w.name);
        for (c, n) in cells.iter_mut().zip(p.cell_counts) {
            *c += n as u64;
        }
        for m in 0..p.module_count() {
            modules += 1;
            let tops = p.module(m);
            // Every module is one begin holding only defines.
            if tops.len() != 1
                || p.head_keyword(tops[0]) != Some(kind::BEGIN)
                || p.list(tops[0])[1..].iter().any(|&e| p.head_keyword(e) != Some(kind::DEFINE))
            {
                shape_ok = false;
            }
            for (sym, _) in p.definitions(m) {
                defs += 1;
                if sym < event::COUNT {
                    *event_defs.entry(sym).or_default() += 1;
                }
            }
        }
        for (id, n) in p.nodes.iter().enumerate() {
            let Node::List { len, .. } = n else { continue };
            let l = p.list(id as u32);
            if *len == 0 {
                continue;
            }
            match p.node(l[0]) {
                Node::Keyword(kind::SET) => set_ok &= l.len() == 3 && matches!(p.node(l[1]), Node::Symbol(_)),
                &Node::Builtin(op) => {
                    builtin_ids.insert(op);
                    let (min, more) = builtins::arity(op).unwrap_or_else(|| panic!("{}: undefined op{op}", w.name));
                    let args = l.len() - 1;
                    assert!(args >= min as usize && (more || args == min as usize), "{}: op{op} with {args}", w.name);
                }
                _ => {}
            }
        }
    }
    assert_eq!(modules, 2591);
    assert_eq!(defs, 5008);
    assert!(shape_ok, "every module is one begin of defines");
    assert!(set_ok, "every set names a symbol");
    assert_eq!(cells[kind::LIST as usize], 52_799);
    assert_eq!(cells[kind::SYMBOL as usize], 31_754);
    assert_eq!(cells[kind::INT as usize], 24_379);
    assert_eq!(cells[kind::BUILTIN as usize], 21_112);
    assert_eq!(cells[kind::FLOAT as usize], 1_804);
    assert_eq!(cells[kind::BOOL as usize], 672);
    assert_eq!(cells[kind::OBJECT as usize], 3_825);
    assert_eq!(cells[kind::VECTOR as usize], 1);
    assert_eq!(cells[kind::RANGE as usize], 1);
    assert_eq!(cells[0] + cells[37], 633, "empty lists: kind 0 {} + 0xFF tags {}", cells[0], cells[37]);
    let keywords = [
        (1, 1325),
        (2, 7838),
        (3, 0),
        (4, 0),
        (5, 22),
        (6, 1055),
        (7, 24),
        (8, 127),
        (9, 16),
        (10, 62),
        (11, 307),
        (12, 21),
        (13, 7),
        (14, 4889),
        (15, 3116),
        (16, 0),
        (17, 284),
        (18, 0),
        (19, 0),
    ];
    for (k, n) in keywords {
        assert_eq!(cells[k], n, "keyword kind {k}");
    }
    // Every other cell kind is absent.
    for k in [23, 24, 26, 31, 33, 34, 35, 36] {
        assert_eq!(cells[k], 0, "kind {k}");
    }
    assert_eq!(builtin_ids.len(), 74);
    // Every defined operation occurs except 90 (round), which scripts.md 11 defines but no script uses.
    assert_eq!(
        builtin_ids.iter().copied().collect::<Vec<_>>(),
        builtins::defined().filter(|&op| op != 90).collect::<Vec<_>>()
    );
    let want = [
        (0, 1007),
        (1, 56),
        (2, 639),
        (3, 109),
        (4, 24),
        (5, 37),
        (6, 4),
        (7, 0),
        (9, 400),
        (10, 615),
        (11, 245),
        (12, 7),
        (13, 5),
        (14, 16),
        (15, 59),
        (16, 35),
        (17, 3),
        (18, 0),
        (19, 3),
        (20, 47),
        (21, 26),
        (22, 65),
        (23, 36),
        (24, 11),
        (25, 43),
        (26, 4),
        (27, 3),
        (28, 3),
        (29, 5),
    ];
    for (e, n) in want {
        assert_eq!(event_defs.get(&e).copied().unwrap_or(0), n, "definitions of event {e}");
    }
    let named: usize = d.worlds.iter().map(|w| w.stream.records.iter().filter(|r| r.script >= 0).count()).sum();
    assert_eq!(named, 2075, "records naming a module");
    let mut unnamed = 0;
    for w in &d.worlds {
        let used: BTreeSet<i16> = w.stream.records.iter().map(|r| r.script).collect();
        unnamed += (0..w.program.module_count() as i16).filter(|m| !used.contains(m)).count();
        for r in w.stream.records.iter().filter(|r| r.script >= 0) {
            assert!((r.script as usize) < w.program.module_count(), "{}: script index {}", w.name, r.script);
        }
    }
    eprintln!("modules no record names: {unnamed} of {modules}");
}

const SPLASH: &str = "(begin (define s0 (lambda (s66) (op101 s66 1015 2))))";
const PARIS_2: &str = "(begin (define s0 (lambda (s66) ((op102 s66 10)))))";
const PARIS_5: &str = "(begin (define s75 null-object) \
     (define s0 (lambda (s66) (op99 s66 1173 #f) (set s75 (op103 1398 null-object)))) \
     (define s76 (lambda (s66 s75) (if (op31 (op102 s66 1173)) (begin ((op102 s75 267)))))) \
     (define s2 (lambda (s66) ((op102 (op103 1397 null-object) 10)) \
     (if (op31 (op102 s75 1006)) (op97 2.5 s76 s66 s75)) ((op102 s66 10)))))";

#[test]
fn decoded_test_vectors() {
    let Some(d) = disc("decoded_test_vectors") else { return };
    for (name, module, range, text) in [
        ("Splash", 0usize, 0x1B..0x41, SPLASH),
        ("Paris, France", 2, 0x72..0x94, PARIS_2),
        ("Paris, France", 5, 0x1AE..0x2A4, PARIS_5),
    ] {
        let w = world(d, name);
        assert_eq!(w.layout.scripts.scripts[module], range.start, "{name} module {module} start");
        let mut p = Program::new();
        let end = p.read_module(&w.bytes, range.start, false).unwrap();
        assert_eq!(end, range.end, "{name} module {module} end");
        let top = w.program.module(module)[0];
        assert_eq!(w.program.display(top).to_string(), text, "{name} module {module}");
    }
    let splash = world(d, "Splash");
    assert_eq!((splash.program.module_count(), splash.program.custom_options), (1, 0));
    assert_eq!(world(d, "Paris, France").program.custom_options, 9);
}

#[test]
fn script_option_mapping() {
    let Some(d) = disc("script_option_mapping") else { return };
    assert_eq!(d.exe.va_to_offset(option_map::TABLE_VA), Some(option_map::TABLE_FILE_OFFSET));
    let m = option_map(d);
    assert_eq!(m.class_count(), option_map::CLASS_COUNT);
    // Stored sorted by script option ID in every class.
    let table = d.exe.bytes_at(option_map::TABLE_VA, option_map::CLASS_COUNT * 8).unwrap();
    for c in 0..option_map::CLASS_COUNT {
        let w = |i: usize| u32::from_le_bytes(table[i..i + 4].try_into().unwrap());
        let (count, addr) = (w(c * 8) as usize, w(c * 8 + 4));
        if count > 0 {
            let raw = d.exe.bytes_at(addr, count * 8).unwrap();
            let ids: Vec<u32> =
                (0..count).map(|i| u32::from_le_bytes(raw[i * 8..i * 8 + 4].try_into().unwrap())).collect();
            assert!(ids.windows(2).all(|p| p[0] < p[1]), "class {c} sorted");
        }
    }
    let mut per_id: BTreeMap<u16, BTreeSet<u16>> = BTreeMap::new();
    for c in 0..option_map::CLASS_COUNT as i16 {
        for &(s, o) in m.class(c) {
            per_id.entry(s).or_default().insert(o);
        }
    }
    assert_eq!(per_id.len(), 964);
    assert_eq!(*per_id.keys().next().unwrap(), 1);
    assert_eq!(*per_id.keys().last().unwrap(), 1172);
    let multi_class = {
        let mut n: BTreeMap<u16, u32> = BTreeMap::new();
        for c in 0..option_map::CLASS_COUNT as i16 {
            for &(s, _) in m.class(c) {
                *n.entry(s).or_default() += 1;
            }
        }
        n.values().filter(|&&v| v > 1).count()
    };
    // scripts.md section 8 says 343 IDs map to different stream options in different classes; the table holds 161
    // such IDs (436 IDs appear in more than one class). Open question in docs/script.md.
    assert_eq!(per_id.values().filter(|s| s.len() > 1).count(), 161);
    assert_eq!(multi_class, 436);
    // Paris module 2's objects: script option 10 is stream option 2 ("remove from the world", an action).
    let paris = world(d, "Paris, France");
    let users: Vec<_> = paris.stream.records.iter().filter(|r| r.script == 2).collect();
    assert!(!users.is_empty());
    for r in users {
        assert_eq!(m.lookup(r.class, 10), Some(2), "class {}", r.class);
    }
}

#[test]
fn state_machines() {
    let Some(d) = disc("state_machines") else { return };
    let table = OptionTable::sly1();
    let (mut total, mut auto) = (0, 0);
    for w in &d.worlds {
        let recs = &w.stream.records;
        for (i, r) in recs.iter().enumerate().filter(|(_, r)| r.class == 136) {
            let m = StateMachine::from_record(&w.bytes, r, &table).unwrap_or_else(|e| panic!("{}: {e}", w.name));
            total += 1;
            auto += m.auto_start as usize;
            // The tables end where the option list starts.
            let body = r.offset + 6;
            let size = 1 + 4 * m.states.len() + 1 + 13 * m.transitions.len();
            assert_eq!(body + size, r.options.unwrap().start, "{} record {i}", w.name);
            // One owned animation per animated transition, all class 131.
            let owned: Vec<_> = recs[i + 1..r.subtree_end as usize]
                .iter()
                .filter(|o| o.parent == Some(i as u32) && o.context == Context::StateMachine)
                .collect();
            assert_eq!(owned.len(), m.transitions.iter().filter(|t| t.animated).count(), "{} record {i}", w.name);
            assert!(owned.iter().all(|o| o.class == 131));
            // Transitions name states of the machine.
            for t in &m.transitions {
                assert!(m.state(t.from).is_some() && m.state(t.to).is_some(), "{} record {i}", w.name);
            }
        }
    }
    assert_eq!(total, 755);
    assert_eq!(auto, 101);
}

/// Proxy class. Proxies share their OID with the objects they place; the mock's finders prefer other records, and a
/// proxy that clones an earlier source, or has a single source, stands for that source (its class).
const PROXY: i16 = 83;

/// A mock engine over a world's records: object `i` is record `i`.
struct Mock<'a> {
    w: &'a WorldData,
    clone_class: BTreeMap<u32, i16>,
    /// What `is_action` answers for stream options the encoding table does not list.
    unknown_actions: bool,
    log: Vec<String>,
    options: BTreeMap<(u32, u16), Value>,
    time: f32,
}

impl<'a> Mock<'a> {
    fn new(w: &'a WorldData) -> Self {
        let mut clone_class = BTreeMap::new();
        for (proxy, source) in w.stream.back_reference_pairs() {
            clone_class.entry(proxy).or_insert(w.stream.records[source as usize].class);
        }
        // A proxy with a single source of its own stands for that source.
        let recs = &w.stream.records;
        for (i, _) in recs.iter().enumerate().filter(|(_, r)| r.class == PROXY) {
            let mut sources = recs.iter().filter(|s| s.parent == Some(i as u32) && s.context == Context::ProxySource);
            if let (Some(s), None) = (sources.next(), sources.next()) {
                clone_class.entry(i as u32).or_insert(s.class);
            }
        }
        Mock { w, clone_class, unknown_actions: false, log: Vec::new(), options: BTreeMap::new(), time: 0.0 }
    }
    fn oid(&self, o: ObjectId) -> i16 {
        self.w.stream.records[o.0 as usize].oid
    }
}

impl ScriptHost for Mock<'_> {
    fn class_of(&self, o: ObjectId) -> Option<i16> {
        let class = self.w.stream.records.get(o.0 as usize)?.class;
        Some(if class == PROXY { self.clone_class.get(&o.0).copied().unwrap_or(class) } else { class })
    }
    fn get_option(&mut self, o: ObjectId, option: u16) -> Value {
        self.log.push(format!("get {} {}", self.oid(o), option));
        use formats::options::OptionEncoding as E;
        // Options no list stores have the kinds of options-by-class.md 1.1.
        let live = match option {
            28 | 849 | 850 => Some(Value::Bool(false)),
            29..=31 | 35..=39 => Some(Value::Vector([0.0; 3])),
            32..=34 => Some(Value::Matrix(Box::new([
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ]))),
            127 => Some(Value::Float(0.0)),
            205 | 844 | 848 | 880 | 1084 => Some(Value::Object(None)),
            _ => None,
        };
        if let Some(v) = live.filter(|_| !self.options.contains_key(&(o.0, option))) {
            return v;
        }
        self.options.get(&(o.0, option)).cloned().unwrap_or_else(|| match OptionTable::sly1().get(option as i16) {
            Some(E::Float) => Value::Float(0.0),
            Some(E::Vector) => Value::Vector([0.0; 3]),
            Some(E::Pair) => Value::Range([0.0; 2]),
            Some(E::Byte) => Value::Bool(false),
            _ => Value::Int(0),
        })
    }
    fn set_option(&mut self, o: ObjectId, option: u16, v: &Value) -> Result<(), ScriptError> {
        self.log.push(format!("set {} {} {}", self.oid(o), option, v));
        self.options.insert((o.0, option), v.clone());
        Ok(())
    }
    fn call_action(&mut self, o: ObjectId, option: u16, args: &[Value]) -> Value {
        self.log.push(format!("action {} {} {}", self.oid(o), option, args.len()));
        Value::Void
    }
    fn is_action(&self, _: u16) -> bool {
        self.unknown_actions
    }
    fn find_object(&mut self, oid: i16, _: Option<ObjectId>) -> Option<ObjectId> {
        self.log.push(format!("find {oid}"));
        let recs = &self.w.stream.records;
        recs.iter()
            .position(|r| r.oid == oid && r.in_world && r.class != PROXY)
            .or_else(|| recs.iter().position(|r| r.oid == oid && r.in_world))
            .map(|i| ObjectId(i as u32))
    }
    fn find_objects(&mut self, oid: i16, _: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        out.extend(
            self.w
                .stream
                .records
                .iter()
                .enumerate()
                .filter(|(_, r)| r.oid == oid && r.in_world && r.class != PROXY)
                .map(|(i, _)| ObjectId(i as u32)),
        );
    }
    fn find_by_class(&mut self, class: i16, _: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        out.extend(
            self.w
                .stream
                .records
                .iter()
                .enumerate()
                .filter(|(_, r)| r.class == class && r.in_world)
                .map(|(i, _)| ObjectId(i as u32)),
        );
    }
    fn player(&mut self) -> Option<ObjectId> {
        self.find_object(5, None)
    }
    fn game_time(&self) -> f32 {
        self.time
    }
    fn poll_event(&mut self) -> Option<RaisedEvent> {
        None
    }
}

fn attached(w: &WorldData) -> Vec<(ObjectId, u32)> {
    w.stream
        .records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.script >= 0)
        .map(|(i, r)| (ObjectId(i as u32), r.script as u32))
        .collect()
}

#[test]
fn interpreter_on_test_vector_modules() {
    let Some(d) = disc("interpreter_on_test_vector_modules") else { return };
    let map = option_map(d).clone();

    // Splash: on load, every object of the module sets script option 1015 to 2.
    let w = world(d, "Splash");
    let mut it = Interpreter::new(w.program.clone(), Some(map.clone()));
    let mut h = Mock::new(w);
    it.load(&mut h);
    let objs = attached(w);
    // No Splash record names the module (see docs/script.md, open questions); attach every placed record whose
    // class maps script option 1015.
    assert!(objs.is_empty());
    let objs: Vec<(ObjectId, u32)> = w
        .stream
        .records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.in_world && map.lookup(r.class, 1015).is_some())
        .map(|(i, _)| (ObjectId(i as u32), 0))
        .collect();
    assert!(!objs.is_empty());
    for &(o, m) in &objs {
        it.attach(o, m);
    }
    it.post_load(&mut h, objs.iter().map(|e| e.0));
    let want: Vec<String> = objs
        .iter()
        .map(|&(o, _)| {
            let r = &w.stream.records[o.0 as usize];
            format!("set {} {} 2", r.oid, map.lookup(r.class, 1015).expect("1015 mapped"))
        })
        .collect();
    assert_eq!(h.log, want);
    assert_eq!(it.error_count(), 0);

    // Paris, module 2: on load, the object removes itself (script option 10 = stream action 2).
    let w = world(d, "Paris, France");
    let mut it = Interpreter::new(w.program.clone(), Some(map.clone()));
    let mut h = Mock::new(w);
    it.load(&mut h);
    let objs = attached(w);
    for &(o, m) in &objs {
        it.attach(o, m);
    }
    let o2 = objs.iter().find(|e| e.1 == 2).unwrap().0;
    h.log.clear();
    it.raise_event(&mut h, o2, event::LOADED, &[]).unwrap();
    assert_eq!(h.log, [format!("action {} 2 0", h.oid(o2))]);

    // Paris, module 5.
    let o5 = objs.iter().find(|e| e.1 == 5).expect("an object on module 5").0;
    h.log.clear();
    it.raise_event(&mut h, o5, event::LOADED, &[]).unwrap();
    assert_eq!(h.log, ["find 1398"]);
    assert!(matches!(it.custom_property(o5, 1173), Some(Value::Bool(false))));
    let s75 = it.lookup(5, 75).unwrap();
    let Value::Object(Some(obj1398)) = s75 else { panic!("s75 = {s75}") };
    assert_eq!(h.oid(obj1398), 1398);
    // Object 1398's option 1006 reads false, so the triggered handler schedules s76.
    let class1398 = h.class_of(obj1398).unwrap();
    let opt1006 = map.lookup(class1398, 1006).expect("1006 mapped");
    h.options.insert((obj1398.0, opt1006), Value::Bool(false));
    h.log.clear();
    it.raise_event(&mut h, o5, event::TRIGGERED, &[]).unwrap();
    let r1397 = w.stream.records.iter().find(|r| r.oid == 1397 && r.in_world && r.class != PROXY).unwrap();
    assert_eq!(
        h.log,
        [
            "find 1397".to_string(),
            format!("action 1397 {} 0", map.lookup(r1397.class, 10).unwrap()),
            format!("get 1398 {opt1006}"),
            format!("action {} {} 0", h.oid(o5), map.lookup(w.stream.records[o5.0 as usize].class, 10).unwrap()),
        ]
    );
    assert_eq!(it.scheduled().len(), 1);
    // 2.5 seconds later s76 runs: the object's custom property 1173 is still false, so object 1398 performs
    // script option 267, an action (its stream option is not in the stream encoding table, so the host says so).
    let o267 = map.lookup(class1398, 267).expect("267 mapped");
    assert!(OptionTable::sly1().get(o267 as i16).is_none());
    h.unknown_actions = true;
    h.log.clear();
    for _ in 0..149 {
        it.tick(&mut h, 1.0 / 60.0);
    }
    assert!(h.log.is_empty() && it.scheduled().len() == 1);
    it.tick(&mut h, 2.0 / 60.0);
    assert_eq!(h.log, [format!("action 1398 {o267} 0")]);
    assert!(it.scheduled().is_empty());
    assert_eq!(it.error_count(), 0);
}

#[test]
fn every_world_loads_and_runs_its_load_events() {
    let Some(d) = disc("every_world_loads_and_runs_its_load_events") else { return };
    let map = option_map(d);
    let mut errors: BTreeMap<String, u32> = BTreeMap::new();
    for w in &d.worlds {
        let mut it = Interpreter::new(w.program.clone(), Some(map.clone()));
        let mut h = Mock::new(w);
        h.unknown_actions = true;
        let mut sink = ErrorSink { inner: &mut h, errors: &mut errors };
        it.load(&mut sink);
        // Every module-level definition is bound after loading.
        for m in 0..w.program.module_count() {
            for (s, _) in w.program.definitions(m) {
                assert!(it.lookup(m as u32, s).is_some(), "{}: module {m} s{s}", w.name);
            }
        }
        let objs = attached(w);
        for &(o, m) in &objs {
            it.attach(o, m);
        }
        it.post_load(&mut sink, objs.iter().map(|e| e.0));
        for (o, _) in &objs {
            it.raise_event(&mut sink, *o, event::UPDATE, &[Value::Float(1.0 / 60.0)]);
        }
        for _ in 0..600 {
            it.tick(&mut sink, 1.0 / 60.0);
        }
    }
    eprintln!("errors by kind over all worlds with a mock engine: {errors:?}");
    // Structural errors would mean the interpreter misreads the language; data errors from the mock are expected.
    for k in errors.keys() {
        for bad in
            ["UnknownOperation", "UnknownKeyword", "BadForm", "BadCell", "TooDeep", "OutOfSteps", "Stale", "Unbound"]
        {
            assert!(!k.starts_with(bad), "structural error {k}");
        }
    }
}

struct ErrorSink<'a, 'b> {
    inner: &'a mut Mock<'b>,
    errors: &'a mut BTreeMap<String, u32>,
}

impl ScriptHost for ErrorSink<'_, '_> {
    fn class_of(&self, o: ObjectId) -> Option<i16> {
        self.inner.class_of(o)
    }
    fn get_option(&mut self, o: ObjectId, option: u16) -> Value {
        self.inner.get_option(o, option)
    }
    fn set_option(&mut self, o: ObjectId, option: u16, v: &Value) -> Result<(), ScriptError> {
        self.inner.set_option(o, option, v)
    }
    fn call_action(&mut self, o: ObjectId, option: u16, args: &[Value]) -> Value {
        self.inner.call_action(o, option, args)
    }
    fn find_object(&mut self, oid: i16, c: Option<ObjectId>) -> Option<ObjectId> {
        self.inner.find_object(oid, c)
    }
    fn find_objects(&mut self, oid: i16, c: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        self.inner.find_objects(oid, c, out)
    }
    fn find_by_class(&mut self, class: i16, c: Option<ObjectId>, out: &mut Vec<ObjectId>) {
        self.inner.find_by_class(class, c, out)
    }
    fn player(&mut self) -> Option<ObjectId> {
        self.inner.player()
    }
    fn is_action(&self, o: u16) -> bool {
        self.inner.is_action(o)
    }
    fn report(&mut self, e: &ScriptError) {
        let mut k = format!("{e:?}");
        if let Some(i) = k.find([' ', '(']) {
            k.truncate(i);
        }
        *self.errors.entry(k).or_default() += 1;
    }
}

/// Where each of the live options (options-by-class.md 1.1) is read and written by the disc's scripts:
/// (stream option, how the read value is used).
fn live_option_uses(d: &DiscData) -> BTreeMap<u16, BTreeMap<String, u32>> {
    const LIVE: [u16; 12] = [127, 205, 844, 845, 846, 847, 848, 849, 850, 880, 981, 1084];
    let map = option_map(d);
    let mut out: BTreeMap<u16, BTreeMap<String, u32>> = BTreeMap::new();
    for w in &d.worlds {
        let p = &w.program;
        // Classes of the objects attached to each module.
        let mut classes: BTreeMap<u32, BTreeSet<i16>> = BTreeMap::new();
        for r in &w.stream.records {
            if r.script >= 0 {
                classes.entry(r.script as u32).or_default().insert(r.class);
            }
        }
        // Parent of every node: (list node, position).
        let mut parent: BTreeMap<NodeId, (NodeId, usize)> = BTreeMap::new();
        for n in 0..p.nodes.len() as NodeId {
            for (i, &c) in p.list(n).iter().enumerate() {
                parent.insert(c, (n, i));
            }
        }
        // Module of every top node, for the attached classes.
        let mut module_of: BTreeMap<NodeId, u32> = BTreeMap::new();
        for m in 0..p.module_count() {
            for &t in p.module(m) {
                module_of.insert(t, m as u32);
            }
        }
        let top = |mut n: NodeId| {
            while let Some(&(up, _)) = parent.get(&n) {
                n = up;
            }
            module_of.get(&n).copied()
        };
        for n in 0..p.nodes.len() as NodeId {
            let items = p.list(n);
            let (Some(&h), Some(&id)) = (items.first(), items.get(2)) else { continue };
            let (&Node::Builtin(op @ (101 | 102)), &Node::Int(id)) = (p.node(h), p.node(id)) else { continue };
            let cls: Vec<i16> =
                top(n).and_then(|m| classes.get(&m)).map(|c| c.iter().copied().collect()).unwrap_or_default();
            // Stream options the ID can name: in the attached objects' classes, else in any class.
            let mut targets: BTreeSet<u16> = cls.iter().filter_map(|&c| map.lookup(c, id as u32)).collect();
            if targets.is_empty() {
                targets = (0..148).filter_map(|c| map.lookup(c, id as u32)).collect();
            }
            for t in targets.into_iter().filter(|t| LIVE.contains(t)) {
                let usage = if op == 101 {
                    let v = items.get(3).map(|&v| format!("{}", p.display(v))).unwrap_or_default();
                    let kind = match items.get(3).map(|&v| p.node(v)) {
                        Some(Node::Int(_)) => "int",
                        Some(Node::Float(_)) => "float",
                        Some(Node::Bool(_)) => "bool",
                        Some(Node::Object(_)) => "null object",
                        Some(Node::List { .. }) => "expression",
                        Some(Node::Symbol(_)) => "symbol",
                        _ => "other",
                    };
                    format!("written: {kind} ({})", v.chars().take(40).collect::<String>())
                } else {
                    match parent.get(&n) {
                        Some(&(up, pos)) => {
                            let head = p.list(up).first().map(|&h| p.node(h).clone());
                            match head {
                                Some(Node::Builtin(b)) => format!("argument {} of op{b}", pos - 1),
                                Some(Node::Keyword(k)) => format!("{} position {pos}", script::tree::keyword_name(k)),
                                _ => format!("call position {pos}"),
                            }
                        }
                        None => "top".into(),
                    }
                };
                *out.entry(t).or_default().entry(usage).or_default() += 1;
            }
        }
    }
    out
}

/// The spec's value kind of each live option (options-by-class.md 1.1): what reading it gives and what writing it
/// takes.
fn live_kind(option: u16) -> &'static str {
    match option {
        127 => "float",
        205 | 844 | 848 | 880 | 1084 => "object",
        845 | 846 => "range",
        847 => "oid",
        849 | 850 => "bool",
        981 => "int",
        _ => "?",
    }
}

fn kind_of(v: &Value) -> &'static str {
    match v {
        Value::Float(_) => "float",
        Value::Int(_) => "int",
        Value::Bool(_) => "bool",
        Value::Object(_) | Value::Nil => "object",
        Value::Range(_) => "range",
        _ => "other",
    }
}

/// Answers reads of the live options with the spec's kind (unset: the kind's zero, the null object for references)
/// and checks every write against it.
struct LiveHost {
    reads: Vec<u16>,
    bad_writes: Vec<(u16, &'static str)>,
}

impl ScriptHost for LiveHost {
    fn class_of(&self, _: ObjectId) -> Option<i16> {
        Some(10)
    }
    fn get_option(&mut self, _: ObjectId, option: u16) -> Value {
        self.reads.push(option);
        match live_kind(option) {
            "float" => Value::Float(0.0),
            "object" => Value::Object(None),
            "bool" => Value::Bool(false),
            "range" => Value::Range([0.0, 0.0]),
            _ => Value::Int(2),
        }
    }
    fn set_option(&mut self, _: ObjectId, option: u16, v: &Value) -> Result<(), ScriptError> {
        let want = live_kind(option);
        // A float option takes integers too (127 is written as 1000000 on the disc).
        let ok = kind_of(v) == want || (want == "float" && kind_of(v) == "int");
        if !ok {
            self.bad_writes.push((option, kind_of(v)));
        }
        Ok(())
    }
}

#[test]
fn live_option_values_against_the_spec() {
    let Some(d) = disc("live_option_values_against_the_spec") else { return };
    // How the disc's scripts use the live options (reads by what consumes them, writes by the value's kind).
    let uses = live_option_uses(d);
    let summary: BTreeMap<u16, BTreeSet<String>> = uses
        .iter()
        .map(|(&o, u)| (o, u.keys().map(|k| k.split(" (").next().unwrap_or(k).to_string()).collect()))
        .collect();
    let want: BTreeMap<u16, BTreeSet<String>> = [
        (127, vec!["written: int"]),
        (205, vec!["written: symbol"]),
        (844, vec!["argument 2 of op101", "written: expression"]),
        (845, vec!["written: expression"]),
        (848, vec!["argument 2 of op101", "written: expression"]),
        (849, vec!["written: bool"]),
        (850, vec!["written: bool"]),
        (880, vec!["written: expression"]),
        (981, vec!["argument 0 of op7", "written: int"]),
        (1084, vec!["written: expression", "written: symbol"]),
    ]
    .into_iter()
    .map(|(o, v)| (o, v.into_iter().map(String::from).collect()))
    .collect();
    assert_eq!(summary, want);
    // Every literal written has the spec's kind (an integer for the float option 127).
    for (o, u) in &uses {
        for k in u.keys().filter(|k| k.starts_with("written: ") && !k.contains("expression") && !k.contains("symbol")) {
            let lit = &k["written: ".len()..k.find(" (").unwrap_or(k.len())];
            assert!(lit == live_kind(*o) || (live_kind(*o) == "float" && lit == "int"), "option {o}: {k}");
        }
    }
    // The expressions written are reads of the same option (844, 848), ranges made by op 49 (845) or objects found
    // by op 103 (880, 1084).
    for (o, u) in &uses {
        for k in u.keys().filter(|k| k.contains("expression")) {
            let ok = match o {
                844 => k.contains("op102 s124 885") || k.contains("op102 s137 885"),
                848 => k.contains("op102 s124 889") || k.contains("op102 s137 889"),
                845 => k.contains("(op49 "),
                880 | 1084 => k.contains("(op103 "),
                _ => false,
            };
            assert!(ok, "option {o}: {k}");
        }
    }
    // Run each pattern with values of the spec's kinds: no built-in rejects them, and every write has its kind.
    // (op7 (op102 s1 id981) 2), (op101 s1 id844 (op102 s2 id844)), (op101 s1 id880 (op103 1375 null)).
    let ids: BTreeMap<u16, i32> = {
        let map = option_map(d);
        let mut ids = BTreeMap::new();
        for o in [981u16, 844, 848, 880, 849, 127] {
            let class = if o == 981 {
                85
            } else if o == 880 {
                99
            } else if o == 127 {
                3
            } else {
                10
            };
            let id = map.class(class).iter().find(|e| e.1 == o).map(|e| i32::from(e.0));
            ids.insert(o, id.unwrap_or_else(|| panic!("no script ID for {o} in class {class}")));
        }
        ids
    };
    let mut classes = vec![Vec::new(); 148];
    classes[10] = ids.iter().map(|(&o, &id)| (id as u16, o)).collect();
    let map = OptionMap::from_lists(classes);
    let obj = Value::Object(Some(ObjectId(1)));
    let mut it = Interpreter::new(Program::new(), Some(map));
    let mut h = LiveHost { reads: Vec::new(), bad_writes: Vec::new() };
    let read = |it: &mut Interpreter, h: &mut LiveHost, o: u16| {
        it.call(h, Value::Builtin(102), vec![obj.clone(), Value::Int(ids[&o])])
    };
    let v = read(&mut it, &mut h, 981);
    let r = it.call(&mut h, Value::Builtin(7), vec![v, Value::Int(2)]);
    assert!(matches!(r, Value::Bool(true)), "{r}");
    for o in [844, 848, 880] {
        let v = read(&mut it, &mut h, o);
        let r = it.call(&mut h, Value::Builtin(101), vec![obj.clone(), Value::Int(ids[&o]), v]);
        assert!(matches!(r, Value::Void), "{o}: {r}");
    }
    let r = it.call(&mut h, Value::Builtin(101), vec![obj.clone(), Value::Int(ids[&849]), Value::Bool(true)]);
    assert!(matches!(r, Value::Void));
    let r = it.call(&mut h, Value::Builtin(101), vec![obj.clone(), Value::Int(ids[&127]), Value::Int(1_000_000)]);
    assert!(matches!(r, Value::Void));
    assert_eq!(h.reads, [981, 844, 848, 880]);
    assert!(h.bad_writes.is_empty(), "{:?}", h.bad_writes);
    assert_eq!(it.error_count(), 0);
    // An object-reference option read as the integer 0 (not the null object) reads as the null object (scripts.md
    // 11, item 15): op 102 on it does nothing, and op 27 still tells it from the null object.
    let r = it.call(&mut h, Value::Builtin(102), vec![Value::Int(0), Value::Int(ids[&844])]);
    assert!(matches!(r, Value::Void), "{r}");
    assert_eq!(it.error_count(), 0);
    let r = it.call(&mut h, Value::Builtin(27), vec![Value::Int(0)]);
    assert!(matches!(r, Value::Bool(false)), "0 is not the null object: {r}");
}

/// A module given by a proxy override (scripts.md 11.2 and 11.3): the target record, the module, and the module
/// the target already had (its outer module), if any.
struct OverrideAttachment {
    target: u32,
    module: u32,
    outer: Option<u32>,
}

/// Override entries that name a module, resolved as the engine does: the target is the first placed source with the
/// OID, else the first object with it below a placed source (in record order). A clone starts from its source
/// record's own module; a source keeps the modules earlier overrides gave it. Returns the attachments and the count
/// of override entries whose target was not found.
fn override_attachments(w: &WorldData) -> (Vec<OverrideAttachment>, usize) {
    use formats::stream::ProxyEntry;
    let table = OptionTable::sly1();
    let records = &w.stream.records;
    let mut module: Vec<i16> = records.iter().map(|r| r.script).collect();
    let mut out = Vec::new();
    let mut unresolved = 0;
    for p in &w.stream.proxies {
        let placed: Vec<(u32, bool)> = p
            .entries
            .iter()
            .map(|e| match *e {
                ProxyEntry::Source { record, .. } => (record, false),
                ProxyEntry::BackRef { source_index } => (w.stream.sources[source_index as usize], true),
            })
            .collect();
        for o in w.stream.proxy_overrides(&w.bytes, p, &table).expect("override entries") {
            if o.script < 0 {
                continue;
            }
            let direct = placed.iter().find(|&&(r, _)| records[r as usize].oid == o.target).copied();
            let below = || {
                placed.iter().find_map(|&(r, clone)| {
                    (r + 1..records[r as usize].subtree_end)
                        .find(|&c| records[c as usize].oid == o.target)
                        .map(|c| (c, clone))
                })
            };
            let Some((t, clone)) = direct.or_else(below) else {
                unresolved += 1;
                continue;
            };
            let old = if clone { records[t as usize].script } else { module[t as usize] };
            out.push(OverrideAttachment {
                target: t,
                module: o.script as u32,
                outer: (old >= 0 && old != o.script).then_some(old as u32),
            });
            if !clone {
                module[t as usize] = o.script;
            }
        }
    }
    (out, unresolved)
}

/// Runs every world with header and override attachments (chaining outer modules or not) under the mock engine;
/// returns the error values by kind, and (chains, chains made, chains refused).
fn run_with_overrides(d: &DiscData, chain: bool) -> (BTreeMap<String, u32>, [u32; 3]) {
    let map = option_map(d);
    let mut errors: BTreeMap<String, u32> = BTreeMap::new();
    let mut counts = [0u32; 3];
    for w in &d.worlds {
        let (overrides, _) = override_attachments(w);
        let mut it = Interpreter::new(w.program.clone(), Some(map.clone()));
        let mut h = Mock::new(w);
        h.unknown_actions = true;
        let mut sink = ErrorSink { inner: &mut h, errors: &mut errors };
        it.load(&mut sink);
        let mut objects: BTreeSet<ObjectId> = BTreeSet::new();
        for (o, m) in attached(w) {
            it.attach(o, m);
            objects.insert(o);
        }
        for a in &overrides {
            let o = ObjectId(a.target);
            let outer = if chain { a.outer } else { None };
            counts[0] += u32::from(a.outer.is_some());
            if it.attach_with_outer(o, a.module, outer)
                && let Some(outer) = outer
            {
                counts[1] += 1;
                // Both modules' definitions are visible from the object's module.
                for (s, _) in w.program.definitions(outer as usize) {
                    assert!(it.lookup(a.module, s).is_some(), "{}: module {} misses s{s}", w.name, a.module);
                }
                assert_eq!(it.outer_module(a.module), Some(outer));
            }
            objects.insert(o);
        }
        counts[2] += it.outer_conflicts();
        it.post_load(&mut sink, objects.iter().copied());
        for o in &objects {
            it.raise_event(&mut sink, *o, event::UPDATE, &[Value::Float(1.0 / 60.0)]);
        }
        for _ in 0..600 {
            it.tick(&mut sink, 1.0 / 60.0);
        }
    }
    (errors, counts)
}

#[test]
fn override_attachments_and_outer_modules() {
    let Some(d) = disc("override_attachments_and_outer_modules") else { return };
    let mut named = 0;
    let mut unattached = 0;
    for w in &d.worlds {
        let (overrides, unresolved) = override_attachments(w);
        assert_eq!(unresolved, 0, "{}: override targets not found", w.name);
        named += overrides.len();
        // Modules no header and no override names (scripts.md 11.2).
        let mut used: BTreeSet<u32> = attached(w).iter().map(|e| e.1).collect();
        used.extend(overrides.iter().map(|o| o.module));
        unattached += w.program.module_count() - used.len();
    }
    // scripts.md 11.2: 415 override entries name a module; 101 modules stay unattached.
    assert_eq!((named, unattached), (415, 101));
    // 40 overrides give a module to an object that already has one (scripts.md 11.3); every chain is made.
    let (errors, counts) = run_with_overrides(d, true);
    assert_eq!(counts, [40, 40, 0]);
    // With the chains, every name resolves; without them, 7 references to the outer modules' names are unbound.
    let (unchained, _) = run_with_overrides(d, false);
    assert_eq!(unchained.get("Unbound"), Some(&7), "{unchained:?}");
    // Two handler calls of override-attached objects reach the depth limit under the mock, with or without the
    // chains (they recurse until the game's state changes); nothing else is structural.
    assert_eq!(errors.get("TooDeep"), unchained.get("TooDeep"));
    for k in errors.keys() {
        for bad in ["UnknownOperation", "UnknownKeyword", "BadForm", "BadCell", "OutOfSteps", "Stale", "Unbound"] {
            assert!(!k.starts_with(bad), "structural error {k}");
        }
    }
}
