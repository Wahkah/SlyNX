---
id: sly3-formats-world-file
title: Sly 3 world file - compression and outer layout
side: reference
status: draft
sources:
  - kind: disc
    what: re-check of 2026-11-17 on all 39 worlds and SCUS_974.64 (hash checked), answering the formats-sly3 spec questions
    used_for: stored copy counts versus counts after option set 0
  - kind: disc
    what: >-
      SCUS-97464 image (3,553,755,136 bytes); executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338; all 39 worlds
    used_for: >-
      every world decompressed and walked section by section to its unread tail, every group checked against its
      byte size (7,151 groups); section offsets, counts and value surveys; every test vector below
  - kind: reference-project
    what: >-
      earlier research in this workspace (Games/Sly3-RE-notes.md, tools/sly3_world.py, 2026-11); SafeHouse @ c70803ce
      (2026-11-12), Sly3-Release world loader
    used_for: first reading of the section order; every section re-checked by the group-by-group walk
reviewed_by: pending
date: 2026-11-14
---

# Sly 3 world file: compression and outer layout

Read [../sly2/world-file.md](../sly2/world-file.md) first. Sly 3 keeps Sly 2's idea of a world as a bundle of
self-contained units; this document calls them **groups** (Sly 2's "packages"). Tags as in [disc.md](disc.md).
Strings are `u16` length + bytes unless said otherwise.

## 1. Overview

| Order | Section | Section of this document |
|---:|---|---|
| 1 | Two **key tables** | 3.1 (new) |
| 2 | Group list | 3.2 (Sly 2 3.1, entries changed) |
| 3 | Save variables | 3.3 (as Sly 2 3.3) |
| 4 | Sound tables | 3.4 (as Sly 2 3.2) |
| 5 | String table | 3.5 (now UTF-16) |
| 6 | One option list | 3.6 (Sly 2 had two) |
| 7 | **Option sets** | 3.7 (new) |
| 8 | Partition-tree sets | 3.8 (Sly 2 3.6, now one tree per condition) |
| 9 | Zero padding to a multiple of 16 | |
| 10 | One **group body** per entry of the group list, in list order, each exactly its stated size | 4 |
| 11 | Unread tail | 5 |

Gone from Sly 2's header: the level table (the world list is in the executable now, [disc.md](disc.md) 5.1). [D]

## 2. Compression

**Identical to Sly 1 and Sly 2** ([../sly1/world-file.md](../sly1/world-file.md) section 2). The Sly 1 decoder
decompresses all 39 worlds to the sizes and hashes of [disc.md](disc.md) 6. [D]

## 3. Header sections

### 3.1 Key tables (new)

Two tables in a row, each: `u8` count, then per entry `s32` key, `s8` value, `u8` count `c`, `c` × `s32`. [D]
Entry counts on the disc: first table 0 to 3, second 0 to 12. [D] *Meaning not known; not needed to walk.* Example
(v_apt): first table (2283, 1, [6]), (2409, 2, []); second table starts (1811, 2, []), (2284, 1, [6]), (1810, 1, [6]). [D]

### 3.2 Group list

`s16` group count (15 to 401), then 88-byte entries [D]:

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 64 | ASCII | Name, NUL-terminated, starting with `FK$X` (as Sly 2) |
| 64 | 4 | s32 | **Shared flag** (0 or 1): the group's objects belong to the world itself rather than being kept as templates [R] |
| 68 | 4 | s32 | **Streaming flag** (0 or non-zero); changes how placements of this group are read ([classes/placement.md](classes/placement.md)) [R] |
| 72 | 4 | s32 | **Group id**: the number placements use to name the group [D] |
| 76 | 4 | s32 | A second number, unique within the world (meaning not needed) [D] |
| 80 | 4 | s32 | **Copy count**: how many copies of the group to prepare [R]. Stored values: 1 (3,732 groups), more than 1 (3,419); 0 never occurs [D]. After option set 0 is applied (3.7), as at a fresh load: 0 (36), 1 (3,713), more (3,402) [D] |
| 84 | 4 | u32 | **Body size** in bytes (new in Sly 3): the group's body (section 4) is exactly this long [D] |

Combinations (shared, streaming) over the 7,151 groups: (0, 0) 6,369; (0, 1) 450; (1, 0) 332. [D] A world's own
geometry is the shared group named after the world (for example `v_apt`); other shared groups hold helpers (macro
scripts, rigs, common objects). [D for the names, I for the reading]

Copy counts can be changed by the option set chosen at load (3.7). [R]

### 3.3 Save variables

As Sly 2 3.3: `u16 n`, `n` × (`u16` id, string); `u16 m`, `m` × 10 bytes. [D] 1,201 names in every world. [D]
Example: the first names of v_apt are `t0_npc_kill` (id 256), `t2_heist_boss_a_hit2` (512), `t4_heist_alarm6` (768). [D]

### 3.4 Sound tables

**Same layout as Sly 2 3.2** (the field list there applies field for field). [D] Contents: [audio.md](audio.md).

### 3.5 String table

| Size | Type | Meaning |
|---|---|---|
| 1 | u8 | Language count `L` (1 in every USA world) [D] |
| L | u8[L] | Language codes (0) [D] |
| 2 | s16 | String count (920 to 1,631 per world) [D] |
| var | | Per string: `s16` id, then `L` **UTF-16 strings** (`u16` character count, then 2 bytes per character, little-endian) |
| var | | Then `L` more UTF-16 strings: per language, the list of characters its strings use [R] |

**Difference from Sly 2:** strings are 16-bit characters, and the per-language character lists are new. [D] Example
(v_apt): string 0 begins `Sly 3 © 2005 Sony Computer Entertainment America & Sucker Pu…`; string 512 is
`Jump (tap twice for double jump)`. [D]

### 3.6 World option list

One option list ([object-stream.md](object-stream.md) 1), ending with −1. [D]

### 3.7 Option sets (new)

| Field | Type |
|---|---|
| table A | `u8` count, then count × (`u32`, `u32`) |
| table B | `u8` count, then count × (`u16` n, n × `u16`, `u32`, `u32`) |
| set count `n` | `u8` (1 to 4 on the disc) |
| set ids | `n` × `s32` |
| sets | `n` × (`s32` size, then an option list of exactly that size) |

Every set's option list ends exactly at its stated size. [D] Behaviour [R]: with no job or task in progress (a
fresh load), the game uses set 0. Its id becomes the world's current **condition** (section 6), its option 1032
entries change group copy counts (arguments: group id, count), and some of its options add further conditions. The
other sets apply when the corresponding job or task state is loaded.

