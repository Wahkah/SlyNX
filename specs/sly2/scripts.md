---
id: sly2-scripts
title: Sly 2 world scripts - differences from Sly 1
side: reference
status: draft
sources:
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (src/splice-bif, src/splice-bif-splice-eval, nonmatching/splice-bif-splice-eval/fun_001242b8.c, src/new_100008/RefCallPhitfxSpawn.cpp, the object-match routine); SafeHouse @ c70803ce Sly2-Release (bif.cpp, gamebif.cpp, spliceglue.cpp, ainoise.cpp, lo.h)
    used_for: what game operations 97 to 139 do, the copy root answering to its placement's OID, option 1095; re-checked 2026-11-18 against the executable (built-in table entries, op 104's code at 0x00210B68, option-table entries 1090 to 1096) and the disc (argument forms of every call)
  - kind: disc
    what: >-
      Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 decompressed worlds
    used_for: >-
      every script block of every package parsed into trees (10,930 modules, 7,753 packages); node-kind, keyword
      and built-in histograms; built-in arities checked against the executable's built-in table; event handler
      arities checked against the executable's event signature table; the per-class script option map read from
      the executable and checked against the option IDs scripts use; all test vectors
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), src/splice-eval*, src/splice-bif*, include/cref.h, include/splopt.h, src/data/splice-eval-splice-frame.cpp, src/new_132b08
    used_for: >-
      which keyword number selects which special form, the meaning of the value kinds, the location and record
      shape of the built-in table, the event signature tables and the per-class option map; descriptions of
      built-ins marked [R]. Every table read here was then read from the player's executable and checked against
      the disc data
  - kind: reference-project
    what: >-
      sly2-decomp @ 33e2bab9: src/splice-bif (vector, matrix, angle wrap, Euler, schedule, cancel), src/spliceobj
      (option lookup and get/set/add/ensure), src/splice-eval-splice-frame (evaluator dispatch, lambda, apply),
      src/splice-bif-splice-eval (operation 133), src/new_132b08 (event posting), nonmatching/splice-bif_120f80
      (operations 72 and 76); SafeHouse @ c70803ce, Sly2-Release/bif.cpp and spliceglue.cpp (cross-check of the
      same operations)
    used_for: >-
      answers of 2026-11-17 to the implementation's questions (sections 1, 3, 4.3, 5, 6); the lookup order of
      duplicate map entries was then simulated on the executable's own lists, the argument forms counted on the disc
  - kind: reference-project
    what: earlier research, tools/sly2_world.py (script block reader)
    used_for: the node payload sizes; re-checked by parsing every world
  - kind: disc
    what: 2026-11-18 action options survey (calls of script IDs 33 to 560 in all modules; option-table entries read after the hash check)
    used_for: the action-options paragraph of section 5
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release smrun.cpp, update.cpp (state-change and playback script handlers)
    used_for: the playback-events paragraph in section 6 (2026-11-19)
  - kind: disc
    what: >-
      executable SCUS_973.16 after the hash check: built-in table at 0x2BBB90, entries 133 (routine 0x125510, at
      least 1 argument, fixed) and 134 (routine 0x1255C0, at least 0 arguments, any number); both routines and the
      symbol lookup at 0x127DB8 read as machine code
    used_for: the op133 search rule, its unfound result and operation 134 in section 4.3 (2026-11-20)
  - kind: reference-project
    what: >-
      sly2-decomp @ 33e2bab9, src/splice-bif-splice-eval/fun_00125510.cpp and fun_001255c0.cpp,
      src/splice-eval-splice-frame (symbol lookup), include/cref.h; SafeHouse @ c70803ce, Sly2-Release frame.h,
      frame.cpp, splicemap.cpp, loaders.cpp, activate.cpp, eval.cpp, gamebif.cpp
    used_for: >-
      cross-check of op133 and op134; where module frames come from and when they gain a parent (section 4.3,
      2026-11-20)
  - kind: disc
    what: 2026-11-20 survey of the 209 op133 calls against the placements' bind indexes in all 43 worlds
    used_for: the survey counts in section 4.3
reviewed_by: pending
date: 2026-11-14
---

# Sly 2 world scripts

Sly 2 keeps Sly 1's script language: modules of a small Lisp-like language stored as syntax trees, evaluated
directly, reacting to events raised by objects and acting through object options
([../sly1/scripts.md](../sly1/scripts.md) sections 1 to 5 and 8 apply unless stated below) [R]. This document lists
what changed. Notation as in the Sly 1 specs; `sN` is symbol N and `opN` built-in operation N.

## 1. Where scripts are

