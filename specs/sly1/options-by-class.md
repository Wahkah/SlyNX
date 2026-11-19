---
id: sly1-options-by-class
title: Sly 1 options by class
side: reference
status: draft
sources:
  - kind: disc
    what: SCUS-97198 (USA) disc image sha256 b7d46b478efd1455f2142c00a0c81c3cf900bf46a72d0808e6d8bc477d25f49a, all 46 worlds
    used_for: which option IDs each class uses on the disc, how often, and typical values (section 3)
  - kind: reference-project
    what: ProjectCane @ f77825d6 (2026-10-22), Sly1-Release, files spaprops, brx, lo, spliceobj and the per-class files
    used_for: the meaning of each option listed in section 2 and the default values; [R]
date: 2026-11-13
---

# Options by class

The option lists of the object stream (object-stream.md 4) configure every object. option-encodings.md gives each
option's size; this file says which options each class uses and what the commonly needed ones mean. Script option
IDs differ from these stream option IDs (scripts.md 8).

Tags: [D] disc data, [R] reference behaviour to confirm, [I] inferred.

## 1. How options apply

- Options in an object's list are applied in order, when the object is loaded, after its default values. Applying an
  option has the same effect as a script setting it (scripts.md 8). Later options of the same ID overwrite earlier
  ones; "add" options (marked *add* below) append to a list instead. [R]
- **Defaults.** Every class starts with default values for its options before its list is read. The defaults live in
  the game's start-up code, not in the data; the ones needed so far are given in section 2. [R]
- **Action options** (code `0` to `6`) run an action at load: for example option 2 takes the object out of the world
  (scripts.md 10). [R]
- An option is only valid for the classes that define it; the disc uses each option only on such classes
  (section 3). [D]

### 1.1 Options no option list stores

Scripts reach some stream options that never appear in an option list on the disc [D]. Their encodings and meanings
[R] (codes as in option-encodings.md):

| Option | Class (and derived) | Code | Meaning | Live value |
|---:|---|---|---|---|
| 28 | 2 placed objects | `b` | frozen (read only) | yes |
| 29 | 2 | `v` | local position; writing moves the object there | yes |
| 30 | 2 | `v` | original (loaded) position | stored |
| 31 | 2 | `v` | world position (read only) | yes |
| 32 | 2 | `m` (matrix) | local rotation; writing rotates the object | yes |
| 33 | 2 | `m` | original (loaded) rotation | stored |
| 34 | 2 | `m` | world matrix (read only) | yes |
| 35 | 2 | `v` | Euler angles of the local rotation (read and write) | yes |
| 36 | 2 | `v` | velocity in local axes (read and write) | yes |
| 37 | 2 | `v` | angular velocity (writing sets it) | yes |
| 38, 39 | 2 | `v` | linear and angular acceleration (read only) | yes |
| 127 | 3 solid objects | `f` | mass | stored |
| 205 | 31 water | object reference, *add* | adds an object to the water's list of objects that zap the player | — |
| 844, 848 | 10 guards | object reference | the guard's "give up" volume and "detect" volume | stored |
| 849, 850 | 10 | `b` | detection latched by another object; give-up forced by another object | stored |
| 880 | 99 | object reference | the target object | stored |
| 981 | 85 dropped pickups | enumeration | the pickup's state: 0 normal, 1 being collected, 2 removed, 3 falling, 4 swirling, 5 stuck, 6 lost, 7 attracted | yes |
| 1084 | 25 vehicles | object reference | the curve or shape the vehicle follows | stored |

"Live value" means the game computes it from the object's state when read; "stored" options read back what was set.

### 1.2 "Add" options of buttons and trigger volumes

Each adds to a list instead of replacing [R]:

| Option | Class | Value | Adds |
|---:|---|---|---|
| 251 / 267 | 40 buttons / 41, 42 volumes | animation OID | an "on" animation (run when pressed); up to 16 |
| 252 / 268 | 40 / 41, 42 | animation OID | an "off" animation (run when released); up to 16 |
| 253 / 275 | 40 / 41, 42 | object OID | an object allowed to press it |
| 254 | 40 | object OID | an object that must not press it |
| 255 / 256 | 40 | class ID | a class allowed / not allowed to press it |
| 264 / 276 | 40 / 41, 42 | action, 4 arguments | a state-machine goal record: (on-press flag, root object OID, machine OID, goal state OID); up to 7 records; a second call with the same root and machine replaces that record's press or release goal |

