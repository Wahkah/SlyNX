---
id: sly2-physics
title: Sly 2 physics and contact resolution (differences from Sly 1)
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds walked
    used_for: option values for gravity, gravity ratio, constraints, region spheres and the other physics options on
      every object and placement; the contact defaults read from the executable
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly2-Release/physworld.cpp, solve.cpp, so.cpp, mark.cpp, carry.cpp, pad.cpp
    used_for: the physics frame, the two solver passes and thresholds, the rest rule, contact defaults, constraint
      kinds, option meanings; [R]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), the option table entries for gravity and gravity ratio (facts only)
    used_for: which option IDs set gravity (35) and the gravity ratio (225)
reviewed_by: pending
date: 2026-11-14
---

# Sly 2 physics and contact resolution

A delta of [`../sly1/physics.md`](../sly1/physics.md). Tags: [D] disc data, [O] observable, [R] reference behaviour
to confirm, [I] inferred. Units are game units and seconds; Z is up ([transforms.md](transforms.md)). Collision parts
are in [classes/solid-object.md](classes/solid-object.md).

## 1. What moves

As Sly 1 section 1, with these differences [R]:

- A solid object's collision is a **list of collision parts**, each with its own mesh and tree, a tag and a contact
  kind (a bit filter carried by its contacts). Options and scripts switch parts on and off by tag and change a
  part's contact kind (for example an attack turns a character's cane part into a striking part) [R].
- Each solid has a **linear** and an **angular constraint** (options 33 and 34) instead of Sly 1's single fixed flag
  (section 4).
- Only objects in the update list take part (activation, [clock-and-loop.md](clock-and-loop.md) 3); an object whose
  linear and angular constraints are both fixed is never solved [R].

## 2. Gravity

| Who | Acceleration (z) | Source |
|---|---|---|
| a solid's physics state when created or put to rest | **−1750** units/s² | [R] |
| any object with option 35 | the option's vector | [D] for the values |
| the three playable characters | option 35 = (0, 0, −1750) **and** gravity ratio (option 225) = **1.3**, so **−2275** units/s² | [D] (all 126 character records) [R] for the product |

Option 35 values on the disc [D]: (0, 0, −1750) 215 times (the three characters 42 each, crowd actors of class 57,
and others), (0, 0, −5000) 29, (0, 0, −4000) 23 (class 60), (0, 0, −3000) 13 (guards, class 58), (0, 0, 0) 7,
(0, 0, −2000) 3, (0, 0, −450) 1.

The gravity ratio rule (gravity × r, mass ÷ r) is as Sly 1 section 2 [R]. Option 225 occurs only on the three
characters, always 1.3 [D]. Terminal fall speed and the player-only rules belong to the characters spec.

Carried objects (the carry layouts) use no gravity while held or gone; otherwise gravity, and two carry kinds also
turn toward level with a rotation spring of strength 45 and damping 7.5 [R].

## 3. One physics step

Order within the frame: [clock-and-loop.md](clock-and-loop.md) 5, step 4.6 [R].

1. **Contacts**: refresh the contacts of the objects that moved.
2. **Integrate** every moving entry of the update list (velocity and position from gravity, springs, applied
   accelerations).
3. **Pass 1** (resting contacts): for every active solid not fixed in both constraints, gather its group and solve
   the group's contacts (at most **64** per group) at velocity level. Unlike Sly 1, objects whose position and
   rotation are both still are skipped in this pass.
4. **Contacts** again.
5. **Pass 2** (penetration and impact), with the same thresholds as Sly 1 section 3 [R]:
   - penetration is corrected only if some contact is deeper than **2** units (equality contacts: off by more than
     **0.1**); the target is the depth plus 1 unit;
   - an impact is solved only if some contact approaches faster than **5** units/s (equality contacts: either
     direction);
   - the impact's target separation speed is the approach speed × (1 + elasticity × k), k random in **[0.9, 1.1]**
     per contact.
6. **Coming to rest** (new): after pass 1, if every body of a group moves slower than **5** units/s (speed² ≤ 25)
   and turns slower than **0.05** rad/s (angular speed² ≤ 0.0025), and neither of its transforms is animation-driven,
   each body is moved back by half a frame along its velocity and its velocity is set to zero [R].

Contact records older than **0.25 s** are discarded each frame [R].

**Friction and elasticity.** Sly 2 has no surface-material tables. Every new contact starts with friction **0.5**
and elasticity **0.5**, read from the executable at virtual addresses **`0x002BFD30`** and **`0x002BFD34`** [D];
classes may override them when the contact is created (carried objects use elasticity **0.25**) [R].

## 4. Constraints

Options 33 (linear) and 34 (angular) take a value 0 to 7 [R]:

| Value | Kind | Axis (in the object's frame) |
|---:|---|---|
| 0 | free | — |
| 1 | fixed | — |
| 2, 3, 4 | axis-limited, form A | X, Y, Z |
| 5, 6, 7 | axis-limited, form B | Z, Y, X |

Values on the disc [D]: option 33: 0 (313), 4 (48), 5 (27), 1 (19); option 34: 0 (53), 1 (36), 3 (15), 4 (6),
5 (5), 2 (2). Which of the two axis-limited forms means "only along the axis" and which "only across it" is not yet
established (open question 1).

Putting a solid to rest (a class operation) sets gravity to (0, 0, −1750) and both constraints to fixed [R].

## 5. Other physics options

| Option | Meaning [R] | Disc values [D] |
|---:|---|---|
| 37 | pairs of this object are re-tested every frame (on/off) | 1 (61) |
| 39 | the object is a physics root of its own group | 1 (52, mostly class 81) |
| 40 | iced (frozen in place) | — |
| 52 | can be iced | 1 (8) |
| 57 | fade value; 0 or less removes the object | 0.01 (12) |
| 61 | a 4-bit physics mode field | 2 (86) |
| 63 | one physics flag bit | 1 (28) |
| 75, 76 | set velocity, angular velocity (object frame) | 76: 66 uses |
| 80, 81, 82 | the object's sphere entry: radius, mass, height offset (its use is open question 2) (default radius 150 for the player characters, 75 otherwise, mass 1) | 80: 50 (13), 1,000 (12), 25 (6), …; 81: 500 (19), 1,000 (9), 2 (2); 82: −210, −220 |
| 225 | gravity ratio | 1.3 (126) |

## 6. Test vectors

| Case | Result |
|---|---|
| a solid with default gravity dropped from rest, 1 s | falls 875 units, speed 1,750 units/s |
| a playable character (−2275) dropped from rest, 1 s, ignoring the characters' terminal-speed rule | falls 1,137.5 units |
| a contact between two objects with default values | friction 0.5, elasticity 0.5; bounce target 1.45 to 1.55 × the approach speed |

## 7. Open questions

1. The meaning of the two axis-limited constraint forms (along vs across the axis) and of the angular ones.
2. What the sphere entry of options 80 to 82 is used for (characters pushing apart, or a broad-phase region) [I].
3. How water acceleration (a per-class operation) and buoyancy work in Sly 2.
4. The solver's iteration count and friction model (as Sly 1 open question 1).