- Scripts live **per package**: each package section starts with a script block ([world-file.md](world-file.md) 4.1),
  not once per world [D].
- A block is a `u32` module count, then one tree per module. Over all 43 worlds: 7,753 packages, 1,558 of them with a
  non-zero module count, 10,930 modules [D].
- An object record's script index ([object-stream.md](object-stream.md), object record) names a module **of its own package**
  [D]: on all 7,138 top-level records that name a module, and on all 10,554 such records at every depth (records
  nested inside solid bodies included), the index is below that package's module count.
- **Nested records attach too.** Records inside a solid body are created by the same object-list reader as top-level
  records, and object creation binds the record's module whatever the depth [R]. So all 10,554 records attach to
  their module, not only the 7,138 top-level ones.
- With the debug flag set (never on the disc), 16 bytes of source position follow each tag byte and three strings
  follow each module [R].

## 2. Cell kinds

Same encoding as Sly 1 (a tag byte, kind = `tag AND 0x7F`, bit 7 = another cell follows, `0xFF` = end of the
sequence), but **the numbering moved up by one** for values and lists [D] [R]:

| Kind | Sly 1 kind | Value | Payload | Cells on the disc |
|---:|---:|---|---|---:|
| 21 | 20 | integer | `s32` | 146,907 |
| 22 | 21 | float | `f32` | 6,850 |
| 23 | 27 | boolean | `u32`, non-zero = true | 5,672 |
| 24 | 28 | symbol | `u32` ID | 154,946 |
| 25 | 29 | built-in operation | `u32` ID | 117,345 |
| 26 | 22 | vector | 3 × `f32` | 0 |
| 27 | 23 | matrix | 16 × `f32` | 0 |
| 28 | 24 | three coefficients | 3 × `f32` | 7 |
| 29 | 25 | range | 2 × `f32` | 0 |
| 30 | 26 | smoothing parameters | 3 × `f32` | 0 |
| 31 | 30 | list | one nested sequence | 302,646 |
| 33 | 32 | object | `u32`, always 0 (the null object) on the disc | 19,874 |
| 0 | 0 | empty list | none | 4,134 |

The payload sizes are confirmed by the walk: every package's script block ends where its material tables begin
[D]. Note that boolean, symbol and built-in now come right after float.

## 3. Special forms

Keyword kinds when they head a list. Counts are list heads on the disc [D]; meanings as in Sly 1 section 4 [R]:

| Kind | Sly 1 kind | Form | Count | Notes |
|---:|---:|---|---:|---|
| 1 | 1 | set | 8,474 | always 3 cells long |
| 2 | 2 | define | 34,763 | always 3 cells long |
| 5 | 5 | assert | 262 | now `(assert expr message)`: the message is only printed in debug builds; no effect in play [R] |
| 6 | — | (new) evaluate one expression, or give "no value" without one | 0 | [R] |
| 7 | 6 | if | 8,257 | 3 or 4 cells |
| 8 | 7 | or | 273 | |
| 9 | 8 | and | 1,423 | |
| 10 | 9 | cond | 130 | |
| 11 | 10 | else | 190 | marker inside `cond` and `case` |
| 12 | 11 | case | 4,772 | |
| 13 | 12 | let | 0 | |
| 14 | 13 | while | 30 | |
| 15 | 14 | lambda | 24,194 | |
| 16 | 15 | begin | 14,644 | |
| 17 | — | (new) gives "no value" | 0 | [R] |
| 18 | 17 | quote | 3,883 | always 2 cells long |
| 19 | — | no special form: the evaluator has no rule for it (a list headed by it would be applied as a call and fail) | 0 | [R] |
| 20 | 19 | **rest-parameter marker** inside a `lambda` parameter list: the parameter after it receives the list of the remaining arguments; the parameters before it are required | 0 | [R] |

Kinds 3, 4, 19 and 20 never occur anywhere on the disc, as heads or as plain cells [D].

**Heads versus cells.** The counts above count lists headed by the keyword. Counting every cell of the keyword's
kind adds exactly two: one `or` cell (274 cells, 273 heads) and one `and` cell (1,424 cells, 1,423 heads), each an
element of a quoted data list, `(quote (... or ...))` in package `i_temple_ext_boss` of the temple hub and
`(quote (... and ...))` in package `npc_goose_halftrack` [D]. Inside a quote a keyword cell is data, not a form.

As in Sly 1, every module is one `begin` holding only `define`s (10,929 modules;
the remaining one is empty) [D].

## 4. Built-in operations

### 4.1 The built-in table (executable)

