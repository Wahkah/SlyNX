---
id: sly3-classes-solid-object
title: Sly 3 solid layout
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97464 image, all 39 worlds
    used_for: every solid body (11,524 at every depth) parsed to its declared size; the counts below
  - kind: reference-project
    what: earlier research (Games/Sly3-RE-notes.md, tools/sly3_geom.py, 2026-11); SafeHouse @ c70803ce, Sly3-Release object loaders
    used_for: first reading of the field order; re-checked by the parse
reviewed_by: pending
date: 2026-11-14
---

# Solid layout (Sly 3)

The body of every class marked **solid** or **solid + name** in [../object-stream.md](../object-stream.md) 3.2. Same
plan as Sly 2's solid layout ([../../sly2/classes/solid-object.md](../../sly2/classes/solid-object.md)); the
differences are the mesh set ([../geometry.md](../geometry.md)) and two extra bytes per collision part. Tags as in
[../disc.md](../disc.md).

## 1. Layout

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | transform list | [../transforms.md](../transforms.md) 2 | |
| 2 | zone mask | `u32` | |
| 3 | mesh set | [../geometry.md](../geometry.md) 2 | |
| 4 | collision part count `c` | `s16` | |
| 5 | `c` collision parts | section 2 | 5,632 parts on the disc [D] |
| 6 | mass and inertia | `f32`, then 3 × `vec3` | only if `c` > 0 |
| 7 | option list | [../object-stream.md](../object-stream.md) 1 | |
| 8 | children | object list ([../object-stream.md](../object-stream.md) 2.1) | |
| 9 | name | `u16` length, then that many bytes | only for **solid + name** classes (49, 50, 52 to 88) |

Every solid body ends exactly here. [D]

## 2. Collision part

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | shape ID | `s16` | as Sly 2 (−1 on level geometry) |
| 2 | (unknown) | `s32` | |
| 3 | (unknown) | `s8`, `s8` | |
| 4 | (unknown) | `f32` | |
| 5 | (unknown) | `s8` | **new in Sly 3** |
| 6 | collision mesh | Sly 2 2.1 | positions, surfaces with edges |
| 7 | collision tree | Sly 2 2.2 | three `u32` counts, nodes |
| 8 | (unknown) | `u32`, `u16` | |
| 9 | (unknown) | `s8` | **new in Sly 3** |
| 10 | (unknown) | `s16`, `s8` | |

[D for the sizes]

## 3. Classes and counts

Bodies over all depths: 11,524, of which 436 carry a name. [D] Per class: [../object-stream.md](../object-stream.md)
3.2. Class 47 is the common prop and scenery class (8,078 bodies); class 48 (68 bodies in 38 worlds) is the large
level-geometry object of each world. [D]

## 4. Test vectors

World slot 5 (v_apt), level group, first record (class 48, object 1, body of 402,021 bytes at `0x6C4C93`) [D]: 2
transforms (0: parent −1, kind 1, ID 1, identity; 1: parent 0, kind 6, ID 1, identity), no limbs, mirrors or chains,
trailing references (0, 1, 1), zone mask `0xFF`, 101 meshes (all strip meshes, none conditional) giving 4,447
triangles, 1 collision part (transform −1; 1,390 positions, 1,356 surfaces, 3,411 tree nodes), 1 option, no
children.

Sly in v_apt (class 52, object 24, body of 1,090,509 bytes at `0x37FDE3`, name `jt`) [D]: 146 transforms, 4 limbs,
1 mirror (transform 6 → 73, rotation), 4 chains, references (1, 2, 2), 97 meshes (52 strip, 38 outline, 7 effect;
4 conditional), 3,019 triangles, 28 collision parts, 25 options, 356 children.

## 5. Open questions

- The unknown collision-part fields, including the two new `s8`.
