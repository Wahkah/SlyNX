---
id: sly2-formats-world-file
title: Sly 2 world file - compression and outer layout
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97316 image (2,884,141,056 bytes); executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds
    used_for: >-
      every world read from the disc, decompressed with the Sly 1 decoder and walked section by section to its
      unread tail (7,753 packages); section offsets, counts and value surveys; every test vector below
  - kind: reference-project
    what: >-
      sly2-decomp @ 33e2bab9 (2026-11-12), src/ and nonmatching/ readers for the world, sound, save-variable,
      level-table, string, tree, script, material, graph and object sections; earlier research in this workspace
      (Sly2-RE-notes.md, tools/sly2_world.py)
    used_for: first reading of the section order and sizes; every section re-checked by the end-of-world walk
reviewed_by: pending
date: 2026-11-13
---

# Sly 2 world file: compression and outer layout

Tags: [D] checked on the disc, [R] from a reference, [I] inferred. Conventions (little-endian, packed, `vec3`,
length-prefixed string = `u16 n` + `n` bytes) as in [../sly1/world-file.md](../sly1/world-file.md). A **file record**
is the 64-byte record of [disc.md](disc.md) 4.

## 1. Overview

A Sly 2 world is a bundle of **packages**: self-contained groups of scripts, materials, texture data and objects.
The last package is the level itself; the others are props, characters and effects the level places or spawns.
[D] [I] A world file is: [D]

| Order | Section | Section of this document |
|---:|---|---|
| 1 | Package list | 3.1 |
| 2 | Sound tables | 3.2 |
| 3 | Save variables | 3.3 |
| 4 | Level table | 3.4 ([disc.md](disc.md) 6) |
| 5 | String table | 3.5 |
| 6 | World partition tree | 3.6 |
| 7 | Two option lists | 3.7 |
| 8 | Zero padding to a multiple of 16 | 3.8 |
| 9 | One **package body** per entry of the package list, in list order | 4 |
| 10 | Unread tail | 5 |

Compared with Sly 1, the header word pair, the debug flag, the name table and the per-world shader, object and
texture sections are gone; materials, texture data and objects live in each package body instead. [D]

## 2. Compression

**Identical to Sly 1** ([../sly1/world-file.md](../sly1/world-file.md) section 2): same 8 KiB window, absolute 13-bit
source positions, lengths 3 to 10. [D] The decoder written for Sly 1 decompresses all 43 Sly 2 worlds to the sizes
and hashes of [disc.md](disc.md) 7. [D]

## 3. Header sections

### 3.1 Package list

| Size | Type | Meaning |
|---:|---|---|
| 2 | s16 | Package count (3 to 326 on the disc) [D] |
| 84 each | | Package entries |

Package entry (84 bytes): [D]

| Offset | Size | Type | Meaning |
|---:|---:|---|---|
| 0 | 64 | ASCII | Name, NUL-terminated, always starting with the 4 bytes `FK$X` (for example `FK$Xf_nightclub_cellar`) |
| 64 | 4 | s32 | **Keep flag**: 1 = the package's objects stay as loaded; 0 = they are kept as templates and copied when placed [R] |
| 68 | 4 | s32 | **Shared flag** (0 or 1) [D]; how placements use it is in [object-stream.md](object-stream.md) [R] |
| 72 | 4 | s32 | **Set id**: the number placements use to name this package ([object-stream.md](object-stream.md)) [R] |
| 76 | 4 | s32 | A second id, unique within the world (meaning not needed to walk) [D] |
| 80 | 4 | s32 | **Copy count**: how many copies of the package to prepare (1 or more) [R] |

Combinations on the disc (keep, shared): (0, 0) 6,871 packages, (0, 1) 621, (1, 0) 261. [D] The level package is
always last, with keep flag 1. [D]

### 3.2 Sound tables

The layout needed to step over the section [D], with the meanings read from the reference [R] (the audio spec
([audio.md](audio.md)) gives playback):