| Item | Value |
|---|---|
| Virtual address | `0x002BBB90` (file offset = address − `0x100000` + `0x80`) |
| Entry | 12 bytes: `u32` address of the operation's code, `u32` **minimum argument count**, `u32` **more arguments allowed** (0 or 1) |
| Entries | 169 (IDs 0 to 168), followed by a zero word at `0x002BC37C` |

A port does not run the executable's code; it needs the table only to check argument counts. Before reading it the
packager checks the executable's SHA-256 (sources). Every one of the 117,333 built-in calls on the disc matches its
entry's argument rule [D].

### 4.2 Operations used on the disc

97 of the 169 IDs occur [D]. "Args" is from the executable table (`+` = more allowed); "Calls" counts calls on the
disc. Meanings are ours [R] unless marked [I]; where a Sly 1 operation does the same, its Sly 1 ID is given and
Sly 1 section 7 applies.

**Arithmetic, comparison, maths**

| ID | Args | Calls | Operation | Sly 1 |
|---:|---|---:|---|---:|
| 0, 1, 2, 3 | 1+ | 926, 645, 631, 291 | add, subtract, multiply, divide | 0–3 |
| 7 | 2 | 3,385 | integers equal | 7 |
| 8, 9, 10, 11 | 2 | 258, 118, 419, 182 | <, ≤, >, ≥ | 8–11 |
| 12 | 2 | 826 | same value | 12 |
| 13 | 2 | 1 | equal (lists element by element) | 13 |
| 25 | 1 | 197 | is the empty list | 25 |
| 27 | 1 | 1,866 | is the null object | 27 |
| 30 | 3 | 6 | near | 30 |
| 31 | 1 | 3,535 | not | 31 |
| 53 | 2 | 600 | random float in a range | 53 |
| 54 | 1 | 4 | square root | |
| 55, 56 | 1 | 31, 31 | sine, cosine | 55, 56 |
| 59 | 1 | 3 | arc cosine | |
| 61 | 1 | 4 | wrap an angle into −π..π (4.3) | |
| 62 | 2 | 36 | angle from y and x (two-argument arc tangent) | |
| 63 | 2 | 3 | dot product | 63 |
| 65 | 1 | 32 | vector length | 65 |
| 66, 67 | 1+ | 37, 8 | distance, squared distance | 66, 67 |
| 68 | 1+ | 42 | normalise a vector | 68 |
| 72 | 2 | 15 | angle and axis between two vectors (4.3) | |
| 76 | 3 | 3 | interpolate between the rotations of two matrices; position dropped (4.3) | |
| 79 | 1 | 1 | Euler angles of a matrix (4.3) | |
| 81 | 2+ | 5 | look-at matrix | 81 |
| 83 | 2 | 13 | evaluate three coefficients | |
| 85 | 4 | 6 | fit three coefficients | 85 |
| 86 | 2 | 10 | clamp a number to a range | 86 |
| 88, 91 | 1 | 36, 19 | float to integer, discarding the fraction [R] | 91 |
| 90 | 1 | 6 | float to integer, rounding to nearest | |
| 92 | 1 | 43 | absolute value | 92 |
| 93, 94 | 1+ | 106, 132 | maximum, minimum | 93, 94 |
| 95 | 2 | 76 | integer remainder | 95 |
| 96 | 0 | 66 | current game time [R] | 96 |
| 135, 136 | 0+ | 11, 6 | bitwise AND, bitwise OR of integers | |

**Lists and values**

| ID | Args | Calls | Operation | Sly 1 |
|---:|---|---:|---|---:|
| 32 | 2 | 97 | make a pair (an element in front of a list) | |
| 33, 34 | 1 | 635, 139 | first element, rest of the list | 33, 34 |
| 35, 36 | 2 | 3, 3 | replace the first element, replace the rest (in place) | 35 |
| 37 | 1 | 220 | length | 37 |
| 38 | 2 | 901 | n-th element | 38 |
| 39 | 2 | 334 | is an element | 39 |
| 40 | 0+ | 721 | make a list | 40 |
| 41 | 1 | 146 | a shuffled copy of a list [R] | |
| 42 | 0+ | 154 | append lists | 41 |
| 43, 44, 45 | 2 | 218, 177, 1,885 | map, filter, for each | 42, 43, 44 |
| 47, 48 | 0+ | 1,708, 39 | make a vector, make a matrix (argument forms in 4.3) | 46 |
| 49, 50 | 3, 2 | 134, 178 | make three coefficients, make a range | 48, 49 |
| 52 | 1+ | 518 | element of a vector, matrix, coefficients, range or smoothing parameters | 51 |

**Game state, world and objects**

