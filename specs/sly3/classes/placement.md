---
id: sly3-classes-placement
title: Sly 3 prefab placement layout
side: reference
status: draft
sources:
  - kind: disc
    what: re-check of 2026-11-17 on all 39 worlds and SCUS_974.64 (hash checked), answering the formats-sly3 spec questions
    used_for: placement counts, parents, modes and binds by context
  - kind: disc
    what: SCUS-97464 image, all 39 worlds
    used_for: every placement body (19,192 at every depth) parsed to its declared size; the counts below
  - kind: reference-project
    what: earlier research (Games/Sly3-RE-notes.md, tools/sly3_world.py, 2026-11); SafeHouse @ c70803ce, Sly3-Release placement loader
    used_for: the fresh/copy rule, the bind targets and the matrix composition; marked [R]
  - kind: disc
    what: re-check of 2026-11-17, all 39 worlds, every placement at every depth with the mode-1 selection of a fresh load
    used_for: section 2.1 (copy placements against copy counts; fresh placements against copy counts)
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly3-Release/loaders.cpp (placement loader), sw.h (group record)
    used_for: copy versus fresh, the error when no copy is left, streaming proxies borrowing copies; marked [R]
reviewed_by: pending
date: 2026-11-14
---

# Prefab placement layout (Sly 3)

Class 8 ([../object-stream.md](../object-stream.md) 3.2) places a copy of a **group**
([../world-file.md](../world-file.md) 3.2), like Sly 2's classes 6 and 7
([../../sly2/classes/placement.md](../../sly2/classes/placement.md)). 8,121 top-level records; 11,061 more at other
depths (11,044 nested in solid bodies, 17 in the extra objects of copy placements; fresh-placement data holds none). [D] Tags as in [../disc.md](../disc.md).

## 1. Layout

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | group id | `s32` | the id of a group in the world's group list |
| 2 | mode | `s8` | −1, 0, 1 or 2 |
| 3 | parent | `s16` transform reference | −1 on all 8,121 top-level placements; `0x1000` \| n (a transform of the owning solid) on all 11,044 placements nested in solid bodies; `0x2000` (the owner on top of the stack, as Sly 2) on all 17 placements in copy extras [D] |
| 4 | matrix | 48 bytes | three axis images and a translation |
| 5 | zone mask | `u32` | |
| 6 | bind count | `u8` | top-level: 0 (6,733), 1 (1,081), 2 (267), 3 (25), 4 (6), 5 (9); the 17 in copy extras have none; in solid bodies: 0 (6,302), 1 (4,426), 2 (299), 3 (11), 4 (2), 5 (4) [D] |
| 7 | binds | each: `s32` target, `s16` class ID, `s16` index, `u16` size, then an option list of exactly `size` bytes | |
| 8 | rest | 1.1 or 1.2 | |

The placement is **fresh** when its mode is above 0, or −1 with a group whose streaming flag is set; otherwise
(mode 0, or −1 with a non-streaming group) it is a **copy**. [R for the rule; D that it parses] Counts [D]: top-level copies 6,657 (mode −1: 6,637, mode 0: 20) and fresh 1,464 (mode −1 with a streaming group:
850, mode 1: 289, mode 2: 325); copy extras: 7 copies and 10 fresh (mode −1); solid bodies: 9,082 copies and 1,962
fresh. (An earlier version gave 1,474 fresh, 860 mode −1 and 6,743 without binds: those added the 10 fresh
copy-extra placements to the top-level counts.)

**Binds are the proxy overrides** [D] [X]. Each bind entry is (target, class ID, module index, size, options):

- the target object takes the options (custom properties through option 8, removal through 27, and so on);
- when the **module index is not −1**, the target is also attached to **module n of the placement's own group**,
  not the placed group.
- The loader (`0x001FA778`, the reference's LoadProxyFromBrx) looks the module up in the current block
  (`0x00249B78`). The module's frame takes **the object's previous frame as its parent**, and the object's
  environment becomes the module frame (scripts.md 7.4).
- A handler in the override can reach the object's original handler with built-in 141 (the symbol's value in
  the defining environment), as `(op141 (quote s5))` does.