### 3.8 Partition-tree sets

`u8` set count, then per set: `s32` condition, `u16` node count, then that many 20-byte nodes. [D] The node record
is Sly 2's partition-tree node ([../sly2/world-file.md](../sly2/world-file.md) 3.6: plane normal, distance, two `s16`
children). [D for the size; I for the reading, supported by unit normals] Node totals per world 0 to 104. [D]
Example (v_apt): one set, condition 12, 5 nodes; node 0 = plane (0, 1, 0) at distance 2856.19, children 1 and 3. [D]

## 4. Group body

Each group body is **exactly** its list entry's body size (3.2); all 7,151 match. [D] A group with copy count 0 is
skipped by size in the game [R]; all of them parse anyway. [D] In order:

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | Texture-data size `G` | s32 | Size of item 6 |
| 2 | Build stamp | s32 | A time stamp (v_apt's first group: `0x4310CF2E`, 2005-08-27) [D] [I] |
| 3 | Debug flag | s32 | 0 in all 7,151 groups [D] |
| 4 | Script block | `u32` count, then script nodes | 4.1 |
| 5 | Material tables | | [textures.md](textures.md) 2 |
| 6 | Padding, then **texture data** | | zero bytes to the next multiple of 16, then `G` bytes ([textures.md](textures.md) 3) |
| 7 | Graph block | | as Sly 2 4.2 (25 groups carry one) [D] |
| 8 | Object list | | [object-stream.md](object-stream.md) 2 |
| 9 | Padding | | zero bytes to the next multiple of 16 |

### 4.1 Script block

As Sly 2 4.1 with the node kinds above 18 **renumbered down by 2** [D]:

| Kind | Payload bytes |
|---|---:|
| 19, 20, 21, 22, 23, 31 | 4 |
| 24, 26, 28 | 12 |
| 25 | 64 |
| 27 | 8 |
| 29 | one nested node |
| others | 0 |

With the debug flag set (never on the disc), each cell carries 16 more bytes and each module three strings. [R]
1,524 groups carry script modules, 11,007 modules in all. [D] Meanings: scripts spec (later round).

## 5. Unread tail

After the last group body every world has 1,797 to 6,732 bytes that are never read [D]; as in Sly 2, every 32-byte
chunk of it repeats earlier content of the file. [D] A reader stops after the last group; accept a tail of up to 8 KiB.

## 6. Conditions and world filters

Two per-world selection mechanisms appear throughout the format [R; D that the fields parse]:

- A **world filter** is a list of world slot numbers ([disc.md](disc.md) 5.1). It matches when it is empty or holds
  the current world's slot.
- A **condition list** is a list of `s32` condition ids. A condition matches when the world's current condition is −1,
  the condition is −1, it equals the current condition, or it is one of the extra conditions added by the option
  set (3.7). A list matches when it is empty or any entry matches.

They gate object records ([object-stream.md](object-stream.md) 2), meshes ([geometry.md](geometry.md) 2), palettes
and bitmaps ([textures.md](textures.md) 2), and colour tables.

## 7. Test vectors

Section boundaries (hexadecimal, start..end) [D]:

| Section | Slot 0 (dvd_menu) | Slot 2 (i_trainer) | Slot 5 (v_apt) |
|---|---|---|---|
| Key tables | 0..2 | 0..2 | 0..4A |
| Group list (count) | 2..52C (15) | 2..40FC (189) | 4A..2CFC (130) |
| Save variables | 52C..17308 | 40FC..1AED8 | 2CFC..19AD8 |
| Sound tables | 17308..1C9A0 | 1AED8..32960 | 19AD8..528D3 |
| String table (strings) | 1C9A0..2B754 (920) | 32960..433D6 (923) | 528D3..70035 (1,465) |
| Option list | 2B754..2B75A | 433D6..43400 | 70035..7005F |
| Option sets | 2B75A..2B7B3 | 43400..4355F | 7005F..70434 |
| Partition sets (nodes) | 2B7B3..2B7BA (0) | 4355F..43566 (0) | 70434..7049F (5) |
| First group body | 2B7C0 | 43570 | 704A0 |
| Level group (name, start..end) | dvd_menu, 321790..321960 | i_trainer, 85EA30..8DDEF0 | v_apt, 61D5B0..72B0F0 |
| Tail | 1,797 | 2,184 | 5,436 |

Group list entries of v_apt [D]:

| Index | Name | Shared | Streaming | Id | Second | Copies | Body size |
|---:|---|---:|---:|---:|---:|---:|---:|
| 0 | `v_chair_fancy` | 0 | 0 | 138293 | 140925 | 50 | 29,328 |
| 1 | `v_divan` | 0 | 0 | 137751 | 140926 | 2 | 40,416 |
| 129 | `stock_objects` | 1 | 0 | 9900 | 16866 | 1 | 95,552 |

The v_apt key tables are the 74 bytes `02eb0800 00010106 00000069 09000002 00061307 00000200 ec080000 01010600
00001207 00000101 06000000 f8080000 01010600 00000009 00000101 06000000 10090000 01010600 0000` (as stored). [D]

## 8. Open questions

- The meaning of the key tables (3.1), of option-set tables A and B (3.7) and of the group entry's second number.
- The exact effect of the streaming flag beyond the placement rule.