| ID | Args | Calls | Operation | Sly 1 |
|---:|---|---:|---|---:|
| 97 | 0 | 686 | the current world's level-table row ([disc.md](disc.md) 6), an integer [R] | |
| 99 | 0 | 73 | the current episode number, an integer [R] | |
| 100, 101, 102 | 0 | 336, 89, 6 | the **current job**, the **current task** and the **checkpoint task** ([missions.md](missions.md)); each is an object, null when none [R] | |
| 103 | 0 | 80 | the current difficulty help, a float 0 to 1 ([missions.md](missions.md)) [R] | |
| 104 | 0 | 67 | always the integer 0: the routine behind it returns 0 unconditionally in the retail executable (its two instructions at `0x00210B68` are a return with result 0) [D] | |
| 105 | 2+ | 1,824 | schedule a call: delay, procedure, arguments; gives an integer call ID (4.3) | 97 |
| 106 | 1 | 312 | cancel the scheduled call with that ID (4.3) | |
| 107 | 2 | 157 | postpone an object's updates | 98 |
| 108 | 3 | 2,814 | add a custom property to an object | 99 |
| 109 | 3 | 156 | ensure a custom property | 100 |
| 110 | 3 | 11,789 | set an option | 101 |
| 111 | 2 | 52,334 | get an option | 102 |
| 112 | 2+ | 17,969 | find an object by OID | 103 |
| 113 | 2+ | 27 | find all objects with an OID | 104 |
| 114 | 2+ | 565 | find several objects at once: a list of OIDs and a context object; gives a list with one entry per OID, in order (null where nothing is found), searching below the context object, or the whole world when the context is the null object (4.3) [R] | |
| 115 | 2+ | 916 | find the nearest object with an OID | 105 |
| 117 | 0 | 770 | the world object (the object that reads world option list 1) [R] | 108 |
| 118 | 0 | 1,454 | the **sound manager** object (music and sound control; the scripts use the options of class 152 on it) [R]; not the player | |
| 119 | 0 | 194 | the camera object (the object that reads world option list 2) [R] | 109 |
| 120 | 2 | 302 | find all objects of a class | 110 |
| 121 | 2 | 1 | the solid objects whose collision overlaps an axis-aligned box given by two corner points; a list, each object once (4.3) [R] | |
| 123, 124 | 3 | 5, 6 | hit-test a segment against a list of objects (4.3): 123 gives every hit as (object, hit position, surface normal); 124 gives the first hit as (object, surface normal) [R] | |
| 125, 126 | 3 | 55, 51 | convert a position, a direction between object spaces | 115, 116 |
| 129 | 2 | 96 | play a sound effect | 119 |
| 130 | 2 | 265 | controller rumble | 121 |
| 131 | 1 | 9 | read a saved progress value by a path: a quoted list of name symbols that names a save variable ([saving.md](saving.md)) [R]; counterpart of op 132, which writes one. (Not a sound operation.) | |
| 133 | 1 | 209 | the definition of a symbol that the running module replaced: looked up from the parent of the running procedure's module frame; unfound gives operation 134, a no-op (4.3) | |
| 137 | 0 | 802 | the on-screen **prompt** object of the HUD ([user-interface.md](user-interface.md)); scripts call its option 979 to show a prompt; class 154's options [R] | |
| 138, 139 | 1, 2 | 15, 18 | owned-item flags of the item table ([characters.md](characters.md) 6, [saving.md](saving.md)): 138 gives whether item n is owned (false for n = −1); 139 sets item n's flag to a boolean and, when it sets it, raises task event 59 as a purchase does [R]. Not pickpocket items | |

IDs not listed never occur on the disc and may be treated as errors.

### 4.3 Details of the new operations

Argument forms counted on the disc [D]; behaviour [R] from the reference sources of the operations.

- **114, find several objects** [D for the forms]: of the 565 calls, the first argument is a symbol bound to a list
  of OIDs in 450, the result of getting an option in 71, a quoted list of integers in 33, and a list built by op 40
  or op 38 in the rest; the second argument is the null object in 551 calls (search the whole world) and a symbol
  bound to a context object in 14 (search only below it, as op 112 does with a context) [R].
  An empty list gives the empty list [R].
- **121, objects in a box**: two points, the box's opposite corners; the result lists the distinct solid objects
  whose collision geometry meets the box [R]. Its one call passes an object's option 34 (its position) and a vector [D].
- **123 / 124, segment hit tests**: arguments (list of objects, start point, end point). Every collision part of
  every listed object is clipped against the segment, skipping parts marked as not hit-testable [R]. 123 collects
  all hits in list order; 124 stops at the first hit in list order, not the nearest, and returns (object, normal)
  [R]. With no hit both give the empty list [I]. On the disc 124 is always a downward probe of 100 units from an
  object's position (end = position − (0, 0, 100)) [D].
