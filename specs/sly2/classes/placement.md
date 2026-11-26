---
id: sly2-classes-placement
title: Sly 2 object stream, prefab placement layout
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97316 (USA) disc image; executable SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 decompressed worlds
    used_for: >-
      all 8,791 placement records parsed to their declared body size; 1,050 copy patches and 425 fresh option
      blocks parsed as option lists to the byte; kind, transform-reference and block counts below
  - kind: reference-project
    what: Games/Sly2-RE-notes.md, tools/sly2_world.py (earlier research); sly2-decomp @ 33e2bab9, nonmatching/new_1c90d0
    used_for: field order, the fresh/copy rule and the meaning of the package set ID; marked [R] where not checked
  - kind: executable
    what: >-
      SCUS_973.16 (SHA-256 a511a94b…6a73), class 6's script map (script IDs 1017-1025 → stream options 1174-1181) and the handlers
      0x0010E248, 0x0010E2B8, 0x0010E328, 0x0010E398, 0x0010E420 with the routines they call (2026-11-21)
    used_for: section 2.1 [D]
  - kind: executable
    what: >-
      SCUS_973.16 (SHA-256 a511a94b…6a73), the placement loader's fresh/copy test 0x001C9BC8-0x001C9BF4, the object creation at 0x001C9C08
      and the package-entry pointer at 0x001C9C60, the package table fill 0x0013DA18 (word 0 at 0x0013DDCC) (2026-11-26)
    used_for: which placements are objects and what 1019 tests (section 2.1) [D]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9, nonmatching and src/new_1c90d0 (lease routines)
    used_for: what the lease routines do (section 2.1) [R]
date: 2026-11-26
---

# Prefab placement layout (Sly 2)

Classes 6 and 7 ([../object-stream.md](../object-stream.md) section 3) place a copy of a **package** (a prefab;
[../world-file.md](../world-file.md)) into the world. 8,551 + 240 top-level records [D]; 8,057 more are nested in
other bodies [D]. Sly 1 has no equivalent (its closest relative is the proxy, ../../sly1/classes/proxy.md).

## 1. Layout

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | package set ID | `s32` | matches the set ID of one package in the world's package list (world-file.md). Every top-level placement matches [D]; 378 placements nested in solid bodies do not (section 1.3) |
| 2 | placement kind | `s8` | −1 (8,528), 1 (241), 0 (22) [D] |
| 3 | parent | `s16` transform reference | −1, `0x1000 | n`, or `0x2000` (2,656 placements, all in owned lists; transforms.md 1.1) [D]. For a copy placement this reference is also what its extra objects' own `0x2000` references resolve to; the placement's matrix is then applied to each extra object once, at load (transforms.md 1.1) [R] |
| 4 | matrix | 4 × `vec3` | three axis images and a translation (transforms.md 1) |
| 5 | zone mask | `u32` | |
| 6 | rest | section 1.1 or 1.2 | |

The record is **fresh** when its kind is 1, or −1 with a package whose "shared" flag is set (world-file.md);
otherwise (kind 0, or −1 with an unshared package) it is a **copy** [R for the rule, D that it parses].
On the disc: 2,090 fresh, 6,701 copies [D].

### 1.1 Fresh placement

| Field | Type | Notes |
|---|---|---|
| block count | `u8` | 0 (1,612), 1 (295), 2 (176), 3 (7) [D] |
| blocks | each: `s32` target object ID, `s16` class ID, `s16` index, `u16` size, then an option list of exactly `size` bytes [D] | the same shape as a copy patch (1.2). Every one of the 1,876 blocks on the disc (all depths) is an option list ending exactly at its size [D]. Targets: object 5 in 1,486 blocks and 6 in 327 (the copy's root objects [R]), other objects in 63 (for example guard 11539) [D]; see options-by-class.md |
| skip size S | `s32` | |
| (unknown) | `s16` | |
| data | S − 2 bytes | not needed to walk |

### 1.2 Copy placement

| Field | Type | Notes |
|---|---|---|
| patch count | `u8` | 0 (5,742), 1 (879), 2 (74), 3 (1), 4 (5) [D] |
| patches | each: `s32` target object ID, `s16` class ID, `s16` index, `u16` size, then an option list of exactly `size` bytes [D] | options applied to the copied package's object with that ID (IDs 5 and 6 mean the copy's root) [R] |
| (unknown) | `s32` | |
| extra objects | object list ([../object-stream.md](../object-stream.md) 2.1) | objects added to this copy only; their matrices are relative to this placement's matrix [R] |

