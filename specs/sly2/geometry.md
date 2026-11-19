---
id: sly2-geometry
title: Sly 2 model geometry (mesh sets), skinning data and outlines
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97316 (USA) disc image; executable SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 decompressed worlds
    used_for: >-
      every mesh set in all 11,250 solid bodies parsed to the byte (53,994 meshes, 148,154 strip lists, 2,821,629
      triangles); weight sums, edge-record ranges, strip-start bits, shader-index ranges and type and pass counts
      measured over all worlds; test vectors below
  - kind: reference-project
    what: Games/Sly2-RE-notes.md and tools/sly2_geom.py (earlier research; it renders all 43 levels and poses the characters); sly2-decomp @ 33e2bab9
    used_for: field order; meanings marked [R]
  - kind: public-doc
    what: PS2 VIF unpack formats (element sizes) and GS triangle strips (vertex kick, no-drawing flag)
    used_for: the sizes of the vertex arrays of skinned and streamed meshes; strip decoding
date: 2026-11-13
---

# Sly 2 geometry

Terms as in [../sly1/geometry.md](../sly1/geometry.md): **mesh set**, **mesh**, **vertex group** (Sly 1's
"submesh" holds one strip; a Sly 2 vertex group holds shared vertex arrays and one or more strip lists),
**strip list**, **outline edges**. The mesh set is part of the solid layout (classes/solid-object.md row 3). Tags
as in object-stream.md. All fields packed, little-endian.

## 1. What changed from Sly 1

| Item | Sly 1 | Sly 2 |
|---|---|---|
| Where | drawable block of the placed layout | solid layout; one object can carry many meshes on many transforms |
| Mesh kinds | one (strip submeshes) | four body types (section 4) |
| Vertex data | per-submesh arrays, 4-byte strip entries | per-group arrays shared by strip lists; 1-byte position index per strip vertex plus a bit array for strip breaks |
| Instances | instance meshes with a matrix | "copies" of a mesh, each with a matrix and a transform |
| Per-level variants | none | conditional meshes (2.2) |
| Outlines | outline parts | outline edges inside skinned and streamed meshes (section 6) |
| Weights | `f32`, sum 1.0 | bytes, sum 255 |

## 2. Mesh set

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | no-bounds flag | `s8` | non-zero: meshes and copies omit their bounding-sphere fields (3.1, 7) |
| 2 | bounding sphere ? | `vec3`, `f32` | [I] |
| 3 | (unknown list) | `u8` count, each `u16` and 32 bytes | |
| 4 | (unknown list) | `u8` count, each `s32` | |
| 5 | slot count S | `s16` | |
| 6 | meshes | section 2.1 | |
| 7 | bounding tree | section 2.3 | |

### 2.1 Reading the meshes

Repeat while the number of used slots is below S:

- Read `s8` condition count `c`.
- If `c` > 0: read `c` × `s16` level rows and an `s32` size, then a mesh of exactly that size (section 3). A
  conditional mesh does **not** use a slot: reduce S by one instead [D for the sizes, R for the rule].
- Otherwise read a mesh (section 3); it uses `1 + copy count` slots.

On the disc: 53,994 meshes, 10,820 of them conditional; no conditional mesh has copies, so the count works the same
whether a condition holds or not [D].

### 2.2 Conditional meshes (per-level variants)

