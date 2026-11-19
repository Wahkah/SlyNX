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
reviewed_by: pending
date: 2026-11-14
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
combat-dodge clips (`0x2000`, `0x2004`: objects 281 and 282) each move 500 in 0.3333 s (1,500/s) [D].

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

**Jump clips** (lookup slots `0x3E0` to `0x41C`) [D for the resolution, R for the roles]: five take-off clips chosen
by the stick's direction relative to the heading (straight or none, then two angle bands at about 1.10 and 2.04 rad
[R]), five falls and up to five landings:

| | Take-off (`0x3E0`…`0x3F0`) | Fall (`0x3F4`…`0x404`) | Landing (`0x408`, `0x40C`, `0x414`, `0x418`, `0x41C`) |
|---|---|---|---|
| Sly | 152, 156, 155, 153, 154 | 160, 164, 163, 161, 162 | 158, 159, 169, 601, 602 |
| Murray | 590, 594, 593, 591, 592 | 595 (the others are not clips in the hub) | 600 (at `0x414`) |
| Bentley | 590, 594, 593 (the others are not clips in the hub) | 595 | 478 (`0x408`), 600 (`0x414`) |

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
within **0.15 s**, within an angle of **1.5 rad** of his heading (3.15 rad when standing); its height between
−50 and 50 blends the low and high swing clips; standing still he turns to face it. The swing has three modes
(standing, moving, air), each a two-clip blend chosen by the side the weapon is on; a repeat press restarts it.
Size Destabilizer shrinks the guards it hits (item table).

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
entries: gaits for walk/run (one clip blend per gait) [R], five punch clips for Murray, a two-clip blend per crossbow swing mode for Bentley, a four-clip blend for
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
| `0x2000` / `0x2004` | 281 / 282 | 0.3333 | Combat Dodge |
| `0x2018` / `0x2024` / `0x2030` | 259 / 260 / 261 | 1.0 | Stealth Slide blend |
| `0x1F64` / `0x1F70` / `0x1F7C` / `0x1F88` | 307–310 | 1.5 | hanging blend (four clips) |
| `0x1974` / `0x1980` / `0x198C` | 165 / 166 / 167 | 0.6667 | swinging blend (three clips) |
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
| `0x174C` … `0x17E4` (six pairs, 0x1C apart) | 560–571 | 0.5 each | the crossbow swing's six two-clip blends (5.3) |
| `0x1820` | 311 | 0.75 | (a Bentley action clip; use not established) |
| `0x1874` | 341 | 0.3333 | Size Destabilizer (item 8) |
| `0x1878` | 588 | 0.1667 | state 14 (Adrenaline Burst; section 1.1) |
| `0x1444` | 447 | 0.75 | picking up |

The complete slot list (about 500 entries for the three characters) is reproducible from the tables of
[animation.md](animation.md) 9; moves not in these tables are still unnamed.

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
