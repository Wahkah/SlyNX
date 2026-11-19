---
id: sly3-formats-geometry
title: Sly 3 geometry
side: reference
status: draft
sources:
  - kind: disc
    what: re-check of 2026-11-17 on all 39 worlds and SCUS_974.64 (hash checked), answering the formats-sly3 spec questions
    used_for: bounding-tree counts per mesh set; mesh offset and size convention
  - kind: disc
    what: SCUS-97464 image, all 39 worlds
    used_for: >-
      every mesh set of every solid body (11,524 bodies, 51,967 meshes) parsed to its declared size; weight sums,
      value surveys, triangle counts and the test vectors below
  - kind: reference-project
    what: earlier research (Games/Sly3-RE-notes.md, tools/sly3_geom.py, 2026-11); SafeHouse @ c70803ce, Sly3-Release mesh loaders
    used_for: first reading of the field order, the colour-table rule and the outline edge order; re-checked on the disc
reviewed_by: pending
date: 2026-11-14
---

# Sly 3 geometry

Read [../sly2/geometry.md](../sly2/geometry.md) first; terms as there (mesh set, mesh, vertex group, strip list,
outline edges, copies). The mesh set is item 3 of the solid layout ([classes/solid-object.md](classes/solid-object.md)).
Tags as in [disc.md](disc.md).

## 1. What changed from Sly 2

| Item | Sly 2 | Sly 3 |
|---|---|---|
| Per-level variants | level-row condition list per mesh | world filter **and** condition list per mesh (2.1) |
| Bone table | first in each body | in the **mesh header**, for every body type (3.1) |
| Header | | the body is preceded by a fog factor and a billboard byte (3.1) |
| Rigid body colours | vertex colours, vertex-map table | vertex colours plus **colour tables** that override them (4.2) |
| Copies | | one more `s8`; colour tables per copy (6) |
| Body type numbers | 0 rigid, 1 skinned, 2 streamed, 3 indexed | 0 strips, 1 outline, 2 effect outline, 3 indexed (same four shapes, renamed by use) |

## 2. Mesh set

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | no-bounds flag | `s8` | non-zero: meshes and copies omit their bounding fields (63 of the 11,524 sets) |
| 2 | bounding sphere ? | `vec3`, `f32` | [I] |
| 3 | animation records | `s8` count, each a texture-animation record ([textures.md](textures.md) 2.3: `u16` class, 36 bytes) | |
| 4 | (unknown list) | `u8` count, each `s32` | |
| 5 | slot count S | `s16` | |
| 6 | meshes | 2.1 | |
| 7 | bounding tree | 2.2 | |

### 2.1 Reading the meshes

Repeat while the used slots are below S:

- Read `u8` filter count `f` and `u8` condition count `c`.
- If either is non-zero: read `f` × `s16` world slots, `c` × `s32` conditions and an `s32` size, then a mesh of exactly
  that size. It counts **one** slot. It is used only when the filter and the conditions match
  ([world-file.md](world-file.md) 6). [D for the sizes; R for the rule]
- Otherwise read a mesh; it uses `1 + copy count` slots.

Every mesh is therefore preceded by its two count bytes. Mesh offsets and sizes in this spec (section 8 and
worlds.md) start at the mesh's type byte, after those two bytes, and end where the next mesh's two count bytes
begin: consecutive unconditional meshes lie *size + 2* bytes apart. For a conditional mesh, the stored `s32` size is
the same quantity. [D]

51,967 meshes, 9,987 of them conditional; no conditional mesh has copies. [D] Some character bodies exist only as
**hidden** conditional meshes, one per episode (for example a creature body filtered to slots 2..7 for Venice and
31..34 for the pirate episode). [D] [R]

### 2.2 Bounding tree

