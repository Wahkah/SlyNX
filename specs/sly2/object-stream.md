---
id: sly2-object-stream
title: Sly 2 world file, object stream
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2: Band of Thieves, USA, SCUS-97316, disc image 2,884,141,056 bytes; executable SCUS_973.16 (LBA 271,
      2,597,048 bytes, SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73); all 43
      decompressed worlds
    used_for: >-
      every layout below was checked by a scratch walker that reads every package section of all 43 worlds to the
      end (42,097 object records, each consumed exactly to its declared size); option lists inside 11,250 solid
      bodies, 12,532 top-level simple-layout bodies, the two world-level lists and 1,475 placement option blocks were parsed
      option by option; counts and histograms come from that walk
  - kind: executable
    what: SCUS_973.16, option table at virtual address 0x002AC880 (read as data)
    used_for: the value type and argument count of each option ID
  - kind: reference-project
    what: >-
      earlier reference-side research: Games/Sly2-RE-notes.md and tools/sly2_world.py, sly2_geom.py (2026-11);
      sly2-decomp @ 33e2bab9 (2026-11-12), nonmatching/new_13f888 (object list loader), src/sm-so (solid loader)
    used_for: >-
      first reading of the record framing, the record-kind rules, the placement layout and the option encoding;
      every layout re-walked on the disc
  - kind: disc
    what: same disc and worlds, every-depth body walk on 2026-11-14
    used_for: value type 22 and argument tag 15 counts, the option-26 count, the corrected test-vector offset
date: 2026-11-13
---

# Sly 2 world file: the object stream

Sly 2 keeps Sly 1's idea of a stream of object records with option lists ([../sly1/object-stream.md](../sly1/object-stream.md)),
but the world is split into **package sections** (see [world-file.md](world-file.md)) and every object record now
carries its **own byte size**. That one change means a loader can step over any object it does not understand.
This document starts where world-file.md hands over: the option lists at world level and the object list at the
end of each package section. Notation as in the Sly 1 spec, section 1, plus:

- **transform reference**: an `s16` naming a transform of an object's transform list (transforms.md 2): −1 none,
  `n` (0..0x0FFF) the object's own transform `n`, `0x1000 | n` transform `n` of the enclosing object.

Tags: [D] disc data, [R] reference research, [I] inference, [O] observation (as in
[../sly1/gameplay-overview.md](../sly1/gameplay-overview.md)).

## 1. Option lists

Same shape as Sly 1 (Sly 1 section 4.1): `s16` option ID, value, repeated; a negative ID (−1, `FF FF`) ends the
list [D]. Differences:

- The encoding of each ID comes from the **option table in the executable**, not from a table in this spec
  (section 1.1) [D].
- Typed argument lists use a different tag format (section 1.3) [D].

Option lists occur: twice at world level (world-file.md), in every solid body (classes/solid-object.md), in the
simple layouts (classes/simple-layouts.md), and as size-prefixed blocks inside placements (classes/placement.md) [D].
464 distinct option IDs occur on the disc, the highest 1,448 [D].

### 1.1 The option table (executable)

| Item | Value |
|---|---|
| Virtual address | `0x002AC880` (file offset = address − `0x100000` + `0x80`) |
| Entry size | 40 bytes, entry *n* for option ID *n* |
| Entry +0 | `u32` **value type** (section 1.2) |
| Entry +4 | `u32` flags; if bit `0x400` is set the value is a **typed argument list** (section 1.3) |
| Entry +24 | `s32` **argument count** for typed argument lists |

The packager reads the table from the player's own executable after checking its SHA-256 [D]. Test vectors: see
section 6.

### 1.2 Value types (fixed-size values)

Only these types occur in values on the disc [D]; each was confirmed by the walk (a wrong size would break it):

| Value type | Bytes | Occurrences |
|---|---:|---:|
| 0 | 1 | 8,842 |
| 1 | 4 | 10,670 |
| 5 | 4 | 717 |
| 6 | 4 | 4,243 |
| 13 | 4 | 465 |
| 2 | 12 | 5,906 |
| 9 | 12 | 26 |
| 7, 14, 15, 16 | 2 | 250, 6, 43, 115 |
| 10 | 8 | 2,899 |
| 22 (`0x16`) | 2 + 2 × n: a `u16` count n, then n × `u16` | 21,952 (n is always 2); only options 1262 (class 132) and 1295 (class 133), both with flags `0x80` |
| `0x1000` to `0x1FFF` (an enumeration number in the low bits) | 4 | 6,834 in 26 types |
| `0x2000` to `0x2FFF` | 4 | 5,210 in 18 types |

