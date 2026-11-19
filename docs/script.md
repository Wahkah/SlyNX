# The `script` crate

`crates/script` runs the Sly 1, Sly 2 and Sly 3 world scripts: it reads the script table into a syntax tree, interprets it, and
talks to the game engine only through a trait. It also runs state machines (class 136). Written from
`specs/sly1/scripts.md` (with `object-stream.md`, `option-encodings.md` and `classes/animation-and-state.md`).

- `#![no_std]` with `alloc`, `#![forbid(unsafe_code)]`, no third-party crates. Depends only on `formats` (without
  its `std` feature). Builds for `aarch64-nintendo-switch-freestanding`.
- Nothing panics on bad data: malformed cells give a `ParseError`, failed operations give error values.
- Float maths (`math`) uses no libm, so results are the same bits on PC and Switch.

## Quick start

```rust
use script::{Interpreter, ObjectId, OptionMap, Program, event};

let world = formats::world::World::load(&bytes)?;
let program = Program::from_world(&bytes, &world.layout)?;        // every module of the world
let options = OptionMap::from_executable(&disc.executable)?;      // checks the executable's SHA-256
let mut scripts = Interpreter::new(program, Some(options));
scripts.load(&mut host);                                          // evaluate each module once, in table order
for (i, r) in world.objects.records.iter().enumerate() {
    if r.script >= 0 { scripts.attach(ObjectId(i as u32), r.script as u32); }
}
// and each proxy override entry with a script index attaches its target (scripts.md 11, item 2)
scripts.post_load(&mut host, attached_objects_in_stream_order);   // event 0
// each frame:
scripts.raise_event(&mut host, obj, event::UPDATE, &[script::Value::Float(dt)]);
scripts.tick(&mut host, dt);                                      // scheduled calls (scripts.md 5)
```

`host` is the engine's implementation of `ScriptHost`. `ObjectId` is any `u32` the engine chooses (a record
index, a slot in its object table); the interpreter never looks inside it.

## Modules

| Module | Main items | Spec |
|---|---|---|
| `tree` | `Game`, `Program` (`from_world`, `from_sly2_world`, `package_module`, `read_table`, `read_module`, `module`, `node`, `list`, `definitions`, `display`, `cell_counts`, `custom_options`), `Node`, `NodeId`, `kind`, `ParseError` | scripts.md 2, 4; world-file.md 4.3 |
| `value` | `Value`, `ObjectId`, `ScriptError`, `Cons`, `Procedure`, `same_value`, `equal` | scripts.md 2, 3 |
| `interp` | `Interpreter`, `Limits`, `ScheduledCall` | scripts.md 1, 3 to 6 |
| `builtins` | `arity`, `defined`, `launch`, `fit`, `FIND_LIMIT`, `CLASS_FIND_LIMIT` | scripts.md 7 |
| `host` | `ScriptHost`, `RaisedEvent`, `NullHost` | scripts.md 7, 8 |
| `option_map` | `OptionMap` (`from_executable`, `from_sly2_executable`, `read`, `read_layout`, `from_lists`, `lookup`, `class`), `MapLayout`, the table constants, `MapError` | scripts.md 8; sly2 5 |
| `sly2` | `BuiltinTable`, `EventSignatures`, `arity`, `defined`, `sly1_equivalent`, `HOST_OPERATIONS`, the table addresses | sly2 4, 6 |
| `event` | the event numbers 0 to 29 under descriptive names (`LOADED`, `UPDATE`, `TRIGGERED`, ...) | scripts.md 6 |
| `machine` | `StateMachine` (`from_record`, `parse_body`, `first_steps`), `MachineRun`, `MachineHost`, `Step`, `choose` | scripts.md 9 |
| `math` | `sqrt`, `sin_cos`, `atan2`, `floor`, `trunc`, vector helpers | |

## The syntax tree

`Program` keeps all nodes of a world in one arena; list elements are contiguous in `Program::children`. A module
is a chain of top-level cells (on the disc always one `begin` of `define`s). `Program::display(id)` prints the
notation of scripts.md section 10 (`sN`, `opN`, `null-object`, `#t`/`#f`).

