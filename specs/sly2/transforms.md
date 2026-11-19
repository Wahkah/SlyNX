---
id: sly2-transforms
title: Sly 2 transforms, parent references and skin binding
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97316 (USA) disc image; executable SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 decompressed worlds
    used_for: >-
      the transform-list layout parses inside all 11,250 solid bodies; kind, flag and parent-reference counts over
      53,067 transforms; the test vectors below
  - kind: reference-project
    what: Games/Sly2-RE-notes.md, tools/sly2_geom.py (earlier research, which rendered all 43 levels and posed the characters with these rules); sly2-decomp @ 33e2bab9
    used_for: field order, the composition rule and the skin-binding rule, marked [R]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (GPL-3.0, facts only), src/vec-alo (transform-reference resolver and owner stack), src/alo-lo/InitXfm_file.cpp (owned-list loader), src/tn, src/dzg-emitter (track and explosion-group loaders), nonmatching/new_1c90d0 (placement loader)
    used_for: section 1.1, the owner stack and what 0x2000 resolves to (2026-11-17), marked [R]
  - kind: disc
    what: every-depth walk of all 121,153 bodies (2026-11-17)
    used_for: section 1.1 occurrence counts
date: 2026-11-13
---

# Sly 2 transforms

Sly 1 gives each object one local matrix and position ([../sly1/transforms.md](../sly1/transforms.md)). A Sly 2
object with the solid layout carries a **list of transforms** (one for a simple prop, up to 139 for the player
character), and meshes, bones and children each name a transform. Tags as in object-stream.md.

## 1. Matrices and references

- **Matrix** (48 bytes): four `vec3`. The first three are the images of the local X, Y and Z axes, the fourth is
  the translation [R]. A point `p` maps to `x·a0 + y·a1 + z·a2 + t`. This is the same meaning as Sly 1's stored
  matrices (../sly1/transforms.md 1); written with row vectors it is `p' = p·R + t` with the axis images as rows.
- **Composition**: a transform's world matrix is its local matrix followed by its parent's world matrix [R]. Same as
  Sly 1 (../sly1/transforms.md 2).
- **Transform reference** (`s16`) [D for the values]:

  | Value | Meaning |
  |---|---|
  | −1 | none (the root: identity, or the placement matrix of the copy the object belongs to) |
  | `n` (0 to 0x0FFF) | transform `n` of the same object |
  | `0x1000 | n` | transform `n` of the **enclosing** object (the solid whose child list holds this object) |
  | `0x2000` | the transform on top of the loader's **owner stack** (section 1.1) [R] |

  Over all transform lists: 41,823 parents are local, 4,365 are −1 and 6,879 refer to the enclosing object [D].

### 1.1 The owner stack and reference `0x2000`

Three kinds of object read a list of further objects as part of their own body, and while that list is read the
loader keeps a transform on a stack; `0x2000` names the top of the stack [R]. In each case the transform pushed is
**not** the owner's own transform but the transform the owner itself hangs from (its own parent reference resolved),
and after the list is read every object in it has its local matrix composed **once** with the owner's local matrix
(the owner's matrix applied outside, as in section 1) [R]. So a list member using `0x2000` ends up at
`owner's parent world · owner's local · member's local`, the same place as if the owner were its parent, but it is
baked at load time: the members are attached to the owner's parent object, not to the owner, and do not follow the
owner if it later moves [R].

| Owner | Its list | What is pushed | What is applied afterwards |
|---|---|---|---|
| Copy placement (classes 6/7, section 1.2 of classes/placement.md) | the extra objects of the copy | the transform named by the placement's own parent field (none for −1) | the placement's matrix |
| Explosion group (class 31) | its parts | the group's parent transform | the group's local matrix |
| Camera track (class 15) | its optional path curve (one object) | the track's parent transform | the track's local matrix |

A reference `0x2000` with an empty stack (none occur on the disc) would name nothing.

Every occurrence on the disc, by where the reference sits [D] (all 121,153 bodies, worlds.md 5):

| Holder of the reference | Context | Count |
|---|---|---:|
| Header of a class-33 explosion part | explosion-group (class 31) part lists | 5,408 |
| Placement parent field | explosion-group part lists | 2,595 |
| Header of a class-10 curve | camera-track (class 15) lists: 202 tracks at top level, 13 nested in solid bodies | 215 |
| Headers of other classes (3 ×197, 11 ×4, 13 ×6, 19 ×3, 39 ×6) | extra lists of copy placements (top-level and nested) | 216 |
| Placement parent field | extra lists of copy placements nested in solid bodies / at top level | 56 / 5 |
| Transform list of a class-106 solid (one transform each) | extra lists of copy placements | 6 |
| **Total** | | **8,501** |

`0x1000 | n` occurs only inside solid bodies (on 13,371 headers, 8,057 placements and 6,879 solid transforms) [D].
No reference with both bits (`0x3000`) occurs [D].
  Nested objects use `0x1000 | n` to hang off their owner's bones (for example the player character's 29 directly
  nested solids all do) [D].