- **Example** (v_canal, group `v_canal_canal_chase`). Placements 139842, 139845 and 139848 place copies of
  `v_boat_enemy`, each with two binds:
  - target 7 (the placement) gets option 8 (2736 = 2330, 2738 = 139853 / 139852 / 139851) and option 27
    (removed at the start);
  - target 8 (the copy's first object, the enemy boat) is class 2 with **module index 5, 6 or 7**, plus option 8
    (2737 = 150689, 150687 or 150685, the enemy's crew placement).
  - Modules 5 to 7 are the overrides that request enemy tasks 2345, 2343 and 2341 on event 5 (missions.md 4.1a).

**Bind targets** (Sly 2's patch targets): 7 names the placement itself and 8 the copy's first object (Sly 2: 5 and
6) [R]; other values are object IDs within the copied group. On the disc: 7 (796), 8 (592), object IDs (the rest). [D]

### 1.1 Fresh placement

| Field | Type |
|---|---|
| size `S` | `s32` |
| data | `S` bytes: an object list (2.1 of the object-stream spec), which the game skips [R]; it parses as one [D] |

### 1.2 Copy placement

| Field | Type |
|---|---|
| (unknown) | `s32` |
| extra objects | object list: objects added to this copy, placed by this placement's matrix |

## 2. Behaviour

- A copy placement takes the group's next unused copy and places it at the matrix: each object's own transform is
  followed by the placement matrix (row-vector order: `local × placement`). Placements inside a placement's object
  list compose. [R]
- Objects carrying option 27 are removed at level start, also inside copies. [R]
- Mode 2 (325) places spawners, created later by gameplay. [R]

### 2.1 Which placements use copies

Only **copy** placements take a copy of their group; fresh placements never do [R]. A copy placement takes the
group's next unused copy; the reference loader treats running out of copies as an error [R]. On the disc, with the selection a
fresh load makes, no group ever runs out [D]:

- Count only copy placements (mode 0, or −1 naming a non-streaming group). Fresh placements (mode 1 or 2, or −1
  naming a streaming group) are excluded.
- Count only records that are loaded: a record of mode 1 whose world filter or conditions fail (with option set
  0's condition) is left out with everything inside it ([../object-stream.md](../object-stream.md)), and placements
  stored inside a group whose copy count after option set 0 is 0 are not read, because that group's body is skipped
  at load [R; D that this makes the counts fit].
- Count placements at every depth: top level, inside solid bodies and in copy extras. A group's body is read once,
  and its copies are made by cloning, so a placement inside a group's body is counted once, not once per copy [R].

With those rules, copy placements per group are at most the group's copy count after option set 0 in all 39 worlds
(0 groups over) [D]. Counting every placement instead gives large excesses; for example v_canal (slot 4) has 603
loaded placements of which 258 are fresh, and v_canal_gondola alone has 133 fresh placements for 40 copies [D].
(Without the selection rules the disc shows 103 copies too many over all worlds; without excluding skipped groups, 14.)

**Streaming groups ignore their count for placements** [R]: a fresh placement makes a streaming proxy object, not a
copy; while the proxy is active it borrows one of the group's copies, and gives it back when it goes inactive. The
copy count is therefore the size of the shared pool, not a limit on placements. On the disc, 100 (world, group)
pairs have more fresh placements than copies (1,258 placements over the counts in all), for example hack_powerup in
h_hack (484 fresh placements, 4 copies) and v_canal_gondola in v_canal (133 for 40). [D]


## 3. Test vectors

World slot 5 (v_apt): 160 top-level placements in the level group; the first (object 139,880) names group 149,054
(`v_alarm_light`) with mode −1, parent −1, matrix axes (−0.02794, 0, −0.99961), (0, 1, 0), (0.99961, 0, −0.02794),
translation (−3481.34, 4987.41, 371.49), zone mask 255, no binds; body 66 bytes. [D] The earlier research found v_apt's
six ceiling fans at ceiling height with this rule. [R]
