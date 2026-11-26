---
id: sly3-scripts
title: Sly 3 world scripts - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 - Honor Among Thieves, USA, SCUS-97464; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338 (checked); all 39 decompressed worlds
    used_for: >-
      every script block of every group parsed into trees (7,151 groups, 11,007 modules); keyword heads and list
      lengths, cell and operation histograms; the built-in table, the per-class option map and the event signature
      tables read from the executable and checked against the scripts; every top-level command script (class 135)
      parsed to its declared size; all test vectors
  - kind: disc
    what: Sly 2 executable SCUS_973.16 (SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73)
    used_for: the Sly 2 built-in table, compared entry by entry with Sly 3's to support the ID correspondence
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce (2026-11-12), Sly3-Release: ref.h, eval.cpp, bif.cpp, serialize.cpp, spliceglue.cpp,
      game/scpt3.h, game/scpt3.cpp, game/scptrun3.h, game/scptrun3.cpp, game/gen/globalmap.inc
    used_for: >-
      first reading of the keyword renumbering, the built-in table's address and its correspondence with Sly 2's,
      the option-map and event-table addresses, the custom-property threshold, and the meaning of the command-script
      commands and condition kinds; every address and count was then read from the player's executable and checked
      against the disc (the correspondence by argument rules, see 4.2)
  - kind: reference-project
    what: earlier research, Games/tools/sly3_world.py (script block reader) and Games/Sly3-RE-notes.md
    used_for: the cell payload sizes; re-checked by parsing every world
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 world scripts

Read [../sly2/scripts.md](../sly2/scripts.md) first. Sly 3 keeps Sly 2's language and evaluator; the cell kinds,
keyword kinds and built-in IDs are **renumbered**, the per-class option map and the event tables move, and Sly 2's
sequence objects become **command scripts** with a tree of commands (section 7). Tags as in [disc.md](disc.md):
[D] disc data, [X] executable data at an address, [R] reference behaviour, [I] inferred.

## 1. Where scripts are

As Sly 2 section 1, per **group** instead of per package: each group body starts with a script block
([world-file.md](world-file.md) 4.1). [D]

| Item | Count |
|---|---:|
| Worlds | 39 |
| Groups | 7,151 |
| Groups with at least one module | 1,524 |
| Modules | 11,007 (none empty) |
| Top-level object records naming a module | 6,698, every index within its group's module count |

The implementation's count of 10,631 records naming a module at every depth (nested records included) is consistent
with this; nested records attach to their module as in Sly 2 [R]. No world sets the debug flag. [D]

## 2. Cell kinds

Same encoding as Sly 1 and 2. **Value kinds above 18 are Sly 2's minus 2** [D] (all 39 worlds parse to the end of
every script block with these payload sizes):

| Kind | Sly 2 kind | Value | Payload | Cells on the disc |
|---:|---:|---|---|---:|
| 19 | 21 | integer | `s32` | 228,405 |
| 20 | 22 | float | `f32` | 17,141 |
| 21 | 23 | boolean | `u32`, non-zero = true | 8,496 |
| 22 | 24 | symbol | `u32` ID | 258,696 |
| 23 | 25 | built-in operation | `u32` ID | 180,961 |
| 24 | 26 | vector | 3 × `f32` | 2 |
| 25 | 27 | matrix | 16 × `f32` | 0 |
| 26 | 28 | three coefficients | 3 × `f32` | 31 |
| 27 | 29 | range | 2 × `f32` | 60 |
| 28 | 30 | smoothing parameters | 3 × `f32` | 1 |
| 29 | 31 | list | one nested sequence | 463,142 |
| 31 | 33 | object | `u32` (the null object, 0) | 25,553 |
| 0 | 0 | empty list | none | 6,556 |

## 3. Special forms (keyword kinds)

The keyword kinds 1 to 18 are **Sly 2's with two kinds removed**: Sly 2's kind 4 and kind 17 have no Sly 3
counterpart, so Sly 2's 5 to 16 become 4 to 15, and Sly 2's 18 to 20 become 16 to 18 [R]. The disc agrees with
every row (counts are lists headed by the keyword outside quoted data; "length" counts the cells of such a list,
the keyword included) [D]:

| Kind | Sly 2 kind | Sly 1 kind | Form | Heads | Lengths on the disc |
|---:|---:|---:|---|---:|---|
| 1 | 1 | 1 | set | 20,132 | always 3 |
| 2 | 2 | 2 | define | 54,209 | always 3 |
| 3 | 3 | 3 | (rejected by the reader; never on the disc) | 0 | |
| 4 | 5 | 5 | assert: `(assert expr message)`, no effect in play | 585 | always 3 |
| 5 | 6 | — | evaluate one expression, or give "no value" | 0 | |
| 6 | 7 | 6 | if | 12,018 | 3 (9,708) or 4 (2,310) |
| 7 | 8 | 7 | or | 492 | 2 to 6 |
| 8 | 9 | 8 | and | 3,358 | 2 to 8 |
| 9 | 10 | 9 | cond | 579 | 2 to 7 |
| 10 | 11 | 10 | else (marker in `cond` and `case`) | 699 | mostly 2 (474), up to 6 |
| 11 | 12 | 11 | case | 7,386 | 3 to 10 and more |
| 12 | 13 | 12 | let | 26 | 3, 4 or 15 |
| 13 | 14 | 13 | while | 153 | always 3 |
| 14 | 15 | 14 | lambda | 31,321 | 3 and more |
| 15 | 16 | 15 | begin | 17,146 | 2 and more |
| 16 | 18 | 17 | quote | 6,924 | always 2 |
| 17 | 19 | — | no rule (rejected by the reader; never on the disc) | 0 | |
| 18 | 20 | 19 | **rest-parameter marker** in a `lambda` parameter list | 0 | |