As Sly 2 2.3: `s16` node count; per node up to 4 entries of `s8` kind (low 4 bits, sign-extended; −1 ends the node),
`vec3`, `f32`, `f32`, `u8`, and an `s16` for kinds 0 and 1. [D] Only 63 of the 11,524 mesh sets have a non-empty
tree, and they are exactly the 63 sets with the no-bounds flag; all other sets store a node count of 0. The 63 trees
hold 14,846 nodes in all. [D]

## 3. Mesh

### 3.1 Header

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | body type | `s8` | 0 strips (40,078), 1 outline (9,516), 2 effect outline (1,137), 3 indexed (1,236) [D] |
| 2 | copy count | `u16` | section 6 |
| 3 | transform | `s16` transform reference | |
| 4 | render pass | `s8` | section 7 |
| 5 | draw-order value | `f32` | largest `f32` = none |
| 6 | (unknown) | `s32` | |
| 7 | hidden flag | `s8` | bit 0: not drawn until gameplay shows it (8,392 meshes) |
| 8 | bounding sphere | `vec3` centre, `f32` radius, `u32`, `f32` maximum draw distance | only when the set's no-bounds flag is 0 |
| 9 | bone table | `u8` count `B`, then `B` × (`s32` object ID, `s16` transform reference) | **moved here from the bodies** |
| 10 | no-body flag | `s8` | non-zero: nothing more (1,620 meshes, never with copies) |
| 11 | fog factor | `f32` | only with a body |
| 12 | billboard mode | `s8` | only with a body |
| 13 | body | section 4 | |
| 14 | copies | section 6 | |

## 4. Bodies

### 4.1 Strips (type 0; Sly 2's rigid)

| Field | Type | Notes |
|---|---|---|
| flags | `u8` | bit `0x20`: a distortion block follows; bit `0x08`: a box follows; values on the disc: `0x08` (18,144), `0x09` (7,430), `0x0C` (6,390) and others |
| texture animation | `s8` | index into the set's animation records, −1 none |
| view-angle block | `s8`; if not −1, 32 more bytes | |
| colour tables | 4.2 | |
| distortion block | only with flag `0x20`: `s32`, `s8` (if not −1: 16 bytes and a 48-byte matrix), 3 × `u8` | |
| box | only with flag `0x08`: 36 bytes | |
| vertex-group count | `s16` | |
| vertex groups | below | |

