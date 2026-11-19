---
id: sly1-scripts
title: Sly 1 world scripts and state machines
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46
      decompressed worlds; executable SCUS_971.98 sha256
      d4ba3ce30047ffb2a825ef13d37a48c4df63aa57977157a194f66bd0b859796c
    used_for: >-
      cell-kind statistics over all scripts; which built-in operations occur and with how many arguments; which
      fixed event symbols are defined; the per-class script option tables read from the executable; state-machine
      and segment option values; the decoded examples in section 10
  - kind: reference-project
    what: >-
      ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files eval, bif, serialize, splicemap, spliceobj, sidebag,
      frame, find, math, lo, sw, sm, alo, so, aseg, asega, spaprops (stream options 974, 977 and the action
      entries of section 11), and the classes that raise script events
    used_for: >-
      evaluation rules of each special form, the meaning and argument order of each built-in operation, the event
      numbers and their arguments, state-machine routing; the reference's built-in operations are partly
      reconstructed, so edge cases are marked "to confirm"
date: 2026-11-14
---

# World scripts and state machines

Each world carries a set of small programs (the script table of world-file.md 4.3) in a Lisp-like language. They
are not bytecode: the stored cells are the program's syntax tree, and the game evaluates that tree directly. Scripts
react to events raised by objects (an object was loaded, touched, triggered, an animation reached a marker, ...) and
act on the world mostly by reading and writing object options. State machines (class 136) are data, not scripts:
they choose animations to reach a goal state.

## 1. The model

- **Modules.** Each entry of the script table is one module. When the world loads (after the object stream and
  before the first frame), every module is evaluated once, in table order, in a fresh environment of its own. The
  module's top-level definitions stay in that environment.
- **Environments.** An environment is a set of bindings from symbol IDs to values, with a parent. Every module
  environment's parent is the world environment. Lookup walks from the innermost environment outwards.
- **Objects and modules.** The third field of an object record (object-stream.md 5.1) is a module index. The object
  is attached to that module's environment. Events raised by the object (section 6) look up their handler there.
  Several objects can share one module; 2,075 records on the disc name a module, the rest name none.
- **Symbols** are 32-bit IDs, local to a world. Symbols 0 to 29 are the fixed event names of section 6; the rest
  are the module's own variables. Names exist only in debug builds; the USA worlds store none.

## 2. Values

| Value | Written in the cells as | Notes |
|---|---|---|
| integer | kind 20 (`s32`) | |
| float | kind 21 (`f32`) | |
| vector | kind 22 (3 × `f32`) | |
| matrix | kind 23 (16 × `f32`) | a 4×4 transform |
| three coefficients | kind 24 (3 × `f32`) | a quadratic g0 + g1·x + g2·x² |
| range | kind 25 (2 × `f32`) | (min, max) |
| smoothing parameters | kind 26 (3 × `f32`) | slow speed, fast speed, fast time |
| boolean | kind 27 (`u32`, non-zero = true) | |
| symbol | kind 28 (`u32` ID) | evaluates to its binding |
| built-in operation | kind 29 (`u32` ID) | section 7 |
| list | kind 30 | the payload is the list's first cell; elements chain through bit 7 of the tag byte |
| the empty list | kind 0, or tag byte `0xFF` | also the "none" result |
| object | kind 32 | always the **null object** on the disc (payload 0 in all 3,825 cells); real objects only appear at run time |
| procedure | (run time only) | made by `lambda` |
| method | (run time only) | an object's action option, bound to the object (section 8) |
| void, error | (run time only) | results of statements and of failed operations |

Kinds 1 to 19 are the language keywords of section 4 when they head a list. Kinds 3, 4, 16, 18 and 19 never occur on
the disc.

**Truth.** Only the boolean false is false. Every other value, including 0, the empty list and the null object, is
true in `if`, `cond`, `and`, `or` and `while`.

## 3. Evaluation

- Integers, floats, vectors, matrices, coefficient sets, ranges, booleans, built-in operations, objects,
  procedures and methods evaluate to themselves.
- A symbol evaluates to its binding, searched from the current environment outwards. An unbound symbol gives an
  error value (not a crash).
