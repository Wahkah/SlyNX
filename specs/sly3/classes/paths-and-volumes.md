---
id: sly3-classes-paths-and-volumes
title: Sly 3 paths, volumes and zones
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97464 image, all 39 worlds
    used_for: every body of these classes parsed to its declared size
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly3-Release loaders for volumes, shapes, paths, mesh zones, zones and groupers
    used_for: first reading of the field order; re-checked by the parse
reviewed_by: pending
date: 2026-11-14
---

# Paths, volumes and zones (Sly 3)

"Header" is the transform header of [../object-stream.md](../object-stream.md) (parent, matrix, zone mask).
"Collision mesh" and "collision tree" are the Sly 2 structures ([../../sly2/classes/solid-object.md](../../sly2/classes/solid-object.md)
2.1 and 2.2). Every body of each class parses to its declared size. [D] Tags as in [../disc.md](../disc.md).

## 1. Volume (class 7)

Header, `u32` (meaning open), collision mesh, collision tree, option list. 25 bodies, all in `…_vz` groups
(visibility zones). [D]

## 2. Shape (classes 19, 20, 21, 23, 25)

Header, option list, collision mesh, collision tree. [D]

## 3. Shape + keys (class 24)

Section 2, then a 64-byte file record ([../disc.md](../disc.md) 4), then `u8` count and that many more 64-byte file
records. 503 bodies, all in macro groups. [D] *Interpretation:* exits to other worlds (the records name world files);
a record counts as set when its words 1 and 15 differ. [R]

## 4. Path (classes 9 to 13)

| Order | Field | Type |
|---:|---|---|
| 1 | parent, matrix, zone mask | header fields |
| 2 | tangent flag `t` | `s8` |
| 3 | (unknown) | `s8` |
| 4 | point count `n` | `u16` |
| 5 | per-point values | `n` × 4 bytes |
| 6 | points | `n` × `vec3` |
| 7 | tangents | only if `t` ≠ 0: `n` × `vec3`, then `n` × `vec3` |
| 8 | options | option list |

Class 9 is the commonest (2,873 bodies, mostly in `…_pz` groups). [D]

## 5. Mesh zone (class 16)

| Order | Field | Type |
|---:|---|---|
| 1 | header, options | as simple-layouts 2 |
| 2 | vertex count `v`, triangle count `t` | `u16`, `u16` |
| 3 | vertices | `v` × (`vec3`, `u8` count `k`, `k` × `u16` neighbour indices) |
| 4 | triangles | `t` × 3 × `u16` |
| 5 | points | `u16` count, then each 3 × `f32`, `u16`, `u16` |

612 bodies, up to 49,437 bytes. [D] *Interpretation:* navigation meshes. [I]

## 6. Zone (class 14)

Header, collision mesh, collision tree, option list, then an object list whose objects are placed by the zone's
matrix (as a placement's extra objects). [D for the layout, R for the placement] 624 bodies. [D]

## 7. Grouper (class 29)

Header, option list, then an object list placed by the grouper's matrix. [D] The game keeps the listed objects out of
the world until gameplay adds them. [R] 112 bodies. [D]