## 2. Transform list (start of every solid body)

| Order | Field | Type | Notes |
|---:|---|---|---|
| 1 | count | `s16` | |
| 2 | transforms | below | |
| 3 | two-bone chains | `u8` count, each: 3 × `s16` references, `vec3`, `s16` reference, `vec3` | 173 bodies have 2, a few up to 8 [D]; inverse-kinematics set-up [R] |
| 4 | mirror pairs | `s8` count, each: `s16`, `s16`, `s8`, `s8` | 141 bodies have one [D] |
| 5 | follower chains | `s8` count, each: `u8`, 4 × `f32`, 2 × `vec3`, `s8` m, m × `s16` references | 230 bodies have 1 to 16 [D] |
| 6 | three references | 3 × `s16` | |

One **transform**:

| Order | Field | Type | Present when |
|---:|---|---|---|
| 1 | parent | `s16` reference | always |
| 2 | kind | `s16` | always. Values on the disc: 0 (7,535), 1 (12,986), 6 (20,059), 12 (6,451), 13 (6,035), 15 (1) [D]. The kind says which part of the local matrix an animation channel replaces: 1 translation, 6 rotation from Euler angles, 12 scale; 0 and 13 are not animated ([animation.md](animation.md) 4) [R] |
| 3 | ID | `s32` | always (often the owning object's ID) [D] |
| 4 | flags | `u8` | always |
| 5 | one `f32` | | flag bit 0 |
| 6 | two `f32` | | flag bit 1, and bit 0 clear |
| 7 | `s8` | | flag bit 2 |
| 8 | `s8`, `f32`, 40 bytes, `s8`, `s8`, `s32` | | flag bit 3 |
| 9 | `s8`, `f32`, `f32` | | flag bit 4 |
| 10 | `vec3` | | flag bit 5 |
| 11 | local matrix | 48 bytes | always on the disc (kinds 8 and 14, which would store an `s16` mirrored-transform reference instead, do not occur) [D for the absence, R for the alternative] |

Flag values on the disc: 0 (49,923), 4 (1,357), 5 (935), 1 (534), 16 (133), 6 (89), 32 (58), 9 (18), 8 (14), 17 (4),
2 (2) [D]. The flagged fields hold joint limits and actuator settings for animation [R]; a static render ignores
them.

## 3. Where transforms are used

- **Meshes** name the transform they are drawn with; a mesh copy adds its own matrix first, then the transform
  (geometry.md 3) [R].
- **Bones** are (object ID, transform reference) pairs (geometry.md 5) [D].
- **Level geometry**: in the level-geometry object (class 49) transforms 0 and 1 are the identity and its meshes are
  already in world space [R; D for world row 3, whose transform 0 is the identity with no parent].
- **Placements** put a whole package copy at the placement matrix; objects of the copy compose with it (classes/placement.md) [R].

## 4. Skin binding

Skinned vertices are stored in the space of their mesh's transform **in the pose the transforms have as loaded**
(the bind pose) [R]. For each bone, the skin matrix is: (inverse of the bone's world matrix as loaded) × (the
mesh transform's world matrix as loaded), then × the bone's current world matrix each frame [R]. With no animation
the skin matrices are the mesh transform's world matrix, so a static render draws skinned meshes undeformed [R]
(the earlier research measured a largest difference of 2e-12 across all worlds).

## 5. Test vectors

World row 3 (`f_nightclub_cellar`), the player character (class 53, object 7, body of 1,354,878 bytes at
`0x2E9AB3`, name `jt`) [D]:

- 139 transforms, 4 two-bone chains, 1 mirror pair, 4 follower chains, trailing references (1, 2, 2).
- Transform 0: parent −1, kind 0, ID −1, flags 0, identity.
- Transform 1: parent 0, kind 1, ID 7, identity rotation, translation (0, 0, 75).
- Transform 2: parent 1, kind 6, ID 7, identity.
- Transform 3: parent 2, kind 1, ID 3782, translation (0, −0.19, 219.511).
- 288 directly nested records (183 of class 109, 26 of class 48, 21 placements, 13 of class 111, 8 of class 29, 7 of
  class 33 and others); among them 29 solids (26 of class 48, 3 of class 105), every one with a parent reference of
  the `0x1000 | n` form; 33 solids at all depths.

## 6. Open questions

- The meaning of the trailing three references and of transform kind 15 (one transform).
