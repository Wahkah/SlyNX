---
id: sly2-camera
title: Sly 2 camera (differences from Sly 1)
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 decompressed worlds
    used_for: >-
      camera zones (class 15, 619 bodies), visibility volumes (class 5, 30), camera objects (class 44, 182) and the
      world camera option list, all read by a scratch walk of every body; option IDs and values; constants read from
      the executable at the virtual addresses given
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly2-Release/cm.h, cm.cpp, cplcy.h, cplcy.cpp, cmworld.cpp
    used_for: what each camera option and constant does, the view stack, the follow rules and the binocucom view; [R]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), src/MANIFEST.tsv
    used_for: confirming which script built-ins drive the camera; [R]
date: 2026-11-14
---

# Sly 2 camera

A delta of [../sly1/camera.md](../sly1/camera.md). Tags: [D] disc data (world bodies, or a value read from the
executable at the given virtual address — file offset = address − 0x100000 + 0x80, after checking the executable's
SHA-256, [disc.md](disc.md)), [O] observable, [R] reference behaviour, [I] inferred. Units: game units, seconds,
radians. Class and option IDs are those of [object-stream.md](object-stream.md).

## 0. What changed, in one paragraph

Sly 1 had one camera-zone class with a convex volume and options 851 to 871. Sly 2 replaces it with a **camera zone
class (15)** whose shape is a collision mesh and tree, whose parameters are options 178 to 214, and which may carry a
curve for the camera to follow; zones can be switched on and off by scripts instead of only by the player's
position. A separate **visibility volume class (5)** chooses which objects are active by the camera's position.
**Camera objects (class 44)** are the cutscene cameras. The camera keeps a **stack of views** ordered by priority,
and the top one drives it. The lens and fog defaults, the follow spring and the manual orbit speed are the same as
in Sly 1. [D] [R]

## 1. Lens and fog

As Sly 1 section 1, with these differences [D]:

| Setting | Sly 2 default | World override (second world option list, [world-file.md](world-file.md) 3.7) |
|---|---|---|
| field of view | 1.0 rad [R] | option 161 (13 worlds: 1.1 ×8, 1.0 ×4, 0.95 ×1); option 162 sets a target the field of view eases toward |
| aspect | 512 / 394.24 = 1.2987 [R] | — |
| near clip | 100 [R] | option 164 (28 worlds; 52 ×8, 70 ×7, also 10 to 30) |
| far clip | **100,000** (Sly 1: 20,000) [R] | option 165 (28 worlds; 70,000 ×7, 52,000 ×6, 10,000 to 62,000) |
| fog start / end / maximum | 500 / 20,000 / 0.5 [D 0x2BD81C] | options 166, 167, 169 (42, 41, 42 worlds; e.g. 3000 / 30000 / 0.3) |
| fog colour | `0x80303030` [R] | option 168 (packed colour, 42 worlds; alpha byte `0x80` in the most common values) |
| draw-distance ratio | 1.0 [R] | option 170 (one world: 6.0) |
| start position, start angles | — | options 171 (position) and 173 (Euler angles in degrees) [R]; not set in any world [D] |

Field-of-view changes are smoothed with (0.5, 0, 0.1, 10) as in Sly 1 [R].

Test vector: nightclub cellar (level-table row 3) sets 161 = 1.0, 164 = 25, 165 = 25,000, 166 = 4000,
167 = 30,000, 168 = `0x8034677C`, 169 = 0.5 [D].

## 2. The view stack

The camera holds up to 8 views, each with a priority, a focus object and an argument; the highest priority drives
the camera and the rest wait [R]. A view entering or leaving the top can cut the camera (Sly 1 section 2). The view
kinds and the priorities they are pushed with [R]:

| View | Pushed by | Priority |
|---|---|---|
| default (holds still) | start of the world | 0 |
| follow (section 3) | a camera zone switching on | zone option 200 + 2 (option 200 is 0 to 3 on the disc: 0 ×24, 1 ×156, 2 ×98, 3 ×269) [D for the values] |
| camera object (section 5) | a camera object entering the world | 10 |
| timed move between two camera states | scripted transitions | 10 |
| attached to a turret or vehicle mount | the player taking a turret or vehicle | 9 |
| two-character framing | some confrontations (both actors in view, wider and farther as they part) | 6 |
| binocucom (section 6) | the binocucom | [R] not established |
| free / scripted | scripts and development | [R] |

