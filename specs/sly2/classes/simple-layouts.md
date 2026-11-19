---
id: sly2-classes-simple-layouts
title: Sly 2 object stream, options-only and header-plus-options layouts
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97316 (USA) disc image; executable SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 decompressed worlds
    used_for: every top-level body of the classes below parsed with these layouts to its declared size (12,532 bodies)
date: 2026-11-13
---

# Simple layouts (Sly 2)

Two small layouts confirmed by parsing every top-level body of these classes to its declared size [D].

## 1. Options only

The body is one option list ([../object-stream.md](../object-stream.md) section 1).

Classes: 2 (177), 112 (60), 113 (880), 114 (3,545), 115 (32), 116 (136), 117 (8), 119 (32), 121 (1), 123 (2),
124 (2,407) [D]. Class 120 (422 bodies, only nested) has the same layout [D]. The shortest body is `FF FF` (an empty list) [D].

## 2. Transform header and options

| Order | Field | Type |
|---:|---|---|
| 1 | parent | `s16` transform reference ([../transforms.md](../transforms.md) 1) |
| 2 | matrix | 4 × `vec3` (three axis images, translation) |
| 3 | zone mask | `u32` |
| 4 | options | option list |

Classes: 3 (1,869), 4 (2,676), 18 (3), 19 (41), 20 (84), 35 (94), 37 (2), 38 (12), 39 (128), 43 (306), 44 (17),
46 (20) [D]. Classes 16, 21, 36, 40, 45 and 47 (only nested) have the same layout [D]. Classes 41 and 42 add a
`u16`-length string after the option list ([paths-and-volumes.md](paths-and-volumes.md) 7).

Example [D]: a class-3 body of 56 bytes in world row 3: `ff ff` (no parent), 48 bytes of matrix, `ff 00 00 00`
(zone mask 255), `ff ff` (no options).

Nested bodies of these classes are parsed with the same layouts and end at their declared size too [D].
