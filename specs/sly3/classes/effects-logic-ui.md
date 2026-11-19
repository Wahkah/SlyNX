---
id: sly3-classes-effects-logic-ui
title: Sly 3 emitters, effects, lists and screen objects
side: reference
status: draft
sources:
  - kind: disc
    what: re-check of 2026-11-17 on all 39 worlds and SCUS_974.64 (hash checked), answering the formats-sly3 spec questions
    used_for: position-kind option value types; element counts split by nesting; class-157 modes
  - kind: disc
    what: SCUS-97464 image, all 39 worlds
    used_for: every body of these classes parsed to its declared size (one class-157 body excepted); value counts
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly3-Release emitter, effect, list and screen-object loaders
    used_for: first reading of the field order and the element kinds; re-checked by the parse
reviewed_by: pending
date: 2026-11-14
---

# Emitters, effects, lists and screen objects (Sly 3)

Tags as in [../disc.md](../disc.md). "Header + options" as in [simple-layouts.md](simple-layouts.md) 2.

## 1. Emitter (classes 27, 30, 31)

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | header + options | | |
| 2 | emission mesh | only when the emitter's position kind is 3: `u16` count, count × `vec3`; `u16` triangle count, each 3 × `u16` and `f32` area; then a `vec3` | the position kind is the value of option 617 (class 27) or 670 (classes 30, 31): a plain enumeration value (value type `0x101E`, 4 bytes; flags `0x104`, not a typed argument list) [X], whose value 3 selects the mesh [R]; 36 emitters carry a mesh, all with kind 3 [D] |
| 3 | colours | `u16` count `n`; if `n` > 0: `u8`, then `n` × `u32` colours | the game keeps at most 32 [R] |

## 2. Effect (class 45)

| Order | Field | Type |
|---:|---|---|
| 1 | header + options | |
| 2 | component count | `s16` |
| 3 | components | each with an object-record header (mode, filters, conditions, `s16` class, `s32` ID, `s16`, `s32` size; [../object-stream.md](../object-stream.md) 2.2), then `size` bytes |

7,172 effects; their 17,768 components are of classes 191 (15,300) and 192 (2,468). [D] A component body is an option
list, followed by an emission mesh (section 1) when its position kind (option 558, a plain enumeration value of value type `0x101E`, 4 bytes, not a typed argument list [X]) is 3 [R]; all 17,768 component
bodies parse this way to their size. [D] *Interpretation:* particle systems; the component options give position,
velocity, spin, colour and timing. [R]

## 3. Key list (class 143)

An option list, then `u8` count and per entry `s32`, `s8`, `s8`, `s32`, `s32` and a 64-byte file record. 114 bodies,
all in macro groups. [D]

## 4. Stream list (class 151)

An option list; `s32` count, per entry a 64-byte file record, `s32`, `s32`; then `s32` count, per entry an `s8` kind
followed by two `s32` (kind 1), one `s32` (kinds 0 and 2) or nothing. 38 bodies (Sly's character group and the menu
world). [D]

## 5. Pair list (class 157)

An option list, then `u16` count and count × (`s32`, `s32`). 15 bodies, all in the menu world's `all_wayback` group;
14 parse this way. [D] The option list always holds options 1246 (an index, 0 to 8), 1247 (a mode) and 1248, plus
1249 in the six bodies of mode 1; the mode is 0 in 8 bodies, 1 in 6 and **2 in one** (object 165149). [D] Modes 0 and
1 are exactly the pair-list layout above. [D]

The mode-2 body (object 165149, 360 bytes, at `0x31E6B` in the menu world) declares a count of 5, but only its first
pair (`0x2877A`, `0x6BE3B`) looks like the others; the remaining 332 bytes contain fragments of other records of the
same group, for example an end-of-options marker followed by the size 154 and the options of object 165151, and 11 of
its 18 16-byte runs occur elsewhere in the world file. [D] *Interpretation:* mode 2 is a different layout whose
reader we do not know, or the body is padded with stale data like the unread tails of world files
([../world-file.md](../world-file.md) 5). [I] A loader that only needs to walk the world skips it by its size. Its
layout stays open.

## 6. Screen object (classes 158 to 167)

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | options | option list | |
| 2 | external flag | `s8` | |
| 3 | (unknown) | `f32`, `u32`, `u32`, `u32` | the two middle words are the canvas size, for example 640 × 480 [R] |
| 4 | external: file record | 64 bytes | the element data lives in a separate file (234 bodies) [D] |
| 4 | inline: element set, group tree | 6.1, 6.2 | 2,172 bodies [D] |

### 6.1 Element set

`u16` count, then count × (`u16` index, element). An element is `u32` kind, then [D for the sizes]:

| Kind | Data | Occurrences |
|---:|---|---:|
| 1 | `s32`; `u16` polygon count; per polygon `u8` vertex count `v`, 4 bytes, then `v` × (`u32` colour, `u8`, `s16` x, `s16` y) | 25,062 (17,652 in element sets, 7,410 as glyphs inside kind 8) |
| 3 | `u32` colour, 3 × `f32`, `s8`, `s8` text kind, `s8`; `s32` text id when the text kind is 0 or 1 | 9,531 |
| 4 | `s32`, `u16` group reference | 9,497 |
| 6, 7 | 4 × `s16` rectangle, `s32` texture id | 232, 47 |
| 8 | `u8`; `u16` count, per entry `u16` character, `f32` advance, then a nested element | 117 |
| 0, 2, 5, 9 | nothing | 78 (all as glyphs inside kind 8), 0, 279, 1,526 |

The occurrence counts include the elements nested as glyphs inside kind-8 elements; only kinds 1 and 0 occur
there. [D] *Interpretation:* kind 1 vector polygons, 3 text boxes, 4 references to a group of 6.2, 6/7 textured quads, 8 fonts
(characters with widths and glyphs). [R]

### 6.2 Group tree

`u16` group count; per group `u16`, `u16`, `u16` part count; per part `f32`, `s32`, `u16` key count; per key `f32`
time, `u8` item count `i`, `u8` colour count `c`, `u8` slot count `w`, `u8`, `u8`, `s32`, then `i` items (`u16`, `u8`,
`u8`, `u8`, `u8`, `s32`, nine `f32`), `c` colour pairs (2 × `u32`), `w` × `u8`. [D for the sizes] *Interpretation:*
keyframed 2-D animation (the user-interface movies, Sly 2's screen movies). [R]

External screen files begin `ZRB.` and a `u16` version, then the same element set and group tree. [R]
