---
id: sly1-classes-shared-structures
title: Sly 1 object stream, shared sub-structures
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46
      decompressed worlds
    used_for: every size rule below was exercised by a full walk of all 46 worlds (each one ends exactly at the texture block)
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files geom, bsp, tn, crv, crvl, crvc, aseg, ac*, emitter
    used_for: field meanings
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), File Extractor GUI / NativeTools / BrxRebuilder (ObjectWalker, GeometryWalker)
    used_for: retail byte sizes of each record kind
date: 2026-11-13
---

# Shared sub-structures

Structures used by more than one class layout. Notation is in [`../object-stream.md`](../object-stream.md),
section 1. Each structure is given with enough detail to step over it; meanings are given where they are known and
needed in phase 1.

## Collision mesh

A triangle mesh with adjacency, used for collision and camera collision. Not drawn.

| Field | Type | Notes |
|---|---|---|
| bounding radius | `f32` | |
| position count P | `u16` | |
| positions | P × `vec3` | local space of the owning object |
| triangle count T | `u16` | |
| edge total E | `u16` | must equal the sum of the per-triangle edge counts below |
| T triangles | | see below |

Triangle record:

| Field | Type | Notes |
|---|---|---|
| corner indices | 3 × `u16` | indices into positions, each below P |
| triangle radius | `f32` | |
| triangle centre | `vec3` | |
| edge count n | `s8` | 0 or more |
| n edges | n × 6 bytes | `u16` position index, `u16` position index, `u16` index of the triangle on the other side (below T) |

The triangle's plane is not stored; it follows from the three corners (normal = normalise((p1 − p0) × (p2 − p0))).

## Collision tree

Follows each collision mesh in the solid layout.

| Field | Type | Notes |
|---|---|---|
| node count N | `u16` | |
| full node count | `u16` | not needed to parse |
| N nodes | N × 6 bytes | `u16` triangle index (below the collision mesh's T), `u16` negative child, `u16` positive child. A child is a node index below N or `0xFFFF` for none |

## Convex volume

Planes plus a small tree, used by volumes, exits, camera zones and flash volumes.

| Field | Type | Notes |
|---|---|---|
| plane count P | `u16` | |
| P planes | P × 16 bytes | `f32` distance d, then `vec3` normal n; the plane is n · x = d |
| node count N | `u16` | |
| N nodes | N × 6 bytes | `u16` plane index, `s16` negative child, `s16` positive child (−1 for none) |

## Curve

Introduced by a curve kind (`u8` or `s8` depending on the owner). Kind 0 is a polyline, kind 1 a cubic curve.
Any other kind is corrupt.

| Field | Type | Notes |
|---|---|---|
| closed flag | `u8` | non-zero for a closed loop |
| control point count C | `u8` | |
| C control points | kind 0: C × 16 bytes; kind 1: C × 40 bytes | kind 0: `f32` parameter, `vec3` position. Kind 1: `f32` parameter, `vec3` position, `vec3` incoming tangent, `vec3` outgoing tangent |

## Scalar animation curve

Introduced by an `s8` (or `u8`) curve kind.

| Kind | Meaning | Layout |
|---|---|---|
| 0 | Bezier keys | `u16` count K, then K × 16 bytes |
| 1 | weighted Bezier keys | `u16` count K, then K × 24 bytes |
| 2 | linear keys | `u16` count K, then K × 6 bytes (`s16` time in 1/60 s frames, `f32` value) |
| 3 | twist blend | nothing |
| 4 | pose blend | nothing |

Key times are in frames at 60 per second in every key kind (an `s16` at the start of each key). Other kinds are
corrupt.

## Vector animation curve

Introduced by an `s8` kind.

| Kind | Meaning | Layout |
|---|---|---|
| 0 | per-component | `u8` component mask, `vec3` base value, then for each of bits 0, 1, 2 (x, y, z) that is set: `u8` scalar-curve kind and that scalar curve |
| 1 | Bezier keys | `u16` count K, then K × 38 bytes |
| 2 | blend | nothing |

## Animation events

An event list is `s8` count, then that many events. In a "framed" list each event has a `s16` frame number right
after its type byte. Event type is `s8`; its payload:

| Type(s) | Payload bytes | Layout |
|---|---|---|
| −1, 1, 2, 3, 8, 11, 14, 15, 34, 42, 43, 49, 50, 53, 55, 56 | 0 | |
| 0 | 24 | two `s16`, two 4-byte values, three `f32` |
| 4, 7, 9, 10 | 6 | `s16`, `f32` |
| 5, 33, 57, 58 | 4 | `f32` |
| 6, 12, 13, 17, 18, 22, 24, 29, 30, 31, 32, 35, 36, 39, 41, 59 | 2 | `s16` |
| 16 | 4 | `s16`, `s8`, `s8` |
| 19, 20 | 3 | `s16`, `u8` |
| 21, 23 | 10 | `s16`, `f32`, `f32` |
| 25, 26 | 36 | `s16`, `s16`, `desc32` (a level) |
| 27 | 3 | `u8`, `s16` |
| 28 | 12 | `s16`, `s16`, `f32`, `f32` |
| 37, 38, 45, 47 | 4 | `s16`, `s16` |
| 40 | 2 | `u16` |
| 44 | 32 | `desc32` (a sound) |
| 51 | 32 | `desc32` (a video) |
| 46, 52 | 6 | three `s16` |
| 48 | 10 | three `s16`, `f32` |
| 54 | 4 + `str` | `s16` line, `s16` duration, `str` subtitle text |

Types not in the table never occur on the disc and are corrupt. Event meanings belong to the gameplay specs.

## Emitter mesh

Present only in emitters whose origin kind is 4 (object-stream.md, section 4.5).

| Field | Type |
|---|---|
| point count | `u16` |
| points | count × `vec3` |
| triangle count | `u16` |
| triangles | count × 10 bytes |
| extra vector | `vec3` |

## Emitter colours

| Field | Type | Notes |
|---|---|---|
| colour count C | `u16` | |
| colour-range flag | `u8` | only if C > 0 |
| colours | C × `c` (packed RGBA, 4 bytes) | only if C > 0 |