A conditional mesh is loaded only when the current level row (the world's row in the level table, disc.md) is one of
its listed rows [R]. Lists hold 1 to 24 rows; the rows are 0 to 43 [D]. Example [R]: one character body is listed
for rows 0, 12, 22 and 38 to 43, an alternative body for row 11. A renderer for a given world keeps the meshes whose
list contains that world's row, plus all unconditional ones.

### 2.3 Bounding tree

| Field | Type |
|---|---|
| node count | `s16` |
| nodes | each up to 4 entries: `s8` kind (−1 ends the node early), `vec3`, `f32`, `f32`, `u8`, and an `s16` when the kind is 0 or 1 |

Used for culling [R]; 104 mesh sets have a tree [D]. Not needed to draw.

## 3. Mesh

### 3.1 Header

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | body type | `s8` | 0 rigid (41,232), 1 skinned (9,325), 2 streamed (1,082), 3 indexed (2,352) [D] |
| 2 | copy count | `u16` | section 7 |
| 3 | transform | `s16` transform reference (transforms.md 1) | the mesh is drawn with this transform's world matrix |
| 4 | render pass | `s8` | section 8 |
| 5 | maximum draw distance | `f32` | largest `f32` = unlimited |
| 6 | (unknown) | `f32`, `s32` | |
| 7 | (unknown) | `s8` | −1 in 51,023 meshes, else 0 to 2 [D] |
| 8 | (unknown) | `s8` | 0 or 1 [D] |
| 9 | bounding sphere | `vec3` centre, `f32` radius, `u32`, `f32` | only when the set's no-bounds flag is 0 (41,679 meshes) [D] |
| 10 | body | section 4, by type | |
| 11 | copies | section 7 | |

## 4. Bodies

Every body has a **bone table**: `u8` count `B`, then `B` × (`s32` object ID, `s16` transform reference)
(section 5), first in the body except for rigid bodies (4.1) [D].

### 4.1 Rigid (type 0)

In order [D]:

| Field | Type | Notes |
|---|---|---|
| flags | `u8` | |
| (unknown) | `s16` | |
| bone table | as above | |
| fog block | `s8`; if not −1, 32 more bytes | |
| vertex-map table | section 4.5 | |
| morph block | only if flags bit 2: `s32`, `s8` (if not −1: 16 bytes and a 48-byte matrix), then 3 × `u8` | |
| vertex-group count | `s16` | |
| vertex groups | below | |

(The rigid body is the one type whose bone table is not first.)

**Vertex group**:

| Field | Type | Notes |
|---|---|---|
| strip-list count `L` | `u8` | 2 in every group on the disc [D] |
| strip-list headers | `L` × (`s32` vertex count, `s32` shader index, `s32` flags) | flags 1, 3 or 35 seen [D] |
| position count P | `u8` | at most 255 |
| positions | P × `vec3` | |
| texture coordinates | P × 2 × `f32` | |
| normals | P × `vec3` | |
| colours | P × 4 bytes R, G, B, A | 0x80 = 1.0 [R] |
| (unknown) | `s8` | |
| bone slots and weights | only if B > 0: 4 × `u8` slots (indices into the bone table), then min(B, 4) bytes per position | section 5 |
| strip indices | (sum of the `L` vertex counts) × `u8` position index | the strip lists one after another |
| strip-break bits | ceil(sum / 32) × `u32` | bit `i` of the concatenated list (bit `i mod 32` of word `i / 32`) |

Both strip lists of a group are drawn, each with its own shader; they always differ in their indices [D].

### 4.2 Indexed (type 3)

Bone table, then `s16` group count, each group: `u16` shader index, `u8` index count n, `u8` position count P,
P × `vec3`, (if B > 0: 4 slot bytes and min(B, 4) × P weight bytes), n × `u8` indices, ceil(n / 32) × `u32`
strip-break bits [D]. No texture coordinates, normals or colours.

### 4.3 Skinned (type 1)

Bone table, then `s16` group count, each group [D]:

| Field | Type |
|---|---|
| vertex count n | `u8` |
| positions | n × `vec3` |
| weights | only if B > 0: `u8` slot count k (1 to 4), k slot bytes, then k × n weight bytes |
| outline-edge count e | `u8` |
| outline edges | e × 4 `u8` (section 6) |

### 4.4 Streamed (type 2)

Bone table, then `s16` group count, each group: `u16` shader index, then the same fields as 4.3 [D]. The edge
records' vertex numbers are **twice** the vertex index (divide by 2) [R; D that all are in range after halving].

### 4.5 Vertex-map table

`u16` count `m`; if `m` > 0: `m` × (`u16` count, then that many 5-byte entries), then `u16` count and that many
`s16` [D]. These are **pre-baked vertex colour sets** for static lit meshes: each 5-byte entry is `u16` colour A,
`u16` colour B (both 5-5-5-1) and `u8` alpha of B, one entry per source vertex; the trailing `s16` list gives a set
index per submesh [D for the layout, R for the use]. Their use when drawing is in rendering.md 2.5.

## 5. Bones and weights

- Bone table entries name an object (usually the mesh's own object; in 1.2 % another object of the same nested
  tree) and one of its transforms [R]. All 34,048 entries resolve [R].
- A rigid or indexed group has 4 slot bytes; each position has min(B, 4) weights, one per slot in order. Skinned and
  streamed groups have k slots and k weights per vertex.
- **Every vertex's weights sum to 255** (330,732 rigid and indexed positions; all 11,418 skinned and streamed
  groups) [D]. Weight w means w / 255.
