---
id: sly1-formats-collision
title: Sly 1 collision data and what it means
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 image (SHA-256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a), all 46 world files; executable SCUS_971.98
    used_for: >-
      all 4,637 solid records parsed (collision meshes 455,004 surfaces and 533,267 edges; camera meshes 203,374
      surfaces and 216,269 edges); edge convexity, plane and tree-side
      tests (worlds 1, 2, 12, 30); convex-volume inside test (worlds 1, 2, 12, 21); option 128 values on all
      worlds; the two material tables read from the executable; every test vector below
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release/geom.cpp, so.cpp, cm.cpp, shadow.cpp, mark.cpp, jump.cpp, jt.cpp, spaprops.cpp
    used_for: >-
      first reading of the camera-fade groups, the material option and its tables, and the floor, wall and ledge
      thresholds. The data facts were re-checked on the disc; the thresholds could not be and are marked
reviewed_by: pending
date: 2026-11-14
---

# Sly 1 collision

Byte layouts are in the object-stream specs: the solid layout in [classes/solid-object.md](classes/solid-object.md)
and the collision mesh, collision tree and convex volume in
[classes/shared-structures.md](classes/shared-structures.md). This document says what those bytes mean and what the
game derives from them. Notation as in [disc.md](disc.md).

## 1. Collision mesh

A set of planar **surfaces** (triangles) over a shared position list, in the owning object's local space (world
space for the static world mesh, which is placed at the identity).