- Every module is one `begin` (kind 15) holding only `define`s (kind 2, three cells each). [D]
- `lambda` parameter lists: 27,220 are lists, 4,101 are the empty list. [D]
- Quoted data: 9,279 `quote` cells in all, 2,355 of them inside quoted lists; one `and` cell sits inside quoted data
  (3,359 cells, 3,358 heads). [D]
- Kinds 3, 5, 17 and 18 never occur on the disc, as heads or as cells. [D]

## 4. Built-in operations

### 4.1 The built-in table (executable)

| Item | Value |
|---|---|
| Virtual address | `0x00342030` (file offset = address − `0x100000` + `0x1000`) |
| Entry | 12 bytes, as Sly 2: `u32` code address, `u32` minimum argument count, `u32` more arguments allowed (0 or 1) |
| Entries | **178** (IDs 0 to 177), followed by a zero word at `0x00342888` |

[X] Read after the executable's SHA-256 check. Every built-in call on the disc obeys its entry's argument rule
except **one call of 31 (not) with two arguments** [D]; a port should evaluate the extra argument and ignore it, as
Sly 2's evaluator does for surplus arguments [I].

**Calls.** 178,038 lists headed by an operation (outside quoted data) call 116 different IDs; the 180,961 cells of
kind 23 also count operation cells used as values or inside quoted data. [D]

### 4.2 Correspondence with Sly 2

Sly 3 inserted 11 new operations and dropped 2; every other ID is a Sly 2 ID shifted by a constant per range [R]:

| Sly 2 IDs | Sly 3 IDs | Shift |
|---|---|---:|
| 0 to 45 | 0 to 45 | 0 |
| 46 to 53 | 47 to 54 | +1 |
| 54 to 87 | 56 to 89 | +2 |
| 88 to 95 | 92 to 99 | +4 |
| 96 | 101 | +5 |
| 97 | 103 | +6 |
| 98, 99 | none | |
| 100 to 104 | 104 to 108 | +4 |
| 105 to 119 | 111 to 125 | +6 |
| 120 to 139 | 128 to 147 | +8 |
| 140 to 168 | 149 to 177 | +9 |

New in Sly 3: 46, 55, 90, 91, 100, 102, 109, 110, 126, 127, 148. [X]

Evidence [X]: under this correspondence the argument rule (minimum count and "more allowed") of the Sly 3 entry
equals that of the Sly 2 entry for 161 of the 167 corresponding pairs; the 6 that differ are game operations
(Sly 3 138, 160, 161, 162, 171, 172), none of them changing the meaning of a call on the disc. The argument
counts used on the disc (4.3) fit the Sly 2 meanings throughout.

### 4.3 Operations used on the disc

"Args" is the executable's rule (`+` = more allowed); "Calls" counts calls on the disc [D]. Where the Sly 2 ID is
given, Sly 2 section 4.2 and 4.3 apply unchanged [R]; otherwise the meaning is ours [R] unless marked [I].

**Arithmetic, comparison, maths**

