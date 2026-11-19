---
id: sly1-classes-markers-and-volumes
title: Sly 1 object stream, markers, points, volumes, warps, exits, camera zones and explosions
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46 decompressed worlds
    used_for: layouts verified by full walks; counts
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files xform, pnt, tn, chkpnt, explg, emitter
    used_for: field order and meaning
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), File Extractor GUI / NativeTools / BrxRebuilder (ObjectWalker, CidLoaders.generated.inc)
    used_for: retail layout; which classes share each layout
date: 2026-11-13
---

# Markers, points, volumes, warps, exits, camera zones and explosions

These classes carry a position (and usually a rotation) but no model. Their positions are relative to the parent
placed object (transforms.md). None of them draws anything in a static scene, and none has a child list.

## 1. Transform marker (classes 115, 117, 118, 119)

Classes: transform marker (115), target marker (117), handle marker ? (118).

| Order | Field | Type |
|---|---|---|
| 1 | local rotation | `mat3` |
| 2 | local position | `vec3` |
| 3 | options | option list |

## 2. Point (classes 126 to 129)

Classes: point (126), point variants (127, 128), perch point (129).

| Order | Field | Type |
|---|---|---|
| 1 | local position | `vec3` |
| 2 | options | option list |

## 3. Scan point (class 130)

| Order | Field | Type |
|---|---|---|
| 1 | local position | `vec3` |
| 2 | options | option list |
| 3 | text | `str` |

## 4. Volume (classes 123, 124)

Classes: volume (123), rat hole (124).

| Order | Field | Type |
|---|---|---|
| 1 | local rotation | `mat3` |
| 2 | local position | `vec3` |
| 3 | shape | convex volume (shared-structures.md), in the volume's local space |
| 4 | options | option list |

## 5. Warp point (class 116)

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | local rotation | `mat3` | |
| 2 | local position | `vec3` | |
| 3 | options | option list | |
| 4 | animation count A | `s16` | |
| 5 | A animations | each: `s16` class ID, `s16` OID, `s16` script index, then an animation-segment body (animation-and-state.md) | owned by the warp |
| 6 | name | `str` | |
| 7 | hidden-object count H | `s16` | |
| 8 | hidden objects | H × `s16` OID | objects hidden while the warp plays |

## 6. Level exit (class 125)

The body has a layout of its own; it does not contain the placed layout.

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | destination level | `desc32` | a level descriptor (disc.md 4.1); all zero for none |
| 2 | alternative destination | `desc32` | all zero for none |
| 3 | local rotation | `mat3` | |
| 4 | local position | `vec3` | |
| 5 | trigger volume | convex volume | |
| 6 | options | option list | |
| 7 | animation count A | `s16` | |
| 8 | A animations | each: `s16` class ID, `s16` OID, `u16` script index, then an animation-segment body | owned by the exit |

The level descriptor's identity word (disc.md 4.1) equals the key in the executable's level table for the target
level; for example the Paris hub is 10 and the hideout 9.

## 7. Camera zone (class 99)

The body has a layout of its own; it does not contain the placed layout.

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | local rotation | `mat3` | |
| 2 | local position | `vec3` | |
| 3 | zone volume | convex volume | |
| 4 | path kind | `s8` | −1: no path, a fixed transform follows (row 5). 0 or 1: a curve of that kind follows (row 5) |
| 5 | path | if −1: `mat3` + `vec3`; else a curve (shared-structures.md) | |
| 6 | options | option list | |

## 8. Explosion group (class 120)

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | local rotation | `mat3` | |
| 2 | local position | `vec3` | |
| 3 | options | option list | |
| 4 | member count M | `s16` | |
| 5 | M members | each: a full object record (6-byte header and body) | members are explosion groups or explosion emitters (or proxies of them). They are owned by the group and taken out of the world |

## 9. Explosion emitter (classes 121, 122)

| Order | Field | Type | Notes |
|---|---|---|---|
| 1 | local rotation | `mat3` | |
| 2 | local position | `vec3` | |
| 3 | options | option list | sets the origin kind (options 366 or 421) |
| 4 | origin curve kind | `s8` | −1 for none, else a curve follows |
| 5 | origin mesh | emitter mesh | only when the origin kind is 4 |
| 6 | colours | emitter colours | |

Rows 4 to 6 are the same as the particle emitter's (placed-object.md, section 5.1).