Reading rules beyond world-file.md 4.3: an `0xFF` tag as the first cell of a nested list makes the list empty; an
`0xFF` tag after other elements is an empty-list element. Kind 0 is the empty list. The reader is iterative (the
nesting reaches 72 on the disc). `cell_counts[k]` counts cells of kind `k`; index 37 counts `0xFF` tags.

`Program::from_world` takes the script offsets from `formats::world::ScriptTable::scripts`, so no new `formats`
accessor was needed. `Program::read_table` reads a whole table at an offset, including the debug-build strings.

## The interpreter

- **Environments** live in an arena (`EnvRef` = slot plus generation) and are reclaimed by mark and sweep, because
  closures stored in the environment that made them form cycles. Roots: the world and module environments, custom
  properties, quoted lists and scheduled calls. Collection runs automatically at the end of `load`,
  `raise_event` and `tick` when enough environments were made (`Limits::collect_after`), or on
  `collect_garbage()`. A procedure value kept outside the interpreter across a collection gives
  `ScriptError::StaleEnvironment` when called, never a wrong environment.
- **Outer modules** (scripts.md 11.3): when a proxy override gives a module to an object that already has one, the
  engine calls `attach_with_outer(object, new, Some(old))`. The new module's environment then continues in the old
  module's before the world's, so event handlers, module bindings and the procedures the new module defines see both
  modules' names (the new module's first; `set` reaches the old module's bindings). The chain belongs to the module
  environment, which all objects attached to that module share: a module keeps the first outer module it gets, and
  a different one later, or a chain that would loop, is refused and counted (`outer_conflicts`).
  `outer_module(m)` reports the chain. `load` starts every module over without outer modules, so attach after it.
- **Truth**: only `#f` is false.
- **Errors** are ordinary values (`Value::Error`, scripts.md 11, item 6). Each new error is passed to
  `ScriptHost::report` once and counted (`error_count`). They do not propagate: in a test an error counts as true;
  an operation that gets one treats it as an argument of the wrong kind (operations that take any value, such as
  40, keep it); calling an error value gives `NotCallable`.
- **Limits** (`Limits`), so a broken or hostile script can neither hang a frame nor exhaust memory or the stack:
  - nesting depth 256 (`TooDeep`);
  - one million evaluation steps per entry into the interpreter (`OutOfSteps`, so an endless `while` cannot hang a
    frame). Built-ins that walk or build lists use one step per element. All the scheduled calls that fall due in
    one `tick` share one budget;
  - lists made by operations 40 and 41 hold at most 65,536 elements, shared tail included (`TooLarge`), so repeated
    appends cannot double a list without bound;
  - at most 1,024 scheduled calls wait at once (operation 97 gives `TooLarge` beyond that; a NaN delay is a bad
    argument);
  - the reader refuses lists nested more than 128 deep (`tree::MAX_NESTING`); the disc stays well below it. Quoting
    and printing recurse once per level, so this bounds their stack use.
- **Stack use**: reaching the depth limit takes about 83 KiB of stack in a release build on x86-64 (335 bytes per
  level) and about 292 KiB in a debug build (measured by the `stack_use_at_the_depth_limit` test, which prints the
  figure). Run the interpreter on a thread with at least 256 KiB of stack on the Switch (release); the libnx main
  thread's default is larger. AArch64 frames are of similar size; confirm on hardware with the same test.
- **Cyclic and shared lists**: operation 35 can store a list inside itself. The collector visits each cell once;
  `equal` (operation 13) and printing walk with an explicit stack, stop past 256 levels or 65,536 elements (an
  unfinished `equal` is false; printing ends with `...`); dropping a list never recurses. A list that contains
  itself is never freed (reference counting); this costs memory only for that list.
- **Special forms** as scripts.md 4 and 11 (item 7): `define` gives the value bound and `set` the value stored;
  `if` whose test is false and has no else, `while`, a `cond` or `case` with no matching clause, and an empty
  `begin` give void; a `cond` clause without body gives its test value; `case` clause values are data, compared by
  operation 12. Choices where the spec is silent: `set` of an unbound symbol is an error; a `let` binding may be a
  bare symbol (bound to `()`); a quoted list is the same list each time its `quote` is evaluated.