**Timed move** [R]: over a set time the position, Euler angles and field of view blend from state A to state B
along one of four cubic curves stored as (u³, u², u) coefficient triples, 16 bytes apart, at 0x2BDA70 [D]: linear
(0, 0, 1), (−1, 2, 0), (−1, −1, 1) [the third reads oddly; to be checked] and smoothstep (−2, 3, 0).

## 3. Camera zones (class 15)

### 3.1 Shape, path and switching

- Layout: [classes/paths-and-volumes.md](classes/paths-and-volumes.md) 2. The collision mesh and tree are the zone's
  volume; the player is inside when a walk of the tree from node 0 ends behind a node with no negative child
  (as Sly 1's convex volumes, [../sly1/collision.md](../sly1/collision.md) 5) [R]. 619 zones, 4 with no options
  [D]. Per-world counts in section 8.
- The single attached object, when present (215 zones), is the **curve the camera follows** [R].
- **Switching** [R]: option 214 sets the zone's mode: −1 (the default, 401 zones) follows the player's position
  (on when the player is inside; while the player is airborne the zone keeps its state unless option 201 is set or
  the camera cuts); 0 (210 zones) forced off and 1 (8 zones) forced on, until a script changes it. A zone switching
  on pushes the follow view with its parameters; switching off revokes it; each switch raises the zone's script
  event (on / off) [R].

### 3.2 Framing parameters

A zone has up to 4 framing entries, one per focus class; with none set it uses the default entry. Every entry starts
from the default entry at 0x2C4120 [D], which equals Sly 1's default framing [D values; R meaning]:

| Field | Option | Default [D] | Disc values [D] |
|---|---|---|---|
| focus class the entry applies to | 176 | 48 (any solid object) | never set |
| distance along the path / behind the focus | 178 | −450 | −400 ×188, −450 ×120, −600 ×56, −500 ×50, −100 ×43 |
| x offset quadratic (c0, c1, c2) | 179 | (0, 1, 0) | (0, 1, 0) ×111, (−750, 1, 0) ×11 |
| y offset quadratic | 181 | (0, 1, 0) | (0, 1, 0) ×138, (0, 0.5, 0) ×22 |
| z offset quadratic | 183 | (125, 1, 0) | (80, 1, 0) ×114, (150, 1, 0) ×93, (300, 1, 0) ×63 |
| x, y, z offset limits | [R] | unlimited | — |
| screen offset x | 185 | 0 | 0, ±0.2 to 0.4 |
| screen offset y | 186 | −0.1 | −0.2 ×169, −0.1 ×128, 0.3 ×91 |
| focus point between base (0) and adjusted (1) position | 187 | 1 | 0.5 ×161, 1 ×147, 0 ×106 |
| passive-turn gain while the player moves | 188 | 1 | 0 ×106, 3 ×44, 1 ×19 |
| rate of turning toward the samples | 189 | 1 | 1 ×17 |
| focus-point coefficient scale (x, y, z) | 191 (190 sets x and y together) | (1, 1, 1) | (0.7, 0.7, 1) ×32, (1, 1, 1) ×23 |
| distance and screen-offset spring / damping scale | 192 / 193 | 1 / 1 | 1 / 1 ×85, 0.125 / 0.5 ×42 |
| tilt limits | 194 | unlimited | (0.55, 0.9) ×32, (0.5, 0.6) ×26 |

Zone-wide options [D IDs and values; R meaning]:

| Option | Meaning | Default | Disc values |
|---|---|---|---|
| 196 | coefficients are cylindrical (angle, radius, height) | 0 | 1 ×6 |
| 195 | cylindrical angle change per frame limit | — | 0.2 ×1 |
| 197 | allowed behaviour bits: 1 first sample, 2 second sample (needs a path), 4 player may turn the camera | 7 | 4 ×499, 5 ×90, 1 ×38 |
| 198 / 199 | return toward the sample after the player stops (1) / after a manual turn (2) | 0 | 1 ×85; 1 ×8 |
| 203, 204 / 205, 206 | return 1 time and strength / return 2 time and strength | 2, 1 / 2, 1 | 204: 0.1 ×42, 5 ×20 |
| 200 | view-stack priority base | 0 | see section 2 |
| 201 | follow position even while the player is airborne | 0 | 1 ×13 |
| 207 | cut the camera when the zone switches on | 0 | 1 ×20 |
| 208 | field of view while the zone is on (0 = keep) | 0 | 1.1 ×191, 1.0 ×174, 0.8 ×88, 1.2 ×34 |
| 209 | no obstruction avoidance | 0 | 1 ×52 |
| 210 | look-toward object (an OID) | none | 3 zones |
| 211, 212 | lead toward that object: quadratic of the distance, and its limits | (0, 0.5, 0), (0, unlimited) | (800, 0.02, 0) and (1100, 0.02, 0) ×42 each; (500, 2000), (1100, 1800) ×42 each |
| 214 | switching mode (3.1) | −1 | 0 ×210, 1 ×8 |

**Two samples.** With a path, the camera computes two candidate eye positions ("samples") from the framing entry: one
at the path parameter `distance` behind the focus's nearest point and one the same distance ahead (option 178 is the
offset of the two path parameters), each offset by the three quadratics in the zone's frame and clamped. It turns
toward the nearer sample, or blends by how the player faces the path (weight d²(3 − 2|d|), d the facing's dot
with the path's normal) [R]. Without a path there is one sample (as Sly 1's fixed frame) [R].