| ID | Args | Calls | Operation | Sly 2 |
|---:|---|---:|---|---:|
| 0, 1, 2, 3 | 1+ | 1,536, 1,474, 1,033, 431 | add, subtract, multiply, divide | 0–3 |
| 7 | 2 | 5,505 | integers equal | 7 |
| 8, 9, 10, 11 | 2 | 1,055, 259, 1,167, 444 | <, ≤, >, ≥ | 8–11 |
| 12 | 2 | 1,364 | same value | 12 |
| 19 | 1 | 4 | is a vector | 19 |
| 25 | 1 | 651 | is the empty list | 25 |
| 27 | 1 | 4,771 | is the null object | 27 |
| 30 | 3 | 13 | near | 30 |
| 31 | 1 | 7,676 | not (one call has a surplus argument, 4.1) | 31 |
| 54 | 2 | 719 | random float in a range | 53 |
| 55 | 0 | 2 | the largest float [R] | new |
| 56, 57, 58 | 1 | 6, 49, 51 | square root, sine, cosine | 54–56 |
| 59 | 1 | 2 | tangent | 57 |
| 61 | 1 | 3 | arc cosine | 59 |
| 63 | 1 | 13 | wrap an angle into −π..π | 61 |
| 64 | 2 | 72 | two-argument arc tangent | 62 |
| 65, 66 | 2 | 25, 21 | dot product, cross product | 63, 64 |
| 67 | 1 | 63 | vector length | 65 |
| 68, 69 | 1+ | 58, 153 | distance, squared distance | 66, 67 |
| 70 | 1+ | 80 | normalise a vector | 68 |
| 71 | 2 | 5 | component along a normal | 69 |
| 73 | 3+ | 1 | launch velocity to reach a point under gravity [R] | 71 |
| 74 | 2 | 8 | angle and axis between two vectors | 72 |
| 78 | 3 | 1 | interpolate rotations (position dropped) | 76 |
| 81 | 1 | 12 | Euler angles of a matrix | 79 |
| 83 | 2+ | 61 | look-at matrix | 81 |
| 84 | 1 | 1 | tilt a matrix upright [R] | 82 |
| 85, 86 | 2, 3 | 38, 19 | evaluate three coefficients; evaluate them within a range [R] | 83, 84 |
| 87 | 4 | 1 | fit three coefficients | 85 |
| 88 | 2 | 139 | clamp a number to a range | 86 |
| 89 | 2 | 26 | is a number within a range [R] | 87 |
| 90, 91 | 4 | 29, 1 | smooth a float, smooth a vector toward a goal (value, goal, rate, time step) [R] | new |
| 92, 94, 95 | 1 | 29, 46, 11 | float to integer: down, to nearest, discarding the fraction [R] | 88, 90, 91 |
| 96 | 1 | 139 | absolute value | 92 |
| 97, 98 | 1+ | 154, 162 | maximum, minimum | 93, 94 |
| 99 | 2 | 86 | integer remainder | 95 |
| 100 | 3 | 43 | linear interpolation a + t·(b − a) [R] | new |
| 101 | 0 | 684 | current game time | 96 |
| 102 | 0 | 36 | a second clock (the real-time clock [I]) | new |
| 143, 144 | 0+ | 11, 16 | bitwise AND, bitwise OR | 135, 136 |

**Lists and values**

| ID | Args | Calls | Operation | Sly 2 |
|---:|---|---:|---|---:|
| 32 | 2 | 373 | make a pair | 32 |
| 33, 34 | 1 | 1,975, 538 | first element, rest of the list | 33, 34 |
| 35, 36 | 2 | 16, 8 | replace the first element, replace the rest | 35, 36 |
| 37 | 1 | 825 | length | 37 |
| 38 | 2 | 2,629 | n-th element | 38 |
| 39 | 2 | 533 | is an element | 39 |
| 40 | 0+ | 6,895 | make a list | 40 |
| 41 | 1 | 594 | a shuffled copy | 41 |
| 42 | 0+ | 275 | append lists | 42 |
| 43, 44, 45 | 2 | 363, 796, 3,625 | map, filter, for each | 43, 44, 45 |
| 46 | 2 | 39 | sort a list with a comparison procedure [R] | new |
| 47 | 1 | 153 | evaluate a value as an expression [R] | 46 |
| 48 | 0+ | 2,577 | make a vector (forms of Sly 2 4.3: 0, 1 or 3 numbers; on the disc 1,098 / 78 / 1,401) | 47 |
| 49 | 0+ | 72 | make a matrix (Sly 2 4.3; on the disc 0, 9 or 16 numbers) | 48 |
| 50, 51 | 3, 2 | 173, 4,709 | make three coefficients, make a range | 49, 50 |
| 52 | 3 | 22 | make smoothing parameters [R] | 51 |
| 53 | 1+ | 962 | element of a vector, matrix or the like | 52 |

**Game state, world and objects**

