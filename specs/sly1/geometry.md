---
id: sly1-formats-geometry
title: Sly 1 model geometry (mesh sets) and placement
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 image (SHA-256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a), all 46 world files
    used_for: >-
      layouts walked on all 46 worlds (68,242 submeshes, no out-of-range reference); strip winding measured
      against stored normals (162,789 triangles agree, 1,659 disagree, 149 degenerate, worlds 1 and 12); instance
      matrix convention measured against stored bounding-sphere centres (worlds 1, 2, 12); value surveys; all test
      vectors below
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), native world-file walker (mesh-set and object-transform parts), triangle-strip converter, runtime transform and mesh-set loaders
    used_for: >-
      first reading of the record layouts, the strip-break bit and the world-transform composition; layouts
      re-walked and the winding and matrix rules re-measured on the disc data as listed above
reviewed_by: pending
date: 2026-11-13
---

# Sly 1 model geometry and placement

Terms (chosen for this spec):

- **mesh set**: all the geometry of one drawable object;
- **mesh**: one entry of a mesh set; either real geometry or an **instance** (a reference to an earlier mesh of the
  same set, with its own matrix);
- **submesh**: one draw: a triangle strip with a single shader;
- **outline part**: edge data for the cartoon (cel) outline drawn around a mesh.

Little-endian, packed, **no alignment except the one noted in 3.1**. `vec3` = 3 x `f32`. The material side
(shaders, bitmaps, palettes) is in [textures.md](textures.md).

## 1. Where geometry sits

Mesh sets are not in a section of their own. Every object class that can be drawn has, in its body in the object
stream ([world-file.md](world-file.md) 4.7), a **drawable block** laid out as below. Many classes put other fields
before the block ([classes/](classes/) lists them per class); the block itself is always the same.

### 1.1 Drawable block

