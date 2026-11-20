---
id: sly3-behaviour-physics
title: Sly 3 physics - gravity, constraints, physics options, collision parts - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), all 39 worlds, untruncated option survey; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: values of the physics options on every class; the collision mesh of v_apt
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/game/gen (class and option correspondence tables)
    used_for: the Sly 2 to Sly 3 option correspondence (scores in brackets); the statement that Sly 3 runs Sly 2's solver; marked [R]
reviewed_by: pending
date: 2026-11-19
---

# Sly 3 physics

Read [../sly2/physics.md](../sly2/physics.md) first: what moves, one physics step, constraints and contacts are as
Sly 2 [R]. Tags as in [guards.md](guards.md).

## 1. Options [D] [R]

| Sly 3 | Sly 2 (score) | Meaning (Sly 2) | Sly 3 values [D] |
|---:|---:|---|---|
| 33 | 33 (95) | linear constraint | 356 uses: 0 on all characters; 4 ×21, 0 ×9 on class 47 |
| 34 | 34 (95) | angular constraint | 65 uses |
| 35 | 35 (96) | gravity vector | characters (0, 0, −1,750); Carmelita (60) (0, 0, −2,275); Dimitri (59) (0, 0, −4,000); props (0, 0, −1,700) and others |
| **254** | 225 (95) | **gravity ratio** | **1.3** on Sly, Murray, Bentley (classes 52 to 54) and the Guru, Panda King, Penelope (55 to 57) |
| 37 | 37 (96) | pairs re-tested every frame | 44 |
| 39 | 39 (95) | physics root of its own group | 113 |
| 40 | 40 (93) | iced | 10 |
| 52 | 52 (96) | can be iced | 7 |
| 59 | 57 (95) | fade value | not used |
| 65 | 63 (96) | one physics flag bit | 74, all 1 |
| 77 | 76 (96) | set angular velocity | not used |
| 82, 83, 84 | 80, 81, 82 (96) | sphere entry: radius, mass, height offset | radius 250 / 450 on props, 150 / 200 on guards; mass 40 / 20 on props, 1,000 on guards |
| 63 | 61 (17, low) | physics mode field | 2 on 93 |

So the playable gang falls at **−1,750 × 1.3 = −2,275** units/s², exactly as in Sly 2; Carmelita (no ratio option; a default ratio of 1 assumed [I]) gets the same
value directly through option 35, and Dimitri falls at −4,000 [D] [R for the product]. Sly 2's "set velocity" (75)
has no Sly 3 counterpart [R].

## 2. Collision parts

- Two extra bytes per collision part ([classes/solid-object.md](classes/solid-object.md) 2) [D]; their meaning is
  open.
- **Surfaces are convex polygons** whose outline is the edge list; the three corners fix the plane only
  ([classes/solid-object.md](classes/solid-object.md) 2.1) [D]. A contact test must use the polygon.
- Surface normals point out of the solid ((c1 − c0) × (c2 − c0) from the stored corners) [D].

## 3. Constants

Sly 2's default gravity (−1,750 for a solid created or put to rest), friction 0.5 and elasticity 0.5 are taken as
unchanged [R]; their Sly 3 storage was not located (open question 1).

## 4. Test vectors

| Case | Result |
|---|---|
| a playable character dropped from rest, 1 s (no terminal-speed rule) | falls 1,137.5 units |
| Dimitri dropped from rest, 1 s | falls 2,000 units |

## 5. Open questions

1. Where Sly 3 keeps the default gravity, friction and elasticity.
2. The two new collision-part bytes.
3. Every [R] rule needs checking in play.
