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
  - kind: disc
    what: >-
      SCUS-97316, every script module of all 43 worlds searched for stream option 1438 (script ID 1224), read
      2026-11-19
    used_for: the disc uses of the two-character framing (section 2.1)
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/scpt.cpp (command-script camera blocks), cmworld.cpp, spliceglue.cpp
    used_for: which commands and options drive the timed move, the two-character framing and the lens; [R]
  - kind: disc
    what: >-
      SCUS-97316, whole-disc walk 2026-11-19 of options 682, 683 and 684 in every class-114 body (121,153 bodies, 0
      failures); executable option-table entries 682 to 684 at 0x002AC880
    used_for: the camera-block argument layout, counts and value spread (section 2.1.1)
  - kind: disc
    what: >-
      SCUS-97316, whole-disc walk 2026-11-19 of every clip's extra-curve records (all 43 worlds, 151 records) and
      clip 150285 in level row 0; executable SCUS_973.16 (SHA-256 above), floats at the addresses given
    used_for: the focal-length lens (section 2.1) [D]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/asega.cpp (extra-curve linking), cmworld.cpp (camera object parameter), cplcy.cpp (field of view)
    used_for: how a clip's extra curve drives the lens (section 2.1) [R]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 src/missile/InitAccmiss_file.cpp (the camera-object view's activation, 0x12E798); SafeHouse @ c70803ce Sly2-Release/cplcy.cpp (its per-frame update, 0x12E858), cm.cpp (the camera's field-of-view target)
    used_for: what happens to the field of view when a clip stops driving the lens (section 2.1) [R]
  - kind: executable
    what: SCUS_973.16 (SHA-256 above), machine code of 0x0012E858 read 2026-11-19 (loads of the in-use count at +0x12C and the focal length at +0x128, the constants 0x418F…, 0x4040… and 0x3C23…, the call that sets the field of view)
    used_for: confirming the per-frame lens path (section 2.1)
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce, Sly2-Release/cmworld.cpp (camera-object build, post-load push, add/remove push and
      revoke), cm.cpp (view-stack push and revoke), cplcy.cpp (camera-object view activate/deactivate/update),
      activate.cpp (add/remove-from-world hierarchy slots), scpt.cpp (timed view push and the end of a run);
      sly2-decomp @ 33e2bab9 config/eu/c_status.tsv (names of 0x12E648, 0x12E6A0, 0x12E6E8)
    used_for: what ends a camera object's view (section 7.2.1) [R]
  - kind: executable
    what: SCUS_973.16 (SHA-256 above, checked 2026-11-20), machine code of 0x0014E328 to 0x0014E3C4
    used_for: the strict priority comparison that puts an equal-priority push beneath (sections 2.1 and 7.2.1) [D]
  - kind: disc
    what: >-
      SCUS-97316, level row 0 (00_0x05d_hub) walk 2026-11-20 (3,340 bodies, 0 failures): every option list holding
      the bytes of 150483 or 150484, camera 150484's options, sequences 1682, 1645 and 1646, task 1680, and the
      nesting of rig 150483; every script module in row 0 searched for those IDs
    used_for: row 0's camera 150484 case (section 7.2.1) [D]
  - kind: executable
    what: >-
      SCUS_973.16 (SHA-256 above, checked 2026-11-20), machine code of 0x0014DC58 (call at 0x0014DFCC), 0x0014E000,
      0x0014E078, 0x0014E148, 0x002273F0, 0x00227440, 0x00147F40, 0x00148300, 0x001492F0
    used_for: where the visibility mask comes from and how activation applies it (sections 7.1.1 and 7.2.1) [D]
  - kind: disc
    what: >-
      SCUS-97316, level row 0 walk 2026-11-20 (3,340 bodies, 0 failures): the eight class-5 volumes (transform,
      masks, option 175, vertices), rig 150483's transforms, camera 150484, trigger volumes 150620 and 150658
    used_for: row 0's visibility masks under camera 150484 (sections 7.1.1 and 7.2.1) [D]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/cm.cpp, cmworld.cpp, game.cpp, activate.cpp, trigger.cpp, loaders.cpp, lo.h
    used_for: the meaning of the mask fields, the activation kinds and the trigger volume's update (sections 7.1.1 and 7.2.1) [R]
  - kind: disc
    what: >-
      SCUS-97316, level row 0 walk 2026-11-20: raw bytes of camera 150484 (option 1055), sequence 1682 commands 70 to
      85 (raw arguments of 635 and 670), sequences 1689, 1690, 1697, tasks 1680 and 1696, clip 150489's body (duration,
      event lists, option 109), and a byte search of all of row 0 for kind-7 / kind-8 clip events naming each of its
      eight class-44 objects
    used_for: what really ends camera 150484's view (section 7.2.1, open question 7) [D]
  - kind: executable
    what: >-
      SCUS_973.16 (SHA-256 above, checked 2026-11-20): every caller of 0x0014E4C8 (13) and of 0x0014E328 (12) with
      their argument loads; option-table entries 109, 634, 635, 636, 642, 670, 671 at 0x002AC880; machine code of
      0x00108890, 0x001E0ED8, 0x001E01F8 and 0x00130918 (the 0x38 test at 0x00130A90 before the call to 0x00130B18)
    used_for: the view-stack callers, command 670's queued record and the clip's retract at its end (section 7.2.1) [D]
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce, Sly2-Release/scpt.cpp (command 635 kind 0xA, its wait, the end of a run), asega.cpp
      (playback update, retract, event dispatch), asegevent.cpp (event kinds 7 and 8), asega.h (clip speed);
      sly2-decomp @ 33e2bab9 config/eu/c_status.tsv (name of 0x1E0ED8)
    used_for: how clip 150489's retract removes camera 150484 (section 7.2.1) [R]
