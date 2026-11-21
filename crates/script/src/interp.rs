//! The tree interpreter: modules, environments, special forms, calls, events and scheduled calls
//! (scripts.md sections 1 to 6).

use alloc::collections::BTreeMap;
use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::builtins;
use crate::env::{EnvRef, Heap};
use crate::host::ScriptHost;
use crate::option_map::{CUSTOM_BASE, OptionMap, SLY2_CUSTOM_BASE, SLY3_CUSTOM_BASE};
use crate::sly2::BuiltinTable;
use crate::tree::{Game, Node, NodeId, Program, kind};
use crate::value::{ObjectId, Procedure, ScriptError, Site, Value, same_value};

/// Limits that keep a broken script from hanging or overflowing the stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Deepest nesting of list evaluations and calls.
    pub max_depth: u32,
    /// Evaluation steps allowed per entry into the interpreter (an event, a scheduled call, a module).
    pub max_steps: u32,
    /// Environments made between automatic collections (also scaled by the number alive).
    pub collect_after: usize,
    /// Longest list a built-in may make (operations 40 and 41). Built-ins that walk or build lists also use one
    /// step of the budget per element.
    pub max_list_len: usize,
    /// Most scheduled calls waiting at once (operation 97 fails beyond it).
    pub max_scheduled: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits { max_depth: 256, max_steps: 1_000_000, collect_after: 4096, max_list_len: 1 << 16, max_scheduled: 1024 }
    }
}

/// Custom-property entries each object has room for (scripts.md 11, item 9).
pub const CUSTOM_SLOTS: usize = 16;

/// Sly 1 methods (scripts.md 11, item 17): every stream option that reads as a method bound to its object, with the
/// number of arguments it takes (`None`: as many as the call passes). Sorted by option. Every other option is a
/// value, including 521 (which script option 306 names in class 147).
pub const METHODS: [(u16, Option<u8>); 170] = [
    (1, None),
    (2, None),
    (4, None),
    (5, None),
    (9, None),
    (10, None),
    (13, Some(1)),
    (27, None),
    (86, Some(0)),
    (87, Some(0)),
    (88, Some(3)),
    (90, Some(2)),
    (94, Some(1)),
    (107, Some(5)),
    (108, Some(0)),
    (117, Some(1)),
    (118, Some(1)),
    (120, Some(1)),
    (121, Some(1)),
    (122, Some(0)),
    (124, Some(1)),
    (126, Some(2)),
    (146, Some(1)),
    (147, Some(1)),
    (161, Some(4)),
    (162, Some(0)),
    (174, Some(0)),
    (185, Some(4)),
    (186, Some(5)),
    (187, Some(1)),
    (197, Some(3)),
    (198, Some(0)),
    (199, Some(1)),
    (200, Some(2)),
    (207, Some(0)),
    (228, Some(0)),
    (264, Some(4)),
    (276, Some(4)),
    (282, Some(0)),
    (283, Some(1)),
    (296, Some(1)),
    (297, Some(4)),
    (311, Some(1)),
    (312, Some(0)),
    (315, Some(1)),
    (316, Some(1)),
    (362, Some(1)),
    (363, Some(0)),
    (364, Some(0)),
    (481, Some(0)),
    (482, Some(6)),
    (489, Some(0)),
    (490, Some(6)),
    (514, Some(3)),
    (522, Some(0)),
    (546, Some(3)),
    (553, Some(2)),
    (554, Some(0)),
    (555, Some(1)),
    (556, Some(1)),
    (557, Some(1)),
    (558, Some(1)),
    (559, Some(1)),
    (560, Some(1)),
    (561, Some(1)),
    (562, Some(3)),
    (563, Some(2)),
    (564, Some(1)),
    (565, Some(1)),
    (566, Some(0)),
    (568, Some(0)),
    (569, Some(2)),
    (570, Some(0)),
    (571, Some(0)),
    (573, Some(1)),
    (574, Some(1)),
    (575, Some(2)),
    (576, Some(1)),
    (577, Some(2)),
    (578, Some(0)),
    (579, Some(0)),
    (580, Some(1)),
    (581, Some(1)),
    (595, Some(1)),
    (596, Some(0)),
    (597, Some(0)),
    (598, Some(3)),
    (599, Some(0)),
    (600, Some(0)),
    (601, Some(3)),
    (606, Some(0)),
    (607, Some(0)),
    (613, Some(0)),
    (622, Some(0)),
    (657, Some(2)),
    (658, Some(1)),
    (663, Some(2)),
    (667, Some(0)),
    (677, Some(1)),
    (721, Some(1)),
    (722, Some(0)),
    (732, Some(3)),
    (733, Some(0)),
    (734, Some(1)),
    (737, Some(1)),
    (738, None),
    (739, Some(1)),
    (740, Some(0)),
    (743, Some(1)),
    (744, Some(1)),
    (745, Some(1)),
    (821, Some(0)),
    (822, Some(1)),
    (888, Some(0)),
    (889, Some(0)),
    (911, Some(1)),
    (912, Some(1)),
    (913, Some(2)),
    (914, Some(0)),
    (915, Some(1)),
    (917, Some(0)),
    (928, Some(0)),
    (939, Some(0)),
    (974, Some(0)),
    (975, Some(0)),
    (1001, Some(0)),
    (1026, Some(0)),
    (1087, Some(1)),
    (1088, Some(5)),
    (1091, Some(0)),
    (1103, Some(1)),
    (1104, Some(1)),
    (1105, Some(1)),
    (1113, Some(2)),
    (1115, Some(0)),
    (1116, Some(1)),
    (1117, Some(0)),
    (1118, Some(1)),
    (1119, Some(1)),
    (1120, Some(1)),
    (1121, Some(1)),
    (1122, Some(1)),
    (1123, Some(2)),
    (1124, Some(4)),
    (1125, Some(3)),
    (1126, Some(3)),
    (1127, Some(3)),
    (1128, Some(2)),
    (1129, Some(1)),
    (1131, Some(0)),
    (1132, Some(0)),
    (1143, Some(0)),
    (1144, Some(1)),
    (1147, Some(0)),
    (1152, Some(1)),
    (1153, Some(0)),
    (1156, Some(2)),
    (1157, Some(1)),
    (1158, Some(0)),
    (1161, Some(3)),
    (1162, Some(3)),
    (1163, Some(2)),
    (1164, Some(2)),
    (1165, Some(2)),
    (1166, Some(2)),
    (1168, None),
    (1171, None),
    (1174, None),
    (1175, None),
    (1178, Some(2)),
];

