---
id: sly1-transforms
title: Sly 1 object transforms, units and axes
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46
      decompressed worlds
    used_for: >-
      matrix statistics (orthonormality, determinants, scale); column convention for object matrices checked with
      skinned meshes against their bone objects; test vectors in section 8
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files alo, proxy, math, cm, bis
    used_for: >-
      composition order of the parent chain, proxy placement, the ancestor override, the rotation-vector formula,
      the camera basis (which axis is up)
date: 2026-11-13
---

# Transforms, units and axes

This file is consistent with [geometry.md](geometry.md) section 6, which fixes the matrix convention for meshes and
instance meshes. It adds the object-side rules: which classes carry a transform, the parent chain including proxies,
the special cases, and the conversion to a GPU space.

## 1. Stored transforms

| Layout | Stored transform | Spec |
|---|---|---|
| placed layout, solid layout and the layouts that extend them | `mat3` rotation M + `vec3` position p | classes/placed-object.md, classes/solid-object.md |
| proxy | M + p | classes/proxy.md |
| transform marker, warp, volume, explosion group and emitters | M + p | classes/markers-and-volumes.md |
| level exit, camera zone | M + p | classes/markers-and-volumes.md |
| point, scan point | p only (rotation = identity) | classes/markers-and-volumes.md |
| base layout, curve shapes, maps | none: positions inside them are in the parent's space | classes/base-object.md, classes/paths-and-maps.md |

A `mat3` is three `vec3` stored one after the other; they are the **columns** c0, c1, c2 of M, the images of the
local X, Y and Z axes. A point x of the object's local space is at M·x + p in the parent's space, that is
x0·c0 + x1·c1 + x2·c2 + p.

## 2. World transform

For an object with local (M, p) and parent world transform (Mw_parent, pw_parent):

- Mw = Mw_parent · M
- pw = pw_parent + Mw_parent · p

Top-level objects (parent = the world) use Mw = M and pw = p. A vertex v of the object's mesh set is drawn at
Mw·v + pw; an instance mesh with instance matrix (I, t) draws its source mesh's vertices at Mw·(I·v + t) + pw
(geometry.md 6.3).

The parent is defined in object-stream.md 7.2. In stream order a parent is always complete before its children are
read, so world transforms can be computed while walking.

As a 4×4 column-vector matrix: W = [[Mw, pw], [0 0 0, 1]], and W_child = W_parent · L with L = [[M, p], [0, 1]].

## 3. Proxies

From classes/proxy.md, section 2:

- A proxy's own world transform W_proxy follows section 2 like any object.
- Each source (or clone of a source) is drawn at W_proxy · L_source, where L_source is the transform stored in the
  source's body. Its descendants compose below it as usual.