date: 2026-11-20
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

### 2.1 What drives the timed move, the framing and the lens (added 2026-11-19)

- **Timed move** [R]. Not a script option: it is a camera block in a command script (class 114 sequence data), an
  opening command of kind 0x38 and an end command of kind 0x39. The opening command carries a position, Euler angles
  (three values each), a field of view and a curve index (one of the four of section 2); any of those left zero
  means "the camera's current value". The block has its own duration (0 means the sum of the commands inside it).
  Starting the block pushes the timed-move view at priority 10 from the camera's state to the command's; the end
  command pops it. A command script made only of three or more camera blocks plays as a multi-shot cinematic. Byte
  layout and disc counts: 2.1.1.
- **Two-character framing** [R, D for the calls]. Stream option 1438 (script ID 1224) called as a method with one
  integer: 1 pushes the pair view at priority 6 on Murray and the called object, 0 removes it. Only class 61 (Rajan)
  implements it. The disc calls it 10 times, as five on/off pairs: the `rajan` package in worlds `09_0x04c`,
  `00_0x057_hub` and `07_0x05a`, and the `i_temple_macro` package in `00_0x057_hub` and `07_0x05a` [D].
- **Focal-length lens** (corrected 2026-11-19). Not a command and not a transform channel: a clip drives a camera
  object's (class 44) focal length through one of its **extra-curve records** ([animation.md](animation.md) 2: kind
  byte, target object ID, scalar curve) [D for the layout, R for the use]. While the clip plays, the record's object
  is looked up (among the playing object's nested objects, else the nearest object with that ID); a camera object
  accepts the curve as its focal length whatever the kind byte, and counts the parameter as in use until the playback
  is removed [R]. While that camera object is the view and its focal length is in use, the field of view is
  2·atan(17.996 / f), clamped to 0.01–3.0 rad (`0x418FF79B` = 17.996, `0x3C23D70A` = 0.01) [R].
  **After the clip stops** (2026-11-19): the camera **keeps the clip's last value**; it does **not** return to option
  1054 [R]. The camera object's option 1054 (field of view, its field +0x130) is applied once, when its view becomes
  active, and only if it is not 0 (the decomp's activation routine `0x12E798`) [R]. Each frame the view's update
  (`0x12E858`) sets the field of view from the focal length only while the in-use count (+0x12C) is above 0; when the
  playback is removed the count drops to 0 and nothing writes the field of view any more, so the last value set from
  the lens stays until another view (or a later activation of this one) sets it [R; the load of +0x12C, the focal
  length at +0x128 and the field-of-view call are present in the machine code, D]. The field of view the camera
  shows eases toward that target as for any field-of-view change (section 1) [R].
  Disc [D]: 151 extra-curve records in all 43 worlds; every one has kind byte 0 and names a class-44 object; values
  lie between 15 and 220; the curve is a scalar kind 1 (101 records) or kind 0 (50). Examples: the intro title card,
  clip 150285 (2.0 s, level row 0) on camera 150283, f from 35 to 65 (field of view about 0.95 rad to 0.54 rad);
  clip 150491 on camera 150487, 45 to 70; clip 150276 on camera 150274, 220 to 35 over 7.67 s; clip 123963 on camera
  141488, from 63 to 35 over 9.5 s. Sly's own package has three (clips 4324, 642 and 4351, all on its camera object
  4332: 35 held for 0.3333 s, 15 to 50 over 0.8333 s, 20 to 35 over 1.6667 s); Murray's and Bentley's have none.

#### 2.1.1 Camera-block argument layout and disc counts (added 2026-11-19)

The command kinds 0x38 and 0x39 are not stored as raw commands: the class-114 option list holds stream options whose
builders append those commands in list order (one option per command). Each argument is stored as a u16 type tag
followed by its value; tag 2 = three f32 (12 bytes), tag 1 = one f32 (4 bytes), tag 0 = a 1-byte boolean, tag
`0x106D` = an s32 enumeration (the curve index) [D].

| Option | Command | Args | Tags in order | Bytes | Fields in order |
|---:|---|---:|---|---:|---|
| 682 | 0x38 open block | 5 | 2, 2, 1, 1, `0x106D` | 46 | position (x, y, z); Euler angles (x, y, z) in degrees; field of view (radians); duration (seconds); curve index |
| 683 | 0x39 end block | 5 | 2, 2, 1, 0, 0 | 40 | position; Euler angles (degrees); field of view; "end the timed view here"; "cut" |
| 684 | 0x3A end the runner's view | 1 | 0 | 3 | "cut" |

- Executable option-table entries at `0x002AC880 + 40 × ID` [D]: 682 value type `0x12`, flags `0x401`, routine
  `0x00109110`, 5 arguments; 683 the same with routine `0x001091F0`; 684 routine `0x001092D0`, 1 argument. The
  in-memory 0x39 command holds the position, the angles converted to radians, the field of view and the two booleans
  as words [D for the layout, R for the radians conversion]; the 0x38 builder also converts degrees to radians [R].
- **Counts** (whole-disc walk of all 121,153 bodies, 0 failures) [D]: option 682 in 5,306 class-114 records (1,071
  distinct argument sets), 683 in 5,309 records (1,055 distinct), 684 in 669 records (136 distinct); none outside
  class 114.
- **Values** (over distinct sets) [D]: curve index 0 in 974, 3 in 91, 1 in 4, 2 in 2. Duration 0.0 in 539 (the block
  lasts as long as its contents), otherwise mostly 2.0 (89), 3.0 (65), 1.0 (54), 1.5 (33), 5.0 (20), 0.75 (20), 0.5
  (19). Field of view 1.0 in 423 of the opens. End "end the timed view here" 1 in 867, 0 in 188; end "cut" 1 in
  686, 0 in 369. Option 684's flag 1 in 75, 0 in 61.
- **Example** [D]: world row 0, sequence 1638 opens with position (5424.82, −506.52, 1144.68), angles (0, 11.66,
  15.47)°, field of view 1.33, duration 0, curve 0, and ends with (5434.33, −510.34, 1169.6), (0, 13.21, 7.22)°,
  1.22, 1, 1.
- **Behaviour** [R]: the block moves the camera from state A (the open's values) to state B (the end's values) over
  the duration using the chosen curve; a position or angle triple with every component below 1e-4 in size, or a
  field of view of 0, means "the camera's current value"; B's angles are reached the short way round. The timed view
  is pushed at priority 10. At the end command, "end the timed view here" = 0 leaves the view up for the next block
  (the camera holds B); 1 removes it, cutting when "cut" = 1 and blending back otherwise. A duration of 0 sums the
  durations of the commands between the open and its matching end at the same nesting depth.
- **Interaction with camera objects** (2026-11-20) [D for the comparison, R for the rest]. The timed view and the
  camera-object view share priority 10, and a push goes beneath entries of equal priority. Whichever of the two was
  pushed first therefore stays on top. Section 7.2.1 has the rule and row 0's case.

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

### 7.1.1 Where the visibility mask comes from (added 2026-11-20)

**Rule.** The mask follows the camera's actual eye position, whatever view put the eye there. No view kind is treated
specially: the follow camera, a timed view and a camera-object view all count [D unless marked].

- **When it is computed.** Every rebuild of the camera's view matrices (`0x0014DC58`) ends by calling the volume sort
  `0x0014E078` (call at `0x0014DFCC`). The camera's matrices are rebuilt each frame, whatever view is on top.
- **What position is tested.** The sort reads the camera's own position (camera record +0x90) and nothing else. It
  does not use the player's position or the view's target.
- **Which volumes are tested.** It gathers up to 32 class-5 objects. Each gets a point-in-volume test (`0x002273F0`)
  in the volume's own frame. A change of the inside state (+0x110) also stores the game time at that change (+0x114),
  in `0x00227440`.
- **Which volume wins.** The sort's comparison (`0x0014E000`) orders the volumes in this order:
  1. inside before outside;
  2. higher +0xE4 first (option 175; default 0);
  3. **earlier** state change first;
  4. address.
- **Correction to 7.1.** For two inside volumes of equal option 175, the one the camera entered **first** wins, not
  the most recent one.
- **Only applied when the camera is inside one.** The current volume and mask change only if the first volume is
  inside. In that case `0x0014E148` stores the volume and copies its +0xE0 into the camera record's mask (+0x14).
  - +0xE0 is the second `u32` of the class-5 body [R for the field name].
  - When the camera is in no volume, the **last mask stays**. The `0xFF` default applies only until the camera has
    first entered a volume [R for the default].
- **Two masks per volume.** A class-5 body carries two masks. The first `u32`, after the transform, is the volume's
  own object mask, like every placed object's. The second is the mask it hands to the camera. In row 0, both are equal
  for 150851 (1), 150857 (4) and 150862 (0x20 against own 0x30). The others have their own mask 0xFF or 0x80.
- **Who reads it.** Activation (both the per-frame test `0x00147F40` and the immediate test `0x00148300`) reads the
  camera's mask at +0x14 and position at +0x90. Section 3 of clock-and-loop.md has the rule.
- **Row 0's volumes** (axis-aligned boxes unless noted; world coordinates; z −1222 to 3778 unless noted) [D]:

| Volume | Mask given | Option 175 | Extent x | Extent y |
|---|---|---|---|---|
| 150851 | 0x01 | 2 | −38081 … −11734 | −11303 … 20411 |
| 150856 | 0x02 | 1 | −11895 … −5477 | 13561 … 18216 |
| 150857 | 0x04 | 0 | −9267 … 4059 | 6653 … 21039 (a turned box) |
| 150858 | 0x08 | 1 | −1216 … 11330 | 6673 … 9733 |
| 150859 | 0x10 | 2 | 10685 … 15321 | 4543 … 7974 (a turned box) |
| 150862 | 0x20 | 1 | −9225 … 30750 | −21918 … 4899 (an 18-point prism, z −2300 … 4975, notched near x 8200–13400) |
| 150875 | 0x40 | 0 | 1106 … 12769 | −1672 … 5102 (a turned box) |
| 150878 | 0x80 | 1 | 1354 … 9829 | −8595 … −303 (a 10-point prism) |

## 7.2 Cutscene cameras (class 44)

Camera objects (182 on the disc, header + options layout) are the cutscene and briefing cameras [D] [R]:

- Entering the world pushes the camera-object view at priority 10; the camera sits at the object's transform (driven
  by animation clips, [animation.md](animation.md)) and looks along its matrix or at its target [R].