- A list whose first cell is a keyword (kinds 1 to 17) is a special form (section 4).
- Any other list is a call: evaluate the first element; it must be a built-in operation, a procedure or a method.
  Evaluate the arguments left to right. Too few arguments for the callee give an error value.
  - Built-in: run it on the argument values (section 7). Operations marked "+" take any number of further
    arguments.
  - Procedure: make a new environment whose parent is the procedure's defining environment; bind the formal
    parameters to the arguments (a rest parameter, if declared, receives the remaining arguments as a list);
    evaluate the body expressions in order; the result is the last one's.
  - Method: perform the object action (section 8) with the arguments.

## 4. Special forms

Written (keyword operand ...). Kinds and their counts on the disc in brackets.

| Kind | Form | Meaning |
|---|---|---|
| 1 | (set symbol expr) [1,325] | evaluate expr and store it in the nearest existing binding of symbol; the result is the value |
| 2 | (define symbol expr) [7,838] | evaluate expr and bind symbol in the current environment (shadowing outer ones) |
| 5 | (assert expr) [22] | evaluate; a false result is only reported in debug builds. No effect in play |
| 6 | (if test then [else]) [1,055] | |
| 7 | (or expr ...) [24] | first value that is not false, else false |
| 8 | (and expr ...) [127] | first false value, else the last value (true when empty) |
| 9 | (cond (test body ...) ... (else body ...)) [16] | first clause whose test is not false; a clause without body returns its test value |
| 10 | else [62] | marker inside `cond` and `case` |
| 11 | (case expr ((value ...) body ...) ... (else body ...)) [307] | compare expr with each listed value by "same value" (operation 12) |
| 12 | (let ((symbol expr) ...) body ...) [21] | all exprs evaluated in the outer environment, then bound in a new one |
| 13 | (while test body ...) [7] | repeat while test is not false |
| 14 | (lambda (formals ...) body ...) [4,889] | a procedure closing over the current environment |
| 15 | (begin expr ...) [3,116] | evaluate in order; result of the last |
| 17 | (quote datum) [284] | the datum unevaluated (often a list of OIDs) |
| 0 | () | empty list |

Every module on the disc (2,591 over the 46 worlds) is one `begin` holding only `define`s (5,008 top-level
definitions), and every `set` on the disc names a symbol.

## 5. Scheduled calls

Operation 97 (schedule a call) takes a delay in seconds, a procedure and its arguments. The world keeps a list of
pending calls. Every game frame (gameplay-overview.md section 2) each pending call's remaining time is reduced by the
frame time; a call whose time has run out is removed from the list and run. Calls scheduled while running others run
on a later frame.

## 6. Events: how objects call scripts

An object raises an event by number. If the object is attached to a module, and the symbol with that number is
bound there to a procedure, the procedure is called with the object itself as the first argument followed by the
event's arguments. Otherwise nothing happens. Events are raised synchronously, at the moment the condition happens.

| Event | Raised by | When | Extra arguments | Defined on the disc |
|---:|---|---|---|---:|
| 0 | any object | after the world has loaded, once per object | — | 1,007 |
| 1 | placed objects | every frame while the object updates | frame time | 56 |
| 2 | many classes | "triggered": button pressed, alarm sounded, pickup collected, breakable broken, barrier hit, player-specific triggers | — | 639 |
| 3 | buttons, alarms, hiding places, camera zones, pipes, the player | "untriggered": released, alarm switched off, left | — | 109 |
| 4 | placed objects | added to the world (made present) | — | 24 |
| 5 | placed objects | removed from the world | — | 37 |
| 6 | solid objects that ask for touch events | started touching another solid | the other object | 4 |
| 7 | same | stopped touching | the other object | 0 |
| 9 | animation segments | applied | the playback | 400 |
| 10 | animation segments | an event of type 40 is crossed | the playback, the 16-bit value | 615 |
| 11 | animation segments | the playback is removed | its root object | 245 |
| 12 | trigger-volume buttons with option 277 set | when the set of objects inside changes (interactables.md 2) | — | 7 |
| 13 | gun breakables, some solids | fired a projectile | the projectile | 5 |
| 14 | state machines | a state is entered | the run, from-state OID, to-state OID | 16 |
| 15 | guards | changed behaviour state | old state, new state | 59 |
| 16 | dialogues | reached a "call script" step | label OID | 35 |
| 17 | state machines | started | the run | 3 |
| 18 | state machines | stopped | — | 0 |
| 19 | hiding places | the player hiding in it took damage | the damage | 3 |
| 20 | breakables | took a hit | hit count so far | 47 |
| 21 | dialogues | finished | — | 26 |
| 22 | buttons, alarms | a trigger that is checked against the save state | — | 65 |
| 23, 24, 25 | jump guides | started, reached a label (label value), ended | — | 36, 11, 43 |
| 26, 27, 28 | rhythm sequences | started, reached a command (value), stopped | — | 4, 3, 3 |
| 29 | barriers | hit by an object | the object | 5 |