| Size | Type | Meaning |
|---:|---|---|
| 36 | 3 x vec3 | **Local matrix** `M`: three column vectors `c0, c1, c2` (section 7) |
| 12 | vec3 | **Local position** `p` |
| 1 | u8 | Zone selector |
| 1 | u8 | Visibility selector |
| 1 | u8 | Draw-distance selector |
| 4 | u32 | Zone mask |
| 4 | f32 | Maximum draw distance (`3.4028235e38`, the largest `f32`, means unlimited) |
| 4 | f32 | Maximum cel-outline draw distance (same convention) |
| 4 | f32 | Bounding radius of this object's own geometry |
| 4 | f32 | Bounding radius including children |
| var | | Option list ([object-stream.md](object-stream.md)), ends with `ff ff` |
| var | | **Mesh set** (section 2) |
| var | | Extension block (1.2) |
| 1 | u8 | Pose-copy count `q`; then `q` times: `s16` object id and `P` x `f32` (P = the mesh set's pose count) |
| var | | Child objects: `u16` count, then objects ([world-file.md](world-file.md) 4.7) |

The selectors and distances are culling data; a first renderer can ignore them.

### 1.2 Extension block

| Size | Type | Present when | Meaning |
|---:|---|---|---|
| 4 | u32 | always | Flags `e` (0 for most objects) |
| 36 | 3 x vec3 | `e & 0x01` | A pre-rotation matrix (joint set-up) |
| 36 | 3 x vec3 | `e & 0x02` | A post-rotation matrix (joint set-up) |
| 2 | s16 | `e & 0x0C` | Object id of an alternative transform parent: with `0x04` the position follows it, with `0x08` the rotation does |
| 2 | s16 | `e & 0x10` | Object id (inverse-kinematics handle) |
| 30 | s16, vec3, vec3, f32 | `e & 0x20` | Inverse-kinematics data |
| 4 | 2 x s16 | `e & 0x40` | Two object ids |
| 1 | u8 | `e & 0x80` | One byte |

Only the parent redirection (`0x04`/`0x08`) affects where a static object is drawn; the rest is animation set-up.

## 2. Mesh set

| Size | Type | Meaning |
|---:|---|---|
| 1 | u8 | Relight flag (non-zero: the set is lit dynamically) |
| 1 | u8 | Unknown (0 or 1) |
| 1 | u8 | **Bone count** `B` |
| 2 * B | s16[B] | Object ids of the bones (objects elsewhere in the stream whose transforms deform this set) |
| 1 | u8 | **Pose count** `P` (morph targets) |
| 4 * P | f32[P] | Initial pose weights |
| 2 | u16 | **Mesh count** `G` |
| var | | `G` meshes (section 3) |

Sets with `G = 0` are common (lights, cameras, bones and other invisible objects).

## 3. Mesh

A mesh starts with a 16-bit flag word `F`. Optional fields follow **in exactly this order**, each present only when
its bit is set:

| Order | Bit | Size | Contents |
|---:|---|---:|---|
| 1 | `0x001` | 2 + 48 | **Instance**: `s16` index of the source mesh in this set (always lower than this mesh's own index), then the instance matrix: 4 x vec3 = columns `c0, c1, c2` and translation `t` (section 7) |
| 2 | `0x002` | 4 | `u32` zone mask |
| 3 | `0x200` | 4 | `f32` submesh culling radius |
| 4 | `0x004` | 4 | `f32` draw-order value `g` (effective sort key `g * abs(g)`; the largest `f32` means "none") |
| 5 | `0x008` | 4 | `f32` fog factor |
| 6 | `0x010` | 4 | `f32` maximum draw distance (largest `f32` = unlimited) |
| 7 | `0x020` | 4 | `f32` maximum cel-outline draw distance |
| 8 | `0x040` | var | Texture-animation record ([textures.md](textures.md) 2.4) |
| 9 | `0x080` | 28 | A `vec3` direction, then 4 x `f32` (a highlight effect; not needed to draw) |
| 10 | `0x100` | var | Vertex/UV distortion record: `s16` object id, `s8` kind; if kind is not -1, 4 x `f32` and a 48-byte matrix (4 x vec3); then 3 x `u8` |

Then, always:

| Size | Type | Meaning |
|---:|---|---|
| 12 | vec3 | Bounding-sphere centre (in the object's local space; for an instance, after its matrix) |
| 4 | f32 | Bounding-sphere radius |
| 2 | s16 | Object id of the mesh |
| 1 | u8 | Billboard mode: 0 = none (61,464 meshes over all worlds), 1 = turn toward the camera (3,982 meshes; *interpretation*). No other value occurs |
| 1 | u8 | **Render pass** ([textures.md](textures.md) 4.1) |
| 1 | u8 | Mesh flags (values seen `0x10..0x33`; meaning not needed to draw) |

**If the mesh is an instance (bit `0x001`), it ends here.** It draws the source mesh's submeshes. Otherwise:

| Size | Type | Meaning |
|---:|---|---|
| 2 | u16 | Submesh count `S` |
| var | | `S` submeshes (3.1) |
| 2 | u16 | Outline-part count `O` |
| var | | `O` outline parts (3.2) |

Common flag words: `0x203` (instance with zone mask and culling radius), `0x000`, `0x012`, `0x002`, `0x010`, `0x003`.

### 3.1 Submesh

| Size | Type | Meaning |
|---:|---|---|
| 12 | vec3 | Bounding-sphere centre |
| 4 | f32 | Bounding-sphere radius |
| 1 | u8 | Position count `np` |
| 1 | u8 | Normal count `nn` |
| 1 | u8 | Colour count `nc` |
| 1 | u8 | UV count `nt` |
| 1 | u8 | **Strip length** `ns` (number of strip entries) |
| 0..3 | | **Zero padding up to the next offset that is a multiple of 4, counted from the start of the decompressed world file** (all four amounts 0..3 occur) |
| 12 * np | vec3[np] | Positions |
| 12 * nn | vec3[nn] | Normals (unit length) |
| 4 * nc | 4 x u8 | Colours R, G, B, A |
| 8 * nt | 2 x f32 | Texture coordinates (u, v), normalised: 0..1 spans the bitmap once; values outside repeat |
| 4 * ns | 4 x u8 | **Strip entries** (3.3) |
| 2 | u16 | **Shader index** (into the world's shader table) |
| 1 | u8 | Self-illumination (0 on most; non-zero only on kind-0 materials) |
| 1 | u8 | Bone-map length `nb` (0..4) |
| nb | u8[nb] | Bone map: entry `j` is an index into the mesh set's bone list |
| 4 * np * nb | f32 | Weights: for position `i`, weights `w[i*nb + 0 .. i*nb + nb-1]`, one per bone-map entry; each group sums to 1.0 |

If the mesh set's pose count `P` is non-zero, morph data follows:

| Size | Type | Meaning |
|---:|---|---|
| 2 | u16 | Pose position count `pp` |
| 12 * pp | vec3[pp] | Pose positions |
| 2 | u16 | Pose normal count `pn` |
| 12 * pn | vec3[pn] | Pose normals |
| 4 * P * ns | | For each pose: `ns` x `u16` position indices, then `ns` x `u16` normal indices (one per strip entry). An index outside the pose arrays means "same as the base" |

### 3.2 Outline part

| Size | Type | Meaning |
|---:|---|---|
| 1 | u8 | Position count `k` |
| 12 * k | vec3[k] | Positions |
| 1 | u8 | Edge count `e` |
| 4 * e | 4 x u8 | Edges: four position indices. *Interpretation:* bytes 1 and 2 are the edge's end points, bytes 0 and 3 the far corners of the two triangles that share it (used to decide whether the edge is on the silhouette) |
| 1 | u8 | Bone-map length `nb` |
| nb | u8[nb] | Bone map |
| 4 * k * nb | f32 | Weights, same layout as 3.1 |
| var | | If `P > 0`: `u16` count `pp`, `pp` x vec3, then `P * k` x `u16` indices |

Outlines are optional for a first render.

### 3.3 Strip entries and vertices

Each strip entry is one **vertex** of the submesh. Its four bytes:

| Byte | Meaning |
|---:|---|
| 0 | Position index (< `np`) |
| 1 | Normal index (< `nn`). 0xFF would mean "no normal"; never seen |
| 2 | UV index (< `nt`), or 0xFF for "no UV" (40 entries in all of Paris, Hideout, A Rocky Start and A Perilous Ascent) |
| 3 | Bits 0..6: colour index (< `nc`), or `0x7F` for "no colour" (use white `ff ff ff ff`). **Bit 7: strip break** (see below) |

A renderer builds one GPU vertex per strip entry (position, normal, colour, UV, and the four bone indices/weights
looked up through the position index). Vertices are not shared between entries, so `ns` vertices per submesh.

**Triangles.** Entry `k` (counting from 0 within the submesh) with `k >= 2` closes the triangle made of entries
`k-2, k-1, k`, **unless bit 7 of entry `k`'s byte 3 is set**, in which case no triangle ends at `k`. The first two
entries of every submesh have bit 7 set; mid-strip, the bit appears on (at least) two consecutive entries to start a
new strip.

Winding follows the **absolute** parity of `k` (it does not restart after a break):

- `k` even: triangle (`k-2`, `k-1`, `k`);
- `k` odd: triangle (`k-2`, `k`, `k-1`).

With that rule, `(b - a) x (c - a)` (positions of the three vertices in the order given) points the same way as
the stored vertex normals for 99.0 % of triangles (the rest are small or nearly edge-on). So **front faces are
counter-clockwise** in a right-handed space. Restarting the parity after each break gives a mix (62 % agree), which
is wrong.

**Colours.** Vertex colours are 0..255 per channel; most are near `ff` with alpha `ff`. *Not settled:* whether the
PS2 treats colour 0x80 as 1.0 as it does for alpha. The reference port treats `ff` as 1.0 and multiplies vertex
colour by the material colour ([textures.md](textures.md) 4.3).

## 4. Skinning and poses

- A mesh set with `B > 0` is skinned. Positions are given in the object's local space in the bind pose; each
  submesh maps its up to four weights onto the set's bone list through its bone map. For a static first render,
  draw skinned meshes undeformed (*expected to show the bind pose; not verified*).
- A mesh set with `P > 0` has morph targets. With all pose weights at their initial values, use the base
  positions (*same caveat*).

In four worlds surveyed (Paris, The Hideout, A Rocky Start, A Perilous Ascent), 7,291 of 8,002 submeshes have no bones. Skinned sets: 26 in Paris, 7 in The Hideout,
none in Splash.

## 5. Shader binding

The submesh's shader index selects a record in the shader table ([textures.md](textures.md) 2.3). That shader's
single texture record gives the bitmap and palette(s) for frame 0. On all 46 worlds every submesh's shader index is
in range and every used shader has valid bitmap and palette indices. The render pass used for sorting is the
mesh's (3); the shader also stores one, usually the same.

## 6. Placement in the world

### 6.1 Matrix convention

A stored 3 x 3 matrix is three `vec3` **columns**. A point `x` in the child space maps to `M x + p`, that is
`x0 * c0 + x1 * c1 + x2 * c2 + p`. Matrices may contain scale (non-uniform scale and slight shear occur in instance
matrices), so transform normals with the inverse transpose and renormalise.

*Evidence:* for instance meshes, applying the instance matrix with this convention to the source mesh's
bounding-sphere centre reproduces the instance's stored centre to within rounding in 1,554 of 2,311 instances (worlds 1, 2
and 12) and within a small fraction of the radius in almost all of the rest (centres of scaled spheres are recomputed). The transposed convention matches
only where the rotation is symmetric.

### 6.2 Object hierarchy

Objects nest (child objects at the end of the drawable block). For an object with local matrix `M`, local position
`p` and parent world transform (`Mw_parent`, `pw_parent`):

- `Mw = Mw_parent * M`
- `pw = pw_parent + Mw_parent * p`

Top-level objects use `M` and `p` directly. If the extension block's flag `0x04` (position) or `0x08` (rotation)
is set, use the named object instead of the parent for that part.

*The column convention for object matrices is assumed to be the same as for instance matrices; it is not yet
verified independently (most object matrices are the identity, for example the first drawable object of each of
the three test worlds).*

### 6.3 Drawing a mesh

- A real mesh's vertices are in its object's local space: world position = `Mw * position + pw`.
- An instance mesh draws the submeshes of its source mesh through its own instance matrix first:
  world position = `Mw * (I * position + t) + pw`, where `I` is the instance's 3 x 3 part and `t` its translation.
  Instances of instances do not occur: all 36,578 instance meshes over the 46 worlds name a real mesh.

*Interpretation:* Z is up (large ground surfaces have normal `(0, 0, 1)`; for example the first submesh of Paris
is a 4-vertex quad lying in the plane z = -4424.877 with normal `(0, 0, 1)`). Units are game units.

## 7. Test vectors

### 7.1 Totals per world

| World | Mesh sets (non-empty) | Meshes | Instances | Submeshes | Strip entries | Positions | Triangles | Outline parts |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Splash (#0) | 29 (28) | 33 | 0 | 41 | 1,382 | 605 | 764 | 1 |
| Paris, France (#1) | 727 (327) | 2,464 | 1,464 | 2,786 | 150,125 | 80,525 | 91,763 | 193 |
| The Hideout (#2) | 382 (290) | 537 | 164 | 1,006 | 51,537 | 27,550 | 33,777 | 70 |

("Triangles" counts the triangles produced by 3.3 from real meshes only, without multiplying by instances.)

### 7.2 First submesh of each world

| | Splash | Paris, France | The Hideout |
|---|---|---|---|
| Mesh set at (file offset) | 0x1623 | 0xA2BCD | 0x1E531 |
| Owning object (class id, object id); its matrix and position | 87, 1356; identity, (0, 0, 0) | 4, 1; identity, (0, 0, 0) | 4, 1; identity, (0, 0, 0) |
| Set header (relight, unknown, bones, poses, meshes) | 0, 1, 0, 0, 6 | 0, 0, 0, 0, 1910 | 0, 0, 0, 0, 68 |
| First mesh at | 0x1629, flags 0x000 | 0xA2BD3, flags 0x002 | 0x1E537, flags 0x002 |
| Mesh centre / radius | (-5.5996, 15.4781, 27.2721) / 70.6037 | (1298.4694, 17678.6113, -4424.877) / 6180.6021 | (-628.347, 6.3999, -100.5143) / 437.0908 |
| Billboard / pass / mesh flags | 0 / 7 / 0x10 | 0 / 3 / 0x13 | 0 / 3 / 0x13 |
| First submesh at | 0x1642 | 0xA2BF0 | 0x1E554 |
| np, nn, nc, nt, ns; padding | 74, 60, 5, 67, 148; 1 | 4, 1, 0, 4, 4; 3 | 51, 18, 0, 52, 85; 3 |
| First position (bytes) | `b2ae0a42 545267c0 91eda240` = (34.6706, -3.6144, 5.0915) | `17428545 7e344e46 04478ac5` = (4264.2612, 13197.1230, -4424.8770) | `3a1b5cc4 e59b35c3 b176ad42` = (-880.4254, -181.6090, 86.7318) |
| First normal | (0.737117, -0.362808, 0.570113) | (0, 0, 1) | (-0.999972, -0.004500, 0.006000) |
| First colour | `ff ff ff ff` | (none) | (none) |
| First UV | (0.4074, 0.3062) | (1.0, 1.0) | (1.0, 0.8333) |
| First strip entries | `00000080 01010180 02020200 03030300 04040481 05050580` | `000000ff 010001ff 0200027f 0300037f` | `000000ff 010001ff 0200027f 0300037f 0400047f 0500057f` |
| First triangles | (0,1,2) (1,3,2) (4,5,6) (5,7,6) | (0,1,2) (1,3,2) | (0,1,2) (1,3,2) (2,3,4) (3,5,4) |
| Triangles in submesh | 98 | 2 | 58 |
| Shader index; its kind; texture record | 34; 1; bitmap 4, palette 15 | 120; 0; bitmap 160, palettes 418/419/420 | 53; 0; bitmap 220, palettes 507/508/509 |
| Submesh ends at | 0x2120 | 0xA2C78 | 0x1EBA0 |
| Mesh set ends at | 0x3D9E | 0x24EF23 | 0x60D1B |

Class ids are those of [object-stream.md](object-stream.md). The owning object of each first mesh set is a top-level object.

### 7.3 Instance example (The Hideout)

The first instance mesh of The Hideout is in the set at 0x1E531, at offset 0x5D2BE, flags `0x003`, source mesh 11.
Instance matrix columns `(-1, 0, 0)`, `(0, -1, 0)`, `(0, 0, 1)`, translation `(512.015, 3.3878, 6.457)` (a
half-turn about Z). Source centre `(256.007, -162.353, 170.941)` maps to `(256.008, 165.741, 177.398)`, the stored
instance centre. Radius 142.362 in both.

## 8. Open questions

- Byte 1 of the mesh-set header, the three selector bytes of the drawable block, the mesh-flags byte and the
  self-illumination scale are not decoded.
- Vertex-colour scale (`ff` or `80` as 1.0) is unconfirmed.
- Object-matrix convention: assumed equal to the verified instance convention.
- Skinned and morphed sets at rest: assumed to show the bind pose with no bone transform applied.