- **Procedures**: `lambda` formals are a list of symbols, optionally ending with a rest marker (a kind-19 cell)
  and one symbol, which receives the remaining arguments as a list (scripts.md 11, item 5). Arguments beyond the
  formals of a procedure without a rest parameter are not evaluated.
- **Events** (`raise_event`): the handler is the event symbol's binding seen from the object's module; it is called
  with the object followed by the event's arguments. Events the host raises while the interpreter is calling it
  (`ScriptHost::poll_event`, polled after every option write and action call) are raised at once, so they stay
  synchronous.
- **Scheduled calls** (`tick(dt)`): each pending call's time is reduced by `dt`; due calls run in scheduling order;
  calls scheduled meanwhile wait at least one frame.
- **Custom properties** (script option IDs from 1173) are kept by the interpreter: up to 16 entries per object
  (`CUSTOM_SLOTS`). Operation 99 always appends an entry (ignored once the 16 are used); 100 overwrites the newest
  entry for its ID, else appends; reading gives the newest entry, `()` when there is none (scripts.md 11, item 9).
  `detach` forgets an object's properties.

## Options (scripts.md 8)

`OptionMap::from_executable` refuses any executable whose SHA-256 is not the supported one
(`formats::disc::Executable::is_supported`), then reads the 148 class records at `0x244950` (file offset
`0x145950`) and each class's (script option ID, descriptor address) list; the stream option ID is
(address − `0x2505D8`) / 32. The table stays in memory on the player's machine only.

For operations 101 and 102 the interpreter asks the host for the object's class, maps the script option ID, and:

- **reading** an action option gives `Value::Method { object, option }`; calling it calls
  `ScriptHost::call_action`. An option is an action when its stream encoding in
  `formats::options::OptionTable::sly1()` is a typed argument list, or, for the stream options that table does not
  list, when it is in `interp::UNLISTED_ACTIONS` (the 117 IDs of scripts.md 11, item 4). Other options go to
  `ScriptHost::get_option`. (`ScriptHost::is_action` is no longer consulted.)
- **writing** goes to `ScriptHost::set_option`, which must have the effect of the option in a stream option list.
- Option operations on the null object or `()` do nothing and give void; operation 101 on a list of objects writes
  each; 102 on a list reads from the last object.

## The host trait

`ScriptHost` (every method has a do-nothing default; `NullHost` uses them all):

| Method | Used by |
|---|---|
| `class_of(object)` | option mapping |
| `get_option(object, stream_option)`, `set_option(object, stream_option, &value)`, `call_action(object, stream_option, &args)` | 101, 102, methods |
| `find_object(oid, context)`, `find_objects(...)`, `find_nearest(...)`, `find_all_nearest(...)` | 103 to 106 (lists are cut to 64). For a proxy's OID, 103 gives the first object the proxy placed, and option mapping uses that object's class (scripts.md 11, item 3) |
| `find_by_class(class, context, out)` | 110 (cut to 256) |
| `objects_in_sphere(centre, radius, out)` | 112 |
| `player()`, `world()`, `camera()` | 107, 108, 109 |
| `convert_point(p, from, to)`, `convert_direction(d, from, to)` | 115, 116 (`None` = world) |
| `postpone_updates(object, seconds)` | 98 |
| `play_sound(id, emitter)`, `rumble(kind, strength)`, `smoke(position, size)` | 119, 121, 122 |
| `game_time()` | 96 |
| `random()` in [0, 1) | 53, state machines (use the game's deterministic generator) |
| `gravity()`, `object_gravity(launcher)` | 71: the launching object's gravity, else the world's (default 1500, physics.md 2) |
| `print(&value)` | 4 |
| `report(&error)` | every new error value |
| `poll_event()` | events raised by the host during a call |

Values handed to the host are `Value`s; the host converts them to the option's type (an `h` option gets an
`Int`, a `v` option a `Vector`, and so on).

## Built-in operations

All 74 operations that occur on the disc are implemented; scripts.md section 7 defines no others, so the other 51
IDs give `UnknownOperation`. `builtins::arity` holds the minimum argument counts ("+" = more allowed); every call
on the disc matches them (disc test).

Behaviour fixed by scripts.md 11 (items 9 to 11) and the remaining choices:

- 0 to 3 compute in single-precision floats; the result is an integer (truncated toward zero) when every argument
  is an integer, so `(op3 7 2)` is 3. With one argument, 1 and 3 return it unchanged. An integer result that is not
  finite (division by zero) is `DivideByZero`. Vectors and matrices add and subtract component-wise; a vector or
  matrix among numbers is scaled by their product; 3 divides vectors, matrices, coefficients, ranges and smoothing
  parameters by numbers.
- 88 (floor), 90 (round, floor of x + 0.5) and 91 (truncate) give integers. 93 and 94 give the winning argument
  itself (the first on a tie), so its kind is kept. 95 is the integer remainder with the sign of the dividend;
  modulo 0 gives 0. 90 occurs on no disc script but is defined, so `defined()` lists 75 operations.
- Argument orders are fixed by position: 38 (list, index), 39 (value, list), 42 to 44 (procedure, list), 84
  (coefficients, range, x), 86 and 87 (range, value), 115 and 116 (from object, to object, vector).
- 66 and 67 measure from the zero vector with one argument; 68 (v [, fallback]) gives the fallback (zero vector if
  absent) when |v| < 0.0001.
- 71 (start, target, steepness [, launching object]): the elevation starts at the angle of the straight line to
  the target (at least 0) and moves toward vertical by `sin(steepness)` of the remaining angle; the speed is the one
  whose arc under the launcher's gravity (`ScriptHost::object_gravity`) passes through the target; zero vector when
  there is no such arc (`builtins::launch`).
- Matrices are row-major with rows 0 to 2 the images of the local X, Y and Z axes and row 3 the position. 51 with
  index 0 to 3 gives that row as a vector; with a second index 0 to 2, that component. 81 (position, target
  [, up]): X points at the target, Y = up x X normalised, Z = X x Y, up defaults to +Z. 82 turns the rotation by
  the shortest rotation that takes local Z to world +Z (half a turn about local X when Z points straight down) and
  keeps the position (`builtins::upright`).
- 85 (a, b, u, c): g0 = a, g2 = ((c - a)/u - (b - a))/(u - 1), g1 = (b - a) - g2, so f(0) = a, f(1) = b,
  f(u) = c (`builtins::fit`).
- 99 and 100: see custom properties above; both accept a list of objects like 101 and give void.
- 33 and 34 on `()` give `()`.

## State machines

`StateMachine::from_record(data, record, table)` reads a class-136 record found by the `formats` walker (the body
starts 6 bytes after `Record::offset`): states, transitions, and whether option 513 is set. The owned animation of
transition `t` is the `animation_index(t)`-th record owned by the machine.

`MachineRun` holds the state, the goal and the step being played. The engine calls `start`, `stop`, `set_goal`
(option 516), `update` once per frame and `animation_finished` when the played animation reaches its end;
`MachineHost` plays and stops step animations, supplies random numbers and is told when a state is entered (so the
engine raises event 14; events 17 and 18 are raised by the engine around `start` and `stop`).

- Routes: breadth-first search for the fewest steps (at least one) from the state to the goal, with two-way
  transitions also walked backwards (`Step::reverse`); a self-loop is a route of length one. Among the first steps
  of the shortest routes, in table order, one is chosen at random weighted by probability, or the last one when all
  weights are zero. No route: the goal is cleared.
- Reaching the goal state moves the goal to that state's next state (which may be the state itself, giving a loop
  through a self-transition). Setting the goal to the current state while idle counts as reaching it.
- Instantaneous transitions are taken in the same call, at most 64 in a row (the rest on the next `update`).
- Changing the goal during an interruptible step stops its animation and ends the step in its destination state
  (with the enter-state call), then chooses again from there; during any other step the new goal is followed when
  the step ends (scripts.md 11, item 12).
- A new run starts in the machine's first state, without an enter-state call, and chooses a step at once. Playback
  speed (option 519) is the engine's to apply.

## Sly 2

Written from `specs/sly2/scripts.md` (with `object-stream.md`); Sly 1 behaviour is unchanged. A `Program` carries
its `Game`, which selects the cell numbering, the built-in operations, the option-map layout and the
custom-property range.