- Options [D values; R meaning]: 1055 target object OID (36 objects); 1054 field of view (28: 1.0 ×17, 0.6 ×8);
  1059 cut when the view ends (93, all 1); 1056, 1057, 1058 three vectors, always (0, 0, 0), (0, 0, −1), (0, 1, 0)
  on the disc (178 each) [I: an eye offset and two axes]. 171 of the 182 carry option 26 (removed at level start
  until a cutscene places them) [D].
- A clip's extra-curve record may set the object's focal length f; while it is in use the field of view is 2·atan(17.996 / f), clamped to 0.01–3.0 rad (section 2.1) [R].

### 7.2.1 What ends a camera object's view (added 2026-11-20)

**Correction (2026-11-20, later the same day).** This section first said the view of camera 150484 lasts until row
0 is left, that its target is 150482, and that there is no "ends when its clip ends" rule. All three were wrong for
the case that matters. The view-stack rule below still holds: the view lives exactly as long as the object is in the
world. What was missed is that **a clip takes the camera object out of the world**. The corrected row-0 account is
under "What really ends camera 150484's view" below; the bullets it replaces are kept, struck through in substance
by notes.

**Rule** [R, with D where marked]. The camera object has no timer of its own. Its view lives exactly as long as the
object is in the world:

- **Start.** At world load, the camera object's post-load step (`0x12E648`) marks it enabled. If the object is in the
  world at that moment, the step pushes its view at priority 10. Otherwise the object's add-to-world step
  (`0x12E6A0`) pushes the view when the object enters, for example when a sequence's command 670 adds it.