/// The argument count of Sly 1 method `option`: `Some(None)` when it takes what the call passes, `None` when the
/// option is not a method.
pub fn method_arity(option: u16) -> Option<Option<u8>> {
    METHODS.binary_search_by_key(&option, |m| m.0).ok().map(|i| METHODS[i].1)
}

/// The 117 stream options scripts.md 11, item 4 listed as actions that no stream contains. Superseded by
/// [`METHODS`] (item 17), which no longer lists 439, 647, 751, 827, 840, 1004, 1016 and 1020.
pub const UNLISTED_ACTIONS: [u16; 117] = [
    4, 9, 27, 86, 87, 90, 94, 107, 108, 117, 118, 124, 126, 162, 174, 197, 198, 199, 200, 207, 228, 282, 296, 311, 312,
    315, 316, 362, 363, 364, 439, 481, 482, 489, 490, 522, 546, 553, 554, 562, 568, 569, 570, 571, 574, 575, 577, 578,
    579, 581, 595, 598, 599, 600, 606, 607, 613, 622, 647, 657, 663, 667, 677, 721, 722, 732, 733, 734, 737, 738, 739,
    740, 743, 744, 745, 751, 821, 822, 827, 840, 888, 889, 911, 912, 913, 914, 917, 928, 939, 974, 975, 1001, 1004,
    1016, 1020, 1026, 1087, 1088, 1091, 1113, 1115, 1116, 1117, 1132, 1143, 1144, 1147, 1152, 1153, 1156, 1157, 1158,
    1168, 1171, 1174, 1175, 1178,
];

/// A pending scheduled call (scripts.md section 5).
#[derive(Clone, Debug)]
pub struct ScheduledCall {
    /// Seconds left.
    pub remaining: f32,
    /// What to call.
    pub callee: Value,
    /// Its arguments.
    pub args: Vec<Value>,
    /// Handle given to the script (Sly 2 operation 105 returns it, 106 cancels by it); 0 until scheduled.
    pub id: u32,
}

impl ScheduledCall {
    /// A call of `callee` with `args` in `remaining` seconds.
    pub fn new(remaining: f32, callee: Value, args: Vec<Value>) -> ScheduledCall {
        ScheduledCall { remaining, callee, args, id: 0 }
    }
}

/// Value type `0x12` in the Sly 2 option table: a procedure of the game, never stored in an option list.
pub const SLY2_VALUE_TYPE_PROCEDURE: u32 = 0x12;

/// Whether a Sly 2 stream option reads as a method: a typed argument list (flag `0x400`), value type `0x12`, or
/// flag `0x1` with arguments (the last two inferred from the disc: the scripts call them; `docs/script.md`).
pub fn is_sly2_method(e: &formats_sly2::exe::OptionEntry) -> bool {
    e.is_typed() || e.value_type == SLY2_VALUE_TYPE_PROCEDURE || (e.flags & 0x1 != 0 && e.arg_count > 0)
}

/// What the interpreter needs only for Sly 2.
#[derive(Clone, Debug, Default)]
struct Sly2Tables {
    builtins: Option<BuiltinTable>,
    /// Per stream option: whether it reads as a method ([`is_sly2_method`]).
    actions: Vec<bool>,
}

/// One world's scripts, ready to run.
pub struct Interpreter {
    program: Program,
    pub(crate) heap: Heap,
    world: EnvRef,
    modules: Vec<EnvRef>,
    attached: BTreeMap<ObjectId, u32>,
    /// Custom-property entries per object, oldest first (at most [`CUSTOM_SLOTS`]).
    custom: BTreeMap<ObjectId, Vec<(u32, Value)>>,
    pub(crate) scheduled: Vec<ScheduledCall>,
    quotes: BTreeMap<NodeId, Value>,
    options: Option<OptionMap>,
    sly2: Sly2Tables,
    next_call_id: u32,
    outer_conflicts: u32,
    depth: u32,
    steps: u32,
    errors: u64,
    /// The innermost event handler running: (object, event, module).
    handler: Option<(ObjectId, u32, u32)>,
    /// The procedure running (for operation 133's lookup).
    running: Option<Rc<Procedure>>,
    /// The limits in force.
    pub limits: Limits,
}