```rust
use script::{BuiltinTable, Interpreter, ObjectId, OptionMap, Program};

let world = formats_sly2::World::parse(&data, &disc.options)?;
let program = Program::from_sly2_world(&world)?;                  // every package's script block
let map = OptionMap::from_sly2_executable(&disc.executable)?;     // checks the executable's SHA-256
let table = BuiltinTable::from_executable(&disc.executable)?;     // argument rules of the 169 operations
let mut scripts = Interpreter::new(program, Some(map));
scripts.set_sly2_tables(Some(table), &disc.options);               // which stream options are actions
scripts.load(&mut host);
// a record's script index names a module of its own package:
if let Some(m) = scripts.program().package_module(package, record.script) { scripts.attach(object, m); }
```

- **Reader.** Each package's script block follows its four header words (`body.offset + 16`): a module count and
  the modules. `Program::packages` gives each package's first module and count; `package_module(package, index)`
  the program-wide module. The Sly 2 cell numbering (values moved up by one and reordered, keywords 1 to 20) is
  read into the same nodes as Sly 1; debug tables carry 16 bytes after each tag. Raw kinds are kept in
  `cell_counts`.
- **Special forms.** Keywords 7 to 16 and 18 are Sly 1's 6 to 15 and 17. `assert` evaluates its expression only
  (the message is for debug builds). The new keyword 6 gives its one expression's value, or void without one
  (`kind::EVAL_ONE`); keyword 17 gives void (`kind::NO_VALUE`). Keyword 20 is the rest-parameter marker
  (`kind::REST`); keyword 19 has no rule (an unknown keyword).
- **Procedures.** A procedure binds as many values as it has parameters; a missing one is void ("no value";
  scripts.md 6), where Sly 1 gives `TooFewArguments`.
- **Built-ins.** `sly2::arity` holds the argument rule of the 97 operations used on the disc (from scripts.md 4.2);
  every call is checked against it and, when loaded, against the executable's table (`BuiltinTable`), which also
  rejects extra arguments where none are allowed (`ScriptError::TooManyArguments`). Operations with a Sly 1
  counterpart run the Sly 1 code (`sly2::sly1_equivalent`; 88 is Sly 1's 91). New ones, as scripts.md 4.3 gives
  them:
  - 32 (element in front of a list), 36 (replace the rest in place), 41 (a shuffled copy, with `random()`);
  - 47, make a vector: none (0, 0, 0), one number n (n, n, n), three numbers (x, y, z); other counts are an error
    value;
  - 48, make a matrix from the identity: one number on the rotation's diagonal, three or four vectors as rows,
    nine or sixteen numbers row by row; other counts give the identity;
  - 54 square root, 59 arc cosine, 61 wrap an angle (one within -pi..pi is unchanged, any other a gives
    ((a + pi) mod 2pi) - pi), 62 atan2(y, x), 72 the angle and axis between two vectors (a two-element list), 76
    rotation interpolation (A's rotation turned by t of the way to B's; position row zero), 79 Euler angles
    (rotation = Rz Ry Rx, the spec's pole rule), 83 evaluate three coefficients, 135 and 136 bitwise AND and OR;
  - 105 schedules a call and returns a handle; 106 cancels by handle;
  - 133 looks a symbol (quoted, or its number) up in the calling environment; unbound, it gives operation 134 as a
    value;
  - the game's own operations go to `ScriptHost::game_operation(op, args)` (`sly2::HOST_OPERATIONS`, scripts.md 4.2
    and 4.3): 97 the level row and 99 its episode; 100 to 102 the current job, the current task and the checkpoint
    task; 103 the difficulty help (0 to 1); 104 a game-state integer, always 0; 114 the objects found for a list of
    IDs; 121 objects in a box; 123 and 124 objects hit by a segment (all, first); 131 a saved progress value read by
    its path of name symbols; 137 the HUD prompt object; 138 and 139 read and set the owned-item flags. 118 (the
    sound manager) runs as Sly 1's 107 and so reaches the host through `player()`;
  - the searches by object ID (112, 113, 115) pass the full `s32` ID to `ScriptHost::find_object_id` and
    `find_objects_id` (Sly 2 IDs do not fit 16 bits; the defaults fall back to the 16-bit searches).
- **Options.** `OptionMap::from_sly2_executable` reads the 199 class records at `0x279AD0` (`MapLayout::SLY2`);
  stream option = (entry address - `0x2AC880`) / 40. Lists keep their duplicate entries, and `OptionMap::lookup`
  is the game's binary search (midpoint rounded down, first match reached), which finds the second entry for
  class 98, IDs 112 and 113 (scripts.md 5). Custom properties start at 1,233, a fixed threshold in the game
  (`Interpreter::custom_base`): getting or setting an ID from it on uses the object's property store, adding
  (108) and ensuring (109) always do, and a property never added reads as void. An option is an action when its
  option-table entry is a typed argument list (flag `0x400`), has value type `0x12`, or has flag `0x1` with
  arguments (option 1095 of noise templates, value type `0x2093`, flag `0x1`, two arguments, is one).
- **Events.** `EventSignatures::from_executable` reads the 30 signatures (type codes, object included). Event 29
  starts at the end of the 51 type words, so its signature is empty (scripts.md 6). Raising events is as in Sly 1.
- **Lists.** Both halves of a list cell can now be replaced, so a list can loop back on itself: every walk is
  bounded (`value::MAX_LIST_ITEMS`).

## Sly 3

specs/sly3/scripts.md: Sly 3 keeps Sly 2's language and evaluator with renumbered kinds and operations.

```rust
let program = Program::from_sly3_world(&world)?;                   // every group's script block
let map = OptionMap::from_sly3_executable(&disc.executable)?;      // checks the executable's SHA-256
let table = script::sly3::builtin_table(&disc.executable)?;        // argument rules of the 178 operations
let mut scripts = Interpreter::new(program, Some(map));
scripts.set_sly3_tables(Some(table), &disc.options);
```

- **Reader.** `Program::from_sly3_world(&formats_sly3::World)` reads every group's block (after its four header
  words), with `packages` and `package_module` as for Sly 2. Value kinds are Sly 2's above 18 renumbered down by 2
  (19 integer, 20 float, 21 boolean, 22 symbol, 23 built-in, 24 vector, 25 matrix, 26 coefficients, 27 range, 28
  smoothing, 29 list, 31 object).