*Working assumption* [I], from the Sly 1 meanings of the same sizes: 0 boolean, 1 float, 2 and 9 vectors, 5 and
6 integers, 7 and 14 to 16 16-bit identifiers, 10 a range, 13 a packed colour. Any other value type is treated
as corrupt input [D].

### 1.3 Typed argument lists

When the entry's flag bit `0x400` is set, the value is the entry's argument count `k` of arguments (0 to 7 and 10 occur), each:

| Field | Type | Notes |
|---|---|---|
| tag | `u16` | bit 15 set: a **list** of values of type `tag & 0x7FFF` |
| count | `u8` | only for a list |
| value(s) | | one value (or `count` values) of the size below |

| Tag (low 15 bits) | Bytes per value | Occurrences on the disc |
|---|---:|---:|
| 0 | 1 | 42,091 |
| 1, 5, 6 | 4 | 40,421, 2,012, 80,045 (+448 lists) |
| 2 | 12 | 24,494 |
| 7, 14, 15, 16, 19, 20, 21 | 2 | 443, 491, 5,904, 5,720, 22, 145, 1,544 (tag 15 only in mission options, classes 132 and 133) |
| 10 | 8 | 291 |
| `0x1000` to `0x1FFF` | 4 | 33,306 in 25 tags (+2,794 lists of tag `0x1010`) |

No other tag occurs [D]. Typed argument lists occur 94,253 times [D].

### 1.4 One option with a load-time effect

Option 26 (a typed argument list with no arguments) marks an object to be **removed when the level starts**
(broken pieces, spare pickups, gadget parts) [R] (from the earlier research, which found it from the executable's
option-table entry). It occurs 6,505 times: 6,167 in object option lists, 338 in placement option blocks [D].
A static render should leave these objects out [R].

## 2. Object lists and object records

### 2.1 Object list

| Field | Type | Notes |
|---|---|---|
| count N | `s16` | |
| N object records | | |

### 2.2 Object record

| Field | Type | Notes |
|---|---|---|
| record kind `k` | `s8` | −2, −1, 0, or a positive count (section 2.3) |
| condition list | `k` × `s16` | only when `k` > 0: level-row numbers |
| class ID | `s16` | section 3 |
| object ID | `s32` | (Sly 1: `s16`) |
| script index | `s16` | −1 for none |
| body size | `s32` | bytes of body that follow |
| body | | layout chosen by the class (section 3); **always exactly `body size` bytes** [D] |

Kinds on the disc: 0 (31,906 records), positive (8,390), −2 (1,684), −1 (117) [D]. The body size is correct for
every record, including placements (8,791 checked) [D], so a loader can always skip a body it does not need.

### 2.3 Record kinds

- **0**: an ordinary object; load it [D for the count, R for the rule].
- **Positive `k`**: load the object only if the current **level row** (the row of the world in the level table,
  disc.md) is one of the `k` listed rows, or if no level is current [R]. Otherwise skip the body; except that
  records of class 132 or 133 are loaded even then, flagged as inactive [R]. Classes with conditional records on
  the disc: 2, 3, 26, 48, 113, 114, 116, 123, 126, 129, 132, 133 [D].
- **−1 and −2**: an object that belongs to a package that is cloned (placement.md): it is skipped if an object
  with the same object ID already exists in the current copy (−1) or anywhere in the shared set (−2), otherwise
  loaded into that copy or set [R].

### 2.4 Where object records appear

| Context | Framing |
|---|---|
| End of each package section (world-file.md) | object list (2.1) |
| End of every solid body (classes/solid-object.md) | object list: the children of that object |
| Inside a "copy" placement (classes/placement.md) | object list: extra objects for that copy |
| Inside an explosion group body (classes/effects-logic-ui.md 2) | object list: the group's parts |

### 2.5 Walking a whole world

1. Read the world prolog and each package section as world-file.md says, using section 1 for the two world-level
   option lists.
2. For each package's object list, read each record header and skip its body by its size. Nothing else is needed
   to reach the end of the package section.
3. To use an object, parse its body with the layout of its class (section 3). Bodies of the solid layout contain
   nested object lists (2.4).

All 43 worlds walk to the end with this rule [D].

## 3. Class table

Class IDs 2 to 133 occur. "Layout" names the on-disc body layout:

- **solid**: classes/solid-object.md (transform list, mesh set, collision, options, children);
- **solid + name**: the solid layout followed by one `u16`-length name string (classes/solid-object.md 6);
- **placement**: classes/placement.md;
- **options**: an option list only (classes/simple-layouts.md);
- **header + options**: transform header and option list (classes/simple-layouts.md);
- **curve**, **track**, **path network**, **triangle surface**, **zone**, **volume**, **warp**, **header + options + name**: classes/paths-and-volumes.md;
- **emitter**, **explosion group**, **mission**, **screen movie**: classes/effects-logic-ui.md;
- **clip**, **state machine**: classes/animation.md.

Counts are records in package object lists, including the extra objects of top-level copy placements ("top"), and
objects nested in solid bodies ("nested") over all 43 worlds; records in explosion-group lists and in the extra
lists of copy placements nested in solids are not in these two columns (worlds.md 5 gives every-depth totals); "worlds" is the number of worlds where the class occurs [D]. Descriptions are ours, from the
package names and name strings in the data [D] or the earlier research [R]; "?" marks a guess.

| Class | Layout | Top | Nested | Worlds | Description |
|---:|---|---:|---:|---:|---|
| 2 | options | 177 | 1 | 42 | settings holder |
| 3 | header + options | 1,869 | 2,746 | 42 | transform marker (rigs) |
| 4 | header + options | 2,676 | 179 | 41 | light (every body carries light options 151 and 152; effects.md) |
| 5 | zone | 30 | 0 | 12 | zone volume (packages named `…_vz`) |
| 6 | placement | 8,551 | 8,057 | 43 | prefab placement |
| 7 | placement | 240 | 0 | 8 | prefab placement (rig packages) |
| 9 | triangle surface | 8 | 1 | 2 | ? |
| 10 | curve | 2,349 | 576 | 37 | curve (paths, rails) |
| 11, 12, 13 | curve | 180, 42, 261 | 66, 1, 92 | 23, 15, 22 | curve variants |
| 15 | track | 378 | 241 | 41 | ? |
| 17 | path network | 974 | 12 | 40 | navigation mesh ? |
| 14 | curve | 14 | 165 | 12 | obstacle along a curve ? |
| 16, 21, 36, 40, 45, 47 | header + options | 0 | 22, 5, 204, 1, 252, 4 | | only nested |
| 23 | volume | 0 | 42 | | only nested |
| 120 | options | 0 | 422 | | only nested |
| 18, 19, 20 | header + options | 3, 41, 84 | 99, 30, 0 | 1, 6, 42 | markers (20: hand-adjust points of the gang) |
| 22, 24, 25, 27 | volume | 13, 320, 2, 38 | 386, 162, 1, 121 | 5, 39, 2, 15 | volumes (24, 25: volumes acting as buttons ?) |
| 26 | warp | 369 | 1 | 43 | warp volume with destination worlds |
| 29 | emitter | 2,506 | 3,724 | 42 | particle effect (packages named `pe_…`) |
| 31 | explosion group | 176 | 1,782 | 42 | explosion (packages named `explode_…`) |
| 32 | emitter | 2 | 50 | 2 | explosion part |
| 33 | emitter | 1,686 | 1,287 | 42 | explosion part (flashes, gas) |
| 35, 37, 38, 39, 43, 44, 46 | header + options | 94, 2, 12, 128, 306, 17, 20 | 63, 13, 0, 41, 786, 165, 34 | | markers and volumes ? |
| 41, 42 | header + options + name | 222, 271 | 6, 11 | 38, 42 | named markers ? |
| 48 | solid | 2,339 | 6,384 | 43 | solid object (props, level geometry pieces) |
| 49 | solid | 118 | 0 | 42 | level geometry (the large static mesh of each level) |
| 50 | solid + name | 12 | 0 | 12 | controllable machine (names `powerchop_player`, `rc_chopper_root`, `TANK_DRIVEABLE_ROOT`, `attract_mode_player`) |
| 53 | solid + name | 42 | 0 | 42 | player character Sly (name `jt`) |
| 54 | solid + name | 42 | 0 | 42 | Murray (name `murray`) |
| 55 | solid + name | 42 | 0 | 42 | Bentley (name `bentley`) |
| 57 | solid + name | 66 | 0 | 34 | swarming creatures |
| 58 | solid + name | 28 | 0 | 28 | guard (name `guard`) |
| 59 to 68 | solid + name | 2, 9, 3, 6, 7, 6, 2, 3, 1, 1 | 0 | | characters and bosses (names `npc_dimitri`, `neela`, `npc_rajan`, `npc_jean_bison`, `npc_carmelita`, `npc_boss_contessa`, `npc_arpeggio`, `npc_bear`, `npc_bear_cub`, `dancer`) |
| 69, 70, 72 to 78 | solid + name | 2, 8, 5, 26, 7, 5, 0, 1, 8 | 4, 0, 0, 0, 0, 0, 22, 0, 0 | | turrets, safe house, minigame actors (`hack_…`), tank, vault |
| 79, 81 to 83, 85, 86, 88, 89 | solid | 81, 132, 20, 68, 72, 51, 8, 41 | 19, 0, 0, 24, 49, 58, 0, 17 | | throwables, projectiles, missiles, buttons, lasers, ?, barrels, ? |
| 91 to 96 | solid | 42, 17, 42, 38, 8, 3 | 0 | | pickups: coin, key, medicine, pickpocket item, clue, minigame critters |
| 97 | solid | 19 | 0 | 18 | sky dome |
| 98 | solid | 574 | 43 | 41 | switchable prop (most prefab props) |
| 99 to 108 | solid | 38, 127, 42, 42, 41, 11, 0, 3, 55, 2 | 0, 0, 0, 0, 0, 0, 252, 7, 0, 0 | | dispenser, gadget objects, mine, trap, loot, effects buffer, ?, ?, mission waypoints, ammo |
| 109 | clip | 255 | 21,633 | 27 | animation clip |
| 111 | state machine | 39 | 2,564 | 11 | animation state machine |
| 112 to 117, 119, 121, 123, 124 | options | 60, 880, 3,545, 32, 136, 8, 32, 1, 2, 2,407 | 83, 35, 326, 17, 5, 482, 0, 0, 0, 22 | | settings and logic objects (macro and puzzle-zone packages) |
| 126 to 131 | screen movie | 1,465, 41, 41, 212, 43, 1 | 194, 0, … | | user-interface screens (`swf_ui`, `dvd_menu`); classes 127, 128 and 130 have the same size in every world (3,310, 185,108 and 19,470 bytes) |
| 132, 133 | mission | 388, 4,080 | | | jobs and tasks (macro packages) |