- **131, read a progress value**: its one argument is a quoted list of name symbols, the path into the save tree
  (all 9 calls) [D]; an unknown path gives "no value" [I].
- **138 / 139, owned items**: 138 takes the item index (all 15 calls pass an integer literal, for example 27, the
  Paraglide); 139 takes the index and a boolean (all 18 calls) [D].

- **47, make a vector.** No argument: (0, 0, 0). **One number n: (n, n, n)** (not a copy of a vector). Three numbers:
  (x, y, z). Integers are converted to float. Other argument counts are not handled by the game. On the disc: 258
  calls with none, 205 with one (always a float literal), 1,245 with three; never two or more than three.
- **48, make a matrix.** Starts from the identity. One number n: n on the three diagonal entries of the rotation
  part. Three or four vectors: they become rows 0 to 2 (and 3, the position) in order. Nine or sixteen numbers: a
  3 x 3 or 4 x 4 block filled row by row. Other counts give the identity. On the disc: 8 calls with no argument and
  31 with nine numbers; the other forms do not occur.
- **61, wrap an angle.** An angle already within −π..π (both ends included) is returned unchanged; any other angle a
  becomes ((a + π) mod 2π) − π with a non-negative modulo, which lies in −π..π.
- **72, angle and axis between two vectors.** Arguments: vector A, vector B. Result: a **two-element list**
  (angle, axis): the angle between them in radians, 0..π (from the clamped cosine of their normalised dot product),
  and the unit axis along A × B. If either vector is shorter than a small epsilon: angle 0, axis (1, 0, 0). If A and B
  are parallel (cross product shorter than about 1e-8): some unit vector perpendicular to A. (Earlier drafts called
  this "a direction from angles", which was wrong.)
- **76, interpolate rotations.** Arguments: matrix A, matrix B, a fraction t (read as a float). The rotation that
  takes A's orientation to B's is turned into an angle (0..π) and an axis; A's rotation part is then rotated by the
  fraction t of that angle about that axis. t = 0 gives A's rotation, t = 1 gives B's; other t extrapolate. **The
  result's position row is (0, 0, 0)**: positions are not interpolated. On the disc all 3 calls take A from an option,
  B from a look-at matrix (operation 81) and t from a symbol.
