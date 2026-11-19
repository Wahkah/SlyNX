---
id: sly1-classes-paths-and-maps
title: Sly 1 object stream, curve shapes, navigation, reach and visibility maps, screen mask
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46 decompressed worlds
    used_for: layouts verified by full walks; visibility-map child encoding checked (indices in range, leaf masks)
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files shape, crv, pathzone, rchm, vis, keyhole
    used_for: field order and meaning
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), File Extractor GUI / NativeTools / BrxRebuilder (ObjectWalker)
    used_for: retail layout, in particular the retail encoding of visibility-map children
date: 2026-11-13
---

# Curve shapes, navigation, reach and visibility maps, screen mask

None of these layouts has a transform of its own or a child list; curve and mesh
positions are in the space of the parent placed object, or world space at the top level.

## 1. Curve shape (classes 110 to 114)

Classes: curve shape (110), curve shape variant ? (111), pipe (112), rail (113).

| Order | Field | Type |
|---|---|---|
| 1 | curve kind | `u8` (0 polyline, 1 cubic) |
| 2 | curve | shared-structures.md, curve |
| 3 | options | option list |

## 2. Path zone, navigation mesh (class 138)

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | options | option list | |
| 2 | vertex count V | `u16` | |
| 3 | boundary edge count | `u16` | |
| 4 | interior edge count | `u16` | |
| 5 | triangle count | `u16` | |
| 6 | V vertices | each: `vec3` position, `u8` neighbour count n, n × `u16` vertex index | |
| 7 | edges | (boundary + interior) × 4 bytes | |
| 8 | triangles | triangle count × 6 bytes | |
| 9 | tree node count | `u16` | |
| 10 | tree nodes | count × 20 bytes: `vec3` normal, `f32` distance, `s16`, `s16` | |

## 3. Reach map (class 139)

One per world that has the player character (41 records).

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | header values | 24 × `s16` | |
| 2 | unknown | `s16` | |
| 3 | mesh | collision mesh (shared-structures.md) | |
| 4 | per-position values | P × `s8` | P = that mesh's position count |
| 5 | tower count | `s16` | |
| 6 | towers | count × 8 bytes | |
| 7 | tree node count | `s16` | |
| 8 | tree nodes | count × 6 bytes | |
| 9 | unknown | `f32` | |
| 10 | options | option list | |

## 4. Visibility map (class 134)

At most one per world (19 worlds have one). When present it is the first top-level record. It splits space into
up to 32 zones; objects and meshes carry zone masks (placed-object.md). A static renderer may ignore it.

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | node count N | `u16` | |
| 2 | N nodes | 24 bytes each (below) | |
| 3 | zone count Z | `u16` | at most 32 |
| 4 | one-hop masks | Z × `u32` | for each zone, the mask of zones visible from it |
| 5 | options | option list | |

**Node** (disc encoding):

| Field | Type | Notes |
|---|---|---|
| normal | `vec3` | split plane: normal · x = distance |
| distance | `f32` | |
| negative side | `u32` | if bit 31 is set: a leaf; bits 0 to 30 are the zone mask of that region. Otherwise the index of a child node (below N) |
| positive side | `u32` | same encoding |

To find the zones of a point, start at node 0, go to the negative or positive side according to the sign of
(normal · point − distance), and stop at a leaf.

## 5. Screen mask (class 142)

One per world (class 142): the shape used for the iris ("keyhole") transition on screen.

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | options | option list | |
| 2 | point count P | `u16` | |
| 3 | points | P × (`f32` x, `f32` y) | 2D |
| 4 | bounds | 4 × `f32`: min x, min y, max x, max y | |
| 5 | five triangle sets | 5 × (`u16` count, count × 3 × `u16` point indices) | one set per mask colour |

- **y runs up**: the points are in a y-up plane; on a y-down screen negate y (the reference does so when loading).
  The bounds are symmetric (−35 to 35), so they are the same either way. [R] [D]
- **Each set has its own colour**, from the world's shaders with object ids **1166 + set** [R]; on the disc, in
  every world [D]: set 0 → shader 1166 colour `07 1e 98 bf` (dark blue), sets 1 and 2 → 1167, 1168 `00 51 de 7f`
  (blue), sets 3 and 4 → 1169, 1170 `ff ff ff 7f` (white). The shape is Sly's mask: set 0 the area around it inside
  the bounding box, sets 1 and 2 the two halves of the mask, sets 3 and 4 the eyes [I from the colours]. None of the
  sets is an opening: the whole screen is covered as the wipe closes (level-flow.md 5).
