---
id: sly2-classes-paths-and-volumes
title: Sly 2 object stream, curve, path, surface and volume layouts
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97316 (USA) disc image; executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 decompressed worlds
    used_for: every body of the classes below, at every depth, parsed field by field to its declared size; counts and value histograms
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), src/splice-ref-crv (curve loader), src/path (path-network loader), src/tn (track loader), src/new_184f98 (triangle-surface loader), src/new_2271f8 (zone, volume and warp loaders), src/mpeg-memcard (obstacle loader)
    used_for: first reading of the field order; every field re-walked on the disc
date: 2026-11-14
---

# Curve, path, surface and volume layouts (Sly 2)

Notation as in [../object-stream.md](../object-stream.md). Two building blocks recur:

- **Transform header**: `s16` parent reference, 48-byte matrix (three axis images, translation), `u32` zone mask
  ([simple-layouts.md](simple-layouts.md) 2, without the option list).
- **Collision mesh and tree**: the collision-mesh and collision-tree formats of [solid-object.md](solid-object.md)
  2.1 and 2.2 (not the whole collision part).

All counts are bodies over all 43 worlds, every depth; every body ends exactly at its declared size [D].

## 1. Curve (classes 10, 11, 12, 13, 14)

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | transform header | | |
| 2 | tangent flag | `s8` | non-zero: the curve stores tangents (rows 6 and 7) |
| 3 | (unknown) | `s8` | |
| 4 | point count `n` | `u16` | |
| 5 | parameters | `n` × `f32` | one value per point (*interpretation: distance or time along the curve*) [I] |
| 6 | points | `n` × `vec3` | |
| 7 | tangents | only if the flag is set: `n` × `vec3` incoming, then `n` × `vec3` outgoing | |
| 8 | options | option list | |

Tangent flag set / clear [D]: class 10: 868 / 2,272; 11: 11 / 235; 12: 25 / 18; 13: 65 / 288; 14: 0 / 179.
Class 14 (*interpretation: an obstacle made of a curve*) adds nothing to the stream [R].

## 2. Track (class 15)

| Order | Field | Type |
|---:|---|---|
| 1 | transform header | |
| 2 | collision mesh, collision tree | |
| 3 | options | option list |
| 4 | attached objects | object list (0 or 1 object on the disc: 404 bodies with none, 215 with one) [D] |

A single attached object is the curve the track follows [R].

## 3. Path network (class 17)

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | transform header, options | | as [simple-layouts.md](simple-layouts.md) 2 |
| 2 | vertex count `v` | `u16` | |
| 3 | triangle count `t` | `u16` | |
| 4 | vertices | `v` × (`vec3` position, `u8` link count `k`, `k` × `u16` linked vertex) | |
| 5 | triangles | `t` × 3 × `u16` vertex indices | |
| 6 | point count `p` | `u16` | |
| 7 | points | `p` × (3 × `f32`, `u16`, `u16`) | 16 bytes each |

*Interpretation:* a navigation mesh for characters, with linked vertices and helper points [I].

## 4. Triangle surface (class 9)

| Order | Field | Type |
|---:|---|---|
| 1 | transform header | |
| 2 | vertex count `v` | `s16` |
| 3 | vertices | `v` × `vec3` |
| 4 | triangle count `t` | `s16` |
| 5 | triangles | `t` × 3 × `s16` vertex indices |
| 6 | neighbours | `t` × 3 × `s16` (the triangle across each edge) |
| 7 | options | option list |

9 bodies in 2 worlds [D].

## 5. Zone (class 5)

| Order | Field | Type |
|---:|---|---|
| 1 | transform header | |
| 2 | (unknown) | `u32` |
| 3 | collision mesh, collision tree | |
| 4 | options | option list |

30 bodies, all in packages named `…_vz` [D].

## 6. Volume (classes 22, 23, 24, 25, 27) and warp (class 26)

Volume:

| Order | Field | Type |
|---:|---|---|
| 1 | transform header, options | as [simple-layouts.md](simple-layouts.md) 2 |
| 2 | collision mesh, collision tree | the volume's shape |

Classes 24 and 25 (*interpretation: volumes that act as buttons when entered*) and 23 and 27 add nothing to the
stream [R].

Warp (class 26): the volume layout, then:

| Order | Field | Type | Notes |
|---:|---|---|---|
| 3 | destination | 64-byte file record ([../disc.md](../disc.md) 4) | a world file; all-zero for none |
| 4 | extra count `n` | `u8` | 0 (206 bodies), 1 (158), 2 (6) [D] |
| 5 | extra destinations | `n` × 64-byte file record | |

A record whose second word equals its last word decodes to size 0 and means "none" [R].

## 7. Markers with a name (classes 41, 42)

Transform header, option list, then one `u16`-length string [D].

## 8. Markers without extra data (classes 16, 21, 36, 40, 45, 47)

Transform header and option list ([simple-layouts.md](simple-layouts.md) 2) [D].

## 9. Test vectors

First body of each layout in the walk (level-table row, body offset in the decompressed world, body size) [D]:

| Class | Row | Body offset | Body size |
|---:|---:|---:|---:|
| 5 | 0 | 141,747 | 450 |
| 9 | 32 | 2,146,788 | 1,176 |
| 10 | 0 | 750,631 | 300 |
| 15 | 0 | 4,596,286 | 516 |
| 17 | 0 | 210,169 | 1,386 |
| 22 | 0 | 2,678,007 | 863 |
| 24 | 0 | 261,880 | 463 |
| 26 | 0 | 7,936,364 | 595 |
| 41 | 0 | 7,589,952 | 72 |

Bodies parsed field-exactly over all worlds and depths [D]: class 5: 30; 9: 9; 10 to 14: 3,140, 246, 43, 353, 179;
15: 619; 17: 986; 22 to 27: 399, 42, 482, 3, 370, 159; 41: 228; 42: 282.