impl Interpreter {
    /// An interpreter for `program`. `options` is the script option mapping (needed by option operations on
    /// built-in options; custom properties work without it).
    pub fn new(program: Program, options: Option<OptionMap>) -> Interpreter {
        let mut heap = Heap::default();
        let world = heap.alloc(None);
        Interpreter {
            program,
            heap,
            world,
            modules: Vec::new(),
            attached: BTreeMap::new(),
            custom: BTreeMap::new(),
            scheduled: Vec::new(),
            quotes: BTreeMap::new(),
            options,
            sly2: Sly2Tables::default(),
            next_call_id: 0,
            outer_conflicts: 0,
            depth: 0,
            steps: 0,
            errors: 0,
            handler: None,
            running: None,
            limits: Limits::default(),
        }
    }

    /// The program.
    pub fn program(&self) -> &Program {
        &self.program
    }

    /// The game whose scripts these are (from the program).
    pub fn game(&self) -> Game {
        self.program.game
    }

    /// Sly 2: the executable's built-in table (argument rules) and option table (which stream options are
    /// actions). Without them every built-in is checked against the spec's rule only and no option is an action.
    pub fn set_sly2_tables(&mut self, builtins: Option<BuiltinTable>, options: &formats_sly2::exe::OptionTable) {
        self.sly2.builtins = builtins;
        self.sly2.actions = options.entries.iter().map(is_sly2_method).collect();
    }

    /// Sly 3: the executable's built-in table and option table, as [`Interpreter::set_sly2_tables`] (both games
    /// share the formats; specs/sly3/scripts.md 4.1, 5).
    pub fn set_sly3_tables(&mut self, builtins: Option<BuiltinTable>, options: &formats_sly2::exe::OptionTable) {
        self.set_sly2_tables(builtins, options);
    }

    pub(crate) fn sly2_builtins(&self) -> Option<&BuiltinTable> {
        self.sly2.builtins.as_ref()
    }

    /// The first custom property ID of the game (Sly 1: 1173; Sly 2: 1233; Sly 3: 1816).
    pub fn custom_base(&self) -> u32 {
        match self.game() {
            Game::Sly1 => CUSTOM_BASE,
            Game::Sly2 => SLY2_CUSTOM_BASE,
            Game::Sly3 => SLY3_CUSTOM_BASE,
        }
    }

    /// What reading a custom property that was never stored gives: Sly 1 the empty list, Sly 2 and 3 "no value"
    /// (specs/sly2/scripts.md 5).
    fn missing_custom(&self) -> Value {
        if self.game() == Game::Sly1 { Value::Nil } else { Value::Void }
    }

    /// Queues a scheduled call and gives it a handle (never 0); fails when the queue is full.
    pub(crate) fn schedule(&mut self, mut call: ScheduledCall) -> Result<u32, ScriptError> {
        if self.scheduled.len() >= self.limits.max_scheduled {
            return Err(ScriptError::TooLarge);
        }
        self.next_call_id = self.next_call_id.wrapping_add(1).max(1);
        call.id = self.next_call_id;
        self.scheduled.push(call);
        Ok(self.next_call_id)
    }

    /// Removes the pending scheduled call with handle `id` (Sly 2 operation 106); nothing when there is none.
    pub fn cancel(&mut self, id: u32) {
        self.scheduled.retain(|c| c.id != id);
    }

    /// A binding of the world environment.
    pub(crate) fn global_lookup(&self, symbol: u32) -> Option<Value> {
        self.heap.lookup(self.world, symbol)
    }

    /// The script option mapping, if one was given.
    pub fn option_map(&self) -> Option<&OptionMap> {
        self.options.as_ref()
    }

    /// Evaluates every module once, in table order, each in a fresh environment whose parent is the world
    /// environment (scripts.md section 1). Calling it again starts the modules over.
    pub fn load(&mut self, host: &mut dyn ScriptHost) {
        self.modules.clear();
        for m in 0..self.program.module_count() {
            let env = self.heap.alloc(Some(self.world));
            self.modules.push(env);
            self.enter();
            for i in 0..self.program.module(m).len() {
                let top = self.program.module(m)[i];
                self.eval(host, top, env);
            }
            self.leave();
        }
        self.maybe_collect();
    }

    /// Binds a symbol in the world environment, visible to every module.
    pub fn define_global(&mut self, symbol: u32, value: Value) {
        self.heap.define(self.world, symbol, value);
    }

    /// Attaches an object to a module (the object record's script index, object-stream.md 5.1).
    pub fn attach(&mut self, object: ObjectId, module: u32) {
        self.attached.insert(object, module);
    }

    /// Attaches an object to `module` with `outer` as that module's outer environment (scripts.md 11.3: a proxy
    /// override gives a module to an object that already had one; both modules' definitions stay visible, the new
    /// module's first). Name lookups in `module` then continue in `outer`'s environment before the world's: event
    /// handlers, module bindings and the procedures the module defines.
    ///
    /// The chain belongs to the module's environment, which every object attached to the module shares. A module
    /// keeps the first outer module it is given: a different one later is refused (the result is `false`, and
    /// [`Interpreter::outer_conflicts`] counts it), and so is a chain that would loop. Call it after
    /// [`Interpreter::load`], which starts every module over without outer modules.
    pub fn attach_with_outer(&mut self, object: ObjectId, module: u32, outer: Option<u32>) -> bool {
        self.attach(object, module);
        let Some(outer) = outer.filter(|&o| o != module) else { return true };
        let (Some(&env), Some(&outer_env)) = (self.modules.get(module as usize), self.modules.get(outer as usize))
        else {
            return false;
        };
        match self.heap.parent(env) {
            Some(p) if p == outer_env => true,
            Some(p) if p == self.world && self.heap.set_parent(env, outer_env) => true,
            _ => {
                self.outer_conflicts += 1;
                false
            }
        }
    }