Vertex group (as Sly 2's, without the bone table):

| Field | Type |
|---|---|
| strip-list count `L` | `u8` |
| strip-list headers | `L` × (`u32` vertex count, `u32` shader index, `u32` flags) |
| position count P | `u8` |
| positions, texture coordinates, normals | P × `vec3`, P × 2 × `f32`, P × `vec3` |
| colours | P × 4 bytes R, G, B, A (0xFF = 1.0; scenery stores white) |
| (unknown) | `s8` |
| bone slots and weights | only if `B` > 0: 4 slot bytes, then P × min(B, 4) weight bytes, **vertex by vertex** |
| strip indices | (sum of the vertex counts) × `u8` |
| strip-break bits | ceil(sum / 32) × `u32` |

### 4.2 Colour tables

`u8` table count; per table `s32` condition, `u16` set count; per set `u16` vertex-group index, `u16` count `k`, then
`k` records of `u16` colour A, `u16` colour B, `u8` amount. [D] 16,141 meshes carry tables. [D] Rule [R]: the game uses
the **last** table whose condition matches ([world-file.md](world-file.md) 6); within it the first set for a group;
one record per position of that group. Colours are 15-bit (5 bits per channel shifted left by 3, top bit = alpha
0x80). A strip list whose flags have bit `0x20` takes its colours from the table instead of the vertex colours.
*Interpretation:* A is a baked ambient colour and B a baked light amount; kinds 1 and 7 draw palette slot 1 × A and add
slot 0 × B at the record's amount. [R]

### 4.3 Outline (type 1; Sly 2's skinned)

`u32` outline colour (mostly `80101010`), `s16` group count; per group `u8` vertex count `n`, `n` × `vec3`; if `B` > 0
`u8` slot count `k`, `k` slot bytes, `n` × `k` weight bytes (vertex by vertex); `u8` edge count `e`, `e` × 4 `u8`. [D]

### 4.4 Effect outline (type 2; Sly 2's streamed)

`s16` group count; per group `u16` shader index, then as 4.3's group (no colour). [D]

### 4.5 Indexed (type 3)

`s16` group count; per group `u16` shader index, `u8` index count `n`, `u8` position count P, P × `vec3`; if `B` > 0:
4 slot bytes and P × min(B, 4) weight bytes; `n` × `u8` indices; ceil(n / 32) × `u32` strip-break bits. [D]

## 5. Bones and weights

As Sly 2 section 5. Every vertex's weights sum to 255 (all 949,012 weighted vertices) and are stored **vertex by
vertex** (each vertex's weights together). [D] 4,808 meshes carry bone tables (53,377 entries). [D]

## 6. Copies

Per copy [D]:

| Field | Type |
|---|---|
| (unknown) | `s8`, `s8` (the second is new in Sly 3) |
| (unknown) | `u32`, only when the set's no-bounds flag is 0 |
| transform | `s16` reference |
| matrix | 48 bytes |
| (unknown) | `s32` |
| flags | `s8` (bit 0: hidden) |
| colour tables | 4.2 |

26,084 copies in all. [D]

## 7. Strips, outlines and passes

- Strip decoding is Sly 2's (section 8 there): vertex `i` closes (i−2, i−1, i) unless its break bit is set, winding
  by the absolute index, degenerate triangles dropped. 2,692,438 triangles over the disc. [D]
- Outline edges are (OA, E0, E1, OB) as in Sly 2; the earlier research confirmed the order on v_apt (74 % of implied
  faces repeat under this order against at most 18 % under any other). [R]
- Render pass by body type [D]: strips 2 (17,186), 9 (11,427), 3 (9,200), 1 (1,229), 10 (390), 4 (370), 0 (159),
  14 (72), 15 (44), 5 (1); outline 8 (9,516); effect outline 6 (1,137); indexed 6 (1,218), 7 (18). The numbers are
  Sly 2's minus one ([textures.md](textures.md) 4).

## 8. Test vectors

World slot 5 (v_apt) [D]:

- Level object (class 48, object 1, body at `0x6C4C93`): 101 strip meshes, 4,447 triangles. First mesh at `0x6C4D2B`
  (its count bytes `00 00` are at `0x6C4D29`; the next mesh's count bytes are at `0x6C4E3F`, the next mesh at
  `0x6C4E41`), 276 bytes: type 0, no copies, transform 1, pass 2, draw order none, flags `0x08`, fog factor = largest `f32`,
  billboard −1, no bones, 1 vertex group of 4 positions; position 0 = (−296.3776, −5000.6582, 1106.9935), texture
  coordinate (0, 1), normal (0, 0.10029, −0.99496), colour `ff ff ff ff`; two strip lists, both shader 50, flags 35:
  indices (1, 1, 2, 3) with breaks on the first three, and (0, 1, 2) with breaks on the first two; triangles (2, 1, 3)
  and (0, 1, 2).
- Sly (class 52, object 24): 97 meshes (52 strip, 38 outline, 7 effect outline; 4 conditional), 3,019 triangles.
  First mesh at `0x382058`, 3,653 bytes: type 0, transform 4, pass 3, flags `0x09`; first group 25 positions, position 0
  (5.6830, −14.3578, −1.3138), two strip lists of shader 7, flags 3; first triangles (0, 1, 2), (2, 1, 3), (2, 3, 4).

Totals per world: [worlds.md](worlds.md).

## 9. Open questions

- The unknown header and copy fields and the mesh set's `s32` list.
- The view-angle block, the box and the distortion block.
- How colour-table amounts and the runtime lighting combine (rendering spec).
