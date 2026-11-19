---
id: sly1-classes-solid-object
title: Sly 1 object stream, solid layout and the static world mesh
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46 decompressed worlds
    used_for: layout verified by full walks; fixed-flag statistics; world-mesh transforms
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files so, geom, bsp, cnvo
    used_for: field order and meaning
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), File Extractor GUI / NativeTools / BrxRebuilder (ObjectWalker, CidLoaders.generated.inc)
    used_for: retail layout; list of classes whose serialized body is exactly this layout
date: 2026-11-13
---

# Solid layout (layout key **solid**)

An object with collision: a collision mesh, physical properties, and a second mesh for camera collision, followed
by the complete placed layout.

## 1. Layout

After the 6-byte object header:

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | physics flags | `u8` | bit 0: **fixed**. A fixed solid never moves; its collision data is used in place. Only 0 and 1 occur on the disc |
| 2 | collision mesh | collision mesh (shared-structures.md) | object space |
| 3 | collision tree | collision tree (shared-structures.md), indices refer to row 2 | |
| 4 | mass | `f32` | |
| 5 | moment of inertia | `mat3` | object space |
| 6 | centre of mass | `vec3` | object space |
| 7 | camera-fade groups | `u16` count, then count × (`s16` group key, `u16` mesh index) | meshes that fade out when they block the camera; the key is 1 at the start of a group and −1 for the following entries (collision.md section 4) |
| 8 | camera-fade node map | `u32` count, then count × `s16` | per collision-tree node: the first row-7 entry of its fade group, or −1 (collision.md section 4) |
| 9 | hit groups | `u32` count H, then H × (`s16` mesh index, `s16` submesh index) | H is 0 in every solid on the disc |
| 10 | hit-group index | (T + 1) × `s16`, only if H > 0 | T is the triangle count of row 2 |
| 11 | camera collision mesh | collision mesh | |
| 12 | camera collision tree | collision tree, indices refer to row 11 | |
| 13 | placed part | placed layout (placed-object.md, section 1), complete | transform, model, extension, children |

The object's transform is in row 13, after the collision data.

## 2. The static world mesh (class 4)

Every USA world has exactly one record of class 4. On the disc it is always:

- in the top-level object list, as the first record, or the second after the visibility map;
- OID 1, script index −1;
- the only record with the fixed flag (row 1) set: 46 fixed solids on the whole disc, one per world;
- placed at the identity: rotation exactly identity and position exactly (0, 0, 0). Its mesh set and collision data
  are therefore already in world space;
- zone mode 1, visibility-cull mode 1, distance-cull mode 1 (per mesh), zone mask `0x0FFFFFFF` in 42 worlds (7, 63 or 511 in the other four, which all have a
  visibility map), maximum render
  distance and outline distance `FLT_MAX`;
- without children.

It carries most of the level's static scenery: in the Paris hub 1910 meshes (1413 of them instance meshes) and 1685
submeshes. The splash world's world mesh has an empty mesh set. Other static scenery lives in placed objects (class 2)
and in proxies.

Because the identity transform is guaranteed on the disc, a renderer may draw the world mesh's meshes with no object
transform (each instance mesh still uses its own instance matrix).

## 3. Classes with exactly this layout

Every class below has a serialized body identical to section 1; what distinguishes them comes from options and
scripts. Only classes that occur on the USA disc are listed; names are those of object-stream.md section 6.

3 solid object, 4 static world mesh, 7 player character, 9 to 13 guards and guard variants ?, 14 companion
character, 15, 17, 18 guard variants ?, 19 boss ?, 20 tank, 21 vehicle ?, 22 turret, 23 vault, 24 puffer ?,
25 vehicle, 26 vehicle variant ?, 27 jet pack, 28 rhythm game object ?, 29 minigame vehicle ?, 31 water,
33 breakable, 34 to 36 breakable variants ?, 37 fragile breakable, 38 electrified breakable ?, 39 breakable piece,
40 button, 41 trigger volume button, 42 trigger volume variant ?, 43 physics bone ?, 46 clue, 47 alarm, 49 laser
sensor, 50 camera sensor ?, 51 pressure sensor ?, 52 barrier, 54 hazard zone ?, 55 hazard volume ?, 56 checkpoint,
58 hiding place ?, 59 bomb, 60 missile, 61 missile variant ?, 62 homing missile ?, 63 mine, 64 fly ?, 65 rat,
66 to 69 minigame actors ?, 70 dart, 71 jump object ?, 72 puffer target ?, 73 solid variant ?, 74 final boss ?,
75 final boss part ?, 76 decoy.

## 4. Conveyor (class 57)

| Order | Field | Type |
|---|---|---|
| 1 | solid part | solid layout (section 1) |
| 2 | surface speed, u | `f32` |
| 3 | surface speed, v | `f32` |