## 4. Following and smoothing

As Sly 1 section 4, with the same spring for the eye distance (strength 15, damping −8, each scaled by options
192 / 193) [D 0x2C426C, 0x2C4270], and these Sly 2 values read from the executable [D; meanings R]:

| Constant | Value | Address |
|---|---|---|
| tilt limits of the follow view | −0.5 to 1.5551 rad | 0x2C41B0 |
| hold after the player stops, before returning | 0.75 s | 0x2C41D8 |
| time after a manual turn before automatic turning resumes fully | 10 s | 0x2C41DC |
| passive-lead limit (rad/s) | 2.5 | 0x2C41E0 |
| manual orbit: pan speed, tilt speed, acceleration | 2.5 rad/s, 1.0 rad/s, 15 rad/s² | 0x2C41E4..0x2C41EC |
| stick-toward-camera lead factor (quadratic of stick y, limits) | (1, −2, −2), 1 to 2 | 0x2C41F0, 0x2C4200 |
| turn-toward-sample smoothing (× rate) | (0.5, 0, 0.5) | 0x2C4208 |
| lead easing | (4, 0, 0.25) | 0x2C4230 |
| return smoothing, normal / fast | (5, 0, 0.25, 35) / (10, 0, 0.1, 100) | 0x2C4240, 0x2C4250 |
| passive tilt toward the limits (× player speed) | (0.75, 0, 2) | 0x2C4260 |
| screen-offset spring / damping | −10 / −5 | 0x2C4274..0x2C4280 |

The focus-adjust springs are as Sly 1 (5 / −3.5 normally; 8 / −6 when the player focuses on a point; 1 / −10 while
hanging still on a ledge) [R].

## 5. Player input

As Sly 1 section 5 for the pan. New [R]: the right stick's vertical axis tilts the camera at y·|y|·1.0 rad/s;
a stick direction within 15° of horizontal turns only the pan, within 15° of vertical only the tilt. The player's
camera turn plays a sound and is reported to the game (used by hints). Zones whose option 197 lacks bit 4 do not
let the player turn the camera. [R] [O]

## 6. Binocucom view