### 1.3 Placements naming a package the world does not hold

378 placements, all nested in solid bodies (parent `0x1000 | n`), name a set ID that no package of their world has
[D]: 296 of placement kind −1 and 82 of kind 1. They name 10 set IDs, each the set ID of a package that exists in
**other** worlds: 4090 `c_judge_display` (130 placements), 582 `bentley_distract_dart` (41), 7502 `c_eagle_egg` (41),
7528 `c_walkie_talkie` (41), 6708 `pe_grp_bug_noise` (39), 6706 `pe_grp_bug_noise_alt` (39), 6783
`pe_booty_energy_mist` (32), 8527 `p_clockwerk_eye` (10), 8246 `a_clockla_forcefield_generator` (4), 8436
`npc_boss_neela` (1) [D]. They sit inside bodies that are the same in many worlds (the playable characters, props).
Reading rule: a kind −1 placement whose package is absent is read as a **copy** (an absent package cannot be
"shared"); kind 1 is fresh as usual. With that rule all 378 parse to their declared size [D]. In play such a
placement places nothing, because the package is not loaded in that world [I].

Example: row 0, the kind-1 placement at `0x1B7069` inside Bentley's body (class 55) names set ID 582 [D].

## 2. Behaviour

- A package whose "no clone" flag is clear is not drawn where it is stored; it is drawn wherever a placement names
  it, once per placement, at the placement's matrix [R]. The earlier research rendered all 43 levels this way and
  matched the levels' expected layout [R].
- Objects carrying option 26 are removed at level start, also inside copies (object-stream.md 1.4) [R].
- A copy placement's patch (1.2) adds options to the copied object even when the package's own record lacks them. For
  example, row 0's placement 150360 gives spot 80398 options 1070 = 1675 and 26 (dialogue-and-voice.md 2.1) [D].

### 2.1 Placements of shared packages: the leased copy (added 2026-11-21)

A placement of a **shared** package (for example `npc_dimitri`, whose objects are pooled) does not own its object.
It **leases** one from the package's pool of copies [R]. Scripts reach the leased object through the placement's
own methods. These are class-6 script IDs; the class-59 character itself has none of them [D for the map]:

| Script ID | Stream option | Handler [D] | What it does [R] |
|---:|---:|---|---|
| 1017 | 1174 | `0x0010E248` → `0x001CB2C0` | whether a copy can be leased now |
| 1019 | 1175 | `0x0010E2B8` → `0x001CB250` | lease a copy and return it (0 if none can be had). It **fails at once unless the placed package has exactly one top-level object** [D]. +0xE0 is the placement's package entry: the 108-byte entry of the world's package table (SW +0x3F08), set by the placement loader at `0x001C9C60`. Word 0 of that entry is the number of top-level objects the package's object list loaded (written at `0x0013DDCC` after the package loads). The test at `0x001CB264`–`0x001CB26C` compares it with 1. No option sets it, so option 1178 is not involved. Only fresh placements have this object at all (section 2.1) |
| 1020 | 1176 | getter `0x001CB2E0` | the copy currently leased by this placement (0 if none) |
| 1021 | 1177 | `0x0010E328` → `0x001CB328` | **the leased copy, leasing one first if there is none**; returns it |
| 1023 | 1179 | `0x0010E398` → `0x001CB488` | **lock** the lease of the given copy, so the pool keeps it for this placement |
| 1024 | 1180 | `0x0010E420` → `0x001CB4B8` | **unlock** that lease, so the pool may hand the copy out again |

So "make the copy", "attach" and "release" are close but not exact: 1021 gets (or leases) the live copy, 1023 locks
it and 1024 unlocks it.

