---
id: sly3-formats-transforms
title: Sly 3 transforms
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97464 image, all 39 worlds
    used_for: every transform list (11,524 solid bodies, 56,652 transforms) parsed; value surveys and test vectors
  - kind: reference-project
    what: earlier research (Games/Sly3-RE-notes.md, tools/sly3_geom.py, 2026-11); SafeHouse @ c70803ce, Sly3-Release transform loader and skin binding
    used_for: the composition rule, the placement order and the skin binding; marked [R]
reviewed_by: pending
date: 2026-11-14
---

# Sly 3 transforms

**Same as Sly 2** ([../sly2/transforms.md](../sly2/transforms.md)): matrices (48 bytes: three axis images and a
translation), composition (local, then the parent's world matrix), transform references (−1, `n`, `0x1000 | n`,
`0x2000`), the transform list at the start of every solid body, and the skin binding (section 4 there). [D for the
layout, R for the rules] Tags as in [disc.md](disc.md).

## 1. Transform list

Field for field as Sly 2 section 2 (count, transforms, two-bone chains, mirror pairs, follower chains, three
references); one transform as Sly 2's (parent, kind, ID, flags, the flagged fields, then the local matrix, or an
`s16` mirrored-transform reference for kinds 8 and 14). [D]

Values on the disc [D]:

| Item | Values (count) |
|---|---|
| Kinds | 6 (21,215), 1 (13,530), 0 (7,980), 12 (7,410), 13 (6,511), 15 (6); kinds 8 and 14 do not occur |
| Flags | 0 (51,812), 4 (2,178), 1 (1,202), 5 (1,051), 16 (196), 32 (109), 8 (62), 6 (27), 9 (13), 17 (1), 64 (1) |
| Parents | local 45,128; none 4,256; enclosing object 7,268 |
| Two-bone chains, mirror pairs, follower chains | 578, 88, 1,209 over all bodies |

Kind 1 is the transform an animation channel moves by translation, 6 by Euler rotation, 12 by scale (as Sly 2;
[animation.md](animation.md)). [R]

## 2. Placements

A copy placement composes each object of the copied group with the placement matrix: in row-vector form
`world = local × placement` ([classes/placement.md](classes/placement.md)). [R] Objects inside a placement's object
list and inside zones and groupers are placed by their owner's matrix the same way. [R]

## 3. Skin binding

As Sly 2 section 4: skinned positions are stored in the mesh transform's space in the loaded pose; each bone's skin
matrix is (inverse of the bone's world matrix as loaded) × (the mesh transform's world matrix as loaded), then × the
bone's current world matrix. [R] Bone table entries resolve inside the character's own tree. [R]

## 4. Test vectors

Sly in world slot 5 (v_apt; class 52, object 24, body at `0x37FDE3`) [D]: 146 transforms; transform 0: parent −1,
kind 0, ID −1, flags 0, identity; transform 1: parent 0, kind 1, ID 24, identity rotation, translation (0, 0, 75);
4 two-bone chains; 1 mirror pair (6, 73, 0, 1); 4 follower chains; trailing references (1, 2, 2). (Sly 2's Sly has the
same transform 1 and the same trailing references.)

Level object of v_apt (class 48, object 1): 2 transforms, both identity; transform 0 has parent −1 and kind 1,
transform 1 parent 0 and kind 6; trailing references (0, 1, 1). [D]

## 5. Open questions

- As Sly 2: the trailing references and kind 15.