Other *add* options: 212 (breakables: an object not to break with it), 289 and 290 (alarms: a breakable and a
sensor to watch). [R]

## 2. Meanings of the options needed by the phase-2 specs

Names are ours. Values are in game units, seconds, radians unless noted. "Default" values are [R].

### 2.1 Common to placed objects (and everything built on the placed layout)

| ID | Meaning |
|---:|---|
| 2 | action: remove the object from the world |
| 21, 22 | initial velocity, initial angular velocity |
| 23, 24 | an animation segment (OID) to play on this object, and the time to start it at |
| 41, 42 | quick blob-shadow radius and depth |
| 43 to 48 | cast a shadow (on/off), shadow shader (OID), shadow near radius, far radius, near and far cast distance |
| 52, 53 | never freeze; freeze mode (when far away or out of view) |
| 54 to 69 | restoring springs and smoothing (physics.md 4) |
| 73, 74 | default drive mode for animated position, rotation |
| 76 to 81 | look at a target: target mode, ignore distance, pan function (three coefficients) and limits, tilt function and limits |
| 91 to 93 | as an attack target: which attacks hit it, target radius, hit test on/off |
| 95 | outline (cel) colour (packed colour) |
| 97 to 100 | pulsing highlight: kind, in-colour, out-colour, pulse time |
| 101 to 106, 109 | an ambient sound on the object: sound ID, start distance, full-volume distance, volume, pitch, repeat interval range, Doppler factor |
| 110 to 115 | how it reacts to the cane, cane sweep, rush, smash, bombs and electric shocks |
| 119 | runs on the real clock |
| 123 | fades when it blocks the camera |
| 125 | hidden |

### 2.2 Solid objects