**Leasing adds the copy to the world** [D for the calls, R for the rest]. Taking a lease (`0x001CAEC8`, reached from
1019 and from 1021 when no copy is leased yet) does the following:

1. Re-applies the placement's option blocks to the copy's objects. The root block (target 5) is skipped; target 6 and
   other targets are applied.
2. For each object of the copy:
   - sets its parent and matrix to the placement's;
   - **adds it to the world**, through the same virtual slot (`0x40`) that script ID 39 uses (call at `0x001CB178`);
   - gives it the placement's activation radius (option 13, `0x001A6C48`) and the placement's zone mask
     (`0x001CB4E0`, unless the object has option 32) [D];
   - raises script event 14 on the placement and on the object.

So a script that calls 1021 gets a copy that is already in the world at the placement's position, whatever the
placement's own option 26 says. Unlocking (1024) does not remove it. Scripts remove it with script ID 40 on the copy.

**Only fresh placements are objects** [D for the calls]. For a fresh placement (kind 1, or −1 with a shared package;
test at `0x001C9BC8`–`0x001C9BF4`) the loader creates a class-6 object (`0x0013E3C8` at `0x001C9C08`) and points its
+0xE0 at the package entry. The copy branch (`0x001C9E14` on) creates no placement object: the copy's objects are taken
from the package's loaded instances, and the copy root answers to the placement's OID (scripts.md 5) [R]. So class
searches (op120 class 6) and the lease methods only ever see fresh placements.

**Which copy a lease takes: grades** (added 2026-11-26) [R; routines `0x001CADE0` and `0x001CA978`].

The single-root test is only the first gate. The lease routine (`0x001CAEC8`) then asks `0x001CADE0` for the best copy
with at least a minimum grade: 2 for 1019 and 1 for 1021. Each copy of the package has a lease record (80 bytes,
after the instances that copy placements use). Its grade is:

| Grade | The copy |
|---:|---|
| 0 | locked by its holder (script ID 1023) |
| 4 | held by no placement |
| 1 | held, and one of its objects is in the world (its in-world flag set and really in the world) |
| 2 | held, an object has the in-world flag but is not really in the world (for example its parent is out) |
| 3 | held, none of its objects has the in-world flag (all removed) |

- **A placement that is not fixed and already holds a copy** only re-takes that same copy, and only if its grade is
  at least 3 (all its objects removed). Otherwise the lease fails.
- **A fixed placement, or one holding nothing,** searches the package's spare copies (from the first one not used by
  copy placements to the last). It takes the one with the highest grade that is at least the minimum.
- **Taking a copy held by another placement** releases it from that holder first.
- So for 1019 a copy is free when it is unheld, or held with all its objects out of the world.
- "The copy must not be in the world" is close to this. The exact rule also covers locks (grade 0), and a non-fixed
  placement can only take back its own copy.
- In row 7, a class-79 placement that auto-leased at load, and whose copy is in the world, therefore fails 1019.

**Automatic leases and option 1178** (script ID 1022, field +0xE4, a flag) [D for the test, R for the rest].

- When a placement enters the world and +0xE4 is 0, it queues a lease for itself (`0x001CA8E0`, test at
  `0x001CA8F4`), so its copy appears with it.
- With 1178 = 1 the placement is **fixed**: it takes no lease on its own. A copy appears only when something asks for
  one.
- Example: the bullet placements inside `npc_dimitri` (12734 → package `npc_dimitri_bullet`, and → `bullet`) set 1178
  to 1 in their root block. So Dimitri's copy comes with no bullet in the world; a bullet is taken from the pool only
  when one is fired [I for "when fired"].
- The stored objects of a shared package (for example 122335, the class-81 bullet in `npc_dimitri_bullet`) are pool
  templates, not placed objects (section 2), so they are not in the world from load [R]. The placement's own option overrides apply to the copy; for example, Dimitri's placement
122332 in row 2 sets the copy's option 273 (its route) to command list 1826 [D] ([missions.md](../missions.md) 4.3).

## 3. Test vectors

- World row 3 (`f_nightclub_cellar`): the level package holds 186 placements [D].
- Totals: worlds.md.