- Bind pose and skin matrices: transforms.md 4.

## 6. Outline edges

Skinned and streamed groups carry no triangles; they are the silhouette hull for the ink outline [R]. Each record
is four vertex numbers in the order (OA, E0, E1, OB): the edge E0–E1 and the vertex opposite it in each of its two
faces — the same order as Sly 1's outline parts (../sly1/geometry.md 3.2) [R]. All 1,234,171 records index valid
vertices [D]. The outline is drawn where one face sees the camera and the other does not [R].

## 7. Copies

Each mesh is followed by its copy count of copies [D]:

| Field | Type |
|---|---|
| (unknown) | `s8` |
| (unknown) | `u32`, only when the set's no-bounds flag is 0 |
| transform | `s16` reference |
| matrix | 48 bytes |
| (unknown) | `s32` |
| flags | `s8` (0 or 1 seen) |
| vertex-map table | 4.5 |

A copy is drawn with: its matrix, then its transform's world matrix [R]. 4,484 meshes have copies [D].

## 8. Strips and triangles

Each strip list is decoded like Sly 1's strips (../sly1/geometry.md 3.3): vertex `i` (from 2) closes the triangle
`(i−2, i−1, i)` unless its strip-break bit is set; the winding alternates with the index `i` counted from the start of **each strip list** (not reset at strip breaks
inside the list) [D that the first two bits of every one of the 145,790 strip lists are set]. Triangles with a
repeated position index are degenerate and dropped. Checked against the stored normals on rows 0, 3 and 12: counting
per list, 319,992 of 321,431 triangles (99.55 %) face along their vertex normals; counting across a group's
concatenated lists, only 74 %, because 8,816 lists have an odd length [D].

## 9. Render pass and shader binding

- Shader index (strip-list header, indexed and streamed groups): an index into the **same package's** shader list
  (textures.md); all 148,142 checked (every list except the 12 in the copy extras of row 32, worlds.md 5) are in range [D].
- Render pass byte (header row 4) by body type, with counts [D]:

  | Body type | Passes (count) |
  |---|---|
  | rigid | 3 (21,186), 10 (9,318), 4 (8,492), 2 (810), 11 (565), 5 (431), 13 (362), 1 (68) |
  | skinned | 9 (9,325) |
  | streamed | 7 (1,082) |
  | indexed | 7 (1,694), 8 (658) |

  *Working assumption* [R]: 3 and 4 opaque (static and animated), 1 sky, 5 cel-shaded scenery, 10 translucent,
  11 and 13 large projected volumes (shadows, light shafts), 7 and 9 outlines, 8 indexed shadow shapes. For a first
  render draw passes 1, 3, 4, 5 and 8 opaque and 10 blended.

## 10. Test vectors

World row 3 (`f_nightclub_cellar`), level-geometry object (class 49, body at `0x878C33`) [D]:

- 541 meshes (539 rigid, 2 indexed), 26,908 triangles.
- First mesh at `0x87933F`, 402 bytes: rigid, no copies, transform 1, pass 10, maximum distance = largest `f32`,
  no bounding sphere (the set's no-bounds flag is set), no bones, 1 vertex group.
- That group: 8 positions; position 0 = (7117.2324, 153.9920, 312.1772), texture coordinate (1.0, 0.5718),
  normal (−0.67462, −0.73807, −0.01160), colour `ff ff ff ff`; two strip lists of 4 vertices each, both shader 146,
  flags 35; first list indices 0, 1, 2, 3 with break bits set on 0 and 1; triangles (0, 1, 2) and (2, 1, 3).

Player character in the same world (class 53, object 7): 76 meshes (40 rigid, 30 skinned, 6 streamed; 18
conditional), 4,407 triangles; first skinned mesh: 24 bone entries starting (object 7, transform 39), first group
88 vertices, slots (7, 11, 12, 8), first vertex weights (255, 0, 0, 0), 180 outline edges, the first two
(0, 1, 2, 3) and (1, 2, 0, 4) [D].

Totals per world: worlds.md.

## 11. Open questions

- The unknown header and copy fields and the two unknown lists of the mesh set (the vertex-map table is resolved: 4.5).
- Why every rigid vertex group holds exactly two strip lists.
- The exact meaning of each render pass (rendering spec).
