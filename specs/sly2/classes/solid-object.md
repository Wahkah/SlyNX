---
id: sly2-classes-solid-object
title: Sly 2 object stream, solid layout
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97316 (USA) disc image; executable SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 decompressed worlds
    used_for: >-
      the layout below parses every solid body on the disc to its declared size: 11,250 bodies (4,368 top-level records, 6,879 nested and 3 in the extra list of a nested copy placement;
      worlds.md 2); collision-surface field check on three worlds (38,166 surfaces); counts below
  - kind: disc
    what: SCUS-97316, rows 0, 3 and the castle hub (00_0x073); every collision surface of every top-level solid body (69,709), re-checked 2026-11-19
    used_for: section 2.1, surfaces as convex polygons (edge ends against the plane, the triangle and the convex hull; sphere containment; convexity of stored edges; perimeter coverage)
  - kind: reference-project
    what: ProjectCane @ f77825d6, Sly1-Release geom.cpp and so.cpp (the engine keeps one corner per surface for its plane and no triangle)
    used_for: "only the plane is kept for contact tests" [R]
  - kind: reference-project
    what: Games/Sly2-RE-notes.md and tools/sly2_geom.py (earlier research); sly2-decomp @ 33e2bab9, src/sm-so
    used_for: field order; meanings marked [R]
date: 2026-11-13
---

# Solid layout (Sly 2)

The body layout of every class marked **solid** or **solid + name** in [../object-stream.md](../object-stream.md)
section 3. It carries an object's transforms, its drawable geometry, its collision, its options and its children.
Compare with Sly 1's solid and placed layouts ([../../sly1/classes/solid-object.md](../../sly1/classes/solid-object.md),
[../../sly1/classes/placed-object.md](../../sly1/classes/placed-object.md)): Sly 2 merges them into one layout and
moves the transform into a list at the front. Tags as in object-stream.md.

## 1. Layout

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | transform list | [../transforms.md](../transforms.md) section 2 | the object's transforms (one for most props, up to about 50 for characters) [D] |
| 2 | zone mask | `u32` | visibility zones ([../geometry.md](../geometry.md) uses none of it) [D] |
| 3 | mesh set | [../geometry.md](../geometry.md) section 2 | may be empty [D] |
| 4 | collision part count `c` | `s16` | 0 in 8,319 bodies; up to 23 [D] |
| 5 | `c` collision parts | section 2 | |
| 6 | mass and inertia | `f32`, then 3 × `vec3` | only if `c` > 0 [D] |
| 7 | option list | [../object-stream.md](../object-stream.md) section 1 | 0 to 203 options per body [D] |
| 8 | children | object list ([../object-stream.md](../object-stream.md) 2.1) | nested objects, which hang off this object's transforms (transforms.md 3) [D] |
| 9 | name | `u16` length, then that many bytes | **only for "solid + name" classes** (50, 53 to 78) [D] |

The body ends exactly here; the record's body size confirms it [D].

## 2. Collision part

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | shape ID | `s16` | −1 (135 parts), 1 (838), 2 (2,710), 3 (451), and many values up to 134 [D]; meaning open |
| 2 | (unknown) | `s32` | |
| 3 | (unknown) | `s8`, `s8` | |
| 4 | (unknown) | `f32` | |
| 5 | collision mesh | section 2.1 | |
| 6 | collision tree | section 2.2 | indices refer to the mesh of row 5 |
| 7 | (unknown) | `u32`, `s16`, `s16`, `s8` | |

**Field meanings found 2026-11-18** [R, from the reference loader; D that the values fit]: row 1 is a **transform
reference** (the part hangs on that transform; −1 none), not a shape ID; row 2 (`s32`) is the part's **tag**, by which
clips (events 15/16) and scripts switch parts on and off; row 3 holds two flags; row 4 is the part's radius; row 7 is
the **contact kind** (`u32`, a hit-kind bit set sent with contacts), the **contact value** (`s16`, the hit amount, −10
by default), a second `s16` (equal to the mesh's position count on the parts surveyed [D]) and a flag. See
[../guards.md](../guards.md) 6.3.

### 2.1 Collision mesh

Like Sly 1's collision mesh ([../../sly1/classes/shared-structures.md](../../sly1/classes/shared-structures.md),
"Collision mesh") **without the leading bounding radius** [D]:

| Field | Type |
|---|---|
| position count P | `u16` |
| positions | P × `vec3` |
| surface count T | `u16` |
| edge total E | `u16` (equals the sum of the per-surface edge counts) [D] |
| T surfaces | 3 × `u16` corners, `f32` radius, `vec3` centre, `u8` edge count n, n × (3 × `u16`: two position indices and the other surface) |

The four floats of a surface are its bounding sphere (radius first, then centre), as in Sly 1: for flat surfaces
the centre lies in the plane of the three corners [D]. The plane is not stored; it follows from the corners
(Sly 1 rule) [D].

**A surface is a convex polygon, not the triangle of its three corners.** The three corners only fix the
surface's plane; the surface itself is a convex polygon in that plane that contains them [D]. Measured on three
worlds (nightclub cellar, intro hub, castle hub; 69,709 surfaces of top-level solid bodies):