| ID | Args | Calls | Operation | Sly 2 |
|---:|---|---:|---|---:|
| 103 | 0 | 661 | the current world (its index) | 97 |
| 104, 105 | 0 | 1,460, 390 | fixed game objects | 100, 101 |
| 107 | 0 | 170 | the player's adaptive-difficulty value | 103 |
| 108 | 0 | 47 | an integer game-state value (always 0 in the release build [R]) | 104 |
| 109 | 0 | 76 | the world's current condition ([world-file.md](world-file.md) 3.7, 6) [R] | new |
| 110 | 2 | 19 | **scaled count**: (object, integer n) gives n, or, when a game-wide scaling is active and applies to the object, `round(n × k)` limited to 1..n, with k a factor kept in the game state (an integer result) [X: the code at `0x125B90` and the routine it calls at `0x284828`] [I: that k is a difficulty factor] | new |
| 111 | 2+ | 491 | schedule a call (Sly 2 4.3) | 105 |
| 112 | 1 | 313 | cancel a scheduled call | 106 |
| 113 | 2 | 112 | postpone an object's updates | 107 |
| 114 | 3 | 3,910 | add a custom property | 108 |
| 115 | 3 | 81 | ensure a custom property | 109 |
| 116 | 3 | 17,281 | **set an option** | 110 |
| 117 | 2 | 68,435 | **get an option** | 111 |
| 118 | 2+ | 18,869 | find an object by OID | 112 |
| 119 | 2+ | 5 | find all objects with an OID | 113 |
| 120 | 2+ | 1,949 | another object search | 114 |
| 121 | 2+ | 828 | find the nearest object with an OID | 115 |
| 123 | 0 | 859 | **the world object** (see below) | 117 |
| 124 | 0 | 1,568 | a fixed object (see below) | 118 |
| 125 | 0 | 135 | the camera object | 119 |
| 126 | 0 | 32 | a fixed object whose script options are those of class 155 [D] | new |
| 127 | 0 | 203 | the first object of a world list, or "no value" when it is empty [R] | new |
| 128 | 2 | 345 | find all objects of a class | 120 |
| 129, 130 | 2 | 25, 8 | find objects inside a box, inside a sphere | 121, 122 |
| 131, 132 | 3 | 19, 24 | objects hit by a segment: all, first | 123, 124 |
| 133, 134 | 3 | 108, 94 | convert a position, a direction between object spaces | 125, 126 |
| 137 | 2 | 116 | play a sound effect | 129 |
| 138 | 2+ | 109 | controller rumble | 130 |
| 139 | 1 | 27 | stop a sound | 131 |
| 140 | 2 | 8 | set a **progress (save) variable**: the first argument names it, the second is the value [R: Sly 2's 132 in the reference's correspondence table] | 132 |
| 141 | 1 | 225 | symbol value in the running procedure's defining environment | 133 |
| 145 | 0 | 237 | a fixed game object (its options 1149 and 1150 are read, the class-158 screen objects' options) [D] | 137 |
| 146, 147 | 1, 2 | 122, 66 | is a pickpocket item collected; mark it collected | 138, 139 |

**The world object.** The objects returned by 123, 124 and 125 are recognised by which options scripts read from
them [D]: the script option IDs read from 123's object (1002, 984, 983, 1000, 989, …) occur only in the option map
of class 1, and the world option list ([world-file.md](world-file.md) 3.6) holds stream options 983 and 988, which
only class 1's map contains. So 123 returns the world object, matching Sly 2's 117. 124's options (290, 291, 280,
…) occur only under class 194 and 125's (759, 1344) under class 198, classes that are created at run time and never
stored in a world [D]. *Note:* the reference port names 123 and 124 the other way round; the disc evidence above
decides for 123 = world.

IDs not listed never occur on the disc.

## 5. Options from scripts

As Sly 2 section 5, with new addresses [X]:

| Item | Value |
|---|---|
| Class map | virtual address `0x002ED518`: **249** records of 8 bytes, one per class ID 0 to 248: `u32` count, `u32` address of the class's list |
| Class list | `count` records of 8 bytes: `u32` script option ID, `u32` address of the option's 40-byte entry in the option table ([object-stream.md](object-stream.md) 1.1) |
| Stream option ID | (entry address − `0x0032CA80`) / 40 |

Read from the disc's executable [X]: 16,803 list records, every list sorted by script option ID; 1,587 distinct
script option IDs from 1 to 1,815; stream option IDs 0 to 2,043; 237 script option IDs map to different stream
options in different classes.

**Duplicate entries.** Eleven script option IDs appear twice in a row in one class's list, each time with two
different stream options [X]. With Sly 2's lookup (binary search, midpoint rounded down, first matching record
reached; Sly 2 section 5 [R], assumed unchanged [I]) the record found is:

| Class | Script ID | Stream options (first, second) | Found |
|---:|---:|---|---|
| 74 | 216 | 1565, 92 | **second: 92** |
| 88 | 939 | 1968, 937 | first: 1968 |
| 88 | 940 | 1965, 938 | first: 1965 |
| 100 | 145 | 883, 65 | first: 883 |
| 111 | 111 | 1138, 44 | **second: 44** |
| 111 | 112 | 1141, 45 | **second: 45** |
| 111 | 114 | 1139, 47 | first: 1139 |
| 111 | 115 | 1142, 48 | first: 1142 |
| 115 | 39 | 1209, 25 | **second: 25** |
| 136 | 39 | 1047, 25 | **second: 25** |
| 137 | 39 | 1047, 25 | **second: 25** |

**Custom properties.** The threshold is a fixed number in the game: script option IDs **up to 1,815 (`0x717`)** go
through the class map, **1,816 and above** go to the object's own property store [R]. The class map ends at 1,815
[X], so the first custom ID is 1,816; the scripts on the disc start at 1,817: of the 1,531 distinct option IDs that
scripts pass as literals to 114 to 117, 960 are in no class list, from 1,817 to 2,786 [D].

**Placement OIDs name the placed copy (as in Sly 2).** The Sly 2 rule (Sly 2 [scripts.md](../sly2/scripts.md) 5)
holds in Sly 3:

- [X] When a placement record (class 8, [classes/placement.md](classes/placement.md)) is loaded, its own OID is
  stored in its group's list of copies, in the row given to that copy (the routine at `0x001FAFB0`: the group entry is
  108 bytes, its OID list at +24 and its copy count at +12). Each copy then gets a namespace node keyed by group
  number × 65,536 + row (`0x001FA5A8`). When the node is made (`0x001FA640`) the routine `0x001FA5D0` looks that key up
  and gives the node the placement's OID (node +8). So the copy as a whole carries the placement's OID.