    /// The module whose environment `module`'s continues in (from [`Interpreter::attach_with_outer`]).
    pub fn outer_module(&self, module: u32) -> Option<u32> {
        let parent = self.heap.parent(*self.modules.get(module as usize)?)?;
        self.modules.iter().position(|&m| m == parent).map(|m| m as u32)
    }

    /// Outer modules refused by [`Interpreter::attach_with_outer`] (a second, different outer, or a loop).
    pub fn outer_conflicts(&self) -> u32 {
        self.outer_conflicts
    }

    /// Detaches an object (and forgets its custom properties).
    pub fn detach(&mut self, object: ObjectId) {
        self.attached.remove(&object);
        self.custom.remove(&object);
    }

    /// The module an object is attached to.
    pub fn module_of(&self, object: ObjectId) -> Option<u32> {
        self.attached.get(&object).copied()
    }

    /// A module-level binding (`None` when unbound or the module does not exist or is not loaded).
    pub fn lookup(&self, module: u32, symbol: u32) -> Option<Value> {
        self.heap.lookup(*self.modules.get(module as usize)?, symbol)
    }

    /// The symbols a module's own environment binds, in definition order.
    pub fn module_symbols(&self, module: u32) -> Vec<u32> {
        self.modules.get(module as usize).map(|&e| self.heap.symbols(e)).unwrap_or_default()
    }

    /// Raises event `event` for an object (scripts.md section 6): if the object's module binds the event's symbol
    /// to something callable, calls it with the object followed by `args`. Returns the handler's result, or `None`
    /// when there is no handler.
    pub fn raise_event(
        &mut self,
        host: &mut dyn ScriptHost,
        object: ObjectId,
        event: u32,
        args: &[Value],
    ) -> Option<Value> {
        // Sly 2 hook: an object made at run time gets its module from the host on its first event.
        let module = match self.module_of(object) {
            Some(m) => m,
            None => {
                let m = host.module_of(object)?;
                self.attach(object, m);
                m
            }
        };
        let handler = self.lookup(module, event)?;
        if !handler.is_callable() {
            return None;
        }
        let mut full = Vec::with_capacity(args.len() + 1);
        full.push(Value::Object(Some(object)));
        full.extend_from_slice(args);
        let outer = self.handler.replace((object, event, module));
        self.enter();
        let r = self.apply(host, handler, full);
        self.leave();
        self.handler = outer;
        self.maybe_collect();
        Some(r)
    }

    /// Raises event 0 for every object attached to a module, in the order given (stream order).
    pub fn post_load(&mut self, host: &mut dyn ScriptHost, objects: impl IntoIterator<Item = ObjectId>) {
        for o in objects {
            self.raise_event(host, o, crate::event::LOADED, &[]);
        }
    }

    /// Calls a value (a built-in, procedure or method) from outside.
    pub fn call(&mut self, host: &mut dyn ScriptHost, callee: Value, args: Vec<Value>) -> Value {
        self.enter();
        let r = self.apply(host, callee, args);
        self.leave();
        r
    }

    /// Evaluates one node in a module's environment (or the world environment for `None`).
    pub fn eval_in(&mut self, host: &mut dyn ScriptHost, node: NodeId, module: Option<u32>) -> Value {
        let env = module.and_then(|m| self.modules.get(m as usize).copied()).unwrap_or(self.world);
        self.enter();
        let r = self.eval(host, node, env);
        self.leave();
        r
    }

    /// Advances the scheduled calls by `dt` seconds and runs those whose time ran out, in the order they were
    /// scheduled. Calls scheduled while these run wait for a later frame. All the calls of one tick share one step
    /// budget ([`Limits::max_steps`]); once it runs out, the remaining due calls fail with
    /// [`ScriptError::OutOfSteps`]. Collects unreachable environments when enough were made.
    pub fn tick(&mut self, host: &mut dyn ScriptHost, dt: f32) {
        let pending = core::mem::take(&mut self.scheduled);
        let mut due = Vec::new();
        for mut c in pending {
            c.remaining -= dt;
            if c.remaining <= 0.0 {
                due.push(c);
            } else {
                self.scheduled.push(c);
            }
        }
        // Calls scheduled by these are appended after the ones still waiting.
        self.enter();
        for c in due {
            self.apply(host, c.callee, c.args);
        }
        self.leave();
        self.maybe_collect();
    }

    /// The pending scheduled calls.
    pub fn scheduled(&self) -> &[ScheduledCall] {
        &self.scheduled
    }

    /// An object's custom property: its newest entry (`None` when never set).
    pub fn custom_property(&self, object: ObjectId, id: u32) -> Option<&Value> {
        self.custom.get(&object)?.iter().rev().find(|e| e.0 == id).map(|e| &e.1)
    }

    /// Sets an object's custom property from outside, as operation 100 does (the newest entry is overwritten, or
    /// an entry appended).
    pub fn set_custom_property(&mut self, object: ObjectId, id: u32, value: Value) {
        self.store_custom(object, id, value, false);
    }

    /// Custom-property entries an object holds (at most [`CUSTOM_SLOTS`]).
    pub fn custom_entries(&self, object: ObjectId) -> usize {
        self.custom.get(&object).map_or(0, Vec::len)
    }