- **79, Euler angles.** Gives a vector (x, y, z) in radians such that the matrix's rotation is a turn by x about the
  X axis, then by y about the Y axis, then by z about the Z axis (in column-vector notation, rotation =
  Rz(z) · Ry(y) · Rx(x), whose columns are the matrix's axis-image rows). y = asin(−m02) in −π/2..π/2;
  x = atan2(m12, m22), z = atan2(m01, m00), where mij is row i, component j. Near the poles (|y| ≥ 1.5707) x is 0 and
  z = atan2(−m10, m20) when y > 0, atan2(−m10, −m20) when y < 0.
- **105, schedule a call.** Arguments: delay (seconds, converted to float), a procedure, then any number of argument
  values. The game builds the call with every argument value quoted (so they are passed as values, not evaluated
  again), queues it at current game time + delay, and returns an **integer call ID**. On the disc: 1,705 calls with
  no extra argument, 75 with one, 40 with two, 3 with three, 1 with four.
- **106, cancel.** Its argument is an ID returned by 105; the queued call with that ID is removed (nothing happens if
  it already ran). Gives "no value".
- **133, the shadowed definition.** The argument is evaluated; its 32-bit payload is used as a symbol ID without
  checking its kind, so a quoted symbol and an integer with the same number behave alike. On the disc all 209 calls
  pass a quoted symbol, always one the same module defines, in the form `((op133 (quote sN)) …)` inside the
  definition of sN. The call reaches the definition of sN that the running module **replaced** [D] [R]:
  - From the frame of the running procedure call, the game walks up through enclosing frames until it reaches the
    first **module frame** (the frame a module was evaluated in). Every frame on the way, the module frame included,
    must have exactly one parent; a frame with none or with more than one ends the search unfound.
  - The symbol is then looked up starting at the module frame's **parent**, not in the module frame itself, with the
    ordinary lookup: that frame's own bindings first, then its parents' and theirs, each frame once. The running
    module's own sN is therefore skipped because its whole frame is skipped. Nothing compares the binding with the
    running procedure.
  - A module frame gets a parent only from a placement that **binds** a module to an object of the copied package
    (the `index` of a fresh block or copy patch, [classes/placement.md](classes/placement.md) 1.1, 1.2). The bound
    module's frame takes the object's previous frame, normally the frame of the object's own module, as its parent,
    then replaces it [R]. A copy patch does this at load. A fresh placement's bind to another target (6 is the first
    object) does it when the object is attached, unless the placement holds its objects back. A fresh placement's
    target-5 bind sets the module with no parent. A module bound to an object that had no frame, and every module an
    object names as its own, has no parent, so in it op133 always comes back unfound.
  - **Unfound gives built-in operation 134 as a value**, not "no value". Operation 134 takes any number of
    arguments, ignores them and gives **"no value"**. So when nothing is found, the
    handler's call of the result does nothing [D] [R]. Operation 134 never appears on the disc.
  - Survey of the 209 calls (all worlds, against the placements' bind indexes) [D]: in 47 calls the bound object's
    own module defines the same sN, so the call runs that handler (33 under copy patches, 14 under fresh placements'
    attach-time binds). In 65 calls the result is operation 134: 27 where the bound object has no module, 38 where
    its module lacks sN. In 7 calls the bind's target object was not located by the survey, and 90 calls sit in
    modules that no placement outside solid bodies binds. Those 97 are uncounted. No module containing op133 is
    bound by more than one placement, and none is the own module of a top-level object record.

## 5. Options from scripts

As in Sly 1 section 8: scripts name options by **script option IDs**, mapped per class to stream option IDs
through a table in the executable [D] [R]:

| Item | Value |
|---|---|
| Class map | virtual address `0x00279AD0`: 199 records of 8 bytes, one per class ID: `u32` count, `u32` address of the class's list |
| Class list | `count` records of 8 bytes: `u32` script option ID, `u32` address of the option's 40-byte entry in the option table ([object-stream.md](object-stream.md) 1.1) |
| Stream option ID | (entry address − `0x002AC880`) / 40 |

Read from the disc's executable [D]: 12,010 list records in all, each list sorted by script option ID; 1,091
distinct script option IDs from 1 to 1,232; stream option IDs 0 to 1,448; 210 script option IDs map to different
stream options in different classes, so the lookup must be per class.

**Duplicate entries.** Five classes list 9 script option IDs twice in a row, each time with two different stream
options [D]. The game looks an ID up by **binary search** over the class's sorted list (halving between the first
and last record, midpoint rounded down) and takes whichever matching record it reaches first [R]. Simulating that
search on the executable's own lists gives [D]:

| Class | Script ID | Positions in the list (list length) | Stream options | Record the search finds |
|---:|---:|---|---|---|
| 88 | 141 | 63, 64 (96) | 833, 63 | first: **833** |
| 98 | 110 | 44, 45 (108) | 1040, 44 | first: **1040** |
| 98 | 111 | 46, 47 (108) | 1043, 45 | first: **1043** |
| 98 | 112 | 48, 49 (108) | 1041, 46 | **second: 46** |
| 98 | 113 | 50, 51 (108) | 1044, 47 | **second: 47** |
| 100 | 183 | 74, 75 (95) | 1103, 74 | first: **1103** |
| 101 | 183 | 74, 75 (95) | 1103, 74 | first: **1103** |
| 102 | 37 | 24, 25 (96) | 1113, 24 | first: **1113** |
| 102 | 183 | 75, 76 (96) | 1103, 74 | first: **1103** |

A port should implement the same binary search, or use this table; "always the first" is wrong for class 98 IDs
112 and 113.

**Placement OIDs name the placed copy.** A placement record (class 6, [classes/placement.md](classes/placement.md))
is not where a script's options live. When a package copy is made for a placement, the copy's **first object** (its
root) answers to the placement record's OID as well as to its own: an object search (ops 112 to 115) for the
placement's OID can find that root, and the options a script then reads or sets are the root's, looked up in the
root's class map [R]. So a script that asks a "class-6" object for an option class 6 does not list is in fact talking
to the copy's root; a port should resolve a placement's OID to its copy's root object for scripts, and give the
placement record itself only the options class 6 lists [R] [I for the port rule]. Which object wins when both match
follows the search rules of op 112 (below the context first, then the nearest shared copy, then the whole world) [R].

**Option 1095 (script ID 214) on noise templates (class 117).** It is a method, not a value: called with a position
(vector) and an owner object (or the null object), it creates a new noise from that template at that position,
owned by that object, and **returns the new noise object** [R]. The option-table entry is value type `0x2093`,
flag `0x1`, two arguments [D]. The returned noise is a run-time object; scripts can then set its investigation point (an
option of the noise object, stream option 1098) and it expires by the template's lifetime ([guards.md](guards.md)
4.3) [R]. It is the most-called method in a 90-frame run of all worlds (3,690 calls, as counted by the script host).

