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
date: 2026-11-19
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
| 6 | collision mesh | 2.1 | positions, convex polygon surfaces with edges |
| 7 | collision tree | Sly 2 2.2 | three `u32` counts, nodes |
| 8 | (unknown) | `u32`, `u16` | |
| 9 | (unknown) | `s8` | **new in Sly 3** |
| 10 | (unknown) | `s16`, `s8` | |

[D for the sizes]

### 2.1 Collision mesh: surfaces are convex polygons [D]

The mesh layout is Sly 2's ([../../sly2/classes/solid-object.md](../../sly2/classes/solid-object.md) 2.1): `u16`
position count, positions, `u16` surface count, `u16` edge total, then per surface three `u16` **corners**, `f32`
radius, `vec3` centre, `u8` edge count n and n × (two position indices, the neighbouring surface). **A surface is a
convex polygon, not a triangle:**

- The three corners only fix the surface's plane (normal = normalise((c1 − c0) × (c2 − c0)), pointing out of the
  solid; [../camera.md](../camera.md) 3).
- The polygon's outline is its **edge list**: each edge is one side, from one position to the next, and names the
  surface on the other side. The sides' end positions go round the polygon, and they include positions that are not
  among the three corners.
- A side shared with no other surface may be missing from the list. Example: v_apt's surface 281 below lists 9
  sides of 10.
- The polygon is the convex hull of its corners and edge positions, all in the plane.

Over v_apt's level mesh (1,356 surfaces), counting corners and edge positions together: 1,011 surfaces have 3
positions, 321 have 4, and 24 have 5 to 28.

**Test vector** (v_apt, level object 1, the collision part of section 4):

- Surface **281** has corners (173, 177, 172) and lists 9 sides: (171, 172), (172, 173), (173, 174), (174, 175),
  (175, 176), (176, 177), (178, 179), (179, 180), (180, 171), with neighbours 314, 324, 316, 315, 312, 304, 310,
  311, 313.
- It is a **10-sided floor at z = −33** over positions 171 to 180, for example 171 = (−2,888.1, 6,501.9, −33.0)
  and 172 = (−3,536.0, 5,889.5, −33.0).
- The side (177, 178) is not listed.
- Treating the surface as the triangle of its corners leaves most of this floor without collision.

### 2.2 Facing: one-sided, given by the corners and the tree

Surfaces are **one-sided**. A surface's normal is fixed by the order of its three corners, (c1 − c0) × (c2 − c0)
(computed at load, as Sly 2 [R]). Which side is solid is fixed by the collision tree. At a node, a point at plane
distance below 1e−4 goes to the negative child, otherwise to the positive child. A missing negative child means
**solid**; a missing positive child means **empty** [R; checked on the disc below, D]. The solid is therefore behind
the normal. A down-facing surface is the **underside** of something, never a floor to stand on.

**Test vector** (v_apt, level object 1) [D]:

- Surface **352** has corners (256, 262, 255) at z ≈ −1, for example 256 = (1,853.9, −5,163.6, −1.0). Its normal is
  (0, 0, −1), and its 13 sides span x −2,022 to 1,854.
- Surface **442** has corners (1,304.2, −5,093.1, 51.6), (1,304.2, −4,870.3, 51.6) and (1,269.9, −5,674.7, 51.6).
  Its normal is (0, 0, +1).
- Walking the tree straight down at x 500, y −5,100 gives: empty above z 51.6; **solid from 51.6 down to −1**
  (behind 442, ending at node 1063); empty below −1 (in front of 352, node 1061, no positive child). The same holds
  at (−1,500, −5,500), (−500, −5,300) and (0, −5,000). Below 352, the tree is empty down to at least z −300.
- So 352 is the bottom of a slab about 52 units thick, and **the floor there is 442 at z 51.6**. A walker that
  stands a character on 352 has taken the slab's underside for a floor. It should only stand on surfaces whose
  normal points up (or, more generally, keep the character on the empty side of the tree).
- For comparison, the coffee-shop floor (section 2.1) faces up: (−2,500, 4,000, −20) is empty and
  (−2,500, 4,000, −60) is solid.

**The v_apt arrival floor** [D]. At the arrival point (about (1,586, −5,000)) the tree column gives:

- empty below z −0.5 (under 352);
- solid from −0.5 to **46.0**, behind surface **401**;
- empty from 46.0 to 378.5;
- solid above 378.5, behind surface 400, the ceiling.

Surface 401 is the floor there. Its corners are (1,739.0, −5,099.4, 42.6), (1,304.2, −4,870.3, 51.6) and
(1,304.2, −5,093.1, 51.6), and its normal is (0.021, 0, 1.0): a gentle slope, z 51.6 at x 1,304 falling to 42.6 at
x 1,739. At x 1,700 the floor is at 43.5; at x 1,414 it is 49.5. At y −5,100 the column is solid from −0.5 upward:
that point is inside a wall, just past 401's edge at y −5,099.4. The arrival point is fine. Stand Sly on 401 at
z ≈ 46, not on 442 (whose surface ends near x 1,414) and not on 352.

There are 413 surfaces with |normal z| > 0.9 in this mesh: 228 face up and 185 face down. Floors and undersides are
both common.

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