- **End.** The only thing that removes the view is the object's remove-from-world step (`0x12E6E8`). That runs on
  command 671, on a script removing the object, on removal of an ancestor that takes it out of the world, or when the
  world is unloaded. No other code removes the camera-object view: every other removal of a view names a different
  view kind. These three routine addresses are matched by name in the decompilation's status table [R].
  - [D] Checked in the executable: of the 13 calls to the view-remove routine `0x0014E4C8`, only the one at
    `0x0012E71C` (inside `0x12E6E8`) names the camera-object view kind (view argument at camera +0x4A8, priority
    10, match all fields). The others match by view kind (2) or by argument (8) and name other kinds, or match all
    fields of another kind. So ending a sequence, a camera block, an interaction or a zone never removes it.
  - **Clips remove camera objects.** A clip (class 109) event of kind 7 adds an object to the world and kind 8
    removes it (each with an object ID and a fade time; played backward the two swap) [R]. In row 0 **every one of
    the eight camera objects** has a clip whose events add it and remove it (byte search, each hit inside a class-109
    body, all fade 0) [D]. The usual pattern is kind 7 in the clip's apply list and kind 8 in its retract list, so the
    camera object is in the world exactly while that clip plays [D for 150489, I for the others].
  - **When a clip is retracted** [R, D where marked]: when its time passes its duration and none of the loop or
    hold bits (`0x38` of the apply flags) is set (the test at `0x00130A90`, then the retract call to `0x00130B18`)
    [D]; when an event of kind 6 fires; or when the sequence run that started it and waits on it ends (the end of a
    run retracts the playback it holds). Retracting fires the retract list.
