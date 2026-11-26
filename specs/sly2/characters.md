---
id: sly2-behaviour-characters
title: Sly 2 playable characters - Sly, Murray, Bentley
side: reference
status: draft
sources:
  - kind: executable
    what: SCUS_973.16 (SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73), option-table entries 47, 362, 364, 365 at 0x2AC880 + 40 × ID and their routines; the knock-back routine at 0x1A16C8 (hit-kind tests at 0x1A170C and 0x1A1714) and its floats at 0x2BFACC to 0x2BFAD4 (2026-11-18)
    used_for: what option 364 stores; contact kind 0x8000
  - kind: disc
    what: all 43 worlds (2026-11-18), every option-364 value on class-120 objects and every clip it names (length, events); the scripts of rows 28 and 37 that read option 255 (script ID 381); Jean Bison's module and the sawmill boss job's module
    used_for: the action-to-clip table, Bison's fight rules
  - kind: reference-project
    what: sly2-decomp src/new_100008 (routines 0x104408, 0x1044B0), src/rwm-steprun (0x20AF38, 0x20AF88), src/step-rwm (animation-reference bind, start and done routines), src/aseg (label search); SafeHouse Sly2-Release damage.cpp, damage.h, pcguard.cpp
    used_for: the animation-reference rule and the 0x8000 reaction, checked against the executable where noted
  - kind: disc
    what: >-
      Sly 2 USA (SCUS-97316) image; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds (string tables walked)
    used_for: >-
      the item table at 0x002BCD00 decoded record by record with each record's name and description read from the
      worlds' string tables (ids 129 to 202); the constants marked [D] read from the executable at the addresses given
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly2-Release/gang.cpp, sly.cpp, damage.cpp, jtthrow.cpp, murraycarry.cpp, carry.cpp
    used_for: states, inputs, timings and reactions of the three characters; marked [R]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), src/mpeg-memcard (the second character's files) and src/bbmark-blend (the third character's files)
    used_for: confirmation of the start-up values and the state machine shape of two characters; facts only
  - kind: disc
    what: >-
      nightclub hub world (level-table row 2): every clip of the three characters, root-motion translation of
      transform 1 measured over each clip; the lookup tables at 0x002BE990, 0x002BF1B0, 0x002BC390, 0x002C0270,
      0x002BC9F0 resolved to clips; transform lists of the three characters; constants at 0x002BF3D4, 0x002BF5C4,
      0x002BF730, 0x002BF738, 0x002BF73C read after the hash check (2026-11-17)
    used_for: sections 1.1 and 1.2 (speeds, gaits, jumps), the move-to-clip list of section 9, test vectors
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly2-Release/actanim.cpp, blend.cpp, jump.cpp, stepphys.cpp, xf.cpp, sly.cpp, gang.cpp, jtthrow.cpp, murraycarry.cpp
    used_for: which clip pair each moving state blends and how, the jump launch rules, the release of a carried or held object; marked [R]
  - kind: disc
    what: SCUS-97316, 2026-11-18 re-walk (enemy collision parts and their switch events; every lookup slot of the three characters resolved to its clip and length in the nightclub hub)
    used_for: melee hit amounts, clip per slot [D]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release (sly.cpp, gang.cpp, climb.cpp, interact.cpp clip slots; alo.cpp, so.cpp contacts)
    used_for: which state or item uses which slot [R]
  - kind: disc
    what: SCUS-97316, 2026-11-19 (nightclub hub world, row 2; Sly, Murray and Bentley object trees and meshes, every conditional mesh's level rows, clips 1518-1523 and 5517-5518; executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73, lookup tables at 0x002BE990, 0x002BF1B0, 0x002BC390, 0x002C0270, 0x002BC9F0 for slots 0x1028, 0x438, 0x440-0x470, 0x47C, 0x1D20, 0x1D24)
    used_for: the idle machines and clips (9.2), the paraglider and per-level meshes (9.3) [D]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release (sly.cpp idle-state entry, look mask and paraglider machine at load; gang.cpp idle states; smrun.cpp; jump.cpp states 19 and 20 and the glide parts)
    used_for: idle start states, run rules and the paraglider open/close rule (9.2, 9.3) [R]
  - kind: disc
    what: SCUS-97316, 2026-11-19 (nightclub hub world, row 2; root channels of clips 281, 282, 152-164, 165-167, 259-261, 1519-1523; lookup slots 0x3E0-0x404, 0x440-0x470, 0x174C-0x17E4; idle machine flag words; executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73, floats at 0x002BF554-0x002BF590, 0x002BF818, 0x002C3CB8)
    used_for: dodge sides, jump-clip directions, crossbow slot pairs, blend constants and the glide slot order (1.1, 1.2, 5.3, 9.1.1, 9.2) [D]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release (jump.cpp directional jump clip and glide weights; sly.cpp dodge side, Stealth Slide weights, look mask; slyhang.cpp swing weights; gang.cpp crossbow swing modes; smrun.cpp transition masks); sly2-decomp @ 33e2bab9 src/new_1942f0 (jump direction thresholds)
    used_for: the rules marked [R] in 1.1, 1.2, 5.3, 9.1.1 and 9.2 (2026-11-19)
  - kind: disc
    what: SCUS-97316, nightclub hub (row 2), Bentley's package: object 580 (class 3, body offset 5636301, 56 bytes) and Bentley's transforms 52 and 75; executable SCUS_973.16 (SHA-256 above), lookup record at 0x002BC390 table (flags 0x300, key 0x244, slot 0x17F4) and the zero vector at 0x002A2DD0
    used_for: the crossbow side part and its rest-pose side (5.3) [D] (2026-11-19)
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 src/bbmark-blend/SetLookRigState.cpp (the side test of routine 0x136F48); SafeHouse @ c70803ce Sly2-Release/gang.cpp
    used_for: how the side is read (5.3) [R] (2026-11-19)
  - kind: disc
    what: >-
      SCUS-97316, 2026-11-20 (nightclub hub world, row 2): objects 316, 317, 318 (Sly), 586 and 685 (all three
      characters), the packages goal_object_jt, goal_object_murray, goal_object_bentley and the placements 1569,
      1570, 1571; executable SCUS_973.16 (SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73)
      lookup tables at 0x002BE990, 0x002BF1B0, 0x002BC390, 0x002C0270 and 0x002BC9F0 (records for 316, 317, 318,
      586, 685, 1569, 1570, 1571)
    used_for: the meshes, parents and lookup flags of 9.3.1 [D]
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce, Sly2-Release (sly.cpp cane parts and the mode-change swap, interact.cpp hiding spot,
      loaders.cpp lookup-table binding, trail.cpp and jtthrow.cpp throw marker, jt.cpp target tracking and ring);
      sly2-decomp @ 33e2bab9 (Sly's hide routine, the cane fade, the mode-change routine)
    used_for: the cane rule, the throw marker and the target ring (9.3.1) [R]
  - kind: executable
    what: >-
      SCUS_973.16 (SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73), 2026-11-20: the swing
      floats at 0x002BF4F0 to 0x002BF59C and the catch radius store at 0x0018849C/0x001884B0 in Sly's set-up
    used_for: every swing and catch constant quoted with an address in 9.1.1 [D]
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce, Sly2-Release slyhang.cpp (the swing's pendulum, substates, hands and release), sly.cpp
      (the action-mode routine, the cane catch test, the kept points), climb.cpp (the pipe grip's mode), jumptgt.cpp
      and jt.cpp (hook targeting), jump.cpp (the reach start and jump kind 2's launch)
    used_for: the swing physics (9.1.1) and the cane modes 2 and 11 (9.3.1) [R] (2026-11-20)
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/damage.cpp (damage states, the end of a reaction), task.cpp (event 0x10) (2026-11-26)
    used_for: death: the reactions, the defeat event, the restart (section 2.1) [R]
reviewed_by: pending
date: 2026-11-26
---

# Sly 2 playable characters

The three characters the player controls: **Sly** (class 53, name string `jt`), **Murray** (class 54, `murray`) and
**Bentley** (class 55, `bentley`) [D] ([object-stream.md](object-stream.md), [classes/solid-object.md](classes/solid-object.md)).
Each is one solid-plus-name record per world [D] ([gameplay-overview.md](gameplay-overview.md) 4). Sly's moves are
given as a delta of the Sly 1 spec [`../sly1/player-moves.md`](../sly1/player-moves.md); Murray and Bentley are new.

Tags: [D] disc data, [O] observable, [R] reference behaviour (to be confirmed), [I] inferred. Buttons are named after
the controller: Cross, Circle, Square, Triangle, R1, R3, and the three gadget buttons L1, L2, R2. "Pressed within
t" means a press seen in the last t seconds and consumed by the action.

## 1. What the three share

- **Body layout and parts.** The solid layout with a trailing name string ([classes/solid-object.md](classes/solid-object.md));
  animations are found through the per-character lookup tables of [animation.md](animation.md) 9 [D].
- **Common state numbers** (each character numbers its own states; the shared meanings are) [R]:
  0 standing, 1 running fast (R1 held for Murray/Bentley, see below), 2 in the air (jump controller), damage
  reaction state (Sly 4, Bentley 4, Murray 14), interaction state (Sly 7, Bentley 3, Murray 13), throw-aim state
  (Sly 23, Bentley 12, Murray 15), scripted-animation state (Sly 18 to 20, Bentley 10, Murray 17).
- **Leaving the floor.** In the ground states, being off the floor for more than **0.1 s** starts a fall [R].
- **Binocucom (recon view).** R3 pressed within 0.15 s opens it, for every character, when the camera is not
  under a fixed policy [R].
- **Interaction.** Circle pressed within 0.2 s uses the object in reach (doors, levers, hiding places) [R].
- **Damage immunity.** No character takes damage while the current player is the safe house (class 70); this is
  how heists start safely [R].
- **Gadgets.** Three gadget slots per character on L1, L2 and R2, saved per character; using one spends gadget
  power from a meter of maximum 100; the cost per use is in the item table (section 6) [R] [D for the costs].

### 1.1 Gaits and ground speeds

Ground movement is driven by **root motion**: a moving state plays a **gait**, a pair of clips (a slow one and a
fast one) found through the character's lookup table, and the character's velocity is the gait's blended root
velocity, the per-second translation of transform 1 ([animation.md](animation.md) 5) [R]. The pair is blended with
weight `1 − u` on the slow clip and `u` on the fast one. The two clips are time-warped to a common cycle at their
footfall markers (event kind 11, markers `0x17D` to `0x180`), and each clip's root velocity counts in proportion to
its own footfall interval, so the speed is the blended distance per cycle over the blended cycle time [R]. When a
clip's footfall interval is proportional to its length (true of the two-step cycles below [I]) this is

`speed(u) = ((1 − u)·d_slow + u·d_fast) / ((1 − u)·T_slow + u·T_fast)`

with `d` a clip's root travel over one play and `T` its length [I]. The result is multiplied by the blend's playback
speed (1 unless something changes it) [R]. The heading turns toward the stick direction.

Clips and root motion measured on the nightclub hub (row 2) [D]; which state uses which pair, and at what weight [R]:

| Character, state | Gait (lookup slots) | Slow clip: object, length, travel, speed | Fast clip | Weight `u` | Speed |
|---|---|---|---|---|---|
| Sly, 1 (run) | `0x1334` / `0x1374` | 125: 1.0 s, 160, 160/s | 126: 0.5 s, 320, 640/s | 1 | 640 × **1.3** (`0x002BF3D4` [D]) = **832** |
| Sly, 12 (move) | `0x1C44` / `0x1C84` | 276: 1.3333 s, 300, 225/s | 277: 0.6667 s, 340, 510/s | stick | **225 to 510** (320 at half stick) |
| Sly, 12 with Shadow Power | same | | | stick × **0.75** (`0x002BF5C4` [D]) | |
| Murray, 1 (run) | `0x1334` / `0x1374` | 416: 0.3333 s, 170, 510/s | 414: 0.5 s, 400, 800/s | 1 | **800** |
| Murray, 10 (walk) | `0x1554` / `0x1594` | 489: 1.3333 s, 300, 225/s | 488: 0.5 s, 255, 510/s | stick | **225 to 510** |
| Murray, carrying (5) | `0x1474` / `0x14B4` | 482: 1.0 s, 125, 125/s | 483: 0.6 s, 310, 516.7/s | stick | **125 to 516.7** |
| Bentley, 1 (move) | `0x1334` / `0x1374` | 416: 1.0 s, 100, 100/s | 414: 0.3333 s, 250, 750/s | 1 | **750** |
| Bentley, 7 (walk) | `0x1614` / `0x1654` | 489: 1.3333 s, 220, 165/s | 488: 0.3333 s, 170, 510/s | stick | **165 to 510** |
| Bentley, 14 (timed) | `0x1878` | 588: 0.1667 s, 250, 1500/s | — | — | **1,500**; without clip 588, the run's fast clip at playback 1.5 [R] |

Murray also has a second fast pair (`0x1644` / `0x1684`: clip 431 at 510/s and clip 430 at 800/s) used by one of
his held states [R]. Sly's sneak (state 10) and fight-stance moves use separate root blends, not gaits [R]. Sly's
combat-dodge clips (`0x2000`, `0x2004`: objects 281 and 282) each move 500 in 0.3333 s (1,500/s) [D]: 281
moves transform 1 from (0, 0, 0) to (0, +500, 0), a side-step to Sly's **left**, and 282 to (0, −500, 0), to his
**right** [D] (the body frame has +X forward and +Z up, so +Y is left [R]). The side [R]: with the stick pushed and a
target, the direction 90° clockwise from the stick is tested; when the target lies on that side the dodge is 281
(left), otherwise 282 (right); so, facing the target, stick left gives 281 and stick right 282 [I]. With the stick
released the last side is repeated. The dodge's start velocity is the clip's root velocity at time 0 turned by the
heading [R]. Leaving the dodge halves the
velocity (`0x002BC9B8` [D]).

The same clip object IDs serve the characters in every world they appear in [I]. The gait sets only the horizontal
velocity; the character's own collision part keeps it on the floor [R].

### 1.2 Jumps

All three jump with Cross from the ground states. The launch sets the vertical speed by **jump kind** [R], under the
characters' gravity of −2,275 ([physics.md](physics.md)) [D for the gravity]:

| Jump kind | Used for [R] | Vertical launch | Peak above the take-off floor [I] |
|---|---|---|---|
| 1 (also 7, 15) | the ordinary jump of all three (Cross on the ground), jumps out of hiding spots and off objects | the speed that brings the character's base point to **215** above the last floor (`0x002BF738` [D]): `v = √(2·2275·(215 − h))`, `h` the base point's height above that floor [R] | 215; from standing, `v` ≈ **989.1** and the peak comes after ≈ 0.435 s [I] |
| 6 | **Sly's double jump** (Cross in the air) | chosen so the peak is **315** above the floor of the first jump (`0x002BF73C` [D]); while still rising `v = √(2·2275·(315 − z))`, `z` the height already gained; while falling, a speed that reaches the same peak [R] | 315 |
| 2 | leaving a climb | 700 | 107.7 |
| 3 | climbing up from a hang (Cross after 0.25 s) | 800 | 140.7 |
| 18 | a special launch | 900 | 178.0 |
| 13, 14, 9 | three gadget or move jumps | target heights 500, 250 and 1,200, computed as for kind 1 | 500, 250, 1,200 |

Murray and Bentley have no double jump [R] [O]. Horizontal velocity carries over from the ground; in the air the
stick steers the horizontal velocity toward **515 × stick** in the stick's direction (`0x002BF730` [D]), times a
per-jump factor that is 1 for the ordinary jump [R].

**Jump clips** (lookup slots `0x3E0` to `0x41C`) [D for the resolution, R for the roles]: two sets of five
directional clips and up to five landings. The **take-off set** (`0x3E0`…`0x3F0`) is played by the launch and the
rising and falling phases of an ordinary jump or a fall; the **second set** (`0x3F4`…`0x404`, called "fall" in
earlier notes) is played by the second jump (animation states 2 and 3: Sly's double jump) [R].

Direction index (2026-11-19) [R, D for the angles]: a launch starts with index 0 (straight, or no stick). During the
first **0.15 s** of the take-off or of the second jump (`0x002BF818`), the first frame on which the stick is pushed
past 0.5 picks a direction once: the angle of the stick relative to the body's actual facing (its object matrix,
not the heading goal), counter-clockwise positive. Below **1.0996 rad** (`0x3F8CBE4C`, 63°) → index 3
(**forward**); above **2.0420 rad** (`0x4002B0B4`, 117°) → index 4 (**backward**); otherwise positive → index 2
(**left**), negative → index 1 (**right**). The clip is swapped for the chosen one at the same playback time; a
missing clip falls back to index 0 of its set, then to take-off index 0. Index `i` is slot base + 4·`i`:

| Index (direction) | 0 (straight) | 1 (right) | 2 (left) | 3 (forward) | 4 (backward) |
|---|---|---|---|---|---|
| Sly take-off (`0x3E0`…) | 152 | 156 | 155 | 153 | 154 |
| Sly second jump (`0x3F4`…) | 160 | 164 | 163 | 161 | 162 |
| Murray take-off | 590 | 594 | 593 | 591 | 592 |
| Murray second set | 595 | — | — | — | — |
| Bentley take-off | 590 | 594 | 593 | — | — |
| Bentley second set | 595 | — | — | — | — |

"—": not a clip in the hub, so index 0 plays (Bentley jumping forward or backward plays 590) [D for the absence, R
for the fallback]. Landings (`0x408`, `0x40C`, `0x414`, `0x418`, `0x41C`): Sly 158, 159, 169, 601, 602; Murray 600 (at
`0x414`); Bentley 478 (`0x408`), 600 (`0x414`) [D].

These clips have no root translation on transform 1: a jump's motion comes from the launch and gravity [D].

## 2. Health and damage (all three)

- **Health counter**: **one per character**, each with a capacity of **40** (`0x28`) units [R]. The save keeps
  them separately as `sly/hpCur`, `bentley/hpCur` and `murray/hpCur`, next to the power counters `…/mpCur`
  ([saving.md](saving.md)) [D]. At the start of a world each character's counter is bound to its own saved value; it
  is refilled to 40 (and power to 100) only after a death or a reset [R]. A hit whose amount is at least the capacity
  is fatal unless it is a touch (kinds 0 and 1) or a heal [R].
- **Hit amounts.** The amount is carried by the hit, not by its kind: the hitting object hands over a negative
  **contact value**, and the counter loses that many units after the scaling below [R]. On the disc: thrown objects
  −5, −10, −20, −60, −100 or −500 (option 395), water −1,000 (option 876) ([interactables.md](interactables.md) 7, 8),
  a laser touch 10 ([alarms-sensors.md](alarms-sensors.md)) [D for the values]. With no difficulty help an ordinary
  −10 hit takes a quarter of the counter [I]. The amounts of guard melee attacks are still open ([guards.md](guards.md)).
- **Scaling** [R] [D]: every change except heals is scaled by `(1 − h) × 1 + h × 0.5`, h the job's difficulty
  help (gameplay-overview.md 6); the two limits 1 and 0.5 are the executable's floats at `0x002BDC20` [D].
  A hit from a guard that has been shrunk (Size Destabilizer, Reduction Bomb) is halved again [R].
- **After a hit** the character flashes for **2 s** and takes no further damage while flashing [R].
- **Hiding.** A hit on a character hiding in an object goes to the hiding object instead (a barrel knocks over) [R].
- **Carried loot** is dropped when hit, if the loot is fragile or the hit empties the counter [R].
- **Reactions by hit kind** (the hit's kind bits) [R], with the velocities set:

  | Kind | Reaction | Velocity [D for the values] |
  |---|---|---|
  | `0x40` (hit by Murray) | knocked aside | (1000, ±900, 950) in Murray's frame, sign by which side of the point 150 behind him the victim is (`0x002BFAE0`, `0x002BFAF0`) |
  | `0x100` (hit by a thrown or fired object) | knocked away | 600 horizontally away from the source plus 800 up (`0x002BFB00`) |
  | `0x200` | knocked back | from the hit's own vector |
  | `0x400` | crushed down | (0, 0, −1000) (`0x002BFB10`); down-and-out if the counter is empty |
  | `0x800`, `0x1000` | stunned (two kinds) | stops dead |
  | `0x2000`, `0x4000` | two recoverable reactions (falls into hazards) | — ; fatal variants when the counter is empty |
  | `0x100000` | heal (medicine) | adds to the counter, plays the healing clip and a noise |

  A launch faster than 400 upward plays the character's cry: sound 0x69 Sly, 0x84 Murray, 0x83 Bentley [R].
  A fatal knock-back faster than 250 against the facing turns the character toward the flight by
  `clamp(−1 + 0.004 × |v_xy|, 0, 1)` [R].
- **Getting up**: after a reaction the character stands up if the counter is above 0, else plays the defeat [R].
- While Sly is in a damage reaction, the hanging parts of his costume fall with gravity (0, 0, −750)
  (`0x002BFA00`) [R] [D for the value].

### 2.1 Melee hit amounts from enemies (2026-11-18 findings)

Enemy strikes reach the counter as the contact value of a collision part that the attack clip switches on
([guards.md](guards.md) 6.3) [R for the mechanism, D for the values]:

| Attacker | Contact kind | Value | Units lost at no help / full help [I] |
|---|---|---:|---|
| any guard (class 58) | `0x200` knock-back | −10 | 10 / 5 |
| swarmer bites (class 57) | `0x200`, `0x400`, `0x800`, `0x1000`, `0x8000` (section 2.2) | −3, −5 or −10 | 3–10 / 1.5–5 |
| Dimitri, Rajan, Contessa, Carmelita | `0x200`, `0x400`, `0x800`, `0x1000` | −10 | 10 / 5 |
| Jean Bison | `0x200` (two parts) / `0x400` | −15, −10 / −20 | up to 20 / 10 |
| the bear (`npc_bear`) | `0x200` | −60 and −20 | 60 / 30 and 20 / 10 |
| Neela | `0x2` | −8000 | fatal [I] |

A hit whose amount reaches the capacity of 40 is fatal (section 2) [R], so the bear's −60 strike defeats a character
outright unless the difficulty help exceeds 2/3 [I]. Per-boss windows: [vehicles-and-minigames.md](vehicles-and-minigames.md) 7.4.

### 2.2 Contact kind `0x8000` (2026-11-18 findings)

**Who sends it** [D]: only the swarmers' pop-up cobra and tentacle (class 57) carry collision parts with kind
`0x8000` (guards.md 6.3). No guard, boss or object part on the disc uses it.

**What it does to the gang** [R, with the constants and tests confirmed in the executable D]:

1. It is neither a touch nor a heal: the counter loses the contact value as for any hit (section 2).
2. **Flight:** the character is knocked back along the hit's own vector, exactly as for `0x200`: the knock-back
   routine at `0x1A16C8` tests `0x200` (at `0x1A170C`) and `0x8000` (at `0x1A1714`) together [D]. When the sender is
   a walking character or enemy, the vector is first scaled by 1 + 0.25 (`0x2BFACC` = 0.25), its length capped at
   1,200 (`0x2BFAD4`) and its upward part raised to at least 700 (`0x2BFAD0`) [D for the values, R for the rule]; the
   same boost applies to `0x200` hits.
3. **Reaction:** the knock-back reaction (the character turns against its velocity and lands); when it ends,
   instead of getting up directly, sound 0x6A plays and a second, held reaction follows, after which the character
   recovers as from a stun [R]. The clip for each phase comes from the character's damage-reaction clips.

Like every hit, it also makes the fixed hit noise (template 712) that guards hear (guards.md 4.1) [R].

### 2.1 Death (added 2026-11-26) [R]

There are **no lives**. A gang character whose counter reaches 0 plays a death reaction, then the game counts a
defeat and restarts. In order:

1. **The hit.** The counter is lowered and clamped at 0. The hit's kind (its filter) chooses the reaction:

| Hit kind (filter) | Counter still above 0 | Counter at 0 |
|---|---|---|
| knock-back `0x100`, `0x200`, `0x40` | thrown, then "get up" (reaction 0) | thrown, then reaction **8**: thrown backward when its velocity against the facing is below −250, else **9** |
| stun `0x800`, `0x1000` | reaction 1 or 2, then recover | reaction 1 or 2, then **8** |
| fall `0x400` (velocity set to (0, 0, −1000)) | reaction 4 | reaction **12** |
| water `0x2000` | reaction 3, then recover | reaction **10**, turned to face the camera |
| `0x4000` | reaction 5, then recover | reaction **11**, turned to face the camera |
| burn `0x8000` | reaction 7, then 6 | reaction 7, then 6, then **8** |

2. **The death reaction.** Reactions 8 to 12 play their clip (the character's reaction table) and the character takes
   no input.
3. **The defeat.** When the clip is retracted, the character's eyes close and **task event 0x10** is broadcast with
   the character.
4. **What 0x10 does:**
   - **difficulty help +0.1** ([missions.md](missions.md) 6);
   - during a job, the job restarts at its checkpoint task, whose world and checkpoint load (missions.md 5);
   - outside a job, the current world reloads at the current arrival point with wipe 778
     ([level-flow.md](level-flow.md) 8).
5. **After the restart** the counter is refilled to 40 and power to 100 (section 2).

A carried loot item is dropped by any hit that would empty the counter, or that hits a fragile item.

## 3. Sly (class 53): delta from Sly 1

Unchanged in kind from [`../sly1/player-moves.md`](../sly1/player-moves.md) 2 to 5 (run, jump, climb, hang, walls)
unless listed [O].

- **Start-up values** [R]: physics scale **1.6** (the executable's `0x3FCCCCCD`), a reach value 200, a timing value 15.
  Four pocket slots for pickpocketed items with identities 26, 37, 38, 41.
- **States** (numbers used by scripts and messages) [R]: 0 stand, 1 run, 2 air, 3 swing/hang on a hook, 4 damage,
  5 near a guard (fight stance), 6 combat dodge, 7 interaction, 8 climb (pipes), 10 sneak near a guard,
  11 **pickpocket**, 12 run (sneaking off), 13 and 14 air attacks (Square, Triangle), 17 forced, 18 to 20 scripted,
  21 climbing surface, 22 hanging and following a moving hook, 23 throw aim, 24 **Stealth Slide**.
- **Ground choices each frame** (states 0, 1, 10, 12), first match wins [R]:
  1. Cross pressed within 0.1 s: jump.
  2. A guard within reach and R1 not held: the fight stance (state 5).
  3. R3: binocucom.
  4. A guard of the "can be robbed" rank and Circle pressed within 0.15 s: **pickpocket** (state 11).
  5. Circle within 0.2 s: the interaction in reach.
  6. Stick: idle when the stick is at 0 (or at most 0.1 from standing); R1 held runs (state 1); near a
     guard the move is the sneak (10), otherwise state 12.
- **Air** [R]: Square or Triangle in the air start the two air attacks (13, 14).
- **Swinging/hanging** (3, 21, 22): Square lets go [R].
- **New moves** are the items of section 6 owned by Sly: Smoke Bomb, Combat Dodge, Stealth Slide, Alarm Clock,
  Paraglide, Silent Obliteration, Thief Reflexes, Feral Pounce (sold), and the clue-bottle rewards Mega Jump, Tornado
  Strike, Knockout Dive, Insanity Strike, Voltage Attack, Long Toss, Rage Bomb, Music Box, Lightning Spin, Shadow
  Power, TOM, Time Rush (not sold) [D for the names and records].
- **Pickpocketing** [R] [O]: from behind a robbable guard, Circle takes one entry of its pocket; the take happens at
  an event of the hand animation; the loot is shown and added to the loot carried (sold at the safe house) or to
  coins (gameplay-overview.md 8).

## 4. Murray (class 54)

### 4.1 Start-up values [R], confirmed in the game's own start-up code (decomp) [R]

| Value | Meaning (ours) |
|---|---|
| 50 | standing collision allowance |
| 1500, 3500 | the two throw/launch distances of his attacks (near, far) |
| 60 | his turning-reach value |
| 125 | body height |
| −100, 100, 50, −10, 250, 0, 1, 4500 | his reach box and grab settings |
| −60 s | two "last time" stamps reset to long ago |
| 4 | his character index among the players |

### 4.2 States [R]

0 stand, 1 run (R1 held), 2 air, 3 **Thunder Flop** (body slam from the air), 4 lifting (start of a carry),
5 carrying, 6 held by a script (Bentley ride), 7 and 8 scripted, 9 **stance near an object or guard ahead**,
10 walk, 11 **punch combo**, 12 **Berserker Charge**, 13 interaction, 14 damage, 15 throw aim, 16, 18, 19
held states, 17 scripted animation, 20 pipe climb, 21 **Turnbuckle Launch** stance.

### 4.3 Ground choices (states 0, 1, 10) [R]

First match wins:

1. R1 not held and an object or guard directly ahead: stance (9).
2. Cross within 0.1 s: jump.
3. R3 within 0.15 s: binocucom.
4. Triangle within 0.15 s (when a punch can chain): punch, Triangle pair.
5. Circle within 0.2 s: the interaction in reach, else **lift** what is ahead (state 4).
6. Square within 0.15 s: punch, Square pair.
7. Stick: stand (no input), run if R1 held, walk otherwise.

**Grabbing a launched guard** [R]: when a guard Murray launched is airborne above him, Circle within 0.15 s picks
it up if it is less than **300** away horizontally and **100 to 500** above; Square within 0.15 s hits it with the
juggling punch (punch 4) if less than **400** away and **0 to 600** above.

### 4.4 Punches [R]

Five punch clips. Square alternates punches 0 and 1, Triangle alternates 2 and 3 (the first of each pair is chosen
by which side the target is on); 4 is the juggling punch. The next punch of a pair can be queued with its button
within **0.25 s**. Moving the stick after the punch's "can steer" event ends the combo. During a punch the clip's
root speed is blended with the stick by `g = 0.5 + 0.5 × stick` (clamped 0..1).

### 4.5 Thunder Flop [R] [D for the constants]

From the air (jump not of kind 0, 11, 15, 16): Square or Triangle within 0.15 s, if he will not land within 0.1 s
(both `0x002C08DC`), starts the flop:

- **Phase 0, wind-up**: gravity suspended, horizontal speed 0.
- **Phase 1, fall**: horizontal velocity kept, vertical velocity set to **−500** (`0x002C09A8`); the stick steers
  horizontally toward `0.75 × 515 × stick` (`0x002C09A0`, `0x002BF730`) with a gain of **2.5** (`0x002C09A4`).
  The 515 is the **air-steering speed** shared by every jump (section 1.2), not a ground speed: Murray's fast run is
  800 and his stick-blended walk 225 to 510 (section 1.1) [D for the clips, R for their use].
- **Phase 2, landing**: stops; the landing clip jumps to its landing event. Raging Inferno Flop adds a ring of fire
  on landing (item table).

### 4.6 Lifting, carrying, throwing [R]

- **Lift** (state 4) of an object or guard ahead, then **carry** (5): walking with the load uses a separate gait;
  Circle or Cross release, the throw aims (state 15).
- **Throw aim** (shared with Sly and Bentley, section 7).
- **Carrying Bentley**: Murray picks Bentley up through the throw controller (the toss, section 7) and aims with
  him (state 15) [R]. **When the toss ends without a throw** while Bentley is still held (Murray takes a hit, which
  ends the aim state, or the throw is cancelled) [R]:
  1. Bentley's carried sub-state changes from "held" to "thrown";
  2. his velocity becomes **400** horizontally along the direction from Murray to Bentley (heights ignored; +X when
     the two are at the same spot) plus **600** up, a world-space direction rather than "backward" in Murray's
     frame;
  3. he flies under gravity with normal collisions; once he is on a floor in the "thrown" sub-state he stands
     (state 0) and **becomes the player-controlled character again** [R].
- **Dropping an object carried in state 5** (Murray hit, or falling, while carrying something he lifted) takes a
  different path: the object is detached with its contacts restored and gets **(−400, 0, 600) in Murray's frame**
  (400 backward, 600 up); in the other carry phases its velocity is zeroed; a held guard goes to its own reaction.
  The object raises script event 6 and task event 27 ("released"), and does not collide with Murray until 0.1 s after
  they stop overlapping [R].
- **Held by a script** (state 6): face buttons keep the held states up; when no button is pressed for more than
  **1 s** (`0x002C0900`) the hold drops one step down a table of states [R] [D for 1 s].

### 4.7 Other moves [R]

- **Berserker Charge** (12), **Turnbuckle Launch** (21: Cross launches him high, releasing the gadget button ends
  it), Fists of Flame, Juggernaut Throw, Atlas Strength (jump while carrying), Diablo Fire Slam (while carrying),
  Guttural Roar, Raging Inferno Flop: all items of section 6 [D for the names].

## 5. Bentley (class 55)

### 5.1 Start-up values [R]

| Value | Meaning (ours) |
|---|---|
| 45 | turning-reach value |
| 125 | body height |
| −70 | the jump controller's gravity setting |
| 2 | his character index |
| 5, 7, 9, 11, 13, 4 | the identities of his six carried-gadget abilities, in his part table order |
| (175, 0, 0) | the point ahead his crossbow swing aims at (`0x002BC920`) [D] |
| −50, 50 | the target heights that blend the high and low swing clips |

### 5.2 States [R]

0 stand, 1 move, 2 air, 3 interaction, 4 damage, 5 **near an object ahead**, 6 **holding** (a summoned or carried
gadget), 7 walk, 8 **carried by Murray**, 9 and 11 held, 10 scripted, 12 throw aim, 13 pipe climb, 14 a timed state
that returns to standing after **2 s**.

### 5.3 Crossbow swing (Square) [R] [D for the values]

Square within 0.1 s swings the crossbow. The target is the nearest attackable object near the point 175 ahead
within **0.15 s**, within an angle of **1.5 rad** of his heading (3.15 rad when standing); standing still he turns
to face it. A repeat press restarts the swing. Size Destabilizer shrinks the guards it hits (item table).

**Swing clips** (2026-11-19) [R for the rule, D for the clips]. The swing has three **modes**, set from Bentley's
state every frame: mode 0 in state 0 (standing); mode 1 in states 1, 2, 3 and 7 (move, air, interaction, walk);
mode 2 in state 5 (near an object ahead); other states end it. When the swing starts, the **side** is fixed: 1 when
his weapon part (slot `0x17F4`) is on his left (+Y in his frame), else 0. The blend played is number
`2 × mode + side` of six two-clip blends, and the target height fixes the weights for the whole swing:
`u = clamp((z + 50) / 100, 0, 1)`, `z` the height of the target point the search returns (relative to Bentley
[I]), 0.5 with no target; the first
clip gets `1 − u` (**low**), the second `u` (**high**). The swing's time runs from its start; a mode change
mid-swing swaps the blend and keeps the time, the side and the weights; each mode change seeks 0.0667 s ahead.

**How the side is read** (2026-11-19). Slot `0x17F4` is filled by the record of Bentley's lookup table at
`0x002BC390` with flags `0x300` and key `0x244`: object **580** in his package [D]. On the disc (nightclub hub,
row 2) object 580 is a class-3 transform marker nested directly in Bentley's body (56-byte body at offset 5,636,301):
parent reference `0x1034`, that is Bentley's transform 52 (kind 13; the target of his mirror pair 75 → 52, which
copies transform 75's rotation), identity rotation, offset (26.712, 88.609, 1.574) [D]. When a swing starts (the
change from "no swing" to a mode, not on later mode changes), the game converts the marker's origin (the zero vector
at `0x002A2DD0` [D]) from the marker's frame into Bentley's own object frame, using both objects' current world
matrices, so the current pose counts; side = 1 when the result's **y** is above 0, else 0 [R; the decomp's
byte-level reading of routine `0x136F48` and the reference agree]. At rest the marker sits at (21.2, 145.5, 91.2) in
Bentley's frame [D], so the rest-pose side is **1** (left; blends 1, 3 and 5: clips 562/563, 566/567, 570/571).

| Blend | Mode, side | Low clip (weight 1 − u), slot | High clip (weight u), slot |
|---|---|---|---|
| 0 | standing, weapon right | 560 (`0x174C`) | 561 (`0x1758`) |
| 1 | standing, weapon left | 562 (`0x1768`) | 563 (`0x1774`) |
| 2 | move / air / interaction / walk, right | 564 (`0x1784`) | 565 (`0x1790`) |
| 3 | move / air / interaction / walk, left | 566 (`0x17A0`) | 567 (`0x17AC`) |
| 4 | near an object ahead, right | 568 (`0x17BC`) | 569 (`0x17C8`) |
| 5 | near an object ahead, left | 570 (`0x17D8`) | 571 (`0x17E4`) |

All twelve clips are 0.5 s [D]. "Low" and "high" follow from the weights (the second clip dominates for targets
above Bentley's origin) [R]; the clips' poses were not compared.

### 5.4 Darts (R1) [R] [D for the aim distances]

- R1 pressed within 0.25 s fires a dart along the line from the screen centre at depth 10 to depth 40000
  (`0x002BC970`, `0x002BC980`): the dart flies to the first solid hit, or the far end.
- A dart expires after **4 s**; after firing, the dart reload state lasts **2 s** before the next dart.
- Scripts can change the dart kind (for example noise darts) through an option on Bentley [R].

### 5.5 Bombs and carried gadgets (Triangle and the gadget buttons) [R]

Each of his six carried abilities summons its gadget object (a bomb or device) from a pool and holds it (state 6:
summon, held, falling). Releasing the button (or Triangle) lets the object go or throws it (aim state 12); landing
again before the button is released returns him to standing. While a thrown gadget of that kind is still flying
or lying out, the button triggers it instead of summoning a new one (Trigger Bomb). Abilities refused when a
world option on Bentley disables them (used in one boss world) [R].

### 5.6 Other moves [R]

Cross jumps (Hover Pack extends jumps), Circle interacts, Adrenaline Burst (fast run), Health Extractor, Snooze Bomb,
Reduction Bomb, Temporal Lock: items of section 6 [D for the names]. He is carried and thrown by Murray (5.2 state
8: when thrown and on a floor again he stands and becomes the player).

## 6. Moves and gadgets: the item table [D]

Executable, virtual address `0x002BCD00`, 43 records of 32 bytes (layout in [gameplay-overview.md](gameplay-overview.md) 7).
Read it from the player's executable after checking its SHA-256. The names are the world strings with the record's
name id. Owners: 7 Sly, 8 Bentley, 9 Murray (confirmed by the names). "Button": 2 press, 1 hold, −1 passive (no
button). Records 0 to 6 are empty.

| # | Owner | Button | Price | Power | Episode | Name (game string) |
|---:|---|---:|---:|---:|---:|---|
| 7 | Bentley | 2 | 200 | 0 | 1 | Trigger Bomb |
| 8 | Bentley | 2 | 500 | 5 | 2 | Size Destabilizer |
| 9 | Bentley | 2 | 550 | 5 | 3 | Snooze Bomb |
| 10 | Bentley | 2 | 1000 | 15 | 4 | Adrenaline Burst |
| 11 | Bentley | 2 | 1050 | 20 | 5 | Health Extractor |
| 12 | Bentley | 2 | 1200 | 1 | 6 | Hover Pack |
| 13 | Bentley | 2 | 1700 | 20 | 7 | Reduction Bomb |
| 14 | Bentley | 2 | 2000 | 25 | 8 | Temporal Lock |
| 15 | Murray | 2 | 400 | 10 | 1 | Fists of Flame |
| 16 | Murray | 2 | 400 | 0 | 2 | Turnbuckle Launch |
| 17 | Murray | −1 | 750 | 0 | 3 | Juggernaut Throw |
| 18 | Murray | −1 | 900 | 0 | 4 | Atlas Strength |
| 19 | Murray | 2 | 2100 | 20 | 8 | Diablo Fire Slam |
| 20 | Murray | 2 | 1400 | 10 | 6 | Berserker Charge |
| 21 | Murray | 2 | 1500 | 15 | 7 | Guttural Roar |
| 22 | Murray | 2 | 1150 | 25 | 5 | Raging Inferno Flop |
| 23 | Sly | 2 | 300 | 30 | 1 | Smoke Bomb |
| 24 | Sly | 2 | 600 | 0 | 2 | Combat Dodge |
| 25 | Sly | 2 | 650 | 1 | 3 | Stealth Slide |
| 26 | Sly | 2 | 600 | 5 | 4 | Alarm Clock |
| 27 | Sly | −1 | 600 | 0 | 5 | Paraglide |
| 28 | Sly | −1 | 1300 | 0 | 6 | Silent Obliteration |
| 29 | Sly | 1 | 1600 | 2 | 7 | Thief Reflexes |
| 30 | Sly | 2 | 1900 | 0 | 8 | Feral Pounce |
| 31 | Sly | 2 | — | 0 | — | Mega Jump |
| 32 | Sly | −1 | — | 0 | — | Tornado Strike |
| 33 | Sly | 2 | — | 10 | — | Knockout Dive |
| 34 | Sly | 2 | — | 10 | — | Insanity Strike |
| 35 | Sly | 2 | — | 10 | — | Voltage Attack |
| 36 | Sly | −1 | — | 0 | — | Long Toss |
| 37 | Sly | 2 | — | 25 | — | Rage Bomb |
| 38 | Sly | 2 | — | 15 | — | Music Box |
| 39 | Sly | 2 | — | 20 | — | Lightning Spin |
| 40 | Sly | 2 | — | 3 | — | Shadow Power |
| 41 | Sly | 2 | — | 5 | — | TOM |
| 42 | Sly | 1 | — | 1 | — | Time Rush |

"—": price 0 and episode −1: not sold; these twelve are unlocked otherwise (clue bottles) [O] [I]. Each record's
description string follows its name (id + 1), for example 129 "Trigger Bomb", 130 "Throwable bomb with remote
detonation" [D].

Effects of owning an item that the reference shows in the throw code [R]: **Long Toss** (record 36) raises the
throw range by 1.5× and the predicted flight time by 1.35× (section 7).

**Long Toss and its owner.** Item 36's owner code is 7 (Sly) [D], yet the only gadget grid that shows it is
Bentley's, as its ninth cell, marked not assignable [D]. The two do not conflict in play: the owner code only decides
which character's shop page lists an item, and Long Toss is never sold (price 0, opening episode −1) [D]; ownership is
one flag per item for the whole gang, and the throw rule checks that flag whoever is throwing [R]. So: Long Toss is
a passive reward that improves every character's throw, shown on Bentley's screen [I for the display choice].

## 7. Throw aiming (shared) [R]

When a character holds a throwable (object, guard, Bentley) and presses the throw button, an aim mode starts:

- the aim point moves with the stick at a rate scaled by the stick; its distance is limited between a minimum and
  a maximum range set by the object (×1.5 with Long Toss);
- the launch elevation is `0.15 + 0.55 × far`, `far` the aim distance's fraction of the range; the flight is
  predicted for a set time (×1.35 with Long Toss) in up to 29 steps of 1/29 of it, stopping at the first hit;
- the aim blend turns at most 2 per second and the body at most 3 per second;
- Square/Cross (`0xC0`) within 0.1 s release the throw (at the throw clip's release event); Circle/Triangle
  (`0x30`) within 0.1 s cancel;
- a thrown object fades out over 0.25 s when it is used up.

## 8. Switching characters [R] [O]

- All three are loaded in every playable world; one is the current player, the others are hidden or follow
  scripts ([gameplay-overview.md](gameplay-overview.md) 5).
- In a hub, the character is chosen at the safe house; a job sets its character per task, and the task's choice is
  saved and restored.
- Moves that move one character with another (Murray carrying Bentley, Bentley riding on Murray) change who is the
  current player when they end (Bentley becomes the player when he lands after a toss).

## 9. Animations

Each character finds its clips through its lookup table ([animation.md](animation.md) 9: Sly `0x002BE990` then
`0x002BF1B0`; Bentley `0x002BC390`; Murray `0x002C0270`; shared `0x002BC9F0`) [D]. The states above use those
entries: gaits for walk/run (one clip blend per gait) [R], five punch clips for Murray, a two-clip blend per crossbow swing mode and weapon side for Bentley (5.3), a four-clip blend for
Sly's hanging and a three-clip blend for his swinging [R].

Clips per move found so far (lookup slot → clip object, nightclub hub) [D for the resolution, R for the move]: the
gaits and jumps of sections 1.1 and 1.2; Sly's combat dodge `0x2000` → 281 and `0x2004` → 282; Murray's five
punches `0x17FC`, `0x1800`, `0x1804`, `0x1808`, `0x180C` → 424, 425, 435, 434, 196 (lengths 0.6667 s for the first
four, 0.9167 s for the last). The other entries are still to be named by move.

### 9.1 Clip per lookup slot (2026-11-18 findings)

Lookup records map a **slot** (a byte offset in the character's clip slots) to a clip object; the slots below are
resolved in the nightclub hub (row 2) [D], and the move each slot serves is from the reference [R]. Lengths in
seconds [D]. "—" means the clip is not present in that world.

**Shared slots (all three):** jumps `0x3E0`–`0x3F0` (five kinds) and falls `0x3F4`–`0x404`; landings `0x408`,
`0x40C`, `0x414` (landing into a hang), `0x418`, `0x41C`; climbing a pipe `0xFA4`/`0xFB4` (blend), hold `0xFC4`, idle
`0xFC8`, grab `0xFCC` [R]. Sly's jumps resolve to clips 152, 156, 155, 153 … (2.83 to 3.17 s) [D].

**Sly:**

| Slot | Clip | Length | Move [R] |
|---|---:|---:|---|
| `0x1334` / `0x1374` | — | — | walk gait (slow / fast) |
| `0x1C44` / `0x1C84` | 125 / 126 | — | run gait (section 1.1) |
| `0x1A7C` | 189 | 0.9167 | state 9: Knockout Dive |
| `0x1A80` | 190 | 0.6667 | state 13: air attack (Square) |
| `0x1A84` | 191 | 2.0 | state 14: air attack (Triangle) |
| `0x1BC8` | 280 | 1.0 | state 11: pickpocket |
| `0x1CE0`–`0x1D04` | 285–294 | 0.8333, then 0.6667 each | stance poses 12 to 21 (set by the game's pose modes; meaning of each not established) |
| `0x1D14` / `0x1D18` | 194 / 195 | 0.8333 / 0.6667 | pose modes 22 / 23 |
| `0x1D08` / `0x1D0C` | 295 / 296 | 0.6667 | fight-stance wind-ups (sub-states 6, 7) |
| `0x1D10` | 297 | 1.0 | charged strike (sub-state 8) |
| `0x1FE8` | 348 | 1.0 | Lightning Spin (sub-state 9) |
| `0x1D1C` / `0x2050` | 196 / 198 | 1.1667 / 1.3333 | state 16: juggle strike without / with Silent Obliteration |
| `0x1D34` | 237 | 1.3333 | state 17: forced move |
| `0x2000` / `0x2004` | 281 / 282 | 0.3333 | Combat Dodge: left / right (1.1) |
| `0x2018` / `0x2024` / `0x2030` | 259 / 260 / 261 | 1.0 | Stealth Slide blend: lean left / straight / lean right (9.1.1) |
| `0x1F64` / `0x1F70` / `0x1F7C` / `0x1F88` | 307–310 | 1.5 | hanging blend (four clips) |
| `0x1974` / `0x1980` / `0x198C` | 165 / 166 / 167 | 0.6667 | swinging blend: pumping ahead / pumping back / coasting (9.1.1) |
| `0x14D4` / `0x14D8` | 150 / — | 0.4167 | pose modes 10 / 11 |

**Murray:**

| Slot | Clip | Length | Move [R] |
|---|---:|---:|---|
| `0x17FC`–`0x180C` | 424, 425, 435, 434, 196 | 0.6667 ×4, 0.9167 | the five punches (4.4) |
| `0x13E0` | 417 | 1.6667 | Thunder Flop |
| `0x140C` / `0x1AE0` | 490 / — | 0.75 | throw / alternate throw |
| `0x1438` | 492 | 0.8333 | slam of a carried guard (also Diablo Fire Slam, gadgets.md 5.3) |
| `0x1AE4` | — | — | Diablo Fire Slam's own clip (absent on the disc) |
| `0x1440` / `0x1444` | 452 / 453 | 1.0 / 0.6667 | picking up |
| `0x1848` / `0x184C` / `0x1854` / `0x1904` | 440 / 441 / 442 / 443 | 3.3333 / 0.5 / 2.0 / 1.0 | Berserker Charge: wind-up, charge, bounce off a wall, stop |
| `0x1AC4` | 342 | — (not a clip in the hub) | Fists of Flame and Guttural Roar |
| `0x1AE8` | 508 | 1.6667 | alternate clip of the carry slam |
| `0x1AEC` | 509 | 0.6667 | Turnbuckle Launch (state 21, looping) |

**Bentley:**

| Slot | Clip | Length | Move [R] |
|---|---:|---:|---|
| `0x14E4` / `0x14E8` | 522 / — | 0.5 | held by Murray: still / being moved |
| `0x174C` + 0x1C·i (low), `0x1758` + 0x1C·i (high), i = 0…5 | 560 + 2i, 561 + 2i | 0.5 each | the crossbow swing's six two-clip blends (5.3) |
| `0x1820` | 311 | 0.75 | (a Bentley action clip; use not established) |
| `0x1874` | 341 | 0.3333 | Size Destabilizer (item 8) |
| `0x1878` | 588 | 0.1667 | state 14 (Adrenaline Burst; section 1.1) |
| `0x1444` | 447 | 0.75 | picking up |

#### 9.1.1 Blend weights (2026-11-19) [R, D for the constants and clips]

**Swinging (Sly, state 3; clips 165, 166, 167).** Two values are eased each frame toward targets set by the
substate: *activity* `a` (smoothing parameters 1.0, 0, 0.1 at `0x002BF570`) and *pump* `p` (4.0, 0, 0.1 at
`0x002BF580`). Weights: **165 = a·p, 166 = a·(1 − p), 167 = 1 − a**. Substates and targets, from the stick against
the heading (pushed past 0.3): 0 **pumping ahead** (stick within 1.0472 rad of the heading): `a` → 1, `p` → 1; 2
**pumping back** (stick beyond 2.0944 rad): `a` → 1, `p` → 0; 1 **coasting** (stick released for 0.25 s, or held
ahead or back for more than 1.4 s): `a` → 0, `p` → 1 if `p` > 0.5 else 0; 3 hanging still (swing angle under 0.1 rad
and speed under 80) plays the hang state machine (slot `0x1950`) instead. The blend is **not played at rate 1**: its
time is steered toward `D × (clamp(θ / 1.7, −1, 1) + 1) / 2`, `θ` the swing angle (the hook direction's pitch from
straight up in the heading's frame) and `D` the blend's length, at a rate limited to ±3 (`0x002BF58C` = 1.7,
`0x002BF590` = 3); entering the swing starts it at the middle.

**Swing physics: catch, pump, damping, release (2026-11-20) [R; D for every constant quoted with an address].**

*Which hooks.* A hook is a point-type jump target; its kind (the hook's word at `+0x124`) picks the hang: kind 0
faces where the stick points (within a quarter turn of the hook frame's x axis, else the reverse), kind 1 faces the
hook frame's x axis, kind 2 faces its reverse, and kind 3 is not a swing but the hang that follows a moving hook
(state 22). The jump-target search takes a hook only when its flag word `+0xE8` has bit 0 set, its target mask
matches, Sly is not higher above it than the hook's authored height allowance, it is within 1000 units, a jump
taking 0.25 to 3 s (searched from 0.5 s) reaches it with a fit score under 2500, and the path is clear.

*The catch.* Jumping at a hook (jump states 14 and 15, current player only) puts Sly in action mode 0, the cane
reach. Each physics step while mode 0 is on, Sly keeps three points: his body centre, the cane hook's (object 317)
centre before the step and its centre after it. The catch tests the two segments body centre → cane hook before,
and cane hook before → cane hook after, against the hook's point; **the hook is caught when either segment passes
within 15 units of it** (15.0 stored by Sly's set-up at `0x0018849C`/`0x001884B0`, field `+0x19AC`). The same test
against the nearest pipe points catches pipes (state 8). So the reach is measured from the cane's hook end, swept,
not from Sly's hands, and it is 15, not 135. On a catch Sly enters state 3 substate 1 (kind 3: state 22).

*Substates (stick against the heading, pushed past 0.3).* 0 pump ahead (stick within 1.0472 rad), 2 pump back
(beyond 2.0944 rad), 1 coast, 3 still. From 0: stick back → 2; stick held ahead over 1.4 s (`0x002BF554`) → 1 and
that direction is locked out; stick released 0.25 s (`0x002BF55C`) → 1. From 2 the mirror (1.4 s at
`0x002BF558`). From 1 or 3: stick ahead → 0 and stick back → 2 unless that direction is locked; the lock clears
when the stick is released (magnitude 0.3 or less). 1 → 3 when |angle| < 0.1 rad and speed < 80; 3 → 1 when
|angle| > 0.4 rad and speed > 120 (`0x002BF560`..`0x002BF56C`).

*The pendulum.* Each frame, about the hook (a hook on a moving object adds its point velocity): let `r` be the
distance from the hook to Sly's centre, `ω` the swing's angular velocity (tangential speed / `r` about the
axis across the motion) plus gravity's angular term times `dt`. Split `ω` into its part about the **hook frame's y
axis** (the swing plane's normal) and the rest. The in-plane part is multiplied by `1 + dt·k`, `k = clamp(1.5 −
0.35·|ω|, −5, −0.25)` (`0x002BF4F0`, `0x002BF500`): a light −0.25/s drag up to 5 rad/s, heavier above, −5/s from
about 18.6 rad/s. The out-of-plane part is multiplied by `1 + dt·b`, `b` = −2.5 while pumping (substates 0, 2)
and −1.0 otherwise (`0x002BF508`). **The push:** for the first 0.25 s after entering substate 0 or 2
(`0x002BF510`) only, `5·dt` rad/s (`0x002BF514`) is added about the hook's y axis, negative for substate 0,
positive for 2, both reversed when Sly faces the hook's back; it is scaled by `clamp(0.5 − 0.01·h, 0, 1)`
(`0x002BF520`), `h` = the swing's energy height relative to the hook (Sly's height minus the hook's plus
`v²/2g`): full push while that height is 50 or more below the hook, half at the hook's height, none from 50 above.
So each pump is one 0.25 s impulse of at most 1.25 rad/s, timed by re-pushing the stick (alternating ahead and
back restarts it at once); holding gives nothing more. **The rope:** its length is eased toward `230 −` the
hook's value at `+0x134` with the smoothing parameters (1000, 0, 0.25, 5000) (`0x002BF530`), and `ω` is scaled by
`min(230, r) / max(75, new length)` (`0x002BF540`, `0x002BF544`). Sly's next velocity is the hook's velocity plus
`ω × (the rope vector rotated by the mean ω over half a step)` plus the rope's length rate; the frame's
acceleration is set to reach it (gravity removed). Sly's body is braked toward hanging straight below the hook at
the heading (braking parameters 15, 0, 0.1 at `0x002BF548`). **There is no angle limit and no maximum speed**: the
energy-height fade of the push and the drag are the only bounds.

*Hands on the hook.* While swinging, the hand record is moved each draw so the cane hook (317) sits on the hang
point, the hook's position plus its `+0x134` along the direction from the arm to the hook, the arm's two-bone joint
is solved, and the hand is turned so its x axis points from the arm to the hook, then 1.5 rad about it
(`0x002BF594`); an extra offset is eased to zero with (3000, 1000, 0.15) (`0x002BF598`).

*Release.* Cross, taken after 0.25 s in the state, lets go as jump kind 2: the swing's velocity is kept and its
upward part raised to at least 700. The hook's release sound (its `+0x130`, default 9) plays when the hang mode
ends; the hook's splice events are 5 on entering and 6 on leaving. Whether Square also lets go was not found in this
pass [I].

**Stealth Slide (Sly, state 24; clips 259, 260, 261).** Entering: 260 alone (weights 0, 1, 0). Each frame, with
`ω` the body's yaw rate (rad/s, counter-clockwise positive), `L = 0.5 × clamp(ω / 2.5, −1, 1)` (2.5 = 0.1 × 25, the
first braking value at `0x002C3CB8`): **259 = max(0, L), 260 = 1 − |L|, 261 = max(0, −L)**. So 259 leans into a
left turn and 261 into a right turn, each reaching at most 0.5 [I for the names].

**Glide (Sly, state 20; five clips at slots `0x440`, `0x44C`, `0x458`, `0x464`, `0x470`).** The slots resolve to
**1519, 1520, 1521, 1523, 1522** (note 1523 before 1522) [D]. Entering: 1519 alone. Each frame, with `m` the stick
magnitude and `s`, `c` its sideways and forward parts against the body's facing (`s = m·sin`, positive left;
`c = m·cos`, positive forward) and `q = m / (|s| + |c|)`: weight of slot `0x440` (1519, **centre**) `= 1 − m`;
`0x44C` (1520, **left**) `= s·q` when `s` ≥ 0; `0x458` (1521, **right**) `= −s·q` when `s` < 0; `0x464` (1523,
**forward**) `= c·q` when `c` > 0; `0x470` (1522, **backward**) `= −c·q` when `c` ≤ 0; the unused side of each pair
is 0, and the weights sum to 1. The same stick values set the glide speeds and turn rate (gadgets.md) [R]. Disc
poses [D]: these clips hold the canopy's transform 3 at (0, −0.19, 192.93) for 1519, (0, −16.45, 192.93) for 1520,
(0, 20.16, 192.93) for 1521, (0, −0.19, 179.98) for 1523 and (0, −0.19, 211.17) for 1522.

The complete slot list (about 500 entries for the three characters) is reproducible from the tables of
[animation.md](animation.md) 9; moves not in these tables are still unnamed.

### 9.2 Standing still: the idle machines (2026-11-19 findings)

No character has a single looping idle clip. Standing still runs the state machine in lookup slot `0x1028`
(Sly: state machine 124; Murray and Bentley: state machine 418 of their own package) [D]. The machine plays a chain
of one-shot clips, one per transition, chosen at random by weight; no clip in it is looped on its own [D for the
data, R for the run rules]. Run rules (animation.md 8; base rules in ../sly1/scripts.md 9) [R]: next state −1 means
stay, 0x20 means "pick a random leaving transition allowed by the mask"; weights are relative probabilities; a run
whose start state is not in the machine starts at the machine's first state. Lengths in seconds [D].

**Mask encoding** (confirmed 2026-11-19) [R, D for the values]. A transition's flags word ([animation.md](animation.md)
8) carries bit 0 = usable both ways and **mask bits 2 to 15** (`0x4`–`0x8000`); a transition may be taken only when
its flags AND the current mask is non-zero (route-distance groups use bits 2 to 12). On the disc, "any" is `0xFFFC`
(and `0xFFFD` with bit 0). Entering the standing state applies the idle machine with the **look mask**, and the
standing state re-sends it every frame. The look mask is one bit: the side of the look target in the body's frame is
0 ahead (|angle| < 1.0 rad), 3 behind (> 2.5 rad), 1 left (positive angle), 2 right; then by the look kind: none →
**8**; kind 1 → `0x10 << side` (**16, 32, 64, 128**); kind 2 → `0x100 << side` (**0x100, 0x200, 0x400, 0x800**);
kind 3 → **0x1000**; kind 4 → **0x100**. So every idle mask is one of bits 3 to 12, as the engine assumes; bit 2
(`0x4`) is the mask Murray's and Bentley's other states apply their machines with (Sly's other states use `0xFFFC`),
so it never selects idle transitions.

**Sly** (machine 124) [D]. The start state depends on what Sly did before [R]: 373 by default; 374 after state 12;
375 after state 10; 376 after landing from a fall of at most 400 units, 377 after a longer fall; 379 after a pose
(the pose modes); 380 when a foot is over a drop (teeter).

| From → to | Mask | Clips (weight) | Length |
|---|---|---|---|
| 373 → 5532 | 8 | 4576, 4578, 4580 (1 each) | 1.5167 each |
| 376 → 5532 | any | 5561 | 1.6667 |
| 377 → 5532 | any | 5560 | 2.9833 |
| 379 → 5532 | any | 5562 | 3.5 |
| 5532 → 5533 | 8 | 5539, 5540, 5541 (20 each) | 8.1333, 4.5833, 1.5 |
| 5533 → 5534 | 8 | 5542 | 1.6667 |
| 5534 → 5534 (repeats) | 8 | 5543 (50), 5544 (50), 5545 (5), 5546 (5), 5547 (5), 5548 (5), 5549 (20), 5550 (20), 5551 (20), 5552 (20), 5553 (10), 5554 (5), 5555 (20) | 8.3333, 9.3333, 8.3333, 8.3333, 9.3333, 9.3333, 2.1667, 6.55, 4.55, 8.4167, 7.3333, 8.3333, 9.0833 |
| 5532 → 5532, look ahead | 16 | 4560, 4552 | 1.5167 each |
| 5532 → 5532, look left | 32 | 4566, 4558 | 1.6667 each |
| 5532 → 5532, look right | 64 | 4564, 4556 | 1.6667 each |
| 5532 → 5532, look behind | 128 | 4562, 4554 | 1.6667 each |
| 5532 → 5532, second kind | 0xF00 | 4572, 4570 | 1.3333 each |
| 374 → 5532, second kind | 0xF00 | 4574 | 0.5833 |
| 5532 → 5532, third kind | 0x1000 | 4568 | 1.0 |
| 5533 → 5532, 5534 → 5533 | 0x1FF0 | none (instant) | — |
| 374 → 5532 (mask 0xF0), 375 → 5532 (mask 0x1000) | | none (instant) | — |
| 380 → 5535 (teeter) | any | 5556 | 4.55 |
| 5535 → 380 | any; 5559 mask 8 | 5557 (34), 5558 (33), 5559 (33) | 4.55 each |

So with nothing to look at, Sly plays one of three settle clips, one of three longer clips, clip 5542, and then a
random fidget from the thirteen for as long as he stands. With a look target the fidget states drop straight back
to 5532, which repeats the look clip for that direction. The teeter alternates 5556 and one of 5557–5559 forever.

**Murray** (machine 418) [D]. No start state matches, so he starts at 17776. Every transition allows any mask.

| From → to | Clips (equal weights) | Length |
|---|---|---|
| 17776 → 17777 | 7238, 7240, 7242, 17779 | 3.6667, 6.6667, 5.0, 3.6667 |
| 17778 → 17777 | 7236 | 3.0 |
| 17777 → 17065 | 17780, 7244 | 3.8333, 3.3333 |
| 17065 → 17065 (repeats) | 17782, 17783, 17784, 17785, 17786, 17787, 17788 | 6.6667, 5.6667, 5.3333, 6.5, 5.5, 5.8333, 8.3333 |

Which state enters 17778 was not found.

**Bentley** (machine 418 of his package) [D]. Starts at 92; 379 (after a pose) also leads to 17064.

| From → to | Mask | Clips (equal weights) | Length |
|---|---|---|---|
| 92 → 17064, 379 → 17064 | any | 7608 | 1.5 |
| 17064 → 17065 | any | 7610 | 3.3333 |
| 17065 → 17065 (repeats) | 8 | 7602, 7604, 7606 | 3.0, 3.1667, 3.4167 |
| 17065 → 17064 | 0xFF0 | none (instant) | — |

With a look target Bentley alternates: back to 17064 at once, then 7610 again. A viewer that wants one "idle"
loop can use Sly 5543, Murray 17782 and Bentley 7602 [I]; they are among the most likely repeating clips.

### 9.3 Gameplay-state and per-level meshes on the gang (2026-11-19 findings)

**Paraglider and its strings (Sly).** They are meshes of Sly's own body (object 7), not of a separate object [D]:

| Part | Mesh | Bones | Shader |
|---|---|---|---|
| canopy | 36 vertices, opaque, at transform 2; bind-pose box (−66, −137, 250)–(66, 140, 320) | Sly transforms 5 and 8 | 14 (blue cloth) |
| strings | two 6-vertex meshes from the hands (z ≈ 117) to the canopy (z ≈ 258) | transforms 8, 5, 36, 29 | 14 |
| canopy outline and shadow | 18-vertex ink outline; 19-vertex shadow volume at transform 8 | | |

Transforms [D]: 3 (move, rest offset z 219.5) → 4 (rotate) → 5 (scale) carry the canopy; 6 (move, z 189.2) → 7
(rotate) → 8 (scale) carry the string tops. Their rest scale is 1, so **the bind pose shows the paraglider open**
above Sly's head [D]. Only clips 1518–1523 animate them [D]. Clip 1518 (the opening, lookup slot `0x438`, 0.25 s)
moves transform 3 from (0, 27.3, 24.3) to (0, −0.19, 205.0) and transform 6 from (0, 27.3, 26.8) to (0, −0.19,
150.4), out of a folded position at Sly's back, and scales transform 5 from (0.019, 0.022, 0.144) to (1.0, 1.093,
1.0) and transform 8 from (0.565, 0.065, 0.565) to (1.0, 0.847, 1.0) [D]. Clips 1519–1523 (2.0833 s each) are the five
glide blend poses (slots `0x440`–`0x470`; weights in 9.1.1) and hold the scales near 1 [D].

A second object, 1525 (class 48, nested on Sly transform 2, no meshes, removed at level start), carries state
machine 1526 (slots `0x1D20`, `0x1D24`) [D]. It is applied at load with no goal, so it starts in state 1528 (closed)
[R]. Goal 0x5F7 (state 1527, open) plays clip 5517 (0.25 s: object 1525 transform 2 scale 0 → 1); goal 0x5F8 (1528)
plays clip 5518 (0.25 s, no channels) [D]. Clip 1518 also scales 1525 from 0 to 1 [D].

Rule [R]: entering jump state 19 (R1 in the air, gadgets.md 27) plays clip 1518 and sets the machine's goal to open;
state 19 becomes the glide (20) when the clip ends; leaving 19 or 20 for any state other than 19–20 sets the goal to
closed and drops the glider camera zone (slot `0x47C`, object 1524). So: shown from the jump into state 19 until
the glide ends; hidden otherwise.

*Not established*: what keeps the canopy folded outside the glide. No other clip of Sly's drives transforms 3–8, and
the closing clip 5518 has no channels; object 1525's scale does not reach the canopy's bones in the data read so
far. A renderer should hold transforms 3, 5, 6 and 8 at clip 1518's first frame whenever Sly is not in states
19–20 [I].

**The green shape on Sly's head: a per-level costume, not a gadget or prompt.** Sly's body has meshes that are
loaded only in some level-table rows (geometry.md 2.2) [D]:

| Level row | World | Sly meshes | Reading |
|---|---|---|---|
| 32 | `c_sawmill_ext` | an 85-vertex head mesh at transform 73 (shaders 7 and 28: green, teal and orange) and its 59-vertex outline; box (−25, −26, 143)–(66, 25, 200), with a brim to the front | a cap [I] |
| 11 | `i_palace_ballroom` | an alternative 790-vertex body (shader 41: purple cloth) with hands (transforms 29, 36), cuffs (63, 39), head piece (73) and outlines | the ballroom outfit [I] |
| 17 | `p_castle_ext` | two 46-vertex radial discs over the eyes (transforms 82 and 88), vertex colours (19, 0, 29) to (168, 2, 255) | glowing eye lenses [I] |

The green shape is most likely the row-32 cap, drawn because the renderer kept conditional meshes of every row
[I]. In the nightclub hub (row 2) none of these meshes is loaded [D]. Row 32 also gives Murray (transform 84) and
Bentley (transform 57) an 85-vertex head mesh [D], so all three wear headgear in that hub [I]. Murray's other
per-level meshes: row 33 (`c_sawmill_mulch`) a 323-vertex head mesh and parts at transforms 72 and 84; row 30
(`c_train_b`) four meshes at transforms 3 and 6; rows 12 and 15 one mesh at transform 119; row 32 one more at 119
[D]. Bentley's: rows 8, 29 and 30 a mesh at transform 52; row 17 a translucent 12-vertex mesh at transform 36 [D].

**Other parts nested in the three bodies** [D]. Each body holds nested objects with option 26 (removed at level
start): Sly 28, Murray 22, Bentley 8 (at any depth), the paraglider holder 1525 among them. Their meshes include translucent rings
and bursts (Sly 3839, 3841, 3843: 4-vertex cards), spark strips (4185, 4212–4224) and gadget models (for example
Sly 3689, 3697, 3718, 3725, 3738, which also carry option 13). Being removed at start, they are not drawn on the
standing character [R]; they belong to gadgets and effects (gadgets.md) [I]. Object 586 (class 105, a translucent
61-vertex flat shape on the floor behind the character, options 1305 and 1307) and empty object 685 stay in all three
[D]; section 9.3.1 shows that both are taken out of the world by the character's lookup table at load.

#### 9.3.1 Cane parts, throw marker and target ring (2026-11-20 findings)

These objects carry no option 26 and so look "always in the world" from the stream alone. The characters' lookup
tables (12-byte records: flags, object ID, slot) decide otherwise: **flag 0x200 removes the object from the world
when the table is bound at load** [D for the flags, R for the meaning] (0x300 = 0x200 plus 0x100).

| Object | Owner, package | Lookup record [D] | Meshes [D] | Parent [D] | Drawn when |
|---|---|---|---|---|---|
| 316 | Sly (`jt`), class 48 | `0x002BF1B0` record 0: flags 0, slot `0x19A0` | cane shaft: rigid 9 vertices, shader 18 (ID 4751, kind 1, white, textured), box (−39, −3, −2)–(54, 3, 2); an ink outline and a skinned outline | Sly transform 36 (the hand; `0x1024`), offset (−22.1, −12.9, −5.0) | in the world from load; see the cane rule |
| 317 | nested in 316 | record 1: flags 0, slot `0x19A4` | cane hook: rigid 87 vertices, shader 18, box (−56, −7, −3)–(15, 22, 3); outlines | 316's transform 1 (`0x1001`), offset (74.7, 0.2, 0) | with 316 (it is 316's child) |
| 318 | Sly, class 48 | record 2: **flags 0x200**, slot `0x19A8` | a second whole cane: shaft (15 vertices) and hook (98), shader 18, outlines | Sly transform 73 (`0x1049`); bind-pose world (21.2, 38.1, 150.8) | **removed at load**; see the cane rule |
| 586 | each character, class 105 | Sly and Bentley: shared table `0x002BC9F0` record 44, slot `0x12DC`; Murray `0x002C0270` record 54, slot `0x1A3C`; **flags 0x300** | 61-vertex flat arrow, shader 27 (ID 4841, kind 1, colour (234, 121, 30), gold-orange), render pass 10: a band about 100 wide from x −456 to about 0, then an arrowhead (±92) ending at x 48, all at z = 0; vertex alphas rise from 76 at the tail to 204 at the head | the character's transform 2 (`0x1002`; Murray `0x1003`), offset (−158, 0, −75): on the floor | **removed at load**; shown only while a carried object is being aimed |
| 685 | each character, class 48 | shared table record 46, **flags 0x300**, slot `0x11A4` | none | the character's transform 61 (`0x103D`) | never (no meshes) |
| 3765, 6964, 7355 | top-level objects of the packages `goal_object_jt`, `goal_object_murray`, `goal_object_bentley` | placed into the characters by class-6 placements 1569 (Sly, kind 1, parent `0x1002`), 1571 (Murray, `0x1003`) and 1570 (Bentley, `0x1002`); those placements are in the tables at slot `0x9C4` with **flags 0x300** | a translucent ring in the vertical plane (box (0, −41, −25)–(0, 39, 27)), three meshes, pass 10, draw distances 1000/1100/1200; Sly's shaders white, white at alpha 63, (0, 136, 255, 217) and (0, 68, 127, 115), so blue; Murray's and Bentley's in their own colours | transform chain from the package root | **never where stored**; copies appear on targets (below) |

**The cane rule [R].** Exactly one cane is in the world at a time. At load 316 (with 317) is in, 318 is out. When
Sly's movement mode changes between "mode 2 or 11" and any other mode, the cane for the new side is added, moved to
the pose of the one in use, and the other is removed: 318 while the mode is 2 or 11 (both belong to the pipe climb;
see "Cane modes 2 and 11" below), 316/317 otherwise. While Sly is in a hiding spot his cane 316 is taken out of the world and put back when he leaves it. Sly's
hide routine hides and shows 316, 317 and 318 together. When Sly is the current player, 316 and 317 take Sly's own
fade value each frame, so they fade with him. 317 is also the cane tip for reach tests and for the jump-to-target
aim. Both canes hang on animated bones (36 and 73) and must be drawn with those bones' world matrices; drawn with
only their local offsets, they land near the character's origin, at floor level [I].

**Cane modes 2 and 11 (2026-11-20) [R; D for the clips].** The mode is Sly's *action mode*, a value kept apart
from the gang state and changed by one routine (`0x0018E540`); it is not the movement state. Its values: −1 none;
0 the cane reaching for a jump target (sound 15, its clip held at the middle for 0.1 s); 1 hanging from a hook;
**2, 10 and 11 the pipe climb**; 4 to 7 hang blends (sound 11); 12 to 21 cane swings; 22 and 23. The climb sets
them as follows. Gripping a pipe (state 8, its first substate, current player only) sets **mode 10** when the grip
clip (slot `0x14D4`, clip 150, 0.4167 s in the nightclub hub) exists, else mode 2 at once; when mode 10's clip
ends the mode becomes **2** (the climb proper). Leaving state 8 for state 2 (the air) sets **mode 11** when the
jump-off clip slot `0x14D8` holds a clip, else −1; mode 11 ends with that clip (any mode other than 10 goes to −1
when its clip ends). In the nightclub hub slot `0x14D8` names object 151, which is not a clip there, so that world
never enters mode 11 [D for the slot contents]. So 318 (the cane worn on the back, bone 73) is shown only while
climbing a pipe and during the jump-off clip from one; mode 10 keeps 316 in the hand during the grip clip. Ordinary
jumps, falls and the other air states never use modes 2 or 11: reading "modes 2 and 11" as "the air states and the
climb" is wrong.

**The throw marker (586) [R].** Class 105 is a history-trail object: its vertices are redrawn every frame from a
ring of recent world matrices of its parent transform, each vertex using an older matrix the farther it lies along
the trail's axis; each vertex's alpha is its authored alpha times the trail's gain times an age fade over the
history span (0.5 s when not authored). As the throw marker it is added to the world when the throw controller
enters its **aim** state; each frame its history is then filled from the predicted flight path (from the predicted
landing point back toward the launch point, each point facing along the path, gain 1 where a point exists and 0
beyond), so the gold arrow lies along the arc the thrown object will follow. It is removed when the throw ends or is
cancelled. Outside aiming it must not be drawn. **The gold rod on the floor near Sly in the port is most likely
this marker drawn in its authored rest shape** (flat, gold, at floor height, reaching from 614 behind the character
to about 110 behind it) because the load-time removal of flag 0x300 was not applied [I]. The second cane 318 is the
other candidate, but its rest position is at head height [D]; a cane drawn without its bone matrix is a third [I].

**The target ring (3765, 6964, 7355) [R].** The ring package is a prefab: the stored object is not drawn
(classes/placement.md 2), and the character's placement of it (slot `0x9C4`) is removed at load. While the player
moves, up to four nearby interaction targets (ledges, poles, points a move can reach) are tracked by distance. When
a target is the nearest one with a prompt, a copy of the ring is made from the player's placement, taken out of any
parent, placed unrotated at the target's point and faded in over 0.25 s; it then follows the target point along
its curve; when the target is dropped, its ring fades out over 0.5 s. So the ring is drawn only on the prompted
target, never on the character.

So, for the port: draw 316/317 attached to bone 36; never draw 318 except in modes 2 and 11; never draw 586 except
while aiming a throw; never draw 685; never draw the stored ring objects nor the characters' ring placements, only
the per-target copies [R].

## 10. Test vectors [D]

- Item table record 7: `(2, 8, 200, 0, 1, 129, 130)`; record 42: `(1, 7, 0, 1, −1, 201, 202)` "Time Rush".
- Executable floats: `0x002BC920` = 175, 0, 0, 0; `0x002BC970` = 0, 0, 10; `0x002BC980` = 0, 0, 40000;
  `0x002C08DC` = 0.15, 0.1; `0x002C0900` = 1.0; `0x002C09A0` = 0.75, `0x002C09A4` = 2.5, `0x002C09A8` = −500;
  `0x002BF730` = 515; `0x002BFAE0` = 1000, 900, 950; `0x002BFAF0` = 1000, −900, 950; `0x002BFB00` = 600, 800;
  `0x002BFB10` = 0, 0, −1000; `0x002BFA00` = 0, 0, −750; `0x002BDC20` = 1.0, 0.5.
- Movement constants: `0x002BF3D4` = 1.3 (bytes `66 66 a6 3f`), `0x002BF5C4` = 0.75, `0x002BF730` = `0x002BF734` = 515,
  `0x002BF738` = 215, `0x002BF73C` = 315. Root travel of Sly clip 126 (run): (0, 0, 75) to (320, 0, 75) in 0.5 s; of Murray clip
  414: 400 in 0.5 s; of Bentley clip 414: 250 in 0.3333 s (nightclub hub).
- World string table: ids 129 to 202 are the names and descriptions of the 36 items above; 2,700 distinct string ids occur
  across the 43 worlds.

## 11. Open questions

- (Resolved: one health counter per character, and hit amounts carried by the hit; section 2.) The amount of each
  guard melee attack.
- (Resolved: speeds and jumps, sections 1.1 and 1.2.) Whether the footfall time-warp ever departs from the plain
  length ratio, and which state uses Murray's second fast pair.
- Which clip each remaining move uses (section 9).
- The exact effect of each gadget (radius, duration) and of the twelve unsold Sly moves.
- How switching is offered in hubs (only at the safe house or also elsewhere).
- What keeps Sly's paraglider folded outside states 19 and 20 (9.3), and which state enters Murray's idle state
  17778 (9.2).

## 12. Stick input [X]

Sly 2 reads the analog sticks with **the same routine as Sly 1**: the converter at `0x00187768`, with its pad set-up
at `0x001875B8`. It has the same constants: the wake-up window 96 … 160, the ±0.75 starting ranges, the four
calibration directions at `0x002BE950`, the 0.35 dead zone with full deflection at 0.85, and the output normalised
and scaled by u (zero below a length of 0.0001). The full rule is in Sly 1
[player-moves.md](../sly1/player-moves.md) 1.2. Sly 3 changes only the output step
([../sly3/characters.md](../sly3/characters.md) 11).