- [R] An object search (operations 118 to 121) for the placement's OID finds the copy's **root** object. Options
  that a script then reads or sets belong to the root and are looked up in the root's class map. The placement
  record itself has only the options class 8 lists.
- [D] Check: the script host reports 810 `get option` failures (class 8, option 133) in slots 31, 33 and 34 without
  this rule, and none with it.

A port should resolve a placement's OID to its copy's root for scripts. If a copy root and an ordinary object
answer to the same OID, the search order of operation 118 decides, as in Sly 2 [R].

## 6. Events

As Sly 2 section 6, with new tables [X]:

| Item | Value |
|---|---|
| Signature index | virtual address `0x002EDDC0`: 34 `s32` start positions for events 0 to 33 (0, 1, 2, 3, 4, 6, 7, 8, 11, 14, 15, 16, 17, 19, 22, 25, 28, 29, 31, 33, 34, 37, 39, 40, 41, 42, 43, 45, 47, 49, 50, 51, 55, 56). **No −1 ends it**: unrelated addresses follow |
| Signature types | virtual address `0x002EDCE4`: 55 `u32` type codes, up to the index |

Event *n*'s signature runs from start[n] to start[n+1] − 1. Events 0 to 31 fit inside the 55 type words; event 32's
single position (55) is the first word of the index itself, and event 33 starts past the end, so its signature is
empty [X] [I for how the game treats them].

Handlers on the disc (module-level definitions of the event's symbol as a procedure) [D]:

| Event | Signature | Handlers (parameter count) | Commonest classes of the objects that carry them |
|---:|---:|---|---|
| 0 | 1 | 2,978 (1) | 135, 169, 168, 2, 47 |
| 1 | 1 | 278 (1) | 92, 91, 47 |
| 2, 3 | 1 | 1,216, 1,107 (1) | 134 |
| 4 | 2 | 4,521 (2) | 135, 134 |
| 5 | 1 | 1,881 (1) | 135, 21, 92, 91 |
| 6 | 1 | 1,080 (1) | 135, 21, 116 |
| 7 | 3 | 29 (3) | 58 |
| 8 | 3 | 201 (3) | — |
| 9, 10, 11 | 1 | 32, 1, 2 (1) | 21 |
| 12 | 2 | 227 (2) | 114, 47, 91 |
| 13, 14, 15 | 3 | 57, 85, 1 (3) | 52, 58, 47 |
| 17, 18 | 2 | 844, 572 (2) | 47, 91, 58 |
| 19 | 1 | 88 (1) | 111 |
| 20 | 3 | 1,096 (3) | 169, 168, 170 |
| 21 | 2 | 16 (2) | 169 |
| 22, 23, 24 | 1 | 1,056, 754, 833 (1) | 169, 168 |
| 27 | 2 | 43 (2) | 58, 60 |
| 28 | 2 | **5 (1)**: one parameter fewer | 168 |
| 30 | 1 | 6 (1) | 90 |
| 31 | 4 | 5 (4) | — |
| 32 | 1 | 5 (1) and **5 (5)** | 70, 135 |

Every handler's parameter count equals its signature length except event 28's five handlers (one fewer) and five of
event 32's (four more). As in Sly 2, a procedure binds as many values as it has parameters; a port should bind
missing parameters to "no value" [I]. The type codes are not decoded here (`0x2002` is the commonest, 17 of 55).

## 7. Command scripts (class 135)

Sly 3's equivalent of Sly 2's sequence objects. The body layout is in
[classes/animation.md](classes/animation.md) section 3: an option list, then `s16` command count, then per command
`s8` command ID, `s32` object ID (−1 for none), `s8` flags, `u16` first child, `u16` next sibling, and the command's
arguments. All 5,863 top-level command scripts parse to their declared size (548 more are nested); they hold
196,663 commands, at most 502 in one script, and every script's first command is `0x00` [D].

### 7.1 How a script runs [R]

The commands form a **tree**: the first-child and next-sibling fields are indices into the script's command list
(only their low 11 bits are used; 0 = none). Starting a script makes a **runner** on its first command. A runner
executes a command, then its children in order, then moves to the next sibling. Flag bit 0 is a **wait flag**: a
command that carries it keeps the runner on it until the command's own completion test passes (a dialogue line
ended, a walk arrived, a timer ran out). Commands `0x00`, `0x04` and `0x06` open blocks; several runners can run
side by side in one script, and the script stops when none is left. Time is game time, or the real-time clock
when the script's flags ask for it.

Block structure on the disc [D]: `0x00` and `0x01` occur equally often (13,840), as do `0x02` and `0x03` (1,936)
and `0x04` and `0x05` (1,550); `0x06` and `0x0A` (2,754 each) bracket every conditional, each with exactly one
`0x07` (2,754), optional further branches `0x08` (347) and an optional `0x09` (1,270). The second of each pair
closes the block the first opens [I].

**Conditions** (the first argument of `0x04`, `0x07` and `0x08`) [R; counts D]:

| Kind | Meaning | Uses |
|---|---|---:|
| 1886 (`0x75E`) | always true | 1,114 |
| 1887 | always false | 5 |
| 1888 to 1890 | a test on the object named by the condition's second word (looked up by OID, `0x0017E1B8`), in `0x00226950` [X]: **1888** its +0x54 equals 1. On a task that is the play counter, so 1888 means "the first time this task has been current" [I]. **1889** its mode (+0xFC) is 0, live play. **1890** its mode is 1, being restored (missions.md 4.1a) | 1,673 |
| 1891, 1892 | an alarm of this script is set / is not set | 86, 101 |
| 1894 | no dialogue line is playing | 132 |
| 1895 | a dialogue line is playing | 24 |
| any other value | asks the script's own module (its event handler, with the value) | 1,516 |

### 7.2 Commands

Counts [D]; meanings [R], in our words. IDs not listed occur rarely (fewer than 100 uses) or never and are listed
in section 9.

| ID | Uses | What it does |
|---|---:|---|
| `0x00` / `0x01` | 13,840 | start a block as a new runner (the object ID names the runner) / block end |
| `0x02` / `0x03` | 1,936 | a block that runs when the script finishes / its end |
| `0x04` / `0x05` | 1,550 | repeat the children while a condition holds / end |
| `0x06`, `0x07`, `0x08`, `0x09`, `0x0A` | 2,754, 2,754, 347, 1,270, 2,754 | if: the first branch whose condition holds, else the `0x09` branch; `0x0A` ends it |
| `0x0B` | 2,330 | stop runners: those with a parent, this one, or the one named |
| `0x0C` | 2,970 | wait until none of the named runners is left |
| `0x0D` | 1,557 | rendezvous: the named runners wait for each other |
| `0x0E` | 2,737 | wait until an alarm is set |
| `0x0F`, `0x10` | 411, 206 | set, clear an alarm of this script or of another script |
| `0x11`, `0x17` | 758, 1,691 | stop, start another script |
| `0x12` | 868 | activate one object, by its class (7.3) [X] |
| `0x13` | 1,331 | a job command |
| `0x14` | 674 | set the default target object |
| `0x15` | 6,171 | call this script's module with message 4 and a value (script hook) |
| `0x16` | 2,134 | play an animation on a character, or apply an animation-segment object to an object (7.3) |
| `0x18` | 5,390 | set the runner's context object |
| `0x19` | 5,248 | drive an object's state machine to a goal (snapping when the byte is set) |
| `0x1A`, `0x1B` | 314, 226 | show, hide a screen object |
| `0x1D` | 1,127 | a time-scale ramp |
| `0x1E` | 3,679 | set a task's state as at a checkpoint |
| `0x1F` | 4,387 | pause or resume the run |
| `0x20` | 564 | (a block marker) |
| `0x21` | 315 | (meaning not established) |
| `0x22` | 5,950 | place the target on an object, or start an interaction |
| `0x26`, `0x28` | 46, 15 | move a character along its path |
| `0x29` | 1,114 | set the target character's interaction object |
| `0x2A`, `0x2C` | 40, 137 | (character interactions) |
| `0x2F` | 15,281 | wait a random time between a minimum and a maximum |
| `0x30` | 5,135 | walk a character toward an object until within a radius |
| `0x31` | 1,136 | (a run flag) |
| `0x33`, `0x34` | 955, 348 | a character's movement step along a path or to an object's aim point |
| `0x36` | 1,962 | move a character to an object, or where it stands |
| `0x38`, `0x39` | 5,083, 4,545 | add an object to the world, remove it (fading in or out) |
| `0x3A` to `0x3D` | 130, 146, 77, 70 | (object state switches) |
| `0x3F` | 957 | jolt the camera (waits 0.5 s) |
| `0x40`, `0x41` | 711, 440 | (a timed effect); a character command |
| `0x43` | 987 | (meaning not established) |
| `0x44` | 15 | (meaning not established) |
| `0x45` | 23,653 | a camera key: the camera moves from key to key |
| `0x46`, `0x47` | 278, 278 | open, close the binocucom |
| `0x4A` | 648 | binocucom camera targets |
| `0x4E` | 35 | show a button prompt and wait for the buttons |
| `0x52` | 12,729 | play a dialogue line (line list, quiet flag, sound source, fall-off start and end, volume) |
| `0x53`, `0x54` | 49, 2,011 | stop the dialogue; clear the subtitle |
| `0x55`, `0x56` | 4,222, 3,998 | hold, release a character's talk effects |
| `0x57`, `0x58` | 135, 680 | (meaning not established) |
| `0x59`, `0x5A` | 155, 27 | push, pop a music track |
| `0x5F` | 1,169 | wait until no dialogue line is playing |
| `0x60`, `0x61` | 1,133, 424 | show, clear a hint |
| `0x62`, `0x63` | 2,441, 353 | give a guard commands |
| `0x68` | 472 | fire an exit |
| `0x69` | 360 | put the player back at the nearest mission waypoint (7.3) [X, partly I] |
| `0x6A` to `0x6C` | 79, 95, 172 | (meaning not established) |
| `0x6E` | 41 | close the target character's binocucom |
| `0x6F`, `0x70` | 955, 878 | take, release an instance of a placement |
| `0x75`, `0x76`, `0x77` | 378, 1,331, 1,476 | (meaning not established); (meaning not established); turn a character to face an object |