- Children listed after a single-source proxy are at W_proxy · L_child.
- The game then re-parents these objects (to the proxy's parent, or to the source) keeping their world transforms.
  For a static render, using the proxy as the parent of its sources and children gives exactly the same world
  transforms.

## 4. Special cases

- **Ancestor override** (extension flags bit 2 and bit 3, classes/placed-object.md section 3): the named basis
  object, which is an ancestor of the object, replaces the parent for the position part (bit 2: pw = pw_basis +
  Mw_basis · p) or for the rotation part (bit 3: Mw = Mw_basis · M). Bit 2 never occurs on the disc; bit 3 occurs
  on 50 objects, all with meshes (42 skeleton bones, 8 placed objects), so a renderer should honour it: find the
  basis object by OID among the object's ancestors (nearest first) and take Mw from it.
- **Degenerate matrices**: 48 placed objects store −`FLT_MAX` in their second and third columns
  (classes/placed-object.md 1.1). Use the identity instead. They and their children draw nothing.
- **Fixed world mesh**: the static world mesh (class 4) has M = identity and p = 0 on every world, so its meshes are
  already in world space (classes/solid-object.md section 2).
- **Stock templates** (OIDs 12 to 42 at top level): not placed in the world; skip them and their subtrees.
- **Unplaced instance meshes**: 201 instance meshes on the disc have all twelve values of their instance matrix
  at `FLT_MAX` magnitude (signs vary: all negative in 72 of them, mixed in the rest), a bounding-sphere centre of
  (−`FLT_MAX`, −`FLT_MAX`, −`FLT_MAX`) and, in 200 of them, a radius of −`FLT_MAX` (the other has radius 1100).
  They are in 10 worlds; 190 are in Bentley Comes Through, spread over six placed objects (OIDs 1464 to 1469; in
  OID 1464 all 60 instance meshes are in this state, in the others most of them). Do not draw an instance
  mesh whose instance matrix has any component of magnitude above 1e30. Their real meshes (non-instance) in the same
  mesh set are drawn normally. Whether the game fills these matrices in at run time is not established; in a static
  scene they show nothing.
- **Unplaced objects**: one proxy is stored at position (0, 0, `FLT_MAX`) (classes/proxy.md section 4). Any object
  whose world position has a component of magnitude above 1e30 is not drawn, nor are its descendants.

### 4.1 Clarifications

- **Position override (extension bit 2)** is never set on any object of the 46 USA worlds. A loader may still
  implement it as above, but no disc data exercises it, and the test vectors in section 8 do not depend on it.
- **Rotation override (bit 3)** occurs on 50 objects and is exercised by the disc.
- **Sentinel values.** `FLT_MAX` (and −`FLT_MAX`) in a transform means "not placed" or "not set", never a real
  coordinate. The three cases above (degenerate object matrices, unplaced instance meshes, the parked proxy) are
  the only places it appears in a transform on the disc.

## 5. Scale, shear and mirroring

- Object matrices: 37159 stored object and marker matrices on the disc; apart from the 48 degenerate ones all are
  orthonormal to within 1e-3, and none has a negative determinant. Object transforms are rigid.
- Instance matrices (geometry.md 3) carry scale: 23610 of the 36578 instance matrices have a determinant different
  from 1 (non-uniform scale and slight shear occur), and **40 have a negative determinant** (mirrored). For a
  mirrored instance, triangle winding is reversed: a renderer that culls back faces must flip its front-face rule
  for that draw.
- Transform normals with the inverse transpose of the 3×3 part and renormalise (geometry.md 6.1).

## 6. Rotation vectors in options

Options with code `v` that set an orientation, and typed arguments with tag 4, hold a **rotation vector** r: the
rotation axis is r / |r| and the angle is |r| radians, counter-clockwise about the axis in the right-handed sense.
If |r| ≤ 0.0001 the rotation is the identity. With unit axis (x, y, z), angle a, s = sin a, k = cos a and
t = 1 − k, the matrix columns are:

- c0 = (t·x·x + k, t·x·y + s·z, t·x·z − s·y)
- c1 = (t·x·y − s·z, t·y·y + k, t·y·z + s·x)
- c2 = (t·x·z + s·y, t·y·z − s·x, t·z·z + k)

This is the standard axis-angle rotation; no static-scenery option needs it in phase 1.

## 7. Axes, handedness and units

- **Right-handed, Z up.** Evidence: the game's camera basis stores forward, left and up as its three columns, and
  forward × left = up only in a right-handed system; its up vector is +Z; large ground surfaces have normal
  (0, 0, 1) (geometry.md 6.3); every world's vertical extent is much smaller than its horizontal extent (Paris
  world mesh: x −40380 to 40399, y −40398 to 45131, z −6327 to 14925); rotation-vector matrices (section 6) are
  right-handed.
- **Units.** Game units, roughly centimetres: the Paris hub has near clip 70 and far clip 60000, fog from 1000 to
  30000; the player character's skinned body spans about 200 units. Treat the scale as an inference; nothing in
  the data states it. Multiply by 0.01 for approximate metres.
- **Camera.** Field of view (world option 529) is in radians: 1.1 in Paris. Whether it is horizontal or vertical is
  not established here (renderer spec).

### 7.1 Conversion to a right-handed Y-up GPU space

