---
id: sly1-classes-placed-object
title: Sly 1 object stream, placed layout and layouts that extend it
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46 decompressed worlds
    used_for: >-
      layout verified by full walks; value statistics (mode bytes, extension flags, degenerate matrices, instance
      matrices); matrix convention checked with instanced-model centres (see transforms.md)
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files alo, glob (mesh sets), emitter, dialog, flash, find
    used_for: field order and meaning, culling modes, extension semantics
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), File Extractor GUI / NativeTools / BrxRebuilder (ObjectWalker)
    used_for: retail byte layout of the mesh set, the emitter and the dialogue events
date: 2026-11-13
---

# Placed layout (layout key **placed**)

An object with a local transform, render settings, an optional model (mesh set), and children. Most of
what is drawn in a world is a placed object or is built on this layout (the solid layout ends with it).

This layout is the same byte sequence as the **drawable block** of [`../geometry.md`](../geometry.md) section 1.1;
the two tables agree field for field. This file adds the meaning of the three selector bytes and of the culling
fields, the classes that use the layout, and the classes that extend it.

## 1. Layout

After the 6-byte object header:

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | local rotation | `mat3` | relative to the parent; see transforms.md. Usually orthonormal; see 1.1 |
| 2 | local position | `vec3` | relative to the parent |
| 3 | zone mode | `u8` | 0 none, 1 fixed (the mask in row 6 is used as is), 2 dynamic (the game recomputes the mask from the visibility map whenever the object moves) |
| 4 | visibility-cull mode | `u8` | 0 never culled by zones, 1 per mesh (each mesh's own zone mask), 2 whole object (row 6) |
| 5 | distance-cull mode | `u8` | 0 none, 1 per mesh (each mesh's own maximum distance), 2 whole object (rows 7 and 10) |
| 6 | zone mask | `u32` | one bit per visibility zone. `0x0FFFFFFF` means all zones (42 of the 46 world meshes) |
| 7 | maximum render distance | `f32` | `FLT_MAX` means unlimited; the game substitutes 1e10 |
| 8 | outline maximum distance | `f32` | distance beyond which the cartoon outline is not drawn. `FLT_MAX` means "same as row 7, capped at 2000" |
| 9 | render radius, self | `f32` | bounding radius of this object's own model |
| 10 | render radius, all | `f32` | bounding radius including descendants; used with row 7 for whole-object culling |
| 11 | options | option list | |
| 12 | model | mesh set (section 2; geometry.md 2) | may contain no meshes |
| 13 | extension | extension block (section 3; same bytes as geometry.md 1.2) | |
| 14 | pose-weight sets | `u8` count K, then K × (`s16` OID, P × `f32`) | P is the pose count of the mesh set (row 12) |
| 15 | children | object list | `u16` count, then object records (object-stream.md, section 5) |

Rows 3 to 5 on the disc (zone, visibility, distance), most common first: (1,0,0) 9283 objects, (1,2,2) 5452,
(0,0,0) 4807, (1,0,2) 1845, (2,2,2) 1441, (1,2,1) 400, (1,0,1) 391, others under 130.

### 1.1 Degenerate matrices

48 placed objects on the disc (six in each of eight worlds: The Hideout and the seven worlds of the last episode; three chains of two objects in each) have a rotation whose second and third
vectors are all −`FLT_MAX`. All of them have an empty mesh set; in each chain the first object has the second as its only child. A loader must not
propagate these values into world transforms: substitute the identity for such a matrix (any matrix with a
component whose magnitude exceeds 1e30). This changes nothing visible, since these objects and their children draw
nothing in a static scene.

### 1.2 Phase-1 rendering of a placed object

For a first static render:

- Compute the world transform from rows 1 and 2 and the parent chain (transforms.md).
- Draw every mesh of row 12 with that transform; instance meshes add their own matrix (geometry.md 6.3).
- Culling (rows 3 to 10 and the visibility map) can be skipped: drawing everything is correct, only slower. When it
  is implemented: with visibility-cull mode 2, the object is skipped when (zone mask AND camera zone mask) is not
  equal to the camera zone mask; mode 1 applies the same test per mesh. Distance culling compares the distance from
  the camera to the sphere (world position, render radius all) against the maximum render distance, with a fade
  near the limit; its exact curve belongs to the renderer spec.
- Skip stock templates (OID 12 to 42 loaded through an object list) and everything below them.

## 2. Mesh set

Row 12 is a **mesh set**, specified in [`../geometry.md`](../geometry.md) sections 2 and 3:
meshes, instance meshes, submeshes and outline parts. Its pose count P is needed for row 14. A mesh set with zero
meshes is common (lights, cameras, bones, markers).

In this folder an object "has a model" when its mesh set has at least one mesh with at least one submesh, or at
least one instance mesh. Worlds.md counts drawable objects with this definition.

## 3. Extension block

| Order | Field | Present when | Type | Notes |
|---|---|---|---|---|
| 1 | flags | always | `u32` | 0 for 18052 of the 23944 records on the disc that carry this block on the disc |
| 2 | pre-rotation | bit 0 | `mat3` | applied on the object side of an animated rotation |
| 3 | post-rotation | bit 1 | `mat3` | applied on the parent side of an animated rotation (and to animated angular velocity) |
| 4 | basis object OID | bit 2 or bit 3 | `s16` | an **ancestor** of this object (searched up the parent chain). Bit 2: take the position from it instead of the parent; bit 3: take the rotation from it instead of the parent |
| 5 | IK target OID | bit 4 | `s16` | |
| 6 | IK chain | bit 5 | `s16` elbow OID, `vec3`, `vec3`, `f32` twist angle in radians | |
| 7 | joint pair | bit 6 | `s16`, `s16` | parent and child joint OIDs |
| 8 | joint flag | bit 7 | `u8` | |

Rows 2 and 3 only matter when an animation drives the rotation; the stored local rotation (section 1, row 1) is used
unchanged in a static scene. Rows 5 to 8 belong to the animation specs.

Flag values seen on the disc: 0, 2, 8, 16, 18, 19, 32, 34, 64, 128, 130, 131 and a handful of 1 and 3. Bit 2 is
never set on the disc; bit 3 is set on 50 objects.

## 4. Classes with exactly the placed layout

| ID | Neutral name | Records on the disc | With a model (section 2) |
|---:|---|---:|---:|
| 2 | placed object | 8907 | 8015 |
| 78 | camera placement | 101 | 0 |
| 79 | skeleton bone | 4046 | 309 |
| 81 | light | 2254 | 0 |
| 82 | skeleton helper ? | 202 | 0 |
| 84 | sky | 40 | 40 |
| 86 to 91 | charm, life token, coin, key, gold prize ?, boost prize ? | 174 | 174 |
| 92, 93 | lock, lock group | 20 | 2 |
| 94 | tail / chain segment | 1080 | 10 |
| 95 | placed variant ? | 2 | 0 |
| 97 | dynamic shadow | 41 | 41 |
| 98 | waypoint | 12 | 12 |
| 100 | jump location ? | 67 | 2 |
| 102 | speaker | 97 | 0 |
| 103 | world map ? | 48 | 0 |
| 104 | spawner ? | 1 | 0 |
| 105, 106 | hub selector, hub progress | 6 | 1 |
| 107 | integrity check object | 1 | 0 |

Only classes that occur on the USA disc are listed.

## 5. Layouts that extend the placed layout

### 5.1 Particle emitter (class 80)

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | placed part | placed layout (section 1) | its own option list sets the origin kind (options 366 or 421) |
| 2 | origin curve kind | `s8` | −1 for none, else a curve of that kind follows (shared-structures.md, curve) |
| 3 | origin mesh | emitter mesh | only when the origin kind is 4 (object-stream.md, section 4.5); 285 emitters and explosion emitters on the disc have it |
| 4 | colours | emitter colours | shared-structures.md |

### 5.2 Dialogue (class 101)

| Order | Field | Type |
|---|---|---|
| 1 | placed part | placed layout (section 1) |
| 2 | event count | `s8` |
| 3 | events | each: `s8` kind, then the kind's payload |

| Kind | Payload |
|---|---|
| 0 | `desc32` (voice audio) |
| 1, 2 | `s16`, `s16` |
| 3 | `f32`, `f32`, `u8`, `u8`, 4 × `s16`, `str` (subtitle text) |
| 4 | `s16` |
| 5, 6 | nothing |
| 7 | `desc32` (video) |

Other kinds are corrupt.

### 5.3 Flash volume (class 96)

| Order | Field | Type |
|---|---|---|
| 1 | placed part | placed layout (section 1) |
| 2 | volume | convex volume (shared-structures.md) |