"Defined on the disc" counts module-level definitions of that symbol over all 46 worlds. Event 8 is never raised.

## 7. Built-in operations

The operations that occur in the USA worlds (74 of 125 IDs). The argument count is the minimum; "+" means more are
accepted. Every call on the disc matches these counts. IDs not listed never occur and may be treated as an error.

**Arithmetic and comparison**

| ID | Args | Operation |
|---:|---|---|
| 0 | 1+ | add. Numbers: integer result unless an argument is a float. Vectors and matrices add component-wise |
| 1 | 1+ | subtract (one argument: returns it unchanged, see section 11) |
| 2 | 1+ | multiply (numbers; vector times number scales) |
| 3 | 1+ | divide (vector, matrix, coefficients or range divided by numbers divides each component) |
| 4 | 1 | print (no effect in play) |
| 7 | 2 | integers equal (both converted to integers) |
| 8, 9, 10, 11 | 2 | <, ≤, >, ≥ on numbers |
| 12 | 2 | same value: same kind and equal (numbers by value, objects by identity, lists by identity) |
| 13 | 2 | equal: as 12 but lists compared element by element |
| 25 | 1 | is the empty list |
| 27 | 1 | is the null object |
| 30 | 3 | near: \|a − b\| ≤ c |
| 31 | 1 | not (true only for false) |
| 53 | 2 | random float in [a, b) |
| 55, 56 | 1 | sine, cosine (radians) |
| 63, 64 | 2 | dot product, cross product |
| 65 | 1 | vector length |
| 66, 67 | 1+ | distance, squared distance between two vectors (the second defaults to zero) |
| 68 | 1+ | normalise a vector |
| 71 | 3+ | launch velocity from a start point to hit a target point; the third argument steepens the arc, the fourth is the launching object (section 11, item 11) |
| 81 | 2+ | transform at a position looking at a target, with +Z (or a given vector) as up |
| 82 | 1 | the same transform with its rotation tilted upright: the shortest rotation that turns its local Z to world +Z |
| 84 | 3 | evaluate three coefficients at x and clamp to a range |
| 85 | 4 | fit three coefficients: (a, b, u, c) gives the quadratic with f(0) = a, f(1) = b, f(u) = c |
| 86 | 2 | clamp a number to a range |
| 87 | 2 | is a number inside a range |
| 88 | 1 | floor |
| 91 | 1 | truncate to an integer |
| 92 | 1 | absolute value |
| 93, 94 | 1+ | maximum, minimum |
| 95 | 2 | modulo |
| 96 | 0 | current game time (seconds) |

**Lists and values**

| ID | Args | Operation |
|---:|---|---|
| 33, 34 | 1 | first element; rest of the list |
| 35 | 2 | replace the first element (in place) |
| 37 | 1 | length |
| 38 | 2 | n-th element (from 0), or the empty list |
| 39 | 2 | is the value an element (by "same value") |
| 40 | 0+ | make a list |
| 41 | 0+ | append lists |
| 42 | 2 | map a procedure over a list, giving a list |
| 43 | 2 | filter a list by a procedure |
| 44 | 2 | call a procedure for each element (for effect) |
| 46 | 3 | make a vector |
| 48 | 3 | make three coefficients |
| 49 | 2 | make a range |
| 51 | 1+ | element of a vector (index), matrix (row, or row and column), coefficients, range or smoothing parameters |

**World and objects**