- **Plane.** A surface's plane comes from its three corners: normal `n = normalise((p1 − p0) × (p2 − p0))`,
  offset `n · p0`. The normal points **out of the solid** (evidence: convex edges, below, are exactly those where the
  neighbouring surface lies behind this plane; and the tree's "positive" side, below, is the open side).
- **Bounding sphere.** Each surface stores a radius and a centre enclosing it, for quick rejection.
- **Edges.** An edge record (two position indices, the index of the other surface) lies on the planes of **both**
  surfaces it joins. Each edge is stored once, under one of the two surfaces. Only edges that matter for collision
  are stored: on 99.8 % of shared triangle edges the rule is **convex edges are stored, concave edges are not**.
  Measured on the collision meshes (not the camera meshes) of every solid in worlds 1, 2, 12 and 30 [D]:
  - a **shared edge** is a pair of corner indices that is a side of exactly two surfaces of the same mesh (42,352
    sides belong to one surface only and are not counted);
  - with n the unit normal of one of the two surfaces, p one of its corners and q the corner of the other surface
    that is not on the edge, the edge is **convex** if n · (q − p) < −ε, **concave** if > ε, else **flat**;
  - it is **stored** if any edge record of the mesh has the same two position indices (either order).

  With ε = 0.001: convex 34,688 stored and 7 not; concave 43 stored and 18,356 not; 1 flat (not stored). With
  ε = 0.1: convex 34,672 / 7, concave 43 / 18,342, flat 16 stored and 15 not. Other choices (a relative
  tolerance, matching corners by position instead of index) move these counts by a few hundred at most, never the
  rule. Earlier drafts gave 33,992 / 7 / 43 / 17,793; another measurement gave 34,503 / 7 / 41 / 18,226. The
  definitions above are the ones to test against.
  Edge end points are not always corners of the surface that owns the record (T-junctions). A surface can have up
  to 98 edges; 124,367 surfaces have none.
- Totals: the header's edge total always equals the sum of the per-surface counts.

### 1.1 What edges are used for

*Reference port, unconfirmed on the game:* a stored edge whose one surface is **floor-like** (normal z > 0.7) and the
other **wall-like** (normal z < 0.55) is a **ledge** the player can grab; its outward direction is the horizontal
perpendicular to the edge that points the same way as the wall surface's normal. Edges also let moving bodies
collide with corners, not just faces.

## 2. Collision tree

A binary space partition over the surfaces' planes, used to find the surfaces near a point or along a segment.

- Node 0 is the root; every other node has exactly one parent (checked on The Hideout's world mesh).
- A node names a surface; its plane splits space. The **positive** child is on the side the normal points to, the
  **negative** child behind. `0xFFFF` = no child. Child counts [D]: The Hideout's world mesh has 2,169 negative and
  1,996 positive children (4,166 nodes); over every collision and camera tree on the disc (1,721,686 nodes),
  1,002,561 negative, 715,133 positive, 465,969 nodes with both. (An earlier draft said "almost all positive"; that
  was wrong.)
- The tree describes a **solid volume** [R]: following the tests down, ending at a missing positive child means
  open space, ending at a missing negative child means inside the solid. A segment is cut into pieces by the planes,
  and the pieces that end inside are the collisions; each piece records the surface where it enters and leaves.
- A surface can appear in several nodes (The Hideout's world mesh: 1,681 surfaces, 4,166 nodes).
- The "full node count" field equals the node count in every solid; it is the size of a scratch tree a search may
  build.

## 3. Camera collision mesh

A second mesh and tree with the same layouts, used only by the camera. It is empty in 4,283 of 4,637 solids
(including every world mesh); there the camera uses the normal collision mesh.

## 4. Camera-fade groups

(Solid layout rows 7 and 8.) In 83 solids, scenery that blocks the camera fades out instead of being collided with:

- **Group list**: entries of (`s16` kind, `u16` mesh index into the object's mesh set). Kind is `1` (start of a
  group) or `−1` (continues the group started by the previous entry); no other value occurs.
- **Node map**: one `s16` per node of the collision tree (when present its length equals the tree's node count),
  giving the first group-list entry for that node, or `−1`.
- Behaviour (*reference port*): each frame, the segment from the camera to the player is pushed through the
  collision tree; for every node it touches that has a group, every mesh of that group (the first entry and the
  following `−1` entries) is drawn faded for the next 0.1 s.

The "hit group" count (layout row 9) is 0 in every solid on the disc, so rows 9 and 10 never carry data.

## 5. Convex volumes

(Used by trigger volumes, exits, camera zones and flashes.) Planes `n · x = d` and a tree of nodes (plane index,
negative child, positive child, −1 = leaf). To classify a point in the volume's local space: start at node 0; go to
the negative child if `n · x − d < 0`, else to the positive child; when the chosen child is −1 the point is
**inside** if the last test was negative and **outside** if it was positive. (Measured: a point far away always
ends outside; the volume's own origin ends inside for 63 of 75 volumes sampled.)

## 6. Surface materials

A solid's **material** is set by option 128 (`s16`; default 0). Values on the disc: 1 (44 solids), 10 (1), 12 (2).
Materials give friction and bounciness to contacts, from two tables of **13 entries** in the executable:

| Table | Virtual address | File offset in the executable |
|---|---|---|
| Friction | `0x00264840` | `0x165840` |
| Elasticity | `0x002648A8` | `0x1658A8` |

Each entry is two `f32`: a **weight** `w` and a **weighted value** `a`. For a contact between materials `i` and `j`
(a body with no material uses 0):

- friction `μ = (a_i + a_j) / (w_i + w_j)`, at least 0;
- elasticity `e = (a_i + a_j) / (w_i + w_j)`, clamped to 0..1.

The packager reads both tables from the player's executable, after checking the executable's SHA-256 against
the value in disc.md (it refuses on a mismatch). Check values (entry 0 and 1 of each): friction
(1.0, 0.5), (3.0, 2.4); elasticity (1.0, 0.5), (5.0, 1.0). Entry 12 (used by two solids): friction (1.0, −10.0),
elasticity (1.0, 1.0): friction is 0 against every material, and elasticity against material 0 is 0.75.

*Note:* the reference port carries 12 entries per table and differs from the executable at friction entry 9; follow
the executable.

## 7. Floors, walls and ceilings

*Reference port values, to be confirmed by play:*

- A contact counts as **ground** when its normal's z (pointing away from the ground) exceeds 0.7 (about 45.6°).
- A surface is **floor-like** for ledges at normal z > 0.7 and **wall-like** at normal z < 0.55.
- **Back faces.** There is no one-sided face test: a point is colliding when the tree puts it inside the solid,
  whichever side it came from [R]. A body that starts behind a surface (inside the volume) is therefore in contact
  and is pushed out along the entry surface's normal; it does not pass through.
- There are no per-surface type flags in the data: floors, walls and ceilings follow from the normals alone. Z is
  up ([geometry.md](geometry.md) 6.3).

The collision data has no per-surface material or sound field; footstep and slide sounds are chosen outside this
data (gameplay spec), at most from the solid's material of section 6.

## 8. Test vectors

The Hideout (world 2), static world mesh (first object, class 4), solid body starting at `0x8659`:

| Item | Value |
|---|---|
| Collision mesh bounding radius | 1080.9119 |
| Positions / surfaces / edges | 1,347 / 1,681 / 1,637 |
| Position 0 | (−19.0479, 162.2528, −244.6026) |
| Surface 0 | corners 1233, 1234, 382; radius 103.859; centre (−36.580, −130.557, 319.883); no edges; normal (0.0, 0.11291, −0.99360) |
| Tree | 4,166 nodes, full count 4,166; node 0 = (surface 0, no negative child, positive child 1) |
| Camera mesh, fade groups, node map, hit groups | all empty |

Disc-wide: 4,637 solids; 999 have an empty collision mesh; 83 carry camera-fade groups; 122 carry a node map.
Collision meshes total 455,004 surfaces, 533,267 edges and 1,193,176 tree nodes; these totals **exclude** the
camera meshes, which add 203,374 surfaces, 216,269 edges and 528,510 nodes [D].

## 9. Open questions

- The thresholds in 1.1 and 7 (ground and floor-like above normal z 0.7, wall-like below 0.55) come only from the
  reference port. They cannot be read from the disc data: no table or field holds them, and the floats 0.7 and 0.55
  occur in the executable in many unrelated places. Confirm by play: a slope of 45° (z = 0.707) should still be
  ground, 46° (z = 0.695) not; a ledge should be grabbable where the face below it is steeper than
  56.6° from horizontal (z < 0.55) and not where it is shallower.
- The meaning of material values other than 0, 1, 10 and 12 is irrelevant to this disc (they do not occur).
- How the game uses the mass, inertia and centre of mass rows belongs to the physics spec.