- **Keywords** (3) map onto the interpreter's: 1 to 3 as Sly 2, 4 to 15 are Sly 2's 5 to 16 (assert, eval-one, if,
  or, and, cond, else, case, let, while, lambda, begin), 16 is quote, 17 has no rule and 18 is the rest-parameter
  marker.
- **Built-ins** (4). `sly3::arity` holds the rule of the 116 operations used on the disc; with the executable's table
  (`sly3::builtin_table`, 178 entries at `0x342030`) both are checked, except that 31 (not) takes a surplus
  argument and ignores it (one call on the disc). `sly3::sly2_equivalent` gives each operation's Sly 2 ID
  (scripts.md 4.2) and the operation runs as that one (errors name the Sly 3 ID). Operations whose Sly 2 ID Sly 2
  never uses run as Sly 1's (`sly3::sly1_direct`: 66 cross product, 73 launch velocity, 84 tilt upright, 86
  coefficients within a range, 89 within a range). Sly 3's own: 19 is a vector, 46 a stable sort by a comparison
  procedure (true: first argument goes first), 47 evaluates a value as an expression in the world environment, 52
  makes smoothing parameters, 55 the largest float, 59 tangent, 71 the component along a normal ((v . n) n), 90 and
  91 smooth a float or vector toward a goal (a fraction rate x time step, limited to 0..1, of the way), 100 linear
  interpolation, 102 the second clock (read as game time), 141 the symbol value (as Sly 2's 133; unbound, 142).
  Where scripts.md gives a name only (46's comparison sense, 47's environment, 71, 90, 91, 102) the behaviour is a
  working assumption.
- **Game operations** go to `ScriptHost::game_operation` with their **Sly 3 numbers** (`sly3::HOST_OPERATIONS`):
  103, 104, 105, 107 to 110, 120, 126, 127, 129 to 132, 139, 140, 145, 146 and 147. 123 (the world), 124 (the
  player) and 125 (the camera) go to `world()`, `player()` and `camera()`; 118, 119 and 121 use the wide-ID
  searches.
- **Options** (5). `OptionMap::from_sly3_executable` reads the 249 class records at `0x2ED518` (`MapLayout::SLY3`,
  stream option = (entry address - `0x32CA80`) / 40), with the same binary search over duplicates. Custom
  properties start at 1,816.
- **Events** (6). `sly3::event_signatures` reads the 34 signatures: start positions at `0x2EDDC0` (no -1 after
  them), type codes from `0x2EDCE4`. Event 32's one type word is the index's first word; event 33's signature is
  empty.
- **Command scripts** (7) are class-135 objects the engine runs; the interpreter only answers their hooks
  (command `0x15` and module conditions) through the module's event handler.
- **In the engine.** `Program::from_sly3_bodies(data, bodies)` reads the blocks from the world's bytes and group
  bodies as the engine keeps them; `engine::world::sly2::program` uses it for a Sly 3 world, and
  `configure_interpreter` calls `set_sly3_tables`. The Sly 3 loader records each object's program-wide module, and
  the game crate runs Sly 3 worlds with scripts and the executable's option map. The host
  (`engine::game::script_host::sly3`) follows scripts.md 4.3 and takes the Sly 3 numbers above.

## Tests

- Unit tests (`cargo test -p script`): the reader on encoded synthetic trees (nesting, empty lists, debug cells,
  whole tables with strings, errors); the truth rule; every special form; closures, recursion, rest parameters,
  quote identity; arithmetic, geometry and list operations; option mapping, methods, custom properties,
  synchronous host events, the null object and object lists; scheduled-call timing; depth and step limits and
  environment collection; outer-module chains (lookup order, procedures, events, refused loops and second outers);
  robustness (cyclic, shared, long and deep lists, list size and step charges, the
  scheduled-call cap and shared tick budget, the reader's nesting limit, short or oversized option tables, stack use
  at the depth limit); the option-map reader on a synthetic image; state-machine routing, weighted choice,
  chaining, interruption and parsing; the scripts.md 11 behaviour (error values, form results, unevaluated extra
  arguments, rest markers, custom-property slots, unlisted actions, operations 71, 82, 85, 90 and 95).
- Sly 2 unit tests: the reader on the same trees written in Sly 2 numbering (same text, raw kind counts, the new
  keywords, debug cells); renumbered and new operations (47 and 48 forms, 72, 61, 76, 79); argument rules; the
  looping list, scheduling handles and cancelling, operation 133; custom properties from 1,233 holding procedures
  called as methods, adding below the threshold, duplicate map entries found by the binary search, and actions
  from the option table; the built-in and event tables read from a synthetic image.
- Sly 3 unit tests: the reader in Sly 3 numbering, keywords as the interpreter's, operations through their Sly 2
  equivalents and Sly 3's own, the numbering table (every Sly 2 ID but 98 and 99 has one Sly 3 ID) and an
  implementation for every operation used on the disc.
- Sly 2 disc tests (`tests/disc_sly2.rs`, run when `SLYNX_DISC_SLY2` names the image, about 2 s): every script
  block of the 43 worlds (7,753 packages, 1,558 with scripts, 10,930 modules), the cell totals and keyword list
  heads, every module one `begin` of `define`s (one empty), the module counts of nine worlds, every record's
  script index within its package (7,138 top-level, 10,554 at every depth), the built-in table (169 entries and
  the zero word after it) with all 117,333 calls obeying it and the 97 used operations matching `sly2::arity`, the
  option map (12,010 records, 1,091 IDs from 1 to 1,232, stream options up to 1,448, 210 IDs mapped per class, the
  nine duplicate entries and the record the search finds for each), the literal custom-property IDs (664, from
  1,234 to 1,904), event 29's empty signature, the event signatures and handler
  counts and arities, the decoded modules of 7.3, and every world loaded with its load events and 10 s of
  scheduled calls under a mock host with no structural error.
- Live options (`tests/disc.rs`, `live_option_values_against_the_spec`): how the disc's scripts use the options no
  list stores (options-by-class.md 1.1). 981 (an enumeration) is read and compared with operation 7; 844 and 848
  (object references) are read and written straight into another guard's same option with 101; 880 and 1084 are
  written with objects found by 103, 205 with objects held in variables; 849 and 850 with booleans; 845 with ranges
  made by 49; 127 (a float) with the integer 1000000. Each pattern runs through the interpreter with values of the
  spec's kinds, and no built-in rejects them. Built-ins reject an object-reference option that a host reads as the
  integer 0: an unset reference must read as the null object, and a float option must accept integer writes.
- Outer modules over the disc (`override_attachments_and_outer_modules`): the proxy override entries
  (`formats::stream::ObjectStream::proxy_overrides`) resolved as the engine does name 415 modules and leave 101
  unattached (scripts.md 11.2); 40 give a module to an object that already has one, and all 40 chains are made.
  With the chains every name resolves; without them 7 references to the outer modules' names are unbound.
- Sly 3 disc tests (`tests/disc_sly3.rs`, run when `SLYNX_DISC_SLY3` names the image, about 10 s): 39 worlds, 7,151
  groups, 1,524 with scripts, 11,007 modules; every record's module within its group (6,698 top-level, 10,631 at
  every depth); every module one `begin` of three-cell `define`s; the cell totals and keyword heads outside quoted
  data of scripts.md 2 and 3; the built-in table (178 entries, the zero word after it), 178,038 calls of 116
  operations all obeying it but the one two-argument `not`, and `sly3::arity` equal to the table; the option map
  (16,803 records, 1,587 IDs from 1 to 1,815, stream options up to 2,043, 237 IDs mapped per class, the eleven
  duplicates), the literal custom IDs (960, from 1,817 to 2,786); the event signatures and the handlers whose
  parameter counts differ (event 28: five with one fewer, event 32: five with four more); the decoded modules of
  section 8; and every world loaded with its load events and 10 s of scheduled calls under a mock host with no
  structural error (5 error values in all, from mock answers).