The binocucom is its own view in Sly 2 (Sly 1's look-around question is settled for Sly 2) [R]:

- Eye: 150 units above a character focus (no raise for other objects), then 50 units behind it along the view
  [D 0x2A5000 for the 50; R for 150]. Near clip 49 while it is on [R].
- Limits: for a character, pan −10 to 10 rad (effectively free) and tilt −1 to 1 rad; for another object, ±1 rad
  around its own heading, tilt held within ±1.5 rad [D 0x2BD950..0x2BD970].
- Turning: the left stick, target speeds x·|x|·0.9 and y·|y|·0.8 (pan, tilt), accelerating at 10 rad/s², slowing
  near the limits with smoothing (1, 0, 0.2); the turn rate is scaled by 1.3 × the current field of view
  [D 0x2BD990..0x2BD9B0]. "Invert Binocucom Controls" flips the tilt [R].
- Zoom: the right stick's vertical axis; zoom speed = (2 − u²)·y·|y| with u the zoom 0..1 [D 0x2BD980]; the field of
  view runs from the normal value down to 1/7 of it [R]. A zoom sound follows the zoom speed [R].
- Leaving the view cuts the camera to look back the way the view looked [R].

## 7. Keeping the view clear

As Sly 1 section 7, with these values [D 0x2C4110, 0x2C4290; R meaning]: the focus is checked for visibility every
0.5 s at five points (the focus, ±50 sideways, +75 up, −50 down); after 2 s unseen the camera cuts to a clear
position. Zones with option 209 skip obstruction handling.

## 7.1 Visibility volumes (class 5)

New in Sly 2 [D layout; R behaviour]: 30 volumes, only in packages whose names end in `_vz`. Each has the transform
header, a `u32` **object mask**, a collision mesh and tree, and options. Every frame the camera finds the volumes
it is inside (up to 32), prefers the highest option 175 (values 1 ×5, 2 ×3 on the disc; default 0) and then the
most recent entry, and uses that volume's mask (default `0xFF` when in none): only objects whose own mask shares a
bit with it are activated and drawn. Masks on the disc: 1 ×12, 2 ×10, 4 ×3, 8, 16, 32 ×1 each [D].

## 7.2 Cutscene cameras (class 44)

Camera objects (182 on the disc, header + options layout) are the cutscene and briefing cameras [D] [R]:

- Entering the world pushes the camera-object view at priority 10; the camera sits at the object's transform (driven
  by animation clips, [animation.md](animation.md)) and looks along its matrix or at its target [R].
- Options [D values; R meaning]: 1055 target object OID (36 objects); 1054 field of view (28: 1.0 ×17, 0.6 ×8);
  1059 cut when the view ends (93, all 1); 1056, 1057, 1058 three vectors, always (0, 0, 0), (0, 0, −1), (0, 1, 0)
  on the disc (178 each) [I: an eye offset and two axes]. 171 of the 182 carry option 26 (removed at level start
  until a cutscene places them) [D].
- A lens may also be set at run time as a focal length f: field of view = 2·atan(17.996 / f) [R].

## 8. Test vectors

Per world (level-table row: zones / visibility volumes / camera objects) [D]:
0: 22 / 8 / 8; 2: 26 / 2 / 11; 3: 9 / 0 / 2; 4: 10 / 0 / 10; 5: 15 / 0 / 4; 8: 35 / 0 / 8; 12: 19 / 2 / 12;
14: 53 / 2 / 9; 17: 41 / 0 / 8; 27: 35 / 0 / 8; 32: 35 / 2 / 6; 38: 35 / 2 / 7; 43: 9 / 2 / 2.

Zone OID 1524, row 3 (body at decompressed offset `0x32FC8E`) [D]: options 214 = 0, 197 = 4, 178 = −400,
183 = (150, 1, 0), 208 = 1.0, 186 = 0.3, 187 = 1.0, 188 = 3.0, 192 = 1.0, 193 = 1.0, 200 = 3. Expected: a
script-switched zone at priority 5 that keeps the camera 400 behind and 150 above the focus with the focus drawn 0.3
of the screen above centre, field of view 1.0, player turning allowed.

Default framing (no zone): as Sly 1 section 8 (450 behind, 125 above, focus 0.1 below centre) [D 0x2C4120].

## 9. Open questions

1. The third curve of the timed move (0x2BDA70) and which scripts use each curve.
2. The binocucom view's priority in the stack and how the "Invert Binocucom Controls" save flag is stored.
3. What the three camera-object vectors (options 1056 to 1058) are; they never vary on the disc.
4. The camera shake ("jolt") timing and amplitude.
5. Which script built-ins switch zones (option 214) and push the free view; see [scripts.md](scripts.md).
6. All [R] behaviour needs checking against the running game.