| ID | Args | Operation |
|---:|---|---|
| 97 | 2+ | schedule a call: delay, procedure, arguments (section 5) |
| 98 | 2 | postpone an object's updates by a time |
| 99 | 3 | add a custom property value to an object (section 8) |
| 100 | 3 | ensure a custom property: create it with the value if missing, else set it |
| 101 | 3 | set an option on an object or on every object of a list: object, option, value |
| 102 | 2 | get an option of an object (from a list: of the last object): object, option |
| 103 | 2 | find an object by OID: OID, context object (null object: search the whole world; otherwise below the context) |
| 104 | 2 | find all objects with an OID (up to 64), as a list |
| 105 | 2 | find the object with an OID nearest to a context object (object-stream.md 7.4) |
| 106 | 2 | find all nearest objects with an OID |
| 107 | 0 | the player-controlled object |
| 108 | 0 | the world object |
| 109 | 0 | the camera object |
| 110 | 2 | find all objects of a class (class ID, context), up to 256 |
| 112 | 2 | find the solid objects overlapping a sphere (centre, radius) |
| 115 | 3 | convert a position from one object's space to another's (null object = world) |
| 116 | 3 | convert a direction the same way |
| 119 | 2 | play a sound effect (sound ID, emitting object or null) |
| 121 | 2 | controller rumble (kind, strength) |
| 122 | 2 | puff of smoke at a position with a size |

A failed find returns the null object (or the empty list for the list forms). Option operations on the null object
or the empty list do nothing and return nothing.

## 8. Objects' options from scripts

Scripts use their own option numbering, the **script option IDs**, which differ from the stream option IDs of
option-encodings.md:

- Script option IDs below 1173 name built-in options. Which stream option a script option ID means depends on the
  object's class. The executable holds the mapping: a table at address `0x244950` (file offset `0x145950`) of 148
  records, one per class ID, each `u32` count and `u32` address of that class's list. A class's list is `count`
  records of `u32` script option ID and `u32` address of an option descriptor, sorted by script option ID. The
  descriptor's stream option ID is (address − `0x2505D8`) / 32. A packager can extract the whole mapping from the
  player's own disc. Before reading it, the port checks the executable's SHA-256 against the value in disc.md and
  refuses on a mismatch. Read on the disc: 964 distinct script option IDs, 1 to 1172; 436 of them appear in more than one
  class, and **161** of those map to different stream options in different classes, so the lookup must be per
  class (section 11, item 1).
- Script option IDs from 1173 up are **custom properties** made by the world's scripts. The world header's extra
  option count (world-file.md 4.3) is how many there are (Paris: 9, IDs 1173 to 1181). Each object keeps its own
  custom properties; reading one that was never set gives the empty list.
- Reading an option (102) gives its value; for an **action option** (stream options whose code is a digit,
  option-encodings.md) it gives a method bound to the object, which runs the action when called with the action's
  arguments. Writing (101) sets the value through the same path as the stream's option lists, so writing an option
  has exactly the effect the same option has when it appears in an object's option list.
- Adding (99) and ensuring (100) apply to custom properties.

The meaning of each option belongs to the per-system specs; the scripts only move values through them.

## 9. State machines (class 136)

The layout is in classes/animation-and-state.md. Behaviour:

- **States** are named by OID. Each state may have a **next state**: when the machine reaches a state that is its
  goal, the goal becomes that state's next state (so a state can chain into an idle loop), or "none".
- **Transitions** connect two states. Flag bit 0: usable in both directions; walked backwards, its animation plays
  in reverse from its end. Flag bit 1: may be interrupted when the goal changes. A transition with an animation
  plays it (animation.md section 5, played on the machine's root object); a transition without one is
  instantaneous.
- **Choosing a step.** When the machine is idle and has a goal different from its state, it looks for the shortest
  route (fewest transitions) from the current state to the goal, using reverse steps of two-way transitions. A
  transition from the current state to itself counts as a route of length one. Among the first steps of all
  shortest routes it picks one at random, weighted by the transitions' probabilities; if all probabilities are 0
  it picks the last candidate in table order. It then plays that transition. When its animation reaches the end
  (animation.md 5.2), the machine enters the new state (raising event 14) and chooses again. Instantaneous steps are
  taken immediately in the same frame. If no route exists the goal is cleared.
- **Starting.** A machine with option 513 set starts by itself when the world has loaded, on the root object of its
  first transition's animation; others are started by gameplay (checkpoints, alarms, dropped pickups and others start
  their own runs) or by animation event 46 (which starts a run in the given state if none exists; event 47 stops
  one). Option 516 sets the goal state (OID) **of a run**; a machine with no run has no goal to set, so setting it
  never starts a machine [R]. Option 519 sets the playback speed for its animations.