| Size | Type | Meaning |
|---:|---|---|
| 64 | file record | The level's effect bank (a bank file of the Sly 1 kind, file kind 3) [D]. If it differs from the bank already loaded, that bank is unloaded first [R] |
| 2 | s16 | Count `b` |
| b | s8[b] | One bank-slot number per sound [R] |
| 2 | s16 | Size of a lookup table from sound number to entry; the game fills it with −1 and does **not** use it as a read count (it differs from `b`) [R] |
| 2b | s16[b] | `b` sound numbers (**`b` of them, not the count above**); entry `i` is reached through sound number `s[i]` [R] |
| 2 | s16 | Count `m` |
| 64 | file record | Only if `m > 0`: a streamed ambient-sound file [R] |
| 2m | s16[m] | `m` stream numbers (none when `m ≤ 0`; the game keeps at most 6) [R] |
| 1 | u8 | Track count `t` |
| t | bytes | One **language code** per track; the game uses the track whose code equals the console's language [R] |
| 4 | s32 | Entry count `e`; then `e` **voice entries**: `s16`, `s16`, then for each track (language) a VAG file record, an `s8` and an `f32` duration in seconds (only the selected language's are kept), then `s32 k` and `k × 8` bytes [R] |
| 4 | s32 | Record count `r`; then `r` times (`s16` number, file record) [R] |

The file records inside entries point at VAG voice files (for example world 3: sector 1,253,673, 123,968 bytes, with
an `f32` 9.835 that matches a duration in seconds). [D] [I]

### 3.3 Save variables

`u16 n`; `n` × (`u16` id, string); `u16 m`; `m` × 10 bytes. [D] The strings name game-progress variables (for
example `t1_heist_intro`). Decoded contents are the same in every world (960 names, 5,461 records). [D]

### 3.4 Level table

See [disc.md](disc.md) 6. [D]

### 3.5 String table

| Size | Type | Meaning |
|---:|---|---|
| 1 | u8 | Language count `L` (1 on the USA disc) [D] |
| L | u8[L] | Language codes (0) [D] |
| 2 | s16 | String count |
| var | | Per string: `s16` id, then `L` length-prefixed strings |

String ids run from 0 to about 3,250; each world carries only the strings it uses (359 to 654). [D]

### 3.6 World partition tree

`u16 n`, then `n` nodes of 20 bytes: a plane (`vec3` unit normal, `f32` distance), the **front** child, then the
**back** child (`s16` node indices). [D] A negative child is a **leaf region**: regions are numbered 0, 1, 2, … in
the order the negative children appear (node by node, front before back). [R] *Interpretation:* a split of the world into zones, used for visibility. [I] Node counts 0 to 39.
[D]

### 3.7 World option lists

Two option lists in a row, each ending with the option id −1. [D] Their encoding is the option-list encoding of
[object-stream.md](object-stream.md), which depends on a table in the executable. [R]

### 3.8 Padding

Zero bytes to the next multiple of 16 from the start of the file. The first package body starts there. [D]

## 4. Package body

For each package of 3.1, in order: [D]

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | Texture-data size `G` | s32 | Size of item 6 |
| 2 | Build stamp | s32 | A time stamp (`0x411C7BC2` = 2004-08-13 in the cellar world's first package) [D] [I]. The game reloads texture memory when it changes [R] |
| 3 | Debug flag | s32 | 0 in all 7,753 packages [D] |
| 4 | Script block | `u32` count, then script nodes | 4.1 |
| 5 | Material tables | | [textures.md](textures.md) 2 |
| 6 | Padding | | Zero bytes to the next multiple of 16 from the start of the file |
| 7 | **Texture data** | `G` bytes | Texel and palette data addressed by the material tables ([textures.md](textures.md) 3) |
| 8 | Graph block | | 4.2 |
| 9 | Object list | | [object-stream.md](object-stream.md) |
| 10 | Padding | | Zero bytes to the next multiple of 16 |

Every package body starts on a multiple of 16. [D]

### 4.1 Script block

As Sly 1 ([../sly1/world-file.md](../sly1/world-file.md) 4.3: a count, then list nodes with a tag byte, `0xFF` = empty,
bit 7 = another node follows), with these differences: [D] [R]

- The count is a single `u32` (no symbol or extra-string counts).
- Payload sizes by node kind (`tag AND 0x7F`):

  | Kind | Payload bytes |
  |---|---:|
  | 21, 22, 23, 24, 25, 33 | 4 |
  | 26, 28, 30 | 12 |
  | 27 | 64 |
  | 29 | 8 |
  | 31 | one nested node |
  | others | 0 |

- With the debug flag set (never on the disc), 16 bytes follow each tag byte and three strings follow each script.
  [R]

1,558 of the 7,753 package bodies carry at least one script module, 10,930 modules in all ([scripts.md](scripts.md)). [D]

### 4.2 Graph block

`u32 a`; if `a ≠ 0`: `u32 b`, `u32 c`, then `a` entries of 3 × `u16` (an index into the vector list, then two
indices), `b` vectors of 4 × `f32`, and `c` × `s32`. [D for the sizes, R for the split] *Interpretation:* a
navigation graph. Only 4 of
916 packages sampled (worlds 0, 3, 27, 43) carry one. [D] [I]

## 5. Unread tail

After the last package body, every world has 567 to 2,468 bytes that the game never reads; they repeat earlier
content of the file. [D] [R] A reader stops after the last package body; a walker should accept a tail of up to a few
kilobytes. [D]

## 6. Test vectors

Section boundaries (hexadecimal, start..end) for four worlds: [D]

| Section | Row 0 (intro hub) | Row 1 (menu) | Row 3 (cellar) | Row 43 (final boss) |
|---|---|---|---|---|
| Package list (count) | 0..4E6E (239) | 0..FE (3) | 0..3872 (172) | 0..564E (263) |
| Sound tables | 4E6E..AA94 | FE..660 | 3872..15EAB | 564E..15823 |
| Save variables | AA94..1D709 | 660..132D5 | 15EAB..28B20 | 15823..28498 |
| Level table | 1D709..1E421 | 132D5..13FED | 28B20..29838 | 28498..291B0 |
| String table (strings) | 1E421..22628 (419) | 13FED..17157 (359) | 29838..31765 (605) | 291B0..2FB86 (551) |
| Partition tree (nodes) | 22628..22936 (39) | 17157..17159 (0) | 31765..31A23 (35) | 2FB86..2FC50 (10) |
| Option lists | 22936..2297C | 17159..1718D | 31A23..31A75 | 2FC50..2FC9C |
| First package body | 22980 | 17190 | 31A80 | 2FCA0 |
| Level package body | 8FE580..109CF10 | 76E30..3E5140 | 7A2120..ADF6A0 | E1CBF0..F4ABC0 |
| Level package texture data (offset, size) | 904A30, 1,325,792 | 76F30, 34,816 | 7A54F0, 866,096 | E1E190, 111,264 |
| File size / tail | 109D2B6 / 934 | 3E5377 / 567 | ADFE6B / 1,995 | F4B217 / 1,623 |

Other checks: the cellar world's first three package names are `f_pottedplant_flower`, `f_door_transition_1`,
`abandon_icon` (bodies at `31A80`, `3C480`, `3F6F0`; first prologue bytes `40530000 c27b1c41 00000000`). [D] The cellar
world's first partition-tree node is the plane (0, 1, 0) at distance 538.391 with children 1 and 18. [D]

## 7. Open questions

- The meaning of the package entry's fourth value.
- Which side of a partition plane is "front" (presumably the side the normal points to).
- The meaning of the graph block's two per-entry indices and its integer list.