- **Covered, not ended.** While the view is in the stack, a view of strictly higher priority covers it and it comes
  back when that view leaves. No view on the disc is pushed above 10 (section 2's table) [R]. [D] Checked: the 12
  calls to the push routine `0x0014E328` pass priorities 0, 1, 6, 7, 7, 8, 9, 10 (four calls) and zone option 200 +
  2 (at most 5), so nothing ever covers a priority-10 view. The equal-priority case is the trap:
  - A push goes **beneath** every entry whose priority is equal or higher. The push routine `0x0014E328` compares
    with a strict "entry priority < new priority" test (`slt` at `0x0014E398`) before inserting [D].
  - So a camera block (section 2.1) that starts **after** a camera object's view is up pushes its timed view at
    priority 10 underneath, and the camera block is not seen.
  - A camera object added **while** a camera block's timed view is up goes underneath that view. It takes over
    only when the block ends with "end the timed view here" = 1.
  - When the view is covered or removed, the camera cuts only if option 1059 is set. Option 1054 (FOV) is applied
    again each time the view becomes the top one (section 2.1) [R].

**Row 0, camera 150484** [D unless marked]:

- **Objects.**
  - Rig 150483 is a top-level class-48 object, and camera 150484 is the only object directly under it.
  - The camera's options are 1055 = **150466** (target; bytes `c2 4b 02 00`), 26 (removed at level start),
    1056 = (0, 0, 0), 1057 = (0, 0, −1) and 1058 = (0, 1, 0). (Corrected 2026-11-20: an earlier version said
    150482.) 150466 is the top-level class-98 object that 1682 drives with command 662 just after the add.
  - It has no 1054 and no 1059: the view keeps the current FOV, and leaving it is a blend, not a cut.
- **Where it is referenced.** Only one option list in row 0 holds the bytes of 150483 or 150484: sequence 1682
  (class 114, body at decompressed offset `0x97CB9`, 86 commands). No script module in row 0 names either ID.
  (Corrected 2026-11-20: "nothing names the rig at all" was wrong.) **Clip 150489** (class 109, top level, body at
  `0x7434D1`) names both: its one channel drives rig 150483, and its event lists name 150484.
- **Before the add**, 1682's last commands are:
  1. a camera block (682 … 683 with end flags 1, 1);
  2. command 636, which starts sub-sequence 1645 and waits for it. Sequence 1645 is one camera block: 697 holds talk
     effect 63 on object 9, 682 opens a shot at (17925.2, 1186.25, 1493.48) with FOV 0.48, 708 and voice line 695
     follow, and 683 ends it with end flags 1, 1 (the timed view is removed, with a cut);
  3. 696 (clear the subtitle);
  4. 697 (talk effect 63 on object 7).
- **The add.** Then **670 adds 150484** (time 0, flag 1). No timed view is up at that moment, so the camera object's
  view goes on top at priority 10 and becomes the active camera [R for the consequence].
- **After the add**, the rest of 1682 is:
  1. 662 on class-98 object 150466 (1, 150, flag 0, object 9);
  2. 634 = 152646;
  3. a 2 s wait (660);
  4. 634 = 17007;
  5. 635 on class-109 object 150489, i.e. play clip 150489 and wait for it (arguments: clip, object −1, no-wait
     flag 0, apply flags 0). **This is what ends the view**; see below;
  6. a 2 s wait;
  7. 647 on class-3 object 150442 (flag 0, object 9);
  8. 636 starting sub-sequence 1646 and waiting for it. Sequence 1646 holds talk effect 63 on objects 9 and 7, then
     plays two camera blocks, at (17745, 1880.48, 1551.31) FOV 0.32 and at (17386.9, 1419.26, 1458.19) FOV 1.32,
     each with a voice line and each ending with flags 1, 1, then 709;
  9. 642, which sets task 1680 (class 133) to state 2. This is the last command.

  Sequence 1682 never removes 150484 or 150483 (no 671 for either).
- ~~What ends the view in row 0~~ (withdrawn 2026-11-20). This said nothing in row 0 takes 150484 out of the
  world, so the view would stay until the world is left and would hide sequence 1646's two shots. Both conclusions
  were wrong: they missed clip 150489.
- **What really ends camera 150484's view** (added 2026-11-20).
  - **The clip** [D]. Clip 150489 has:
    - duration 5.0 s;
    - one channel on rig 150483;
    - an apply list of one event, kind 7 (add) on 150484 with fade 0;
    - a timed list of two kind-0 events (sound layout) at frames 20 and 260;
    - a retract list of one event, kind 8 (remove) on 150484 with fade 0;
    - option 109 = 3.0, the clip's playback speed [R for the meaning].
  - **The sequence** [D for the commands, R for the behaviour].
    1. 670 adds 150484, and its view goes on top.
    2. 662, 634, the 2 s wait and 634 run with that view up.
    3. Command 635 then plays clip 150489 on the rig, waits for it, and does not hold the clip past its end. The
       apply event adds 150484 again, which does nothing since it is already in.
    4. At speed 3 the 5 s clip reaches its end after about **1.67 s**. With apply flags 0 no loop or hold bit is set,
       so the playback is retracted.
    5. Retracting fires kind 8, which removes 150484 from the world. Its remove step (`0x12E6E8`) takes the view off
       the stack. With no option 1059 the change is a blend, not a cut.
    6. 635 completes and 1682 carries on.
  - **Total.** The view is on top for roughly 2 s plus the length of 662's and 634's work, plus 1.67 s. It is gone
    before command 82.
  - **Afterwards** [R for the consequence].
    - 647 and sub-sequence 1646 run with the camera-object view gone. 1646's two camera blocks therefore **do show**,
      each removed with a cut at its end.
    - After them the next view down is the player's follow view, at priority 1 or a zone's 2 to 5.
    - 642 then sets task 1680 to state 2.
  - **The trigger** [D]. Sequence 1690 is not started from any option list in row 0; a script starts it. Task 1689
    (`t0_chase_hallway_start`) owns module 14 of package `jb_intro_macro`, and that module's handler for event 19
    (the task became current) starts 1690 with no starter (script ID 235, the start-sequence method), with no further
    condition ([missions.md](missions.md) 4.1) [D for the module and call, R for event 19's meaning]. 1689 becomes
    available when its prerequisite 1684 is done and current through the ordinary task rules (missions.md 3). 1690
    ends by adding 150498, 150633 and trigger **150620** (670 commands), then
    sets task 1689 to state 2. Sequence 1697 is the one 150620 starts (its option 441).
  - With the camera back on the player, the eye returns to the volumes around the player, so 150620 can wake once
    the player is near it. Task 1696 can be completed [I, consistent with the mask rule of 7.1.1].
- **For an engine.**
  - Do not end a camera object's view on a timer.
  - End it when the object leaves the world. That happens through command 671, a script, the world change, or, most
    often, **a clip's kind-8 event**, usually in the retract list of the clip that animates the camera's rig.
  - Retract a clip at its end when no loop or hold bit is set, and fire its retract list then. A sequence waiting on
    a clip (635 with no-wait 0) resumes only after that.

**Row 0: the visibility mask while camera 150484's view is up** (added 2026-11-20) [D unless marked]:

- **The eye.** Camera 150484's transform refers to transform 1 of rig 150483. At rest, that transform is at
  (17738.4, 1277.4, 1638.3).
  - That point lies only inside volume 150862. The point is well inside the prism's footprint, east of the notch.
  - So the camera's mask is **0x20** while the view is on top [R for the consequence].
  - The rig's clip may move the eye. [I] A move of several thousand units would be needed to leave 150862 or to reach
    another volume, so the mask stays 0x20.
- **It was already 0x20.** The shots just before the add also lie inside 150862:
  - sub-sequence 1645's shot (17925.2, 1186.25, 1493.48);
  - both shots of 1646;
  - class-3 object 150442 at (17499.9, 1774.4, 1298.6).

  The camera object's view keeps the mask that the cutscene shots set.
- **The two volumes that matter.**

| Trigger volume | Position | Own mask | Option 432 (watched object) | Lies inside |
|---|---|---|---|---|
| 150620 | (−7860.9, 15681.5, 2235.0) | 0x7 | 7 | 150856 (mask 2) and, by extent, 150857 (mask 4) |
| 150658 | (−26554.5, 3323.8, 23.8) | 0x1 | 150849 (a class-48 object at (−14328.6, 16001.4, 0)) | 150851 (mask 1) |

  - 150620 also carries option 441 = 1697 (the sequence it starts) and option 26 (removed at level start).
  - 150658 carries option 440.
  - Neither trigger has option 13.
- **Both triggers are dormant.** With the mask at 0x20, neither trigger shares a bit with it, so both are inactive.
  - A trigger volume (class 24) sets activation kind 0 in its init (`0x001492F0` calls the kind setter with 0) [D].
  - It does the inside test for its watched objects only in its own update (`0x00149440`). That update runs only
    while the trigger is active, because an inactive root is out of the update list [R].
  - No path exempts trigger volumes from the mask.
  - Each trigger wakes only once the camera's eye is inside a volume whose mask shares a bit with the trigger's mask:
    150856 or 150857 for 150620, and 150851 for 150658. Those are the volumes around the triggers themselves, so in
    practice the follow camera must be back near them.
- [I] **A conflict with the rule above** (resolved 2026-11-20). If camera 150484's view really stayed on top until
  the world is left, the eye would stay in 150862. Then 150620 could never fire, sequence 1697 would never start,
  and task 1696 could never complete. The resolution: clip 150489's retract removes 150484 about 1.67 s after
  command 635 starts, inside sequence 1682 ("What really ends camera 150484's view" above). The mask is 0x20 only
  while the cutscene's shots are up.
- **For an engine.** Take the visibility mask from the eye position of the view actually on top, every frame, and keep
  the last mask when the eye is in no volume. Do not take it from the player or from the follow camera alone.
- **Note.** This walk reads option 1055 of 150484 as **150466**, the class-98 object that sequence 1682 drives with
  command 662. Rechecked 2026-11-20 from the raw bytes: 150466 is right, and the row-0 bullets above are corrected
  [D].
- **The target does not end the view.** A camera object's target (option 1055) only sets where it looks. Nothing
  removes the view when the target leaves, dies or is hidden: the only removal is the object's own remove step
  (above) [D for the single removal call, R for the target's role]. Options 1056 to 1058 have no part in the
  lifetime either.

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
6. ~~How many timed-move camera blocks the disc holds and which curves they use~~ answered in 2.1.1 (2026-11-19);
   still open: which scripts use curve 1 and 2 and what the third curve looks like in play.
7. ~~Row 0: what ends camera 150484's view~~ answered 2026-11-20 (7.2.1, "What really ends camera 150484's view").
   Clip 150489, played and waited on by 1682's command 635, removes 150484 with a kind-8 retract event after about
   1.67 s, so the view ends inside 1682. It is not ended by the world change, and 1646's shots are not hidden. Still
   open: a capture of the original to confirm the timing. (What starts sequence 1690 is answered: the event-19
   handler of task 1689's module, missions.md 4.1.) The text below is kept as it was asked.
   Original question: what task 1680 reaching state 2 (set at the end of sequence 1682) triggers, and whether that is
   the world change that finally removes camera 150484's view (7.2.1). Also whether the game really hides sequence 1646's two
   shots under that view, as the equal-priority rule predicts. A capture of the original would settle it.
   Added 2026-11-20: under that view the visibility mask is 0x20, which keeps trigger volumes 150620 (mask 0x7, starts
   1697 for task 1696) and 150658 (mask 0x1) inactive. So the view cannot really last until the world change, or
   row 0 could not be finished. Still to find: what ends or covers it after 1682. Candidates are task 1680's
   state-2 handlers, a script or the player's own input; another sequence may also remove 150484 or add a view above
   10 (7.2.1).
7. All [R] behaviour needs checking against the running game.