**Custom properties.** The boundary is a fixed number in the game, not a world-file count [R]: getting (111) or
setting (110) an option with ID **below 1,233** (`0x4D1`) goes through the class map; an ID of **1,233 or more**
goes to the object's own property store. Adding (108) and ensuring (109) always use the property store, whatever the
ID. Getting a property that was never added gives "no value". The class map ends at 1,232, so the first custom ID is
1,233; the scripts on the disc happen to start at 1,234: the 1,016 distinct option IDs that scripts pass as literals
to operations 108 to 111 include 664 not present in any class list, from 1,234 to 1,904 [D]. Custom properties may hold procedures: modules add a property with a lambda
and later call it as a method (test vector 7.3) [D].

**Action options.** Options whose table entry has a script routine (offset 16) are methods: `((op111 object id)
arguments...)` fetches and calls them [D]. The ones the game session still lacked are described in
[options-by-class.md](options-by-class.md) 2.1 (option 20 set parent, 24 move to an object, 25 add to the world,
44 and 45 show and hide meshes, 64 texture-scroll speeds, 66, 73 and 74 find a texture animation, clip playback or
state-machine playback, 77 new emitter handle, 106 and 368 apply a clip or a state machine, 146 queue a music
stream, 233 guard aim target, 479 stop an emitter) and 2.2 (the class-113 guard command lists, options 595 to 612).
Starting and stopping a class-114 sequence from a script uses options 613 (script ID 235, with a starter object),
614 (script ID 638, the current character as starter) and 615 (script ID 530, stop): dialogue-and-voice.md 2.1.

## 6. Events

As in Sly 1 section 6, an object raises an event by number; if its module binds the symbol with that number to a
procedure, the procedure is called with the object first [R]. Sly 2 numbers 30 events (symbols 0 to 29) and
keeps their argument signatures in the executable [D]:

| Item | Value |
|---|---|
| Signature index | virtual address `0x0027A1D8`: 30 `s32` start positions (event *n* uses positions start[n] to start[n+1] − 1), then −1. Event 29's start is 51, which equals the number of type words (the type list runs from `0x0027A10C` up to the index, 51 words), so **event 29's signature is empty**; the final −1 only ends the index [D] |
| Signature types | virtual address `0x0027A10C`: `u32` type codes, one per argument, the object itself included |

For every event the disc's handlers define, the handler's parameter count equals the event's signature length,
except 8 handlers with one parameter fewer (events 14, 15 and 25, signature 2) and 3 with one more (event 9,
signature 1) [D]. The argument
type codes are not decoded here.

**How many values a handler receives.** The signature only gives the type of each argument; the number of values
is chosen where the game raises the event, and the handler is called with the object followed by those values [R].
A script procedure binds exactly as many values as it has parameters, without checking how many the call supplies
[R]. Event 9 is raised by trigger volumes (class 24) with **no** value beyond the object [R], so in the original the
three event-9 handlers with a second parameter read past the end of their argument list, with an undefined result.
A port should bind such missing parameters to "no value" [I]; the three handlers are in modules attached to class
24 and 133 objects (table below).

Events handled on the disc: symbol, signature length, module-level definitions, and the classes of the objects
attached to those modules (class IDs of [object-stream.md](object-stream.md) 3) [D]. Meanings are not established
except where marked.

| Event | Args | Handlers | Classes of attached objects (most common) |
|---:|---:|---:|---|
| 0 | 1 | 2,100 | 133, 132, 2, 48, 114, 98 — runs once after loading, as in Sly 1 [I] |
| 1 | 1 | 158 | 81, 29, 2, 48, 73 |
| 2 | 1 | 1,113 | 113, 109 |
| 3 | 1 | 759 | 113, 109 |
| 4 | 2 | 3,036 | 114, 113, 98, 109 |
| 5 | 1 | 1,418 | 114, 24, 42, 83, 98, 81 |
| 6 | 1 | 851 | 114, 24, 103, 27 |
| 7 | 3 | 54 | 58, 57, 61 (guards, swarms) |
| 8 | 3 | 79 | 111 |
| 9 | 1 | 33 | 24, 133 |
| 10 | 3 | 6 | 50, 98 |
| 11 | 3 | 57 | 58, 88, 50, 98 |
| 12 | 3 | 1 | 50 |
| 13 | 1 | 1 | 50 |
| 14 | 2 | 501 | 48, 57, 98, 81, 103, 58 |
| 15 | 2 | 309 | 48, 57, 103, 98, 81, 58 |
| 16 | 1 | 63 | 98 |
| 17 | 3 | 639 | 132, 133 (jobs and tasks): a broadcast **task event**, values (event number, object); missions.md 3.2 [R] |
| 18 | 2 | 27 | 133 |
| 19 | 1 | 3,358 | 133, 132: the task became current / the job began [R] |
| 20 | 1 | 263 | 132, 133: the task stopped being current / the job ended [R] |
| 21 | 1 | 907 | 133: after the task's state changed [R] |
| 22 | 1 | 68 | 133: before the task's state changes [R] |
| 24 | 2 | 64 | 58, 57, 63, 64, 62, 61 (characters) |
| 25 | 2 | 3 | 133, 119 |
| 28 | 4 | 22 | (modules with no attached object) |

