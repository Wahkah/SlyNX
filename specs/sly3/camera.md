---
id: sly3-behaviour-camera
title: Sly 3 camera - zones, volumes, lens objects, cutscene cameras - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), every class-5, 7, 14 and 42 body of all 39 worlds with its options; the zone and volume
      meshes of four hubs; executable SCUS_974.64 SHA-256 ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
      and Sly 2's SCUS_973.16 (a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73) for the option tables
    used_for: option counts and values; value types and fields of both games' option tables [X]
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/game/gen (class and option correspondence tables)
    used_for: the Sly 2 to Sly 3 correspondence of classes and options; the statement that Sly 3 runs Sly 2's camera; marked [R]
reviewed_by: pending
date: 2026-11-19
---

# Sly 3 camera

Read [../sly2/camera.md](../sly2/camera.md) first: the view stack, following and smoothing, player input, the
binocucom view, obstruction handling and the timed moves carry over [R]. This document gives Sly 3's classes and
options. Tags as in [guards.md](guards.md).

## 1. Classes [D] [R]

| Sly 3 class | Sly 2 class | Role | Bodies |
|---:|---:|---|---|
| 14 | 15 | **camera zone** | 624, in 36 worlds |
| 7 | 5 | **visibility volume** | 25, in `…_vz` groups |
| 42 | 44 | **cutscene camera** | 134 (6 top-level, 128 nested in the characters' groups and others) |
| 5 | — | **lens object**: carries Sly 2's world lens options (section 2) | 60, in 39 worlds |

## 2. Lens

Sly 2 kept the lens in the world's second option list (options 161 to 170). Sly 3 keeps the fog in the world's
option set ([rendering.md](rendering.md) 6), and the lens on **class-5 objects** [D] [R for the correspondence]:

| Option | Sly 2 | Kind [X] | Meaning | On the disc [D] |
|---:|---:|---|---|---|
| 172 | 161 | float (setter) | field of view (rad) | 1.1 ×4, 0.8 ×2 |
| 173 | 162 | float (setter) | field-of-view target | — |
| 174 | 163 | value type `0x0C` at +0x124 | (Sly 2's 163) | — |
| 175 | 164 | float (setter) | near clip | 70 ×13, 50 ×3 |
| 176 | — | float (setter) | far clip [I from the values] | 70,000 ×11, 1,000,000 ×3 |

Class 5 also carries 167, 168, 181, 183 to 188 (open). Defaults as Sly 2 1 [R].

## 3. Camera zones (class 14)

- Option meanings, Sly 2 numbers and disc counts: [options-by-class.md](options-by-class.md) 2.1.
- **Priority**: option 225 (Sly 2's 200), view-stack priority base; the follow view is pushed at 225 + 2 as in Sly 2
  [R].
- **Switching: a zone is on while it is in the world** [X]. Sly 3 has no switching-mode option (Sly 2's 214) and
  no on/off field. The zone's load-time setup (`0x00273BB0`) sets +0x12C = 7 (the allowed behaviour bits, option
  224), +0x134 = −1 (option 227, never set on the disc) and +0x19C = 0. None of these is a mode, and nothing else in
  the zone's code reads +0x134. When the zone enters the world (vtable slot 25, `0x00273CC0`), it is appended to
  the world's camera-zone list at `0x00479370` (`0x0011AF60`, the list append). When it leaves the world (slot 24,
  `0x00273C28`), it is taken out of every player's remembered-zone list (`0x00166228`) and removed from the world's
  list (`0x0011B028`, the list removal, which also fixes running iterators). A zone
  in the list follows the player's position, as Sly 2's mode −1: it pushes its follow view while the player is
  inside, and the highest priority wins.
- **So a zone starts on**, unless one of the general world-membership rules keeps it out [X for 27, R for the rest]:
  - option **27** (removed at level start; its load method is `0x00100B08`, the only option with that method);
  - its option set not being the one chosen at load (the condition-picked option sets, [conditions.md](conditions.md));
  - a task's **object switch** (1416, [missions.md](missions.md) 3) listing it with states that exclude the
    current ones;
  - its placement or proxy copy not being taken.

  Scripts turn zones on and off with `0x38` / `0x39` (add to / remove from the world), and task switch groups do the
  same through 1416. There is no other flag.
- **The six zones of slot 15** [D for the check]: no command script in `h_ext` adds or removes 4152, 4153 or 4159
  (232 `0x38` / `0x39` commands there; none names them). A raw search of the world finds these IDs only in unrelated
  numeric tables, not in any 1416 switch. Unless they carry option 27 or belong to an option set not chosen at load,
  they are on at the start. Then the priority-5 overhead framing winning is the game's own behaviour, provided the
  player is really inside them. Check the inside test against [classes/solid-object.md](classes/solid-object.md)
  2.2: inside means ending at a missing negative child, behind every bounding surface.
- **The zone's curves**: 206 / 208 / 210, the x / y / z offset quadratics of the distance; the camera's path curve
  is the zone's nested object when it has one [R].
- **Return-toward-sample**: four triples (228 to 239) where Sly 2 had two [R, medium].
- **Shape and inside test**: the collision mesh and tree. Normals = normalise((p1 − p0) × (p2 − p0)) in stored
  corner order and point out of the zone; a point is inside when the tree walk ends at a missing negative child
  [D] [R] ([options-by-class.md](options-by-class.md) 2.1).

## 4. Visibility volumes (class 7)

As Sly 2 7.1: object mask, collision mesh and tree, options [D] [R]. Priority is option **202** (Sly 2's 175),
set on 5 of the 25 volumes (values 5, 1, 0, 10) [D]. The same winding rule holds (8 volumes checked) [D].

## 5. Cutscene cameras (class 42)

| Option | Sly 2 | Meaning | On the disc [D] |
|---:|---:|---|---|
| 1153 | 1054 [R, medium] | field of view | 15: 1.0 ×6, 0.95 ×5 |
| 1154 | 1055 | target object | 4 |
| 1155, 1156, 1157 | 1056 to 1058 | three vectors: always (0, 0, 0), (0, 0, −1), (0, 1, 0) [I: an eye offset and two axes] | 134 each |
| 1158 | 1059 | cut when the view ends | 104, all 1 |
| 27 | 26 | removed at start (placed by a cutscene) | 126 |

Behaviour as Sly 2 7.2 [R].

## 6. Constants [X]

Found by matching Sly 2's constant blocks against Sly 3's executable (16-byte chunks; values equal unless noted):

| Constant (Sly 2 camera.md) | Sly 2 address | Sly 3 address |
|---|---|---|
| follow view tilt limits −0.5 … 1.5551 rad | `0x2C41B0` | `0x003290E8` |
| stick-toward-camera lead factor (1, −2, −2) and limits 1 … 2 | `0x2C41F0`, `0x2C4200` | `0x003290F0`, `0x00329100` |
| turn-toward-sample smoothing (0.5, 0, 0.5) | `0x2C4208` | `0x00329108` |
| lead easing (4, 0, 0.25) | `0x2C4230` | `0x00329130` |
| passive tilt toward the limits (0.75, 0, 2) | `0x2C4260` | `0x00329140` |
| binocucom limits, zoom, turn smoothing (the block `0x2BD950` … `0x2BD9BF`) | `0x2BD950` | `0x00320E80` … `0x00320EBF` |
| obstruction check (first 16 bytes) | `0x2C4110` | `0x003212F8` |

Not found as data words in Sly 3 (probably immediate values, or changed): the 0.75 s hold, the 10 s manual-turn
delay, the 2.5 rad/s passive-lead limit, the manual orbit speeds (2.5, 1.0, 15), the return smoothing (5, 0, 0.25,
35) / (10, 0, 0.1, 100) and the springs (15 / −8, −10 / −5). A port uses Sly 2's values for these (open question 1).

## 7. Open questions

1. The Sly 3 values of the camera constants not found as data (section 6).
2. Class 5's other options, and whether 176 is the far clip.
3. (Settled: zones are on while in the world; section 3.) Whether any zone carries a forced state elsewhere, e.g. in its nested objects.
4. Every [R] rule needs checking in play.