    /// Appends an entry (`append`, operation 99) or overwrites the newest entry for `id` (operation 100); appends
    /// beyond [`CUSTOM_SLOTS`] are ignored.
    fn store_custom(&mut self, object: ObjectId, id: u32, value: Value, append: bool) {
        let entries = self.custom.entry(object).or_default();
        if !append && let Some(e) = entries.iter_mut().rev().find(|e| e.0 == id) {
            e.1 = value;
            return;
        }
        if entries.len() < CUSTOM_SLOTS {
            entries.push((id, value));
        }
    }

    /// Number of error values created so far.
    pub fn error_count(&self) -> u64 {
        self.errors
    }

    /// Number of live environments.
    pub fn live_environments(&self) -> usize {
        self.heap.live()
    }

    /// Frees environments nothing can reach any more. Values the caller kept from earlier calls may then hold
    /// procedures whose environment is gone; calling one gives [`ScriptError::StaleEnvironment`].
    pub fn collect_garbage(&mut self) {
        if self.depth != 0 {
            return;
        }
        let mut envs: Vec<EnvRef> = Vec::with_capacity(self.modules.len() + 1);
        envs.push(self.world);
        envs.extend_from_slice(&self.modules);
        let vals = self
            .custom
            .values()
            .flat_map(|v| v.iter().map(|e| &e.1))
            .chain(self.quotes.values())
            .chain(self.scheduled.iter().flat_map(|c| core::iter::once(&c.callee).chain(c.args.iter())));
        self.heap.collect(envs, vals);
    }

    fn maybe_collect(&mut self) {
        if self.depth == 0 && self.heap.since_collect >= self.limits.collect_after.max(self.heap.live() / 2) {
            self.collect_garbage();
        }
    }