The example dialogue command of section 8 shows the arguments of `0x52`.

### 7.3 Commands traced in the executable

The runner's command switch is at `0x00226EA0`. It reads the command byte and jumps through a 120-entry table at
`0x00326650`. The handlers below are `0x002271C8` (`0x12`), `0x00227454` (`0x16`) and `0x00228CC8` (`0x69`) [X].
"The run's target" is the object that `0x14` / `0x18` set.

**`0x12`, one argument `w`: an object ID, not a class** [X]. The handler finds the object and tests its class in
this order, using the first match:

| Class | What activating does |
|---:|---|
| 93 (contact trigger) | calls a method of its sub-object at +0x21C |
| 21 | calls a method of its sub-object at +0x20C |
| 39, 40 | applies the checkpoint (`0x00280010`) |
| 24 (exit) | fires the exit |
| **138** | **starts the screen fade** (`0x0017D7D0`), below |
| 77, 113 | each has its own start routine (`0x0019E1F8`, `0x002775B8`) |
| 33, 71, 80 | act together with the run's target, and only when that is a character (class 51): a method of the object with the character (33), `0x001E8E40` (71), `0x001716F8` (80, the safe) |
| any other | raises script event 5 on the object (`0x0013BC80`) |

Script 2061's argument 2062 is a class-138 object, so that command **starts a screen fade**. Class 138 (863 bodies,
group `h_macro` and others) is the Sly 3 form of Sly 2's fade object [R]. It holds three colours and three times:

| Option | Field | Meaning [R] | Commonest values [D] |
|---:|---|---|---|
| 1074, 1075, 1076 | +0x40, +0x44, +0x48 | start, middle and end colours (packed RGBA) | 0x01000000 ×764; 0x80000000 ×494; 0x01000000 ×780 |
| 1077, 1079, 1078 | +0x4C, +0x50, +0x54 | the fade-in, hold and fade-out times in seconds (in field order) [I for the order] | 0.02 ×295; 0.05 ×295; 0 ×305 |
| 1080 | +0x5C | a kind (1 ×835, 2 ×27, 0 once) | |

Starting does nothing while the fade's in-world flag (bit 0 of byte +0x2F) is set. Otherwise it writes the game
time (`0x0036BB98`) to +0x58 and adds the object to the world through its vtable [X]. While in the world, the fade
covers the screen. Over the first time it blends from the start colour to the middle colour, then holds the middle
colour, then blends to the end colour over the third time. When all three times have passed, it removes itself
[R: Sly 2's behaviour].

**`0x16`, arguments `wwb`: animation, object, flags; the command's flag byte makes the run wait** [R], with the
dispatch [X]. If the second argument is a character (class 51), the first argument is a **clip ID**. The clip plays
on that character as its script animation, with flags `b | 1` and priority 5. With the wait flag, the run waits for
the clip to end. Otherwise the first argument is an **animation-segment object** (class 130). It is applied to the
second argument when that is a class-47 object, or applied on its own when it is not. With the wait flag, the run
waits until the segment retracts.

**`0x69`, no arguments: acts on the run's target** [X]:

- If the target is a character (class 51), `0x001528E0` runs. It does nothing in two cases: one particular game
  mode (mode 1 with sub-state 35), or when the character's test at `0x001569B8` is set. Otherwise it looks up
  entry 42 with `0x002459F8` and starts it (`0x00244F08`) [I: a sound or screen effect]. Then it does the same as
  for a job.
- If the target is a job or the player record's owner (class 49), the player record is found (`0x001F4A80`,
  records of 4,544 bytes from `0x0036E740`), and `0x00168670(record, 0)` runs.
- `0x00168670` stores the game time in the game state and in the record (+0x444). In mode 1 it also counts (+0x440)
  and broadcasts task event 79. It then gathers up to 16 class-120 waypoints in the world. It keeps those that are
  active (`0x00282750`) and whose mask +0x1A0 shares a bit with the player's mask (default 255). Kinds 0 and 1 are
  candidates. Kind 2 is skipped. Kind −1 is kept as a fall-back, used only when no other candidate is found. It
  then takes the nearest one. It hands that waypoint, and a point 300 × a returned scale above it, to
  `0x00222C50`.

*Interpretation:* `0x69` **returns the player to the nearest suitable mission waypoint**. It is a reset used after
falls and failed sections [I]. The record field names are not established.

### 7.4 Module environments and copied groups [X] [D]

- **One script block per group, loaded once per world load.** The world loader (`0x0014F440`, the reference's
  LoadSwFromBrx) loops once over the groups (`0x0014F750` to `0x0014F9C0`). Per group it calls the script-block
  loader `0x002499F0` once. That loader evaluates each module into a new frame (`0x002BA9A8`), keeps the frames
  in a block {key, count, frames}, and makes the block current (world +0x3F18; block list +0x3F1C). The current
  block is cleared after the group (`0x0014F870`).