| ID | Meaning |
|---:|---|
| 127 | mass |
| 128 | material (collision.md 6) |
| 129 | own gravity vector (default: the world's, physics.md 2) |
| 130 | does not interact (no collisions with other solids) |
| 131, 132 | constraint on its force, torque (lock axes) |
| 133 | collide as a sphere of this radius |
| 134, 135 | buoyancy (default 0.5), viscosity (default 1.0) |
| 136 | ignore locked objects |
| 138 | no gravity |
| 139 to 141 | contact filtering: none at all, none with itself, none at its centre |
| 145 | camera collision style (the static world mesh sets it in every world) [D] |
| 148 | collide at its centre |

### 2.3 Animation segments and state machines

See animation.md 5.1 (options 178 to 189) and scripts.md 9 (513, 516, 519).

### 2.4 World (the world option list)

See object-stream.md 4.4 (524 to 551). Option 537 is a time limit for a timed goal (default 15 s); 538 the
commentary timing (default 180). [R]

### 2.5 Camera zones (class 99)

See camera.md 3 (851 to 871).

### 2.6 Others needed soon

| ID | Classes | Meaning |
|---:|---|---|
| 203, 210, 211 | water | current velocity; wake sound on/off; splash droplets on/off |
| 212 to 247 | breakables, fragile breakables, pieces | what breaks with it (*add*), what remains, break speed thresholds (default 300) and spin (default 3), hits to break, coins dropped and their offset, the break sound, the break explosion, piece fade time (default 1.25) and lifetime (default 10) |
| 251 to 279 | buttons, trigger volumes | animations to play when pushed and released (*add*), what may push it (*add* objects or classes), re-push delay (default 1 s), button type, silent, replay animations, saved with checkpoints, must exit before re-trigger, send change events, a dialogue to play |
| 281 | actors | playable |
| 289, 290, 299, 300 | alarms, sensors | breakables and sensors linked to an alarm (*add*); objects that do or do not trigger a sensor (*add*) |
| 366, 421 | emitters, explosion emitters | particle origin kind (4 = mesh origin, object-stream.md 4.5) |
| 500 to 504 | lights | dynamic on/off, exclude moving objects, falloff distance range, cone angle and hot-spot angle (degrees) |
| 701 to 708 | guards | flashlight object, detection flags and switches, state override, barriers it respects (*add*) |
| 709, 710, 711 | actors | height of the base above the feet (default 35), spin ratio, gravity ratio (physics.md 2) |
| 712 to 720 | bombs | detonation radius (against objects and against the player), detonation and damage kinds, touch damage, mass limits, explosion scale |

### 2.7 Values of options cited by other specs

From the disc values [D] and the reference's use of them [R]:

| ID | Type and values | Meaning |
|---:|---|---|
| 41 | `f` (20 to 200 on the disc) | radius of the quick blob shadow under the object |
| 42 | `f` (250 to 1000) | how far below the object the blob shadow is projected |
| 73, 74, 55, 61, 76 | `h`: 0 none, 1 spring, 2 velocity, 3 smooth, 4 along a curve, 5 driven by physics, 6 smooth with force, 7 smooth and locked, 8 spring and locked, 9 smooth without lock (values 1 and 3 occur on the disc) | how an animated or restored position (73, 55) or rotation (74, 61, 76) is driven toward its goal; option 76 turns the object to look at a target, with 9 used where 3 is given |
| 77 | `f` (1500 to 50000) | beyond this distance the look-at target is ignored |
| 78, 80 | `v` three coefficients | pan and tilt angle as a quadratic of the angle to the target (identity is (0, 1, 0)) |
| 79, 81 | `p` (min, max) radians | pan and tilt limits (for example (−1.5, 1.5)) |
| 91 | `h` bit mask: 2 cane sweep, 4 smash, 8 rush, 16 bomb, 32 electric shock; −1 leaves the mask unchanged | which attacks can hit this object as a target |
| 110 | `h` bit mask, applied to sweep, rush and smash together | how the object reacts to the cane; 111, 112, 113 set the same for sweep, rush, smash separately, 114 for bombs, 115 for shocks. Bits (tentative [R]): 2 the hit rebounds off it (the player recoils), 4 it absorbs the hit with no effect, 8 it is pushed by the hit, 16 it takes the hit (breaks, is collected, is damaged), 32 special reaction (for example a stun) |
| 130 | `b` | 1: does not take part in collisions with other solids (1,864 records, all set to 1) |
| 131, 132 | `h`: 0 free, 1 locked, 2 / 3 / 4 move (or turn) only along its local x / y / z axis, 5 / 6 / 7 move (or turn) only in the plane across its local z / y / x axis | constraint on the object's translation (131) and rotation (132) |
| 709 | `f` (0 to 80; default 35) | height of the actor's base point above its feet |
| 710 | `f` (0.5 to 2) | spin ratio: scales how fast the actor turns |
| 711 | `f` (1.3 to 8) | gravity ratio (physics.md 2) |

The remaining options are left to the system specs that use them (guards.md, interactables.md, effects.md and the
others of gameplay-overview.md 6). Section 3 tells each spec which IDs its classes use.

## 3. Which options each class uses on the disc

Counts are records carrying the option over all 46 worlds (an option can repeat within one record). 108 classes
carry options; 1,334 class/option pairs occur. [D]

| Class | Options used on the disc: ID (records) |
|---|---|
| 2 placed object | 2 (1623), 19 (2), 22 (318), 41 (309), 42 (309), 43 (3), 44 (3), 45 (3), 46 (3), 47 (3), 48 (3), 52 (49), 53 (4), 55 (292), 61 (300), 63 (6), 65 (6), 67 (217), 69 (211), 76 (28), 77 (28), 78 (22), 80 (23), 81 (2), 88 (1), 91 (1), 92 (1), 93 (1), 95 (34), 97 (1), 98 (24), 99 (24), 100 (22), 101 (186), 102 (180), 103 (180), 104 (16), 106 (2), 119 (1105), 125 (1) |
| 3 solid object | 2 (75), 16 (2), 19 (5), 22 (51), 23 (32), 24 (32), 41 (20), 42 (20), 52 (2), 53 (1), 54 (118), 56 (108), 57 (3), 58 (111), 60 (121), 63 (113), 65 (109), 66 (2), 76 (4), 77 (4), 78 (2), 79 (2), 80 (3), 81 (4), 95 (1), 101 (19), 102 (18), 103 (18), 110 (17), 114 (2), 125 (1), 128 (1), 129 (5), 130 (877), 131 (179), 132 (141), 133 (29), 134 (1), 135 (1), 136 (218), 138 (110), 140 (9), 141 (19), 145 (137), 146 (38), 148 (25) |
| 4 static world mesh | 130 (46), 145 (46) |
| 7 player character | 43 (41), 44 (41), 45 (41), 46 (41), 47 (41), 48 (41), 123 (41), 132 (41) |
| 9 guard, smart variant | 41 (18), 42 (18), 91 (18), 110 (18), 281 (18), 653 (18), 654 (6), 656 (192), 659 (3), 681 (18), 684 (18), 685 (18), 696 (6), 701 (18), 703 (12), 707 (9), 710 (9) |
| 10 guard variant ? | 16 (3), 17 (2), 18 (2), 20 (2), 41 (41), 42 (41), 58 (6), 91 (50), 92 (2), 93 (2), 101 (6), 102 (3), 103 (3), 110 (60), 114 (1), 129 (2), 130 (9), 133 (3), 136 (3), 143 (3), 653 (29), 654 (11), 656 (367), 660 (6), 661 (57), 664 (6), 666 (2), 673 (6), 674 (6), 675 (3), 678 (5), 681 (42), 683 (3), 684 (38), 685 (42), 689 (2), 693 (2), 709 (5), 710 (14), 711 (4), 841 (27), 842 (5), 845 (52) |
| 11 guard variant ? | 91 (1), 110 (1), 114 (1), 115 (1), 134 (1), 135 (1), 655 (1), 656 (9), 661 (2), 665 (1), 668 (1), 678 (1), 681 (1), 684 (1), 685 (1), 709 (1), 711 (1) |
| 12 guard variant ? | 41 (1), 42 (1), 91 (1), 110 (1), 654 (1), 656 (7), 665 (1), 668 (1), 682 (1), 685 (1), 691 (1), 709 (1) |
| 13 guard variant ? | 41 (1), 42 (1), 91 (1), 110 (1), 653 (1), 654 (1), 655 (1), 656 (7), 661 (4), 665 (1), 685 (1), 687 (1), 709 (1), 710 (1), 711 (1) |
| 14 companion character | 41 (4), 42 (4), 110 (4), 114 (4), 132 (4), 656 (36), 676 (4), 678 (4), 690 (4), 709 (4), 710 (4), 711 (4) |
| 15 guard variant ? | 111 (1), 113 (1), 134 (1), 135 (1), 654 (1), 656 (4), 662 (1), 668 (1), 678 (1), 681 (1), 685 (1), 710 (1) |
| 17 guard variant ? | 110 (1), 114 (1), 129 (1), 281 (1), 656 (9), 709 (1), 710 (1) |
| 18 guard variant ? | 110 (1), 129 (1), 281 (1), 656 (8), 681 (1), 709 (1), 710 (1) |
| 19 boss ? | 41 (1), 42 (1), 52 (1), 91 (2), 92 (1), 110 (1), 130 (1), 281 (1), 653 (1), 654 (1), 656 (6), 661 (1), 665 (1), 678 (1), 681 (1), 682 (1), 684 (1), 685 (1), 710 (1) |
| 20 tank | 41 (4), 42 (4), 709 (4), 710 (4) |
| 21 vehicle ? | 41 (1), 42 (1), 133 (1), 804 (1) |
| 22 turret | 130 (3), 1041 (3) |
| 23 vault | 110 (20), 130 (20), 145 (20), 1046 (20), 1047 (120) |
| 24 puffer ? | 101 (1), 102 (1), 103 (1), 129 (1), 134 (1), 135 (1) |
| 25 vehicle | 101 (2), 102 (2), 103 (2), 129 (7), 281 (8), 1080 (8), 1081 (8) |
| 26 vehicle variant ? | 129 (1), 281 (1), 1080 (1), 1081 (1) |
| 27 jet pack | 41 (1), 42 (1), 129 (1) |
| 28 rhythm game object ? | 130 (1) |
| 29 minigame vehicle ? | 131 (1), 132 (1), 133 (1) |
| 31 water | 130 (11), 138 (1), 145 (29), 148 (4), 203 (1), 210 (2), 211 (2) |
| 33 breakable | 14 (2), 15 (5), 16 (2), 18 (6), 20 (3), 22 (1), 41 (14), 42 (14), 54 (2), 56 (2), 58 (2), 60 (1), 63 (1), 65 (1), 76 (1), 77 (1), 80 (1), 81 (1), 91 (64), 92 (65), 95 (15), 101 (1), 102 (1), 103 (1), 110 (98), 111 (6), 112 (6), 113 (6), 114 (48), 115 (43), 130 (311), 131 (3), 132 (9), 133 (7), 136 (8), 138 (3), 143 (2), 145 (37), 146 (30), 212 (207), 213 (191), 214 (70), 215 (6), 216 (1), 217 (41), 219 (1), 220 (48), 221 (45), 222 (2), 225 (57), 226 (16), 227 (61), 230 (70), 232 (22), 233 (148), 234 (29), 236 (182), 237 (2), 238 (2), 239 (2), 240 (19), 241 (9) |
| 34 breakable alarm ? | 212 (68), 214 (17), 221 (17), 230 (17), 231 (17) |
| 35 breakable gun ? | 41 (1), 42 (1), 130 (2), 133 (2), 212 (6), 213 (6), 221 (2), 230 (2), 236 (2), 816 (2) |
| 36 breakable variant ? | 41 (2), 42 (2), 91 (2), 129 (2), 130 (2), 133 (2), 143 (2), 212 (4), 226 (2), 233 (2), 236 (2), 237 (2), 238 (2), 240 (2), 1109 (2), 1110 (2), 1112 (2) |
| 37 fragile breakable | 21 (1), 41 (1), 42 (1), 60 (1), 63 (1), 65 (1), 101 (2), 110 (2), 128 (1), 129 (1), 130 (14), 131 (1), 132 (1), 133 (3), 136 (2), 212 (6), 213 (11), 214 (1), 220 (10), 221 (9), 222 (1), 223 (2), 225 (3), 226 (5), 230 (3), 232 (5), 233 (8), 236 (16), 237 (1), 238 (1), 239 (1), 242 (8), 243 (8), 244 (8) |
| 38 electrified breakable ? | 91 (2), 92 (1), 101 (2), 102 (2), 103 (2), 130 (2), 133 (2), 212 (4), 223 (2), 224 (1), 225 (1), 227 (1), 236 (2), 237 (1), 238 (1), 246 (2) |
| 39 breakable piece | 22 (12), 41 (7), 42 (7), 60 (5), 63 (5), 65 (5), 76 (2), 77 (2), 79 (2), 81 (2), 129 (7), 130 (19), 132 (5), 133 (135), 136 (3), 145 (3), 247 (21), 248 (306), 250 (104) |
| 40 button | 2 (1), 14 (7), 19 (7), 41 (7), 42 (7), 54 (1), 57 (1), 58 (1), 60 (1), 63 (1), 65 (1), 88 (3), 91 (11), 92 (9), 93 (9), 110 (19), 111 (7), 113 (9), 130 (66), 131 (2), 132 (2), 133 (10), 136 (7), 138 (3), 141 (1), 145 (9), 146 (16), 148 (1), 251 (18), 252 (3), 253 (22), 255 (11), 257 (16), 258 (72), 260 (52), 261 (3), 263 (21), 264 (19) |
| 41 trigger volume button | 2 (24), 125 (26), 130 (74), 133 (2), 145 (1), 267 (16), 269 (14), 271 (25), 272 (4), 273 (7), 274 (6), 275 (155), 276 (44), 277 (7), 279 (51) |
| 42 trigger volume variant ? | 2 (10), 894 (87), 896 (67) |
| 43 physics bone ? | 41 (3), 42 (3) |
| 46 clue | 41 (19), 42 (19), 91 (28), 110 (28), 115 (19), 130 (9), 132 (19), 133 (28), 143 (19), 288 (28), 623 (19) |
| 47 alarm | 131 (17), 132 (17), 289 (17) |
| 49 laser sensor | 130 (11), 299 (11), 304 (11), 305 (11) |
| 50 camera sensor ? | 73 (38), 74 (38), 76 (38), 77 (38), 78 (38), 80 (38), 81 (38), 130 (38), 302 (38), 305 (38) |
| 51 pressure sensor ? | 130 (22), 145 (3), 299 (22), 306 (8), 321 (22), 322 (22), 323 (22), 324 (22), 325 (22), 326 (22), 327 (22), 328 (22), 329 (22), 330 (22), 331 (22), 332 (22), 333 (22), 334 (22), 335 (22), 336 (22) |
| 52 barrier | 2 (2), 19 (9), 60 (1), 63 (1), 65 (1), 110 (1), 125 (12), 130 (25), 131 (1), 132 (1), 133 (1), 136 (1), 145 (68), 146 (4), 344 (156), 345 (51), 346 (13) |
| 54 hazard zone ? | 2 (4), 88 (1), 101 (2), 102 (2), 103 (2), 104 (1), 130 (66), 136 (3), 143 (3), 145 (12), 148 (31), 609 (34), 610 (1), 614 (3) |
| 55 hazard volume ? | 2 (8), 14 (1), 101 (4), 102 (4), 103 (4), 104 (1), 105 (1), 125 (29), 130 (84), 133 (45), 136 (1), 140 (3), 143 (5), 609 (159), 614 (5), 615 (8) |
| 56 checkpoint | 91 (22), 92 (22), 93 (22), 101 (22), 102 (22), 103 (22), 130 (22) |
| 57 conveyor | 130 (51), 136 (5), 146 (36), 148 (1), 625 (51) |
| 58 hiding place ? | 110 (2), 640 (2) |
| 59 bomb | 2 (21), 22 (20), 41 (3), 42 (3), 60 (3), 63 (3), 65 (3), 91 (4), 92 (3), 93 (3), 101 (3), 102 (3), 103 (3), 110 (23), 128 (3), 129 (23), 130 (1), 131 (20), 132 (20), 133 (29), 712 (3), 713 (22), 714 (3), 716 (3), 726 (4), 727 (4), 728 (4), 730 (3) |
| 60 missile | 41 (4), 42 (4), 91 (2), 92 (2), 93 (2), 101 (9), 102 (3), 103 (3), 104 (3), 105 (3), 109 (3), 110 (2), 128 (1), 129 (4), 133 (13), 138 (1), 713 (2), 715 (1), 716 (8), 720 (2), 723 (3), 725 (6), 726 (7), 727 (6), 728 (6), 729 (2), 823 (1), 826 (1) |
| 61 missile variant ? | 101 (8), 102 (1), 103 (1), 131 (1), 132 (1), 133 (13), 712 (2), 713 (4), 715 (8), 716 (17), 718 (1), 719 (1), 720 (2), 723 (11), 725 (7), 726 (8), 727 (2), 728 (1), 731 (7), 823 (11), 826 (1), 828 (1), 829 (11), 830 (2) |
| 62 homing missile ? | 41 (9), 42 (9), 101 (12), 102 (6), 103 (6), 104 (2), 105 (1), 109 (2), 129 (3), 130 (3), 133 (23), 136 (1), 138 (3), 141 (1), 712 (7), 713 (11), 714 (1), 715 (1), 716 (9), 720 (1), 726 (15), 727 (7), 728 (7), 823 (7), 826 (1), 828 (3), 832 (2), 833 (1), 834 (18) |
| 63 mine | 41 (41), 42 (41), 131 (41), 132 (41), 143 (41), 714 (41) |
| 64 fly ? | 110 (2), 133 (2) |
| 65 rat | 63 (4), 65 (4), 110 (4), 132 (4), 133 (4), 752 (4), 753 (4), 754 (4), 755 (4), 756 (4) |
| 66 minigame actor ? | 41 (1), 42 (1), 114 (1) |
| 67 minigame actor ? | 114 (1), 806 (1), 809 (1) |
| 68 minigame actor ? | 130 (2) |
| 71 jump object ? | 129 (6) |
| 72 puffer target ? | 130 (1) |
| 73 solid variant ? | 1055 (4) |
| 74 final boss ? | 69 (1), 114 (1), 130 (1) |
| 75 final boss part ? | 110 (1), 130 (1), 1148 (1) |
| 76 decoy | 63 (41), 110 (41), 114 (41), 128 (41) |
| 78 camera placement | 119 (1), 602 (147), 603 (99), 604 (99), 605 (99) |
| 79 skeleton bone | 2 (4), 41 (24), 42 (24), 52 (12), 55 (406), 56 (9), 58 (9), 61 (475), 63 (6), 65 (6), 67 (248), 69 (570), 73 (3), 88 (12), 125 (32) |
| 80 particle emitter | 2 (242), 101 (32), 102 (32), 103 (32), 104 (1), 349 (2), 350 (6), 351 (1314), 352 (9), 353 (10), 354 (10), 356 (2), 357 (19), 358 (25), 359 (5), 360 (4), 361 (39), 366 (601), 368 (420), 369 (420), 371 (235), 372 (540), 374 (8), 375 (10), 376 (638), 377 (1321), 378 (13), 379 (1291), 380 (1302), 381 (317), 382 (41), 384 (1294), 385 (4), 386 (16), 387 (1284), 388 (18), 389 (383), 390 (1313), 391 (39), 392 (39), 394 (33), 396 (6), 398 (1321), 399 (1280), 401 (10), 403 (854), 404 (14), 405 (12), 406 (12), 407 (158), 408 (101), 409 (37), 410 (57), 411 (14), 412 (212), 413 (2), 417 (14), 418 (14), 419 (14) |
| 81 light | 491 (2254), 492 (2254), 493 (2254), 494 (2254), 495 (2254), 496 (2254), 497 (2254), 500 (2), 501 (765), 502 (2204), 503 (2), 504 (2) |
| 83 proxy (instancer) | 2 (317), 14 (7), 16 (3), 17 (5), 18 (23), 19 (35), 20 (17), 22 (35), 41 (7), 42 (8), 52 (3), 53 (28), 61 (6), 62 (6), 65 (2), 88 (8), 91 (1), 92 (1), 93 (1), 101 (72), 102 (66), 103 (66), 104 (33), 105 (3), 110 (2), 111 (1), 113 (1), 114 (2), 123 (1), 129 (18), 130 (69), 133 (2), 134 (9), 136 (20), 139 (1), 145 (13), 148 (24), 157 (1), 160 (2), 226 (7), 233 (39), 234 (8), 251 (4), 257 (4), 264 (2), 275 (1), 281 (72), 290 (195), 297 (11), 299 (17), 300 (5), 305 (1), 313 (51), 314 (1), 337 (4), 342 (5), 343 (6), 344 (20), 351 (264), 352 (2), 357 (1), 361 (432), 366 (76), 370 (76), 375 (6), 376 (148), 377 (109), 378 (10), 379 (142), 380 (61), 381 (2), 384 (108), 387 (101), 388 (6), 389 (17), 390 (59), 399 (93), 403 (20), 407 (5), 410 (3), 432 (30), 433 (7), 435 (6), 436 (4), 438 (4), 440 (10), 446 (7), 447 (13), 451 (6), 452 (5), 453 (6), 462 (14), 466 (6), 477 (19), 478 (18), 479 (18), 480 (17), 610 (14), 619 (3), 621 (1), 635 (31), 652 (132), 653 (27), 654 (7), 655 (18), 656 (167), 665 (2), 670 (49), 678 (11), 679 (3), 683 (3), 689 (12), 690 (9), 691 (26), 692 (2), 696 (20), 697 (7), 701 (54), 703 (6), 704 (7), 705 (7), 706 (1), 708 (9), 709 (13), 710 (2), 711 (1), 712 (1), 713 (1), 714 (2), 716 (5), 723 (1), 752 (8), 757 (11), 820 (4), 841 (39), 842 (40), 843 (34), 845 (78), 847 (17), 887 (6), 907 (1), 929 (2), 937 (1), 946 (3), 949 (1), 973 (6), 976 (21), 984 (5), 988 (1), 1029 (2), 1030 (8), 1031 (8), 1035 (2), 1040 (1), 1043 (25), 1044 (1), 1045 (2), 1046 (20), 1048 (20), 1049 (20), 1050 (1), 1080 (4), 1081 (4), 1092 (2), 1093 (10), 1097 (1), 1099 (8), 1100 (8), 1101 (8), 1102 (8), 1150 (1) |
| 84 sky | 608 (9) |
| 86 charm | 41 (28), 42 (28), 91 (28) |
| 87 life token | 91 (46), 115 (46) |
| 88 coin | 91 (48), 115 (48), 982 (18), 985 (30) |
| 90 gold prize ? | 91 (22), 115 (22), 982 (22) |
| 91 boost prize ? | 41 (2), 42 (2) |
| 93 lock group | 1000 (44), 1002 (12) |
| 94 tail / chain segment | 55 (49), 61 (50), 101 (4), 102 (4), 103 (4), 763 (209), 764 (209), 765 (205), 766 (165), 768 (13), 769 (7), 770 (6) |
| 95 placed variant ? | 771 (2), 772 (2), 773 (2), 774 (1), 775 (14), 776 (3), 777 (3), 778 (3), 779 (3), 780 (3), 781 (3), 782 (3), 783 (3), 784 (3), 785 (3), 786 (3), 787 (3), 788 (3), 789 (3), 790 (3), 791 (3), 792 (3), 794 (2), 795 (2), 800 (2), 801 (2), 802 (3), 803 (2) |
| 98 waypoint | 838 (12) |
| 99 camera zone ? | 2 (20), 851 (362), 852 (205), 854 (299), 856 (372), 858 (14), 859 (87), 860 (273), 861 (132), 862 (47), 863 (90), 864 (54), 865 (88), 866 (36), 867 (193), 868 (191), 869 (63), 870 (215), 871 (38), 873 (5), 874 (72), 876 (9), 877 (13), 878 (13), 879 (18), 881 (2), 882 (2), 884 (1), 885 (1), 886 (1) |
| 100 jump location ? | 898 (2), 899 (27), 900 (41), 901 (65), 903 (65), 904 (6) |
| 101 dialogue | 52 (806), 967 (28), 969 (1), 971 (764), 972 (23), 976 (1), 978 (4) |
| 102 speaker | 52 (97), 988 (97), 990 (57), 991 (95), 992 (7), 993 (97), 995 (3), 996 (3), 997 (97) |
| 103 world map ? | 119 (48) |
| 104 spawner ? | 1059 (10), 1061 (2), 1063 (2), 1065 (1), 1070 (2), 1071 (2), 1073 (1), 1074 (1), 1076 (2) |
| 111 curve shape variant ? | 2 (1), 633 (18), 634 (3), 635 (23), 636 (11), 638 (7), 639 (2) |
| 112 pipe | 643 (58), 644 (26), 645 (26), 646 (2) |
| 113 rail | 1032 (28), 1033 (2), 1034 (2), 1035 (11), 1036 (1) |
| 116 warp point | 157 (40), 158 (12), 159 (45), 160 (19), 161 (1) |
| 117 target marker | 616 (124), 617 (111), 618 (30) |
| 118 handle marker ? | 337 (1), 338 (3), 339 (11), 340 (11), 341 (16), 342 (1) |
| 121 explosion emitter | 433 (12), 435 (11) |
| 122 explosion emitter, simple ? | 421 (993), 423 (178), 424 (178), 425 (2), 427 (487), 428 (1590), 430 (28), 431 (1), 432 (1152), 433 (2407), 434 (819), 435 (2215), 436 (364), 437 (965), 438 (220), 440 (2396), 441 (870), 442 (1), 443 (63), 444 (4), 445 (308), 446 (2525), 447 (1892), 448 (63), 451 (2407), 452 (355), 453 (2083), 454 (1377), 455 (1210), 457 (1046), 458 (248), 459 (736), 461 (2386), 462 (1525), 464 (14), 466 (106), 467 (19), 468 (46), 470 (267), 471 (249), 472 (15), 473 (18), 474 (18), 476 (44), 477 (386), 478 (370), 479 (370), 480 (293), 483 (16), 484 (16), 485 (68), 486 (64), 487 (53), 488 (2) |
| 123 volume | 14 (10), 15 (5), 19 (15) |
| 125 level exit | 2 (2), 163 (63), 166 (94), 168 (34), 169 (17), 170 (3), 171 (17), 172 (30), 173 (20) |
| 126 point | 2 (4) |
| 127 point variant ? | 626 (16), 627 (1), 631 (2) |
| 128 point variant ? | 837 (7) |
| 129 perch point ? | 1031 (13) |
| 131 animation segment | 178 (5810), 179 (162), 180 (216), 181 (1708), 182 (1023), 183 (26), 184 (8), 189 (601) |
| 135 freeze group ? | 285 (40) |
| 136 state machine | 513 (101) |
| 137 guard group ? | 694 (21) |
| 139 reach map ? | 746 (41), 747 (41), 748 (41), 749 (41), 750 (41) |
| 140 object variant ? | 907 (73), 908 (8), 909 (6), 918 (41), 926 (1), 927 (4), 929 (4), 932 (6), 933 (16), 934 (3), 935 (1), 936 (1), 937 (3), 938 (3), 940 (11) |
| 141 object variant ? | 1003 (135), 1005 (128), 1006 (135), 1007 (135), 1008 (12), 1009 (116), 1010 (118), 1011 (20), 1012 (15) |
| 143 object variant ? | 1118 (17), 1119 (23), 1120 (2), 1121 (18), 1122 (10), 1123 (24), 1124 (64), 1125 (67), 1126 (4), 1127 (4), 1128 (6), 1129 (5), 1130 (19), 1131 (1) |
| 144 rhythm sequence ? | 1161 (109), 1162 (98), 1163 (35), 1164 (32), 1165 (35), 1166 (35) |
| 145 lightning flash ? | 2 (41), 1133 (77), 1134 (77), 1135 (77), 1136 (77), 1137 (77), 1138 (77), 1139 (77) |

## 4. Open questions

1. Meanings of the options not covered in section 2 (assigned to the system specs).
2. Default values for those options (start-up code; only the ones in section 2 are given).
3. Repeated options: 111 option IDs occur more than once within a single record somewhere on the disc (for
   example 446 in 175 records, 656 in 131, 361 in 122, 212 in 60, 1003 to 1010 in about 60 each), besides the
   world's intermittent-sound triple. Action options run once per occurrence and *add* options accumulate; for the
   others the reference keeps the last value. Each system spec must confirm this for its own options. [D for the
   counts]