- Every edge record lies in the planes of both surfaces it joins: 69,703 of 69,709 surfaces have all edge ends in
  their own plane (the other 6 are within rounding of large surfaces) [D].
- **21,609 surfaces (31 %) are larger than their triangle**: at least one edge end lies in the plane but outside
  the triangle of the three corners [D]. Of these, about 19,965 are four-sided; the largest have 12 or more sides [D].
- The polygon is convex: in 65,912 of the 67,125 surfaces with edges (98.2 %) every edge end lies on the boundary
  of the convex hull of the corners and edge ends; 99 have an end inside it by less than 1 % of the surface's size,
  and 1,114 (1.7 %) by more [D]. The
  corners are usually polygon vertices, but 884 of the larger surfaces have one to three corners inside the polygon
  [D].
- The bounding sphere (the four floats) encloses the whole polygon, not just the triangle, on every one of the 67,125
  surfaces with edges [D].
- **The edge list is not the whole outline.** Each stored edge is a side shared with one other surface, stored once
  under one of the two (see the other-surface field). As in Sly 1 (../../sly1/collision.md 1), only **convex** shared
  sides are stored: of 80,745 stored edges, 77,671 are convex, 664 concave and 2,410 between surfaces in the same
  plane [D]. The stored edges' total length equals the outline's perimeter in only 18,664 of 67,125 surfaces; 2,584
  surfaces store no edge at all [D]. The rest of the outline (concave sides, open borders) is not in the data.
- So a port must not clip a surface to its triangle. The game keeps only the surface's plane for contact tests
  [R]; a surface's extent comes from the collision tree (2.2), whose cells are bounded by other surfaces' planes [I].

Test vector [D]: nightclub cellar (row 3), solid object 136751, collision surface 0, corners 13, 9, 7: a 7-sided
polygon in the plane x ≈ 128 whose seven stored edges, all owned by surface 0, form its closed outline: 7–10,
10–2, 2–1, 1–0, 0–13, 13–9, 9–7. Position 0 is (129.67, 103.0, 164.68), position 7 is (126.56, −165.83, −171.14).
Sly 3 shows the same on its identical layout (../../sly3/classes/solid-object.md 2.1).

### 2.2 Collision tree

Different from Sly 1:

| Field | Type | Notes |
|---|---|---|
| node count N | `u32` | |
| (copy of N) | `u32` | equals N in every tree [D] |
| (count) | `u32` | equals N in 45 of 292 trees sampled [D]; meaning open |
| N nodes | variable | `u16` surface index; if it is not `0xFFFF`: `u32` negative child, `u32` positive child. A node whose surface is `0xFFFF` has no further bytes (a leaf) [D] |

*Working assumption* [I]: the same partition as Sly 1's tree (classes/shared-structures.md, Sly 1 collision.md 2).

## 3. Classes on the disc

| Layout | Classes | Bodies (top + nested) |
|---|---|---:|
| solid | 48, 49, 79, 81, 82, 83, 85, 86, 88, 89, 91 to 108 | 10,887 |
| solid + name | 50, 53 to 55, 57 to 70, 72 to 78 | 360 |

Bodies per class (top-level and nested together) [D]: 48: 8,723; 49: 118; 98: 617; 105: 252; 81: 132; 85: 121;
86: 109; 79: 100; 83: 92; 100: 127; 89: 58; 107: 55; and fewer for the rest.

Name strings seen (game data) [D]: `jt` (class 53, 42 bodies), `murray` (54), `bentley` (55), `guard` (all 28 bodies of class 58), `neela` (60), `npc_carmelita` (63), `npc_boss_contessa` (64), `vault` (78), `safe_house`
(70), and the minigame names `hack_…` (72 to 75).

## 4. The level geometry object (class 49)

118 bodies in 42 worlds [D]. In world row 3 (`f_nightclub_cellar`) the level package's first record is class 49,
object 1, body 2,490,842 bytes at `0x878C33`: 31 transforms (transform 0 identity, no parent), zone mask `0xFF`,
541 meshes (539 rigid, 2 indexed) giving 26,908 triangles, 1 collision part (shape −1; 8,103 positions,
10,386 surfaces, tree of 30,659 nodes), 1 option, no children [D].

## 5. Open questions

- The unknown fields of the collision part and the third tree count. Values seen in four worlds (rows 0, 3, 12, 27;
  674 collision parts) [D]: the shape ID is −1 on the level geometry (class 49) and 1, 2 or 3 elsewhere (a
  class-specific shape selector [R]); the two `s8` flags are 0 or 1; the `f32` is a positive size (15 to about 211,
  *interpretation: a radius*); the `u32` is 0 or a single bit (2, 4, 32, 512, 1024, 4096, …), *interpretation: a
  category mask*; the first trailing `s16` is −10 on 628 of 674 parts (other values −1 to −8000; the game also uses
  −10 as the default for parts it adds itself [R]); the second trailing `s16` is a small even number (0 to 24); the
  last `s8` is 0 or 1.
- The meaning of the shape IDs.