**Playback events** [R]: clip playbacks raise 2 (created), 3 (removed) and 4 (a timed type-12 event passed, with
its integer); state-machine playbacks raise 2, 3 and **8 on every state change, with (old state, new state)**. The
79 event-8 handlers are all in modules attached to state machines (class 111) [D]. Details: options-by-class.md
2.4. State machines do not raise events 14 and 15.

Symbols 34, 57 and 58 are also defined at module level (6, 69 and 3 times) but are beyond the event range: module
variables shared by convention [I]. Events 23, 26, 27 and 29 have no handlers on the disc; event 23 is raised on a task by its option 1284 with one integer (missions.md 3.1) [R].

## 7. Test vectors

### 7.1 Module counts per world

Modules / packages with scripts / packages, by world (folder and file names of the extracted set, the level's own
package name last) [D]:

| World | Modules | With scripts | Packages | Level package |
|---|---:|---:|---:|---|
| intro hub (row 0) | 154 | 40 | 239 | `jb_intro` |
| menu world | 1 | 1 | 3 | `dvd_menu` |
| nightclub hub | 378 | 66 | 287 | `f_nightclub_exterior` |
| nightclub cellar | 237 | 28 | 172 | `f_nightclub_cellar` |
| palace hub | 322 | 57 | 243 | `i_palace_ext` |
| prison hub | 442 | 70 | 284 | `p_prison_ext` |
| castle hub | 462 | 68 | 251 | `p_castle_ext` |
| sawmill hub | 414 | 75 | 281 | `c_sawmill_ext` |
| blimp final boss | 226 | 36 | 263 | `a_blimp_boss_final` |
| all 43 worlds | 10,930 | 1,558 | 7,753 | |

### 7.2 Cell totals (all 43 worlds)

Lists 302,646; symbols 154,946; integers 146,907; built-ins 117,345 (117,333 of them as the head of a call);
null objects 19,874; floats 6,850; booleans 5,672; empty lists 4,134; three-coefficient cells 7; plus the keyword
counts of section 3 [D].

### 7.3 Decoded modules (nightclub cellar)

Package `f_nightclub_cellar_rig`, module 12, attached to one object of class 15:

    (begin (define s6 (lambda (s1002) ((op111 s1002 40)))))

On event 6 the object reads its own script option 40 (an action) and runs it.

Package `f_nightclub_macro`, module 10, attached to object 1745 (class 2):

    (begin (define s0 (lambda (s1002) ((op111 (op118) 273) 28))))

On load: call option 273 of the object returned by operation 118 with the argument 28.

Package `f_nightclub_cellar_rig`, modules 0 and 1 (identical; attached to class-24 volumes 136514 and 136516):

    (begin (define s5 (lambda (s1002) ((op111 (op137) 979) 762 5))))

Package `bentley_mine`, module 0 (attached to object 3931, class 101) begins with

    (begin (define s1020 (lambda (s1002) (define s1021 -1)
                          (case (op111 s1002 960) ((5 7) (set s1021 3968)) ((9) (set s1021 3965)) ...

and its load handler adds custom properties 1238 to 1242 holding procedures, for example
`(op108 s1002 1238 (lambda () ((op111 s1002 110) (s1020 s1002))))`.

## 8. Open questions

1. (Resolved: the first custom property ID is 1,233, a fixed threshold in the game; section 5.)
2. The meaning of most event numbers (section 6) and of the event argument type codes.
3. (Resolved: operations 97 to 139 in section 4.2 and 4.3; the copy root answers to its placement's OID, section 5.)
4. Keyword kinds 6 and 17 never occur on the disc; their exact rule matters only for completeness. (Kind 20 is
   the rest-parameter marker and kind 19 has no rule; section 3.)
5. (Resolved: world-file.md 4.1 now gives the same count, 1,558 package bodies with modules.)
6. What the three event-9 handlers expect in their second parameter in play (section 6): to check in an emulator.