Only classes with the solid layout carry drawable geometry [D]. The record count over all worlds is 42,097 [D].

## 4. Object identity

- Object IDs are `s32` and not unique across packages [D]; references between objects are by object ID within the
  owning package or copy [R].
- The script index refers to the package's script block (world-file.md) [R].

## 5. Differences from Sly 1 at a glance

| Item | Sly 1 | Sly 2 |
|---|---|---|
| Object record | 6-byte header, no size | record kind, conditions, `s32` object ID, `s32` body size |
| Option encoding | table in the spec (option-encodings.md) | table in the executable (1.1) |
| Typed arguments | `s16` tag | `u16` tag with a list bit and count |
| Children | at the end of the placed layout | at the end of the solid layout |
| Per-level variants | none | conditional records (2.3) and conditional meshes (geometry.md) |

## 6. Test vectors

- Option table entry 26 (option ID 26) starts at virtual address `0x2AC880 + 26 × 40 = 0x2ACC90`, file offset `0x1ACD10`
  (address − `0x100000` + `0x80`, section 1.1) [D].
- In world row 3 (`f_nightclub_cellar`, the level package starts at `0x7A2120`): the level package's object list
  has 289 records; its first record is the bytes `00 31 00 01 00 00 00 ff ff da 01 26 00` (kind 0, class 49,
  object ID 1, no script, body size 2,490,842) with the body at `0x878C33` [D].
- The cellar level package's class histogram: class 4 × 91, 6 × 186, 13 × 1, 43 × 10, 49 × 1 [D].
- World-wide counts per world are in [worlds.md](worlds.md).
- **Field-exact walk**: with the layouts of section 3, all 121,153 object bodies of the 43 worlds, at every depth,
  parse field by field to exactly their declared size, with no failures [D]. What is counted where: worlds.md 5.

## 7. Open questions

- Every class that occurs has a specified layout; every body at every depth parses field by field to its declared size (121,153 bodies, worlds.md 5) [D]. Several class roles remain guesses marked "?".
- The roles of many classes are guesses ("?").
- The exact rule for records of classes 132 and 133 whose condition fails.