    fn enter(&mut self) {
        if self.depth == 0 {
            self.steps = self.limits.max_steps;
        }
        self.depth += 1;
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    /// Uses `n` steps of the budget for work a built-in does outside `eval` (walking or building lists).
    pub(crate) fn charge(&mut self, n: usize) -> Result<(), ScriptError> {
        let n = u32::try_from(n).unwrap_or(u32::MAX);
        if n >= self.steps {
            self.steps = 0;
            return Err(ScriptError::OutOfSteps);
        }
        self.steps -= n;
        Ok(())
    }

    /// Whether `op` is the operation operation 133 (Sly 3 141) gives for an unbound symbol.
    fn is_unbound_marker(&self, op: u32) -> bool {
        match self.game() {
            Game::Sly1 => false,
            Game::Sly2 => op == 134,
            Game::Sly3 => op == 142,
        }
    }

    /// A [`ScriptError::TooDeep`] naming the innermost handler running.
    pub(crate) fn too_deep(&self) -> ScriptError {
        ScriptError::TooDeep(match self.handler {
            Some((o, e, m)) => Site { object: Some(o), event: Some(e), module: Some(m) },
            None => Site::default(),
        })
    }

    /// Makes an error value and reports it.
    pub(crate) fn fail(&mut self, host: &mut dyn ScriptHost, e: ScriptError) -> Value {
        self.errors += 1;
        host.report(&e);
        Value::Error(e)
    }

    /// Raises the events the host queued during its last call.
    pub(crate) fn drain_host_events(&mut self, host: &mut dyn ScriptHost) {
        while let Some(ev) = host.poll_event() {
            if self.depth >= self.limits.max_depth {
                let site = Site { object: Some(ev.object), event: Some(ev.event), module: self.module_of(ev.object) };
                let _ = self.fail(host, ScriptError::TooDeep(site));
                continue;
            }
            self.raise_event(host, ev.object, ev.event, &ev.args);
        }
    }

    fn child(&self, list: NodeId, i: usize) -> Option<NodeId> {
        self.program.list(list).get(i).copied()
    }

    fn len(&self, list: NodeId) -> usize {
        self.program.list(list).len()
    }

    /// Evaluates a node.
    pub(crate) fn eval(&mut self, host: &mut dyn ScriptHost, id: NodeId, env: EnvRef) -> Value {
        if self.steps == 0 {
            return Value::Error(ScriptError::OutOfSteps);
        }
        self.steps -= 1;
        if self.steps == 0 {
            return self.fail(host, ScriptError::OutOfSteps);
        }
        match self.program.node(id) {
            Node::Nil => Value::Nil,
            Node::Int(i) => Value::Int(*i),
            Node::Float(f) => Value::Float(*f),
            Node::Vector(v) => Value::Vector(*v),
            Node::Matrix(m) => Value::Matrix(m.clone()),
            Node::Coeffs(v) => Value::Coeffs(*v),
            Node::Range(r) => Value::Range(*r),
            Node::Smoothing(v) => Value::Smoothing(*v),
            Node::Bool(b) => Value::Bool(*b),
            Node::Builtin(o) => Value::Builtin(*o),
            Node::Object(0) => Value::Object(None),
            &Node::Object(_) => self.fail(host, ScriptError::BadCell(kind::OBJECT)),
            &Node::Symbol(s) => match self.heap.lookup(env, s) {
                Some(v) => v,
                None if !self.heap.is_live(env) => self.fail(host, ScriptError::StaleEnvironment),
                None => self.fail(host, ScriptError::Unbound(s)),
            },
            &Node::Keyword(k) => self.fail(host, ScriptError::BadForm(k)),
            &Node::Other(k) => self.fail(host, ScriptError::BadCell(k)),
            &Node::List { len, .. } => {
                if len == 0 {
                    return Value::Nil;
                }
                if self.depth >= self.limits.max_depth {
                    return self.fail(host, self.too_deep());
                }
                self.depth += 1;
                let r = match self.program.head_keyword(id) {
                    Some(k) => self.special(host, k, id, env),
                    None => self.eval_call(host, id, env),
                };
                self.depth -= 1;
                r
            }
        }
    }

    fn eval_call(&mut self, host: &mut dyn ScriptHost, id: NodeId, env: EnvRef) -> Value {
        let n = self.len(id);
        let head = self.child(id, 0).expect("non-empty");
        let f = self.eval(host, head, env);
        // Sly 2 operation 133 (Sly 3 141): a symbol's value in the calling environment (the module's, through the
        // chain); unbound, Sly 2's operation 134 (Sly 3 142) as a value (specs/sly2/scripts.md 4.3).
        let symbol_op = match self.game() {
            Game::Sly1 => None,
            Game::Sly2 => Some(133),
            Game::Sly3 => Some(141),
        };
        if symbol_op.is_some() && matches!(f, Value::Builtin(op) if Some(op) == symbol_op) && n == 2 {
            let arg = self.eval(host, self.child(id, 1).expect("in range"), env);
            let sym = match arg {
                Value::Symbol(s) => Some(s),
                Value::Int(i) => u32::try_from(i).ok(),
                _ => None,
            };
            if let Some(s) = sym {
                // The running procedure's own binding is passed over: on the disc every call names the symbol of
                // the handler it is in, `((op133 (quote sN)) ...)` inside handler sN (all 209 Sly 2 calls), to
                // run the definition it shadows (an outer module's, else nothing: operation 134). Finding the
                // handler itself would recurse without end [I].
                let me = self.running.clone();
                let own = |v: &Value| matches!((v, &me), (Value::Procedure(p), Some(r)) if Rc::ptr_eq(p, r));
                return self.heap.lookup_skipping(env, s, own).unwrap_or(Value::Builtin(symbol_op.unwrap_or(0) + 1));
            }
            return self.apply(host, f, alloc::vec![arg]);
        }
        // Arguments a procedure without a rest parameter has no formal for are not evaluated (scripts.md 11, 5),
        // nor are those of a call of something not callable (item 18).
        let used = match &f {
            Value::Procedure(p) if p.rest.is_none() => (n - 1).min(p.params.len()),
            f if !f.is_callable() => 0,
            _ => n - 1,
        };
        let mut args = Vec::with_capacity(used);
        for i in 1..=used {
            args.push(self.eval(host, self.child(id, i).expect("in range"), env));
        }
        self.apply(host, f, args)
    }

    /// Calls `f` with evaluated arguments.
    pub(crate) fn apply(&mut self, host: &mut dyn ScriptHost, f: Value, args: Vec<Value>) -> Value {
        if self.depth >= self.limits.max_depth {
            return self.fail(host, self.too_deep());
        }
        self.depth += 1;
        let r = match f {
            // Operation 134 (Sly 3 142), the value operation 133 gives for an unbound symbol, never appears on the
            // disc; a handler calling a shadowed definition that does not exist gets it. Doing nothing is a working
            // assumption: the spec does not say what it does [I].
            Value::Builtin(op) if self.is_unbound_marker(op) => Value::Void,
            Value::Builtin(op) => builtins::call(self, host, op, args),
            Value::Procedure(p) => self.apply_procedure(host, &p, args),
            Value::Method { object, option } => {
                let mut args = args;
                // A Sly 1 method with a fixed count reads exactly that many arguments (missing ones as nothing).
                if self.game() == Game::Sly1
                    && let Some(Some(n)) = method_arity(option)
                {
                    args.resize(n as usize, Value::Nil);
                }
                let r = host.call_action(object, option, &args);
                self.drain_host_events(host);
                match r {
                    Value::Error(e) => self.fail(host, e),
                    v => v,
                }
            }
            // Calling something that is not callable is a no-op that gives an error value (scripts.md 11,
            // item 18); the game reports nothing, so neither does the interpreter.
            _ => Value::Error(ScriptError::NotCallable),
        };
        self.depth -= 1;
        r
    }

    fn apply_procedure(&mut self, host: &mut dyn ScriptHost, p: &Rc<Procedure>, mut args: Vec<Value>) -> Value {
        // Sly 2 and 3 bind as many values as there are parameters without checking the count; a missing one is
        // "no value" (specs/sly2/scripts.md 6, specs/sly3/scripts.md 6).
        if self.game() != Game::Sly1 && args.len() < p.params.len() {
            args.resize(p.params.len(), Value::Void);
        }
        if args.len() < p.params.len() {
            return self.fail(
                host,
                ScriptError::TooFewArguments { given: args.len().min(255) as u8, needed: p.params.len() as u8 },
            );
        }
        if !self.heap.is_live(p.env) {
            return self.fail(host, ScriptError::StaleEnvironment);
        }
        let env = self.heap.alloc(Some(p.env));
        let rest = args.split_off(p.params.len());
        for (&s, v) in p.params.iter().zip(args) {
            self.heap.define(env, s, v);
        }
        if let Some(r) = p.rest {
            self.heap.define(env, r, Value::list(rest));
        }
        let outer = self.running.replace(p.clone());
        let r = self.sequence(host, p.node, 2, env);
        self.running = outer;
        r
    }

    /// Evaluates elements `from..` of a list in order; the result is the last one's (void when there are none).
    /// An error in one expression does not stop the following ones.
    fn sequence(&mut self, host: &mut dyn ScriptHost, list: NodeId, from: usize, env: EnvRef) -> Value {
        let mut last = Value::Void;
        for i in from..self.len(list) {
            last = self.eval(host, self.child(list, i).expect("in range"), env);
        }
        last
    }

    /// A datum as a value, unevaluated: lists become lists, symbols and keywords become data.
    fn datum(&self, id: NodeId) -> Value {
        match self.program.node(id) {
            Node::List { .. } => {
                let items: Vec<Value> = self.program.list(id).iter().map(|&c| self.datum(c)).collect();
                Value::list(items)
            }
            &Node::Symbol(s) => Value::Symbol(s),
            &Node::Keyword(k) => Value::Keyword(k),
            Node::Nil => Value::Nil,
            Node::Int(i) => Value::Int(*i),
            Node::Float(f) => Value::Float(*f),
            Node::Vector(v) => Value::Vector(*v),
            Node::Matrix(m) => Value::Matrix(m.clone()),
            Node::Coeffs(v) => Value::Coeffs(*v),
            Node::Range(r) => Value::Range(*r),
            Node::Smoothing(v) => Value::Smoothing(*v),
            Node::Bool(b) => Value::Bool(*b),
            Node::Builtin(o) => Value::Builtin(*o),
            Node::Object(0) => Value::Object(None),
            &Node::Object(_) => Value::Error(ScriptError::BadCell(kind::OBJECT)),
            &Node::Other(k) => Value::Error(ScriptError::BadCell(k)),
        }
    }

    fn special(&mut self, host: &mut dyn ScriptHost, k: u8, id: NodeId, env: EnvRef) -> Value {
        let n = self.len(id);
        match k {
            kind::QUOTE => {
                let Some(d) = self.child(id, 1) else { return self.fail(host, ScriptError::BadForm(k)) };
                // The same quoted list is the same object every time it is evaluated.
                if let Some(v) = self.quotes.get(&id) {
                    return v.clone();
                }
                let v = self.datum(d);
                if matches!(v, Value::List(_)) {
                    self.quotes.insert(id, v.clone());
                }
                v
            }
            kind::SET | kind::DEFINE => {
                let (Some(s), Some(e)) = (self.child(id, 1), self.child(id, 2)) else {
                    return self.fail(host, ScriptError::BadForm(k));
                };
                let Node::Symbol(sym) = *self.program.node(s) else { return self.fail(host, ScriptError::BadForm(k)) };
                let v = self.eval(host, e, env);
                if k == kind::DEFINE {
                    if !self.heap.define(env, sym, v.clone()) {
                        return self.fail(host, ScriptError::StaleEnvironment);
                    }
                    v
                } else if self.heap.set(env, sym, v.clone()) {
                    v
                } else {
                    self.fail(host, ScriptError::Unbound(sym))
                }
            }
            kind::ASSERT => {
                if let Some(e) = self.child(id, 1) {
                    self.eval(host, e, env);
                }
                Value::Void
            }
            kind::IF => {
                let Some(t) = self.child(id, 1) else { return self.fail(host, ScriptError::BadForm(k)) };
                let test = self.eval(host, t, env);
                let branch = if test.is_true() { self.child(id, 2) } else { self.child(id, 3) };
                match branch {
                    Some(b) => self.eval(host, b, env),
                    None => Value::Void,
                }
            }
            kind::OR => {
                for i in 1..n {
                    let v = self.eval(host, self.child(id, i).expect("in range"), env);
                    if v.is_true() {
                        return v;
                    }
                }
                Value::Bool(false)
            }
            kind::AND => {
                let mut last = Value::Bool(true);
                for i in 1..n {
                    last = self.eval(host, self.child(id, i).expect("in range"), env);
                    if !last.is_true() {
                        return last;
                    }
                }
                last
            }
            kind::COND => {
                for i in 1..n {
                    let clause = self.child(id, i).expect("in range");
                    let Some(head) = self.child(clause, 0) else { return self.fail(host, ScriptError::BadForm(k)) };
                    if matches!(self.program.node(head), Node::Keyword(kind::ELSE)) {
                        return self.sequence(host, clause, 1, env);
                    }
                    let test = self.eval(host, head, env);
                    if test.is_true() {
                        if self.len(clause) == 1 {
                            return test;
                        }
                        return self.sequence(host, clause, 1, env);
                    }
                }
                Value::Void
            }
            kind::CASE => {
                let Some(e) = self.child(id, 1) else { return self.fail(host, ScriptError::BadForm(k)) };
                let key = self.eval(host, e, env);
                for i in 2..n {
                    let clause = self.child(id, i).expect("in range");
                    let Some(head) = self.child(clause, 0) else { return self.fail(host, ScriptError::BadForm(k)) };
                    let hit = match self.program.node(head) {
                        Node::Keyword(kind::ELSE) => true,
                        Node::List { .. } => self.program.list(head).iter().any(|&d| same_value(&key, &self.datum(d))),
                        _ => same_value(&key, &self.datum(head)),
                    };
                    if hit {
                        return self.sequence(host, clause, 1, env);
                    }
                }
                Value::Void
            }
            kind::LET => {
                let Some(b) = self.child(id, 1) else { return self.fail(host, ScriptError::BadForm(k)) };
                let mut bound = Vec::new();
                for i in 0..self.len(b) {
                    let binding = self.child(b, i).expect("in range");
                    let (sym, v) = match *self.program.node(binding) {
                        Node::Symbol(s) => (s, Value::Nil),
                        Node::List { .. } => {
                            let sym = match self.child(binding, 0).map(|s| self.program.node(s)) {
                                Some(&Node::Symbol(s)) => s,
                                _ => return self.fail(host, ScriptError::BadForm(k)),
                            };
                            let v = match self.child(binding, 1) {
                                Some(e) => self.eval(host, e, env),
                                None => Value::Nil,
                            };
                            (sym, v)
                        }
                        _ => return self.fail(host, ScriptError::BadForm(k)),
                    };
                    bound.push((sym, v));
                }
                let inner = self.heap.alloc(Some(env));
                for (s, v) in bound {
                    self.heap.define(inner, s, v);
                }
                self.sequence(host, id, 2, inner)
            }
            kind::WHILE => {
                let Some(t) = self.child(id, 1) else { return self.fail(host, ScriptError::BadForm(k)) };
                loop {
                    // Error values count as true, so the loop also ends when the step budget is gone.
                    if self.steps == 0 {
                        return Value::Error(ScriptError::OutOfSteps);
                    }
                    let test = self.eval(host, t, env);
                    if !test.is_true() {
                        return Value::Void;
                    }
                    self.sequence(host, id, 2, env);
                }
            }
            kind::LAMBDA => {
                let Some(f) = self.child(id, 1) else { return self.fail(host, ScriptError::BadForm(k)) };
                let mut params = Vec::new();
                let mut rest = None;
                match *self.program.node(f) {
                    Node::Nil => {}
                    Node::List { .. } => {
                        let formals = self.program.list(f);
                        let mut i = 0;
                        while i < formals.len() {
                            match (self.program.node(formals[i]), formals.get(i + 1).map(|&p| self.program.node(p))) {
                                (&Node::Symbol(s), _) if rest.is_none() => params.push(s),
                                // A rest marker (scripts.md 11, item 5) and the one symbol after it, last.
                                (&Node::Keyword(kind::REST), Some(&Node::Symbol(s)))
                                    if rest.is_none() && i + 2 == formals.len() =>
                                {
                                    rest = Some(s);
                                    i += 1;
                                }
                                _ => return self.fail(host, ScriptError::BadForm(k)),
                            }
                            i += 1;
                        }
                    }
                    _ => return self.fail(host, ScriptError::BadForm(k)),
                }
                Value::Procedure(Rc::new(Procedure { node: id, params, rest, env }))
            }
            kind::BEGIN => self.sequence(host, id, 1, env),
            // Sly 2 keyword 6: one expression's value, or no value without one; keyword 17: no value.
            kind::EVAL_ONE => match self.child(id, 1) {
                Some(e) => self.eval(host, e, env),
                None => Value::Void,
            },
            kind::NO_VALUE => Value::Void,
            kind::ELSE => self.fail(host, ScriptError::UnknownKeyword(k)),
            _ => self.fail(host, ScriptError::UnknownKeyword(k)),
        }
    }

    // ---- option access (scripts.md section 8), used by the built-ins ----

    fn map_option(&mut self, host: &mut dyn ScriptHost, object: ObjectId, id: u32) -> Result<u16, ScriptError> {
        let Some(map) = self.options.as_ref() else { return Err(ScriptError::NoOptionMap) };
        let class = host.class_of(object).ok_or(ScriptError::NoClass)?;
        map.lookup(class, id).ok_or(ScriptError::NoOption { class, option: id })
    }

    fn is_action(&self, option: u16) -> bool {
        if self.game() != Game::Sly1 {
            return self.sly2.actions.get(option as usize).copied().unwrap_or(false);
        }
        method_arity(option).is_some()
    }

    /// Operation 102 on one object.
    pub(crate) fn get_option(&mut self, host: &mut dyn ScriptHost, object: ObjectId, id: u32) -> Value {
        if id >= self.custom_base() {
            return self.custom_property(object, id).cloned().unwrap_or_else(|| self.missing_custom());
        }
        let option = match self.map_option(host, object, id) {
            Ok(o) => o,
            // An option the class's map does not list falls through to the custom properties (scripts.md 11,
            // item 16), so it reads as the empty list unless a script stored it.
            Err(ScriptError::NoOption { .. }) => {
                return self.custom_property(object, id).cloned().unwrap_or_else(|| self.missing_custom());
            }
            Err(e) => return self.fail(host, e),
        };
        if self.is_action(option) {
            return Value::Method { object, option };
        }
        match host.get_option(object, option) {
            Value::Error(e) => self.fail(host, e),
            v => v,
        }
    }

    /// Operation 101 on one object.
    pub(crate) fn set_option(&mut self, host: &mut dyn ScriptHost, object: ObjectId, id: u32, value: &Value) -> Value {
        if id >= self.custom_base() {
            self.store_custom(object, id, value.clone(), false);
            return Value::Void;
        }
        let option = match self.map_option(host, object, id) {
            Ok(o) => o,
            Err(e) => return self.fail(host, e),
        };
        let r = host.set_option(object, option, value);
        self.drain_host_events(host);
        match r {
            Ok(()) => Value::Void,
            Err(e) => self.fail(host, e),
        }
    }

    /// Operations 99 (`append`: always a new entry) and 100 (overwrite the newest entry for the ID, else append),
    /// scripts.md 11, item 9. Sly 1 refuses an ID below the custom base; Sly 2's 108 and 109 (Sly 3's 114 and 115)
    /// use the property store whatever the ID (specs/sly2/scripts.md 5).
    pub(crate) fn add_custom(
        &mut self,
        host: &mut dyn ScriptHost,
        object: ObjectId,
        id: u32,
        value: &Value,
        append: bool,
    ) -> Value {
        if self.game() == Game::Sly1 && id < self.custom_base() {
            return self.fail(host, ScriptError::NotCustom(id));
        }
        self.store_custom(object, id, value.clone(), append);
        Value::Void
    }
}