- On the disc: 755 state machines, 101 of them auto-starting; their animations are all class 131 records.

## 10. Test vectors (decoded scripts)

Written with `sN` for symbol N and `opN` for built-in operation N.

**Splash, the only module** (script table bytes `0x1B..0x41`):

  (begin (define s0 (lambda (s66) (op101 s66 1015 2))))

On load (event 0), every object attached to this module sets its script option 1015 to 2.

**Paris, France, module 2** (bytes `0x72..0x94`):

  (begin (define s0 (lambda (s66) ((op102 s66 10)))))

On load, the object reads script option 10 (an action option: for placed objects it maps to stream option 2, code
`0`, "remove from the world") and calls it: the object takes itself out of the world.

**Paris, France, module 5** (bytes `0x1AE..0x2A4`):

  (begin (define s75 null-object)
         (define s0 (lambda (s66) (op99 s66 1173 #f) (set s75 (op103 1398 null-object))))
         (define s76 (lambda (s66 s75) (if (op31 (op102 s66 1173)) (begin ((op102 s75 267))))))
         (define s2 (lambda (s66) ((op102 (op103 1397 null-object) 10))
                                  (if (op31 (op102 s75 1006)) (op97 2.5 s76 s66 s75))
                                  ((op102 s66 10)))))

On load: give the object the custom property 1173 = false and remember object 1398. When triggered (event 2): remove
object 1397 from the world; if object 1398's option 1006 is false, schedule s76 in 2.5 seconds; remove itself.

Counts to check a script parser against (all 46 worlds): 52,799 list cells, 31,754 symbol cells, 24,379 integer
cells, 21,112 built-in cells, 1,804 float cells, 672 booleans, 633 empty lists, 3,825 null objects, one vector and
one range; plus the keyword counts of section 4.

## 11. Clarifications

These answer the questions raised while implementing this spec. [D] disc, [X] executable at the given address
(after the hash check of section 8), [R] reference behaviour.

1. **The section 8 count.** 161 is the number of distinct script option IDs that map to more than one stream
   option across classes [X]. The 343 earlier in this file counted class/ID pairs whose mapping differs from the
   first class (in table order) that lists the ID; it was not a count of IDs. Measured again on the table at
   `0x244950`: 964 IDs, 436 in more than one class, 161 with differing targets, 343 differing class/ID pairs.
2. **Modules named by no object header.** A proxy's override entry (classes/proxy.md 1.2, "script index") also
   attaches a module, to the override's target [D] [R]. Override entries name 415 modules that no object header
   names, among them Splash's only module: Splash has one override, target OID 1356 (a class-87 object), script
   index 0. With headers (2,075 records) and overrides (415 records), **101 modules remain unattached**. 96 of them
   are the one-line "remove myself on load" module (`(begin (define s0 (lambda (s66) ((op102 s66 10)))))`), the
   other 5 are larger. Every module is evaluated when the world loads [R], so their definitions exist, but no
   object receives their events and no script reaches them (the import keyword, kind 16, never occurs). Treat them
   as unused.
3. **Scripts on proxies.** No proxy record names a script module in its own header (0 of 7,925) [D]. A proxy attaches
   modules only through its override entries (415 on the disc): the module goes to the override's target, the placed
   object with that OID (searched among the proxy's placed objects and below them). If the target already had a
   module, the new module's environment gets the old one as its outer environment, so both modules' definitions stay
   visible [R].
4. **Operation 103 and proxies.** Looking up an OID that belongs to a proxy returns the object(s) the proxy placed:
   the first of its placed sources (or clones), in placement order [R]. A source whose proxy has the same OID as the
   one asked for is skipped as a direct match, so it is found only through its proxy. For Paris OID 1398 (a proxy
   with one source, OID 836) the result is that source object, and option mapping uses **the source's class**.
5. **Script option 267 in class 101.** It maps to stream option 974 [X], which is an **action with no arguments**
   that starts the dialogue [R]; option 1006 in the same class maps to stream option 977, a boolean "has been
   played" that reads and writes [R]. Stream options that scripts can reach but no stream contains have no code in
   option-encodings.md. Of the 526 such stream IDs, the reference treats 117 as actions [R]: 4, 9, 27, 86, 87, 90,
   94, 107, 108, 117, 118, 124, 126, 162, 174, 197 to 200, 207, 228, 282, 296, 311, 312, 315, 316, 362 to 364,
   439, 481, 482, 489, 490, 522, 546, 553, 554, 562, 568 to 571, 574, 575, 577 to 579, 581, 595, 598 to 600, 606,
   607, 613, 622, 647, 657, 663, 667, 677, 721, 722, 732 to 734, 737 to 740, 743 to 745, 751, 821, 822, 827,
   840, 888, 889, 911 to 914, 917, 928, 939, 974, 975, 1001, 1004, 1016, 1020, 1026, 1087, 1088, 1091, 1113,
   1115 to 1117, 1132, 1143, 1144, 1147, 1152, 1153, 1156 to 1158, 1168, 1171, 1174, 1175, 1178. The others are
   values. The reference states argument counts only for 9 (1), 738 (1), 1168 (3), 1171, 1174 and 1175 (1 each);
   for the rest, take the count from the call (every action call on the disc passes what the action needs).
6. **Rest parameters.** A formals list marks a rest parameter with a cell of kind 19 followed by one symbol, which
   receives the remaining arguments as a list (the empty list when there are none) [R]. Kind 19 never occurs on the
   disc, so no script uses rest parameters. Extra arguments to a procedure without one are **not evaluated** and
   are ignored [R].
7. **Error values** are ordinary values [R]. They do not propagate on their own. In tests they count as true
   (only false is false). An operation that gets one treats it like any argument of the wrong kind. Calling an
   error value, or a call with too few arguments, gives an error value.
8. **Results.** `define`: the value bound. `set`: the value stored. `if` whose test is false and has no else:
   void. `while`: void. `cond` and `case` with no matching clause: void. A `cond` clause without body: its test
   value. `begin` with no expressions: void [R].
9. **`case` values are data**, not evaluated: each clause starts with a list of literal values, compared with the
   key by operation 12 [R].
10. **99 and 100** [R]. Each object has room for 16 custom-property entries. 99 always appends a new entry; the
   newest entry for an ID is the one read, so the property takes the new value, and an entry is used up each time
   (appends beyond 16 are ignored). 100 overwrites the newest entry for the ID if there is one, else appends. Both
   return void. Neither keeps an existing value. Both accept a list of objects, like 101.
11. **Argument orders** [R], fixed by position (not by kind):
    - 38 (list, index); 39 (value, list); 42, 43, 44 (procedure, list).
    - 84 (coefficients, range, x); 86 and 87 (range, value).
    - 115 and 116 (from object, to object, position or direction): converts from the first object's space to the
      second's; the null object means world space.
    - 66 and 67: (a [, b]), b defaults to the zero vector. 68: (v [, fallback]): when |v| < 0.0001 the result is
      the fallback (zero vector if absent).
    - 71: (start, target, steepness [, launching object]). The elevation starts at the angle of the straight line
      to the target (at least 0) and moves toward vertical by `sin(steepness)` of the remaining angle; the speed is
      then the one whose arc under gravity passes through the target. The gravity is the launching object's, else
      the world's (physics.md). No solution: zero vector. On the disc: 2 calls, steepness 0.5,
      the fourth argument an object (Prowling the Grounds).
12. **Numbers and matrices** [R].
    - 0 to 3 compute in single-precision floats; the result is an integer (truncated toward zero) when every
      number argument was an integer. So `(op3 7 2)` is 3. With one argument, 1 and 3 return the argument
      unchanged (no negation, no reciprocal). Division by zero follows float rules; when the result must be an
      integer and is not finite, treat it as an error value (the reference leaves it undefined).
    - 88 (floor), 90 (round: floor of x + 0.5) and 91 (truncate) return integers. 93 and 94 return the winning
      argument itself, so its kind is kept. 95 is the integer remainder with the sign of the dividend; modulo 0
      gives 0.
    - Matrices: index `i` (0..2) of 51 on a matrix gives the image of local axis `i` (X, Y, Z) in world space;
      index 3 gives the position; with a second index `j`, component `j` of that vector. No matrix cell occurs on
      the disc, so this is only a run-time convention. 81 is (position, target [, up]): local X points at the
      target, Y = up × X normalised, Z = X × Y, position = the first argument; up defaults to +Z.
    - 85 is (a, b, u, c): f(x) = g0 + g1·x + g2·x² with g0 = a, g2 = ((c − a)/u − (b − a)) / (u − 1),
      g1 = (b − a) − g2. One call on the disc: `(op85 0 1 600 0.01)` in Descent into Danger.
13. **State machines** [R].
    - Start: with no starting state given, a run starts in the machine's **first state** (tentative: the reference relies on a zero-initialised
      state index), without an enter-state event for it; it then chooses a step at once. Event 17 is raised after that.
    - Interrupted step (transition flag bit 1, goal changed during it): the step **ends in its destination state**
      (raising the enter-state event), its animation stops, and the machine chooses again from that destination.
      Without bit 1, the new goal waits until the step ends.
    - Goal equal to the current state: the goal moves on to that state's next state; the machine loops only if the
      next state leads back (for example a state whose next state is itself and that has a self-transition). A
      self-transition takes priority as the one-step route when the goal is the current state again.
14. **The 633 empty lists** are kind-0 cells only. Tag byte `0xFF` never occurs in the scripts of the 46 USA worlds
    [D] (580 kind-0 cells sit inside lists, 53 end them).
15. **Wrong kinds of arguments are not rejected** [R]. No built-in operation fails because an argument has the
    wrong kind; it reads what it can and carries on, every frame, with no error:
    - Reading a number from anything that is not a number gives **0** (true and false give 1 and 0). Reading a
      vector from a non-vector gives the zero vector; a matrix, the identity.
    - Add and multiply (0, 2) **skip** arguments that are not numbers wherever they are; subtract and divide (1, 3)
      read their first argument as a number (so `()` there counts as 0) and skip later non-numbers. With a vector
      or matrix first argument, arguments of another kind are skipped. So `(op0 () 5)` is 5, `(op2 () 3)` is 3,
      `(op1 () 2)` is −2; with no numbers at all add gives 0 and multiply 1 (integers).
    - Element of (51) on something that is not a vector, matrix, coefficients, range or smoothing record, or with an
      index out of range, gives the empty list.
    - Random (53) reads its bounds as numbers: `(op53 () ())` gives 0.
    - Comparisons (8 to 11) give false when either side is not a number.
    So the class-33 module of A Ghastly Voyage and A Grave Undertaking, which reads custom properties 1179 and 1182
    to 1187 that nothing sets (each reads as the empty list, section 8), simply computes with zeros and empty lists;
    the game shows no error. A port should do the same and not report these as failures.
16. **World option 634 is never "unset"** [X for the mapping] [R]. On the world (class 108) script option 634 maps to
    stream option 547 [X: table at `0x244950`], the current level's **adaptive difficulty value** (pickups-and-progress.md
    3.4), a float from 0 to 1 that always exists: reading it gives the value now (0 in a fresh level), writing it
    sets it (clamped to its range). The same class maps 643 to 556 and 650 to 563, both methods (item 17). Reading an
    option that a class's map does not list falls through to the custom properties, so it gives the empty list.
17. **Methods** [R] [X for the class mappings]. Reading an option whose stream option is a method gives a method bound
    to the object; calling it runs the method. Item 5 listed only the methods that no stream contains. The complete
    list, as stream option (argument count, result), from the reference; "?" means the count is taken from the call:
    1 (?, bool), 2 (?, void), 4 (?, void), 5 (?, object), 9 (?, void), 10 (?, bool), 13 (1, bool), 27 (?, void),
    86 (0), 87 (0), 88 (3), 90 (2), 94 (1), 107 (5), 108 (0), 117 (1), 118 (1), 120 (1, map), 121 (1, blend),
    122 (0, blend), 124 (1), 126 (2), 146 (1), **147 (1, bool: is a point inside the object's volume)**, 161 (4),
    162 (0), 174 (0), 185 (4, blend), 186 (5, blend), 187 (1, float), **197 (3: seek an animation)**, 198 (0),
    199 (1), 200 (2), 207 (0), 228 (0), 264 (4), 276 (4), 282 (0), **283 (1, bool: is the object invulnerable to a
    damage kind)**, 296 (1), 297 (4), 311 (1), 312 (0), 315 (1), 316 (1), 362 (1), 363 (0), 364 (0), 481 (0),
    482 (6), 489 (0), 490 (6), 514 (3, value), 522 (0), 546 (3), 553 (2), 554 (0), 555 to 561 (1, bool each; 556 is
    "is this the level's primary …"), 562 (3), **563 (2, integer: a level data value)**, 564 (1, bool), 565 (1,
    bool), 566 (0, integer), 568 (0), 569 (2), 570 (0), 571 (0), 573 (1, integer), 574 (1), 575 (2), 576 (1,
    integer), 577 (2), 578 (0), 579 (0), 580 (1), 581 (1), 595 (1), 596 (0, value), 597 (0, value), 598 (3), 599 (0),
    600 (0), 601 (3, bool), 606 (0), 607 (0), 613 (0), 622 (0), 657 (2), **658 (1: an animation, at once)**, 663 (2),
    667 (0), 677 (1), 721 (1), 722 (0), 732 (3), 733 (0), 734 (1), 737 (1), 738 (?), 739 (1), 740 (0), 743 (1),
    744 (1), 745 (1), 821 (0), 822 (1), 888 (0), 889 (0), 911 (1), 912 (1), 913 (2), 914 (0), **915 (1, bool: fire)**,
    917 (0), 928 (0), 939 (0), 974 (0), 975 (0), 1001 (0), 1026 (0), 1087 (1), 1088 (5), 1091 (0), **1103 (1, bool:
    is a point inside the volume)**, **1104 (1, bool: is a position within the flash)**, **1105 (1, bool)**, 1113 (2),
    1115 (0), 1116 (1), 1117 (0), 1118 to 1122 (1 each), 1123 (2), 1124 (4), 1125 to 1127 (3 each), 1128 (2),
    1129 (1), 1131 (0), 1132 (0), 1143 (0), 1144 (1), 1147 (0), 1152 (1), 1153 (0), 1156 (2), 1157 (1), 1158 (0),
    1161 (3), 1162 (3), 1163 to 1166 (2 each), 1168 (?), 1171 (?), 1174 (?), 1175 (?), 1178 (2). Results not named
    are void. In bold the ones the implementation found called on the disc (with their script IDs per class [X]:
    254 → 147 in classes 3 to 76, → 1103 in 123 and 124, → 1104 in 96; 950 → 915 in 140; 1123 → 1105 in 14;
    712 → 658 in 8 to 19; 650 → 563 and 643 → 556 in 108; 306 → 197 in 146; 441 → 283 in 5 to 29).
    **306 in class 147 maps to stream option 521, which is not a method** but a value (an OID; writing it sends the
    state machine toward that state; it has no reader, so reading it gives nothing useful). A script that calls it
    therefore does nothing (item 18); to send a machine to a state, a port writes the option (101).
18. **Calling something that is not callable** [R]: the call gives an error value, its arguments are **not
    evaluated**, and nothing happens. That covers calling void (the result of an option operation on the null object
    or the empty list, section 7), the empty list, a number or an object. It is a no-op apart from the error value
    it returns, which (item 7) behaves like any other value.

## 12. Open questions

1. The exact numeric behaviour of a few operations (integer vs float results of 1 to 3 with mixed arguments, the
   fit of 85, the tilt of 82) comes from a partly reconstructed reference and should be confirmed by observation.
2. The meaning of each script option per class (section 8) is the job of the per-system specs.
3. The order in which several objects attached to the same module receive event 0 is the order they were loaded
   (stream order); to be confirmed against the game where scripts depend on it.
