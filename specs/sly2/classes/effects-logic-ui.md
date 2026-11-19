---
id: sly2-classes-effects-logic-ui
title: Sly 2 object stream, emitter, explosion-group, mission and screen-movie layouts
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97316 (USA) disc image; executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 decompressed worlds
    used_for: every body of the classes below, at every depth, parsed field by field to its declared size; option values deciding the emitter mesh; item-kind histograms
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), src/new_2584a8, src/new_258658 (emitter loaders), src/dzg-emitter (explosion-group loader), src/new_161458, src/sensor-geom (mission loaders), src/po-screen, src/screen-font and nonmatching/screen-font (screen-movie loaders)
    used_for: first reading of the field order; every field re-walked on the disc
date: 2026-11-14
---

# Emitter, explosion-group, mission and screen-movie layouts (Sly 2)

Notation as in [../object-stream.md](../object-stream.md); "transform header" as in
[paths-and-volumes.md](paths-and-volumes.md). Every body of these classes ends exactly at its declared size [D].

## 1. Particle emitters (classes 29, 32, 33)

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | transform header, options | | as [simple-layouts.md](simple-layouts.md) 2 |
| 2 | origin mesh | only for a mesh origin (below) | `u16` point count `p`, `p` × `vec3`; `u16` triangle count `t`, `t` × 10 bytes; one `vec3` (as Sly 1's emitter mesh, [../../sly1/classes/shared-structures.md](../../sly1/classes/shared-structures.md) "Emitter mesh") |
| 3 | colour count `c` | `u16` | |
| 4 | colour flag | `u8` | only if `c` > 0 |
| 5 | colours | `c` × 4-byte RGBA | only if `c` > 0 |

**When the origin mesh is present** [D, all 14,733 emitter bodies]:

- **class 29:** exactly when option **483 = 3** (84 bodies). Option 483 is an enumeration of its own (value type
  `0x1022`); values 1 and 2 also occur, without a mesh. Options 490 and 501 do not decide it: 42 of the 84 meshes
  have 490 = 5 and 501, the other 42 have neither (they have 483 = 3 and 495 = 3, one such emitter per world);
- **class 33:** exactly when option **542 = 5** (68 bodies); 542 = 0 (42) and 4 (84) occur without a mesh;
- **class 32:** never (52 bodies).

Then the mesh follows (152 bodies).

At most 32 colours are used; extra ones are read and ignored [R].

## 2. Explosion group (class 31)

Transform header, option list, then an object list of the group's parts [D]. Parts of class 30 and its derived
classes (explosion parts, class 32 and 33) become members of the group; other objects are hidden at load [R].

## 3. Mission objects (classes 132, 133)

Class 132 (*interpretation: a job*): option list, then one `u16`-length string [D].

Class 133 (*interpretation: a task of a job*):

| Order | Field | Type |
|---:|---|---|
| 1 | options | option list |
| 2 | first list | `u8` count, count × `s32` |
| 3 | second list | `u8` count, count × `s32` |
| 4 | name | `u16`-length string |

The options of both classes fill tables (toggles, animations, fail and complete events) that the loader keeps
[R].

**Failed level condition** [R]: a class-132 or 133 record whose level-row list (object-stream.md 2.3) does not
contain the current row is still loaded, but marked inactive, so that mission state can refer to it.

## 4. Screen movies (classes 126 to 131)

The game's 2-D screens (menus, the HUD, the map, text) are small vector movies stored in these bodies.

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | options | option list | |
| 2 | streamed flag | `s8` | non-zero: the movie data is a separate file |
| 3 | frame time | `f32` | seconds per frame: 1/30 (`0x3D088889`) or 1/60 (`0x3C888889`) on the disc [D] |
| 4 | width | `u32` | 640 on the disc [D] |
| 5 | height | `u32` | 480 on the disc [D] |
| 6 | (unknown) | `u32` | |
| 7a | file record | 64 bytes, only if streamed | ([../disc.md](../disc.md) 4) |
| 7b | dictionary, sprites | only if not streamed | below |

Streamed: 258 of 1,659 class-126 bodies; never on classes 127 to 131 [D].

**Dictionary**: `u16` item count `n`, then `n` items. Item: `u16` item number, `u32` kind, then:

| Kind | Payload | Items on the disc |
|---:|---|---:|
| 0, 5 | nothing | 86, 266 |
| 1 | shape: `u16` primitive count; per primitive 5 × `u8` (vertex count `k`, three settings, a "has extra bytes" flag), then `k` × (`u32` RGBA, `u8`, `s16` x, `s16` y) | 20,117 |
| 2 | image: 36-byte 3 × 3 matrix, `u16` count `m`, `m` × 24 bytes | 0 |
| 3 | text: `u32`, `u32`, 3 × `f32`, `s8`, `s8` text kind, then an `s32` only if the text kind is 0 or 1 | 4,093 |
| 4 | `u16` | 8,494 |
| 6, 7 | rectangle: 4 × `s16` (left, top, right, bottom), `s32` object ID | 258, 2 |
| 8 | font: a nested dictionary, `u32` count + that many `u32` character codes, `u32` count + that many `f32` advances | 86 |
| 9 | nothing | 1,251 |

**Sprites**: `u16` sprite count; per sprite: `u16` instance count, `u16` slot count, `u16` label count, then per
label: `f32` duration, `s32` label ID, `u16` key count, then per key:

| Field | Type |
|---|---|
| time | `f32` |
| placements | `u8` count, each `u16`, `u16`, `u16`, `s16`, `u8`, `u8`, `s32`, 36-byte 3 × 3 matrix (50 bytes) |
| colour pairs | `u8` count, each 2 × `u32` |
| 16-bit values | `u8` count, each `u16` |
| bytes | `u8` count, each `u8` |
| actions | `u8` count, each `u8` kind, `s32` argument |

Classes 127, 128 and 130 are identical in every world (3,310, 185,108 and 19,470 bytes) [D].

## 5. Test vectors

Body offsets are in the decompressed world file of the given level-table row ([../disc.md](../disc.md) 7) [D]:

| Class | Row | Body offset | Body size | First bytes |
|---:|---:|---:|---:|---|
| 29 | 0 | 322,259 | 196 | `ff ff 00 00 80 3f …` (no parent, identity matrix) |
| 31 | 0 | 198,957 | 1,065 | `02 10 …` (parent: transform 2 of the enclosing object) |
| 33 | 0 | 146,691 | 187 | `ff ff 00 00 80 3f …` |
| 126 | 0 | 559,213 | 9,531 | `63 04 0f 00 00 00 64 04 01 ff ff 00 89 88 08 3d 80 02 00 00 e0 01 00 00`: options 1123 and 1124, end, not streamed, frame time 1/30, width 640, height 480 |
| 128 | 2 | 1,240,129 | 185,108 | `63 04 0e 00 00 00 64 04 01 6d 04 9d 02 ff ff …` |
| 132 | 0 | 569,153 | 64 | `ee 04 02 00 6e 00 6f 00 …` |
| 133 | 0 | 557,882 | 69 | `0f 05 02 00 cc 00 6f 00 …` |

Bodies parsed field-exactly over all worlds and depths [D]: class 29: 6,230; 31: 1,958; 32: 52; 33: 8,381;
126 to 131: 1,659, 41, 41, 212, 43, 1; 132: 388; 133: 4,080 (as records, including nested ones).