- Disc tests (`tests/disc.rs`, run when `SLYNX_DISC_SLY1` names the image): every module of all 46 worlds parses;
  the cell, keyword, module (2,591), definition (5,008) and event-definition counts of scripts.md; every module is
  one `begin` of `define`s and every `set` names a symbol; exactly the 74 operations occur with matching argument
  counts; 2,075 records name a module; the three decoded modules of section 10 (offsets and text); the option map
  (address mapping, sorted lists, 964 script option IDs from 1 to 1172, script option 10 = stream option 2 for the
  Paris module-2 objects); all 755 state machines parse, 101 auto-start, their tables end at their option lists,
  one class-131 animation per animated transition; the three decoded modules run against a mock engine with the
  real option map, checking each host call (Splash's option write, Paris module 2's self-removal, Paris module 5's
  load, trigger and the call scheduled 2.5 s later); and every world loads, raises event 0 and event 1 for its
  attached objects and runs 10 s of scheduled calls with no structural error.

## Not implemented yet

- The meaning of each option (per class) and of each action: the engine's job (options-by-class spec to come).
- Attaching modules from proxy override entries (scripts.md 11, items 2 and 3) is the engine's job through `attach`
  and `attach_with_outer`; the disc test resolves them the same way to check the counts and the chains.
- Debug-build features (assert reports, names); `print` is passed to the host and nothing else.

## Open questions for the spec

Sly 2 (specs/sly2/scripts.md): the earlier questions (duplicate map entries, the first custom ID, event 29, the
operations' argument forms, keyword kinds 19 and 20, nested records) are answered in scripts.md and the crate
follows the answers. Still open:

1. What the three event-9 handlers with a second parameter receive in play (they get void here; scripts.md 8,
   question 6).
2. 36 handler calls reach the depth limit when every world runs its load events under a mock host (handlers that
   recurse until the game's state changes). Worth checking against the game once options are real.

Sly 3 (specs/sly3/scripts.md 9):

1. The comparison sense of 46, the environment 47 evaluates in, and the exact formulas of 71, 90 and 91 (working
   assumptions above).
2. Whether the duplicate-entry lookup is still Sly 2's binary search (assumed, as the spec does).
3. What the game passes to event 28's and event 32's handlers whose parameter counts differ.

Sly 1: none open. The earlier questions (option-mapping counts, unattached modules, operation 103 on proxies, unlisted
actions, rest parameters, error values, form results, `case` values, operations 99 and 100) are answered in
scripts.md 11, and the crate follows those answers.