- **Objects bind to the shared module frame.** New objects (`0x00150690`, PloNew) look up frame n of the current
  block (`0x00249B78`). They store it in their environment slot with a reference-counted assignment
  (`0x002BAA50`), not a copy. Proxy overrides (`0x001FA778`) do the same, then make the frame's parent the
  object's previous frame. So the objects of a group's **first copy** share that module's top-level variables.
  Later copies get their own cloned frames (next item).
- **Group copies clone their objects, and the clone clones the environment** (correction; matches the Sly 2
  finding) [X].
  - After a group's objects are loaded and its block is cleared, the world loader runs `instances − 1` more
    passes (`0x0014F8D8` to `0x0014F91C`, counting to the group's instance count at +0x8). Each pass clones every
    object of the group with `0x001507E0` (the reference's PloCloneLo).
  - PloCloneLo makes a new object (`0x00150690`) and calls the source's clone method, vtable slot 44 (+0x160).
    The base clone (`0x001C8BF0`) clones the source's frame with `0x00129BE0` (CFrame::CloneTo): the same
    parents, the bindings copied, and procedures re-closed over the new frame (CProc::CloneTo, `0x0012ACC8`). It
    then stores that new frame in the clone's environment slot (`0x002BAA50`).
  - Pool leases clone the same way: placement instances taken with script ID 825 / command `0x6F` go through
    `0x001FBB08`, and `0x001FB408` → `0x001FB494` clones the frame.
- So **each copy gets its own copy of the module's top-level variables**, cloned from the first copy's frame
  at the time of cloning. The first copy keeps the frame of the block load. For v_gauntlet's `generator_rig`
  (2 instances, taken by placements 139785 and 140134), the spawner's s1856 / s2324 … are separate per copy, and
  each copy's event 0 `set`s its own. A port must clone the module frame (bindings copied, closures re-bound)
  for every object copy and every pool lease, rather than sharing it.

## 8. Test vectors

All from v_apt (world 5, decompressed SHA-256 `4b8beaf0de21ccf5a3694b9feced105a6dd0a970bcd09be5f1adeccb05f3f017`,
8,320,220 bytes) [D]:

- 22 groups carry scripts, 211 modules.
- Group 9 (`v_macro`) has 144 modules; its script block's module count is at `0xCCA5C`. Its module 1 starts at
  `0xCCC65`, 40 bytes, first bytes `1d 8f 1d 82 96 00 00 00 00 1d 8e 9d 16 d8 04 00`, and reads
  `(begin (define s0 (lambda (s1240) ((op117 (op124) 290) 46))))`: the handler of event 0 gets option 290 of the
  object from 124 and calls it, as a method, with 46. Object 2055 (class 2) names this module.
- Group 9, module 117 at `0xDA05F`, 63 bytes:
  `(begin (define s24 (lambda (s1240) ((op117 (op117 s1240 1295) 2717) s1240 140196 (quote (140197))))))`, named by
  object 2447 (class 169): event 24 reads the object's option 1295, then that object's custom property 2717
  (a procedure), and calls it.
- Command script OID 2088 (class 135, group `v_macro`; record header at `0xEAA2A`, bytes `01 01 00 03 00 87 00 28 08 00 00
  ff ff 50 00 00 00`: mode 1, world filter (3), class 135, OID 2088, no module, size 80; body at `0xEAA3B`) has no
  options and 4 commands:
  `0x00` (argument 0, first child 1) → `0x2F` (wait flag, an 8-byte time range and −1) → `0x52` (wait flag; line
  list (1587), quiet 1, source −1, fall-off 300 to 3,000, volume 1.0) → `0x01`.

Whole disc [D]: 11,007 modules; 178,038 operation calls of 116 IDs; 6,924 quote heads; 5,863 top-level command
scripts with 196,663 commands.

## 9. Open questions

- Which game state the factor of operation 110 comes from, and the fixed objects returned by 104, 105, 124, 126 and 145 (only
  their option classes are known).
- The six game operations whose argument rules changed from Sly 2 (4.2): whether their meanings changed too.
- The event type codes; what the game passes to event 28's and event 32's handlers whose parameter counts differ.
- The commands marked "not established" in 7.2, and the rarer commands (uses in brackets) [D]: `0x1C` (77),
  `0x23` (91; cut the camera [R]), `0x2B` (16), `0x2D` (16), `0x2E` (50), `0x32` (65), `0x35` (74), `0x37` (38),
  `0x3E` (66), `0x42` (13; close or open a character's eyes [R]), `0x44` (15), `0x4B` (78; binocucom zoom [R]),
  `0x4D` (49; who is on the binocucom screen [R]), `0x4F` (59), `0x50` (5; show the title and wait [R]), `0x51`
  (67), `0x5B` to `0x5E` (170, 28, 22, 22), `0x6D` (38), `0x72` (49), `0x73` (12), `0x74` (4). Never used: `0x25`,
  `0x27`, `0x48`, `0x49`, `0x4C`, `0x64` to `0x67`, `0x71`.
- Whether Sly 3's option lookup on duplicate entries is still the binary search of Sly 2 (assumed).