Most GPU conventions (Vulkan with a projection that flips Y, OpenGL, deko3d with the usual set-up) use a right-handed
view space with Y up. Map game coordinates (x, y, z) to GPU coordinates (x', y', z') = (x, z, −y). As a matrix:

C = columns (1, 0, 0), (0, 0, −1), (0, 1, 0), that is x' = x, y' = z, z' = −y.

C is a proper rotation (−90° about X, determinant +1), so handedness and triangle winding are preserved. Convert a
world transform with W' = C · W (positions and directions) and a rotation with M' = C · M · Cᵀ if the whole object
hierarchy is converted; the simplest is to keep all game data in game space and apply C once, in the view matrix.

## 8. Test vectors

World positions computed with sections 2 and 3, all values from the disc. Matrices are written as their three
stored columns.

**Paris (world 1)**: a proxy at the top level holding a breakable whose child is a breakable piece.

| Object (file offset of its record) | Local M columns | Local p | World M columns | World p |
|---|---|---|---|---|
| proxy, class 83, OID 1392 (0x2A6F11) | (−0.525311, 0.85091, 0), (−0.85091, −0.525311, 0), (0, 0, 1) | (1380.6582, 176.8276, −2388.4368) | same as local | (1380.658, 176.828, −2388.437) |
| breakable, class 33, OID 1637 (0x2A6F48), proxy source | identity | (0, 0, −20.1402) | (−0.52531, 0.85091, 0), (−0.85091, −0.52531, 0), (0, 0, 1) | (1380.658, 176.828, −2408.577) |
| breakable piece, class 39, OID 1640 (0x2A9BF1), child of the breakable | (−0.164567, 0.986366, 0), (−0.986366, −0.164567, 0), (0, 0, 1) | (0, 0, −14.589) | (−0.75286, −0.65818, 0), (0.65818, −0.75286, 0), (0, 0, 1) | (1380.658, 176.828, −2423.166) |

**Muggshot's Turf (world 13)**: a solid object with a proxy child holding a breakable.

| Object (file offset of its record) | Local M columns | Local p | World M columns | World p |
|---|---|---|---|---|
| solid object, class 3, OID 1399 (0x3140F4), top level | (−0.318368, −0.886405, 0.336048), (0.941137, −0.338026, 0), (0.113593, 0.316267, 0.941845) | (−1886.7853, −2937.6765, 1050.5444) | same as local | (−1886.785, −2937.677, 1050.544) |
| proxy, class 83, OID 1401 (0x323097) | (0.867224, 0.497918, −0.000345), (−0.497918, 0.867223, −0.001285), (−0.00034, 0.001286, 0.999999) | (−194.9142, −89.0327, −572.6522) | (0.19247, −0.93713, 0.2911), (0.97455, 0.14781, −0.16853), (0.11491, 0.31613, 0.94173) | (−1973.572, −2915.919, 445.694) |
| breakable, class 33, OID 1402 (0x3230CE), proxy source | identity | (0, 0.0027, 47.0392) | same as the proxy's | (−1968.164, −2901.048, 489.992) |

## 9. Evidence for the column convention on object matrices

geometry.md 6.1 verifies the column convention on instance matrices. For object matrices, skinned meshes give an
independent check: a vertex weighted almost entirely (> 0.95) to one bone should lie near that bone object's
origin, expressed in the skinned object's space. Composing the hierarchy with the column convention gives
**median** vertex-to-bone distances of 28.0, 21.8, 22.5 and 22.7 units in Paris, A Rocky Start, A Perilous Ascent
and The Hideout (5204, 4095, 4658 and 369 vertices); reading the stored vectors as rows instead gives 116.2, 86.1,
91.5 and 58.1. These are medians, not means. This measurement resolved each bone OID to the first object with that
OID inside the skinned object's top-level subtree (else anywhere in the world), which is simpler than the lookup of
object-stream.md 7.4; a few vertices then resolve to a far object, and the mean (1,878 in Paris) is meaningless.
With the 7.4 lookup, Paris gives a median of 20.2 and a mean of 27.5. Either way the column reading is about four
times closer, so the column convention is used for object matrices too.
