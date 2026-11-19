---
id: sly2-gameplay-vehicles-and-minigames
title: Sly 2 driven machines, turrets, hacking, dance, the combat car and bosses
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
      Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds
    used_for: >-
      every-depth walk of all 121,153 object bodies (0 failures) collecting every record of classes 50 and 56 to 78
      with its option list; job and task records (job names, world rows); constants read from the executable after
      the hash check (addresses given where used)
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce (2026-11-12), Sly2-Release: turret.cpp/.h, hack.cpp, vehicle.cpp/.h, vehiclecontrol.cpp,
      vehiclebindings.cpp, dance.cpp, guardstart.cpp, po.cpp
    used_for: behaviour rules and option meanings marked [R]; every constant it cites was re-read in the executable
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), data tables naming the dance script commands
    used_for: confirmation that the dance commands are script built-ins [R]
  - kind: disc
    what: SCUS-97316, 2026-11-18 re-walk (collision parts of classes 56-78 with their contact kinds and values; part-switch events 15/16 in every boss package's clips; boss option lists)
    used_for: boss attack parts, hit windows and amounts, escape records [D]
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release (guardstart.cpp boss options 231, 254-258, 1340, 1350; aihit.cpp hit handling; asegevent.cpp events 15/16)
    used_for: what the boss options do and how a fight is staged [R]
reviewed_by: pending
date: 2026-11-17
---

# Sly 2 driven machines, turrets, hacking, dance, the combat car and bosses

Sly 1 had dedicated minigame levels ([../sly1/minigames.md](../sly1/minigames.md)). In Sly 2 the same kind of play
lives inside **jobs** ([missions.md](missions.md)) of ordinary worlds, built from a handful of classes plus scripts.
Tags: [D] disc data, [O] observable, [R] reference behaviour, [I] inferred. Rows are level-table rows
([disc.md](disc.md) 6, 7). Job names are the game's own strings.

## 1. Which job uses what [D]

Job → world rows from the jobs' task records ([missions.md](missions.md)); classes from the walk.

| Job (game name) | Rows | Play | Classes involved |
|---|---|---|---|
| `m2_turret` Battle the Chopper | 8 | turret | 69 (`turret_player`) |
| `m3_turret` Blow the Dam | 12 | turret | 69 |
| `m8_turret` Carmelita's Gunner | 38 | turret | 69 |
| `m5_battlezone` Tank Showdown | 17 | driven tank, turret | 50 (`TANK_DRIVEABLE_ROOT`), 69 |
| `m2_rc_chopper` RC Bombing Run | 8 | remote-control chopper | 50 (`rc_chopper_root`) |
| `m4_rc_chopper` Wall Bombing | 14 | remote-control chopper | 50 |
| `m6_rc_chopper` Aerial Assault | 27, 29 | remote-control chopper | 50 (also in row 30) |
| `m7_robot_war` RC Combat Club | 32, 33 | combat car battle | 77 (`robot_tank`), 108 (ammo) |
| `m4_hack` Train Hack | 14 | hacking | 72 to 76 |
| `m5_hack` Crypt Hack | 22 | hacking | 72 to 76 |
| `m7_boat_hack` Boat Hack | 32 | hacking | 72 to 76 |
| `m2_dance_audition` Dominate the Dance Floor | 11 | dance | 68 (`dancer`), 121 |
| `m2_heist_dance` Tango with Carmelita | 11 | dance | 68, 121 |
| `m1_boss` Printing Press Duel | 5 | boss | 59 (`npc_dimitri`) |
| `m3_boss` Showdown with Rajan | 12 | boss | 61 (`npc_rajan`) |
| `m7_bear` Old Grizzle Face | 32 | boss-like | 66 (`npc_bear`) |
| `m7_boss` Brains over Brawn | 37, 32 | boss | 62 (`npc_jean_bison`) |
| `m8_boss` Showdown with Clock-La | 43 | final boss | see 7.3 |

The hacking packages are also stored in rows 15 and 38, and the `powerchop_player` machine (class 50) in rows 32
to 37 and the `attract_mode_player` machine in row 0, without a job of the table naming them [D]; what they are for
is open. Other job types with no dedicated class (paraglide rides, the water bug, the mega-jump, the engine rooms)
are scripted behaviour of the characters and are not covered here.

## 2. Driven machines (class 50)

Class 50 is a plain **player object**: a solid with the trailing name string ([classes/solid-object.md](classes/solid-object.md))
that the game can make the current player [D for the layout, R for the role]. It has **no machine-specific code**:
the remote-control chopper, the driven tank and the other machines are animated and steered by their own scripts
and state machines through the script built-ins ([scripts.md](scripts.md)), with their tuning stored as **custom
script properties** in the record's option list [R].

| Name | Records | Rows |
|---|---:|---|
| `rc_chopper_root` | 4 | 8, 14, 29, 30 |
| `powerchop_player` | 6 | 32 to 37 |
| `TANK_DRIVEABLE_ROOT` | 1 | 17 |
| `attract_mode_player` | 1 | 0 |

The RC chopper template is identical in its four worlds [D]. Its custom properties (typed option entries of options
3, 4, 5 and 8, each: property ID, value type, value) [D]:

| Property | Type | Value |
|---:|---|---|
| 1529 | object | −1 |
| 1530 | bool | false |
| 1531 | float | 0.25 |
| 1532 | object | 50744 |
| 1533 | float | 0.5 |
| 1534 | object | 3615 |
| 1535 | float | 0.6 |
| 1536 | float | −8000 |
| 1537 | int | 2100 |
| 1538 | float | 0.1 |
| 1539 | float | 20 |
| 1540 | float | 3000 |
| 1542 | int | 1000 |
| 1543 | float | 2.0 |
| 1378 | float | 4000 |
| 1516 | bool | true |

What each property drives is defined only by the chopper's script (open; to be read with the script spec). The
machine's control loop therefore has to be implemented by running its script, not as native behaviour [I].

## 3. Turrets (class 69)

A gun or grapple launcher the player sits in [R]. Six records [D]: `turret_player` (object 28548) nested in job
packages of rows 8, 12, 17 and 38, and `grapple_turret` / `c_grapple_turret` at top level in hubs 2 and 32.

### 3.1 Options

| Option | Meaning | Default [R] | Disc values [D] |
|---:|---|---|---|
| 1000 | tilt limits (rad) | (−0.7, 0.7) | row 8: (−0.75, 1.4); rows 12, 17, 38: (−0.78, 0.78); grapple turrets: (0, 1.5) |
| 1001 | pan limits (rad) | (−1.5, 1.5) | row 8: (−1.5, 3.0); rows 12, 38: (−0.78, 0.78); row 17: (−0.8, 0.8); grapple: (−1.5, 1.5) |
| 1010 | field of view range (rad), near and far | (0.1, 1.0) [D at `0x002C4400`] | (1.0, 1.0) row 8; (0.8, 0.8) others; (0.7, 0.7) row 32 |
| 1007 | build its own rotation controller | 1 | 0 on the grapple turrets |
| 1008 | camera mount object | itself | 42826 on the grapple turrets |
| 1009 | smooth the camera | 1 | 0 on four records |
| 1002, 1003 | projectile pool, launcher | | not set on the disc (set by scripts) |
| 13 | activation radius | | 6000 on `c_grapple_turret` |

### 3.2 Aim, zoom, fire [R]

- **Aim (left stick).** Each axis is cubed and scaled by a gain `g = (1 − f)·0.5 + f` where `f` is the current field
  of view in radians; the angular acceleration is `10·g` times the cubed input. The angular speed decays toward 0 by
  a smoothing of `(5, 0, 0.5)` [D at `0x002C4408`], reverses to 0 when the input opposes it, and is capped at
  `1.3·f·0.9` (pan) and `1.3·f·0.8` (tilt). Up/down follows the game's reverse-aim setting. Pan and tilt are then
  clamped to the option limits.
- **Zoom (right stick, vertical).** The zoom fraction `u` (0..1) changes at `(2 − u²)·y·|y|` per second
  [D for `(2, 0, −1)` at `0x002C4420`]; the field of view is the near value at `u = 1` and the far value at `u = 0`.
  When near equals far (Battle the Chopper's 1.0/1.0), the player may still zoom to half of it.
- **Fire.** Any of the eight face/shoulder buttons pressed this frame takes a free object from the projectile pool,
  throws it from the launcher, and raises script event 5 on the turret.
- **Camera.** While current, the turret takes the view at stack priority 9 ([camera.md](camera.md)).

## 4. Hacking games (classes 72 to 76)

Bentley's top-down shooter, played far above the level (about z = 25,000) [R]. Classes and packages [D]:

| Class | Role | Packages (records) |
|---:|---|---|
| 72 | the ship the player drives | `hack_ship` (5) |
| 73 | enemies, turrets and generators | `hack_enemy`, `hack_turret`, `hack_generator`, `hack_generator_middle`, `hack_generator_slow`, `hack_gunship`, `hack_gunship_slow`, `gunship_generator`, `hack_dreadnought`, `hack_mine_dropper`, `hack_mine_dropper_generator`, `hack_mega_turret`, `base` (26) |
| 74 | shots and timed pieces | `hack_bullet`, `hack_bullet_big`, `hack_bullet_bounce`, `hack_mine` (7) |
| 75 | the exit dock | `hack_lock` (5) |
| 76 | walls | `hack_wall…` (22, nested) |

The hack starts when Bentley uses the job's station; the station's script enables the ship and the camera follows
it [R].

### 4.1 Shared rules [R]

- **Hit points**: option 1363 sets the maximum (default 10), 1364 the current value. On a hit the points drop by the
  hit's damage; at 0 the object dies: it gives option 1365's **points** to whoever killed it, sets off its burst
  object (option 1366), and leaves the world.
- **Knock-back** on a non-fatal hit: away from the hit point in the horizontal plane, at speed `a + d·(b + d·c)`
  where `d` is the damage and `(a, b, c)` is option 1369 (default (300, 50, 0) [D at `0x002BE7D0`]), clamped to
  option 1370 (default (0, 800) [D at `0x002BE7E0`]).
- Contacts between hack objects have no friction and no bounce.

### 4.2 The ship (class 72)

- **States**: −1 out of the world, 0 idle, 1 driven, 2 hit, 3 docked, 4 destroyed [R].
- **Drive (left stick)**: target velocity = stick direction × stick deflection × 1000 [D at `0x002BE7E8`]; the ship
  accelerates toward it with gain 5 per second [D `(0, 5, 0)` at `0x002BE810`] and turns toward the stick heading
  with `(25, 0, 0.25)` [D at `0x002BE800`]. No gravity.
- **Guns (right stick)**: every gun turns to the right stick's heading and fires while the stick is deflected [R].
  Guns are option 1371 pairs (gun part, state machine), up to 4.
- **Hit**: while not hit, docked or destroyed, a hit costs hit points, raises task event 15 and enters state 2 until
  the hit animation ends; at 0 points the ship is destroyed (state 4) [R].
- HUD bar (option 1373) shows `1 − points / maximum`; engine loop sound option 1374 with volume following the stick.

### 4.3 Enemies (class 73)

States −1 out, 0 idle, 1 aim, 2 attack, 3 chase, 4 dying [R]. Per state, option entries give the distance and
angle ranges to enter and to stay (options 1380, 1381) [D for the entries, R for the meaning]. Turn gain
`(25, 0, 0.25) × option 1376` [D at `0x002BE870`]; push gain `5 × option 1377` [D at `0x002BE87C`]; speed scale
option 1382 (default 250). An enemy that carries points dies when it touches the ship [R].

### 4.4 Shots (class 74), docks (class 75), swarms

- **Shots**: a lifetime, a target and a turn rate (options 1389, 1390, 1391); a non-piercing shot dies on its first
  contact; piercing shots (1388) bounce with elasticity 1 [R].
- **Dock**: option 1394 names the dock point. While waiting, the ship within **50** units of the dock point
  [D at `0x002BE880`] starts docking (ship state 3); when the docking animation ends the dock is done and raises
  task event 57, which ends the hack [R].
- **Swarms**: up to 4 entries (path, count, group size, radius); each particle has **30** hit points
  [D at `0x002BE8AC`] [R for the role].

## 5. Dance (classes 68 and 121)

Both dance jobs are in row 11 [D]. A **director** (class 121) runs the dance; **dancers** (class 68, package `dancer`)
play the moves [R]. The dance script issues three commands: set the director's state with the button prompts, add a
beat (dancer, button, music time), and set a dancer's default move [R].

- Director states: 0 off, 1 prompts shown with the game paused, 2 playing [R].
- **Timing window**: 0.4 s early and 0.4 s late around each beat's music time (director defaults) [R].
- **Score of a press**: `1 − u`, where `u` is the time from the beat divided by the early or late window [R].
  Wrong button −0.8, pressed twice −0.2, too early −1.5, missed −1.5 [R]. Negative scores are scaled by
  `clamp(1.2 − 0.6·h, 0, 1)` with `h` the job's difficulty help [D for `(1.2, −0.6, 0)` at `0x002BDBE0`].
- The mean of the last 8 scores (kept within −1.5..1, smoothed) drives the crowd's 3 reaction levels and the job's
  script; each score raises task event 54 [R].

## 6. The combat car battle (class 77)

RC Combat Club (row 33) [D]: the player's car and the computer-driven cars are copies of one wheeled vehicle, `robot_tank`
(object 39009, the only class-77 record) [D].

### 6.1 Data [D]

- **Wheels**: option 1419, six entries (transform ID, radius, suspension range): radius **20**, range **−10 to 5**,
  transforms 39024, 39022, 39020, 39018, 39016, 39014.
- **Weapons**: option 1420, two entries of four object references (weapon index, state machine, aim part, anchor).
- **Steering side**: option 1421, transform 27891, range −5 to 5.
- Engine sound option 1434 = 425.

### 6.2 Behaviour [R]

- **Health**: maximum option 1426 (default 100); a hit lowers it by the hit's damage; at 0 the car is defeated
  (mode 3) and raises its event 5, which the job's script turns into the task result. A HUD bar (option 1428) shows
  the fraction, optionally inverted (1429). Mode changes play state-machine goals 115 to 118 [D for the table at
  `0x002C2208`].
- **Ammunition**: three kinds, each up to 30. Ammo pickups (class 108; kind option 1435, amount 1436, default 30)
  refill a kind on contact, capped at the maximum, then turn to "collected" (goals 119 to 121 [D at `0x002C2248`]).
  A weapon fires only when its kind has at least its cost.
- **Player control**: the stick's camera-relative heading steers; holding **Cross** drives at the maximum speed
  (default 500); weapons fire on their held or pressed buttons.
- **Computer control** (states): 0 approach (with a ready weapon: drive to its range, circling at angle
  `acos(r / max(1.25·r, d))` for tactics 2 and 3 [D for 1.25 at `0x002C20E8`]), 1 attack (stop, turn, fire when the
  aim error is under 0.3 rad in pan and pitch; next attack after a random 1 to 1.5 s), 2 fetch an ammo pickup
  (pickups scored by distance, preferring the one already chosen by `−200 + 0.8·d` [D at `0x002C20C0`]), 3 retreat
  at 0.85 × maximum speed.
- **Contacts**: no friction against other cars or slopes steeper than normal z 0.7, otherwise 0.5; elasticity 0.1.

## 7. Bosses and story characters (classes 59 to 68)

### 7.1 What changed from Sly 1

Sly 1's bosses had fixed phases × hits ([../sly1/minigames.md](../sly1/minigames.md)). In Sly 2 every boss and story
character runs the **shared enemy engine** of the guards ([guards.md](guards.md) 2): its moves are option-243 action
entries, its health is option 254, and its fight is driven by the job's tasks and scripts, which count hits and
switch phases [R]. There is no per-boss phase table in the data [D: no such option or class].

### 7.2 Templates [D]

| Class | Package | Rows stored | Hit points (254) | Block (257) | Near / far range (292 / 293) | Action entries (243) |
|---:|---|---|---:|---:|---|---:|
| 59 | `npc_dimitri` | 2, 5 | 360 | — | 2,200 / 2,500 | 18 |
| 60 | `neela` | 0, 2, 11, 12, 17, 22, 30, 39, 43 | 8,000 | — | — | 5 |
| 61 | `npc_rajan` | 11, 12, 13 | 1,600 | 50 | — | 19 |
| 62 | `npc_jean_bison` | 11, 28, 30, 32, 35, 37 | 900,000 | 500 | 450 / 550 | 10 |
| 63 | `npc_carmelita` | 0, 11, 12, 22, 27, 38, 43 | 999,999,999 | 999,999,999 | 350 / 500 | 18 |
| 64 | `npc_boss_contessa` | 11, 12, 14, 15, 17, 22 | 280 | 40 | 2,200 / 2,500 | 19 |
| 65 | `npc_arpeggio` | 11, 39 | 8,000 | 20 | — | 14 |
| 66 | `npc_bear` | 27, 32, 36 | 10,000 | — | 500 / 600 | 15 |
| 67 | `npc_bear_cub` | 27 | 100 | 10 | — | 18 |
| 68 | `dancer` | 11 | — | — | — | 0 |

- Option 257 is a **block** capacity: hits are absorbed by the block before they reach the hit points (the
  remaining block is option 258) [R].
- Hit points so large that ordinary attacks cannot end the fight (Carmelita, Jean Bison) mark fights won by other
  means: Bison's job counts trap hits through the remaining-hit-points option 255 and advances its tasks on those [R].
- Option 35 (gravity): (0, 0, −4000) on Dimitri, Neela and the Contessa; (0, 0, −1750) on the others that set it [D].

### 7.3 The final boss

The `m8_boss` job runs in row 43 (`a_blimp_boss_fi…`), which holds Neela (60) and Carmelita (63) templates but no
class of 56 to 68 named for the opponent [D]. How the final opponent is built (its class and fight rules) is open.

### 7.4 Boss attacks, windows and how a fight moves on (2026-11-18 findings)

**Attacks.** A boss strike works like a guard's ([guards.md](guards.md) 6.3): a collision part with a contact kind and
value, switched on and off by events 15/16 in the attack clip [R for the mechanism, D for the data]. Parts and windows
found [D]:

| Boss | Clip (length) | Part on from … to | Part tag: contact kind, value |
|---|---|---|---|
| Dimitri (59) | 123819 (2.0833 s) | 1.0 … 1.25 s | 56221: `0x200`, −10 |
| | 123820 (1.5 s) | 0.7 … 0.9167 s | 59482: `0x200`, −10 |
| | 9183 and 422 (1.6167 s) | 0.8333 s … end | 9086: `0x200`, −10 |
| Rajan (61) | 95611 (2.6667 s) | 1.5 … 1.75 s | 95609: `0x400` (crush), −10 |
| | 95613 (3.3333 s) | 2.5833 … 2.6667 s | 95607: `0x400`, −10 |
| | 417 (2.5 s) | 0.75 … 1.0 and 1.5 … 1.6667 s | 56221: `0x200`, −10, then 95607 |
| Contessa (64) | 65630 (2.3333 s) | 0.8333 … 1.6667 s | 19869: `0x400`, −10 and 65518: `0x200`, −10 together |
| | 422 (1.6667 s) | 1.3333 … 1.5 s | 9086: `0x200`, −10 |
| | 65634 (1.5 s) | 0.6667 … 1.1667 s | 19869: `0x400`, −10 |
| bear (66) | 34485 (1.3333 s) | 0.5 … 1.0 s | 34477: `0x200`, −60 |
| | 17901 (1.3333 s) | 0.5 … 1.0 s | 19869: `0x200`, −20 |
| Neela (60) | 1496 (0.6667 s) | 0.35 … 0.4167 s | 8495: `0x2`, −8000 |
| Carmelita (63) | 13179 (1.6667 s) | 0.3333 … 1.0 s | 13056: `0x200`, −10 |

Dimitri also carries parts that none of his own clips switches: tags 59478 and 19869 (`0x400`), 95248 (`0x1000`
stun), 95246 (`0x200`) and 19919 (`0x800` stun), all −10 [D]; they are switched by other packages' clips or scripts
[I]. Jean Bison's parts are −15 and −10 (`0x200`) and −20 (`0x400`, radius 590) [D]; the clips that switch them were not
located. What each amount costs the player: [characters.md](characters.md) 2.1.

**How a fight moves on.** The data holds no phase table [D]. A fight is staged by the boss job's tasks and scripts
([missions.md](missions.md), [scripts.md](scripts.md)) on top of these boss options [R]:

- **Hit points** (254, read back and set through 255) and **block** (256/257 capacity, 258 remaining). A hit first
  wears down the block, playing the boss's block clip while it holds; then it lowers the hit points; at 0 the boss
  is knocked out and the tasks hear task event 23 or 24 ([guards.md](guards.md) 7) [R].
- **Busy** (231 or 1350): while set, hits are not offered to the boss at all; cutscenes set it so scripted blows do
  not restart the fight [R].
- **Escape records** (1340, four object IDs each): places the boss retreats through between stages. Only Dimitri
  carries them on the disc, 4 records [D for the count, R for the meaning].
- **Scripted stages:** scripts read the remaining hit points (255) and move the job's tasks on at their own
  thresholds; for Jean Bison the job counts trap hits through 255 and advances through its later tasks (3282 to 3285)
  [R]. The thresholds live in the scripts, not in option values [D: no boss carries a threshold option].

### 7.5 Jean Bison's fight: attack windows, stages and hit counting (2026-11-18 findings)

**Where.** Bison's package (`npc_boss_canada`; the class-62 object 32250 with option 254 = 900,000 hit points and
257 = 500 block) is stored in rows 11, 28, 30, 32, 35 and 37 [D]. The fight is row 37 (`c_sawmill_boss_pz`), run by
module 165 of the sawmill macro package; there his placement is object 32248 [D] (a placement ID names the copy's
root, scripts.md 5).

**His attacks** [D]. He has two actions (option 233): **19643 → clip 32389** (2.0 s) and **32373 → clip 32387**
(0.6667 s) (guards.md 6.4). Clip 32389 switches his part 32280 (`0x400` crush, −20, radius 590) on from 0.1667 s to
1.5 s and his part 32276 (`0x200` knock-back, −15) on from 0.8333 s to 1.3333 s. His part 32278 (`0x200`, −10) is
switched by no clip on the disc. His own module turns parts 32280, 32278 and 32270 off whenever he raises event 14,
through option 47 (disable a collision part by its tag; executable routine `0x100EB0`) [D for the module, I for when
event 14 is raised].

**When each attack may be used** [D for the scripts; I for the event meanings]. Before using an action Bison raises
event 24 (object, action), and its answer allows or refuses the action:

- action **19643** is always allowed;
- action **32373** is refused until the fight's **12th counted hit**; after that it is allowed at most once per
  random interval: each use sets the next allowed time to now plus a random time between the two ends of a range,
  set to **2.5 … 3.5 s** when the job begins (Bison's custom property 1464).

**Hit counting** (option 255, script ID 381: Bison's remaining hit points) [D for the script; I for the event]:
whenever Bison raises event 11 (object, attacker, value; raised when he is hit), the job script counts the hit only
if the attacker is **not Bentley** (object 8) **and** his remaining hit points went down since the last counted hit.
900,000 hit points do not run out in play, so the count, not the hit points, ends the fight. On counted hits:

| Count | What happens |
|---:|---|
| 4 | after 1 s, if Bentley's current health (option 727) is above 0, task 3282 is requested current |
| 8 | a repeating 2 s call, started by the job's task changes, is cancelled |
| 12 | after 1 s, if Bentley's health is above 0, task 3283 is requested current; action 32373 becomes available |
| 16 | after 1 s, task 3284, same condition |
| 20 | the repeating call is cancelled; after 1 s, task 3285, same condition |

After every counted hit the fight's gauge (screen object 3275, option 982) is set to (20 − count) / 20 [D for the
value, I that it is the boss bar]. The fight is therefore **20 counted hits** in stages at 4, 12, 16 and 20. Hits by
Bentley never count; which hits reach Bison at all is decided by his block and busy options (7.4) and by the scripts
of the stage tasks [R] [I].

## 8. Test vectors [D]

- Combat car: row 33, class 77, object 39009, body at decompressed offset `0x68DEA3`; first wheel entry bytes
  `06 00 70 98 00 00 01 00 00 00 a0 41 0a 00 00 00 20 c1 00 00 a0 40` (int 39024, float 20.0, range −10 to 5).
- Hack ship: row 14, object 17344 at `0x50A9A3`; options 1363 = 100, 1371 = (28012, 28008), 1373 = 28006,
  1374 = 355. Enemy 27921 at `0x5386D3`: 1363 = 5, 1365 = 10, 1376 = 1.0, 1377 = 0.5. Turret 44800: 1363 = 30.
  Generator 27920: 1363 = 50. Gunship 27887: 1363 = 30, 1365 = 10, 1369 = (0, 20, 0), 1370 = (0, 1000).
- Turret: row 8, object 28548 (nested) at `0x8DBF7E`: 1000 = (−0.75, 1.4), 1001 = (−1.5, 3.0), 1010 = (1.0, 1.0).
- RC chopper: object 50723, rows 8 (`0x9396B3`), 14 (`0x29BD03`), 29 (`0x382033`), 30 (`0x5FCE3`), properties as in 2.
- Executable constants (after the hash check): `0x002BE7D0` (300, 50, 0, 0); `0x002BE7E0` (0, 800); `0x002BE7E8`
  1000; `0x002BE800` (25, 0, 0.25); `0x002BE810` (0, 5, 0); `0x002BE870` (25, 0, 0.25); `0x002BE87C` 5;
  `0x002BE880` 50; `0x002BE8AC` 30 (integer); `0x002C4400` (0.1, 1.0); `0x002C4408` (5, 0, 0.5); `0x002C4420`
  (2, 0, −1); `0x002C2208` (115, 116, 117, 118); `0x002C2248` (119, 120, 121); `0x002C20C0` (−200, 0.8, 0);
  `0x002C20E8` 1.25; `0x002BDBE0` (1.2, −0.6, 0).

## 9. Open questions

1. The meaning of the RC chopper's custom properties and how its script flies it.
2. What `powerchop_player` (rows 32 to 37) and `attract_mode_player` (row 0) are used for.
3. The final boss's class and fight rules (7.3).
4. Each boss fight's phase rules (in their scripts), and the damage the gang's attacks deal against the hit points
   and blocks above.
5. The grapple turrets' behaviour in the hubs.
6. The dance director's options (class 121) on the disc; the window and score values above are the reference's
   defaults.
7. Everything [R] needs checking in play.
