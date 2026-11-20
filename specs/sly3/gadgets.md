---
id: sly3-behaviour-gadgets
title: Sly 3 gadgets and moves - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), all 39 worlds walked; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: the item table at 0x343098 (usage-text field), the slot table, the gadget objects carried by the gang (classes 86, 113, 114, 115, 119, 124) and their options; usage strings from the Venice hub string table
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/gadget.cpp, shop.cpp, hudwidget.cpp and game/gen (class and option correspondence tables)
    used_for: >-
      the Sly 2 to Sly 3 correspondence of classes and options (rows the reference scores 80 or more), and the
      statement that Sly 3 runs Sly 2's behaviour for these classes; marked [R]
  - kind: executable
    what: >-
      SCUS_974.64 (SHA-256 ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338) machine code read
      on 2026-11-18: the three characters' item-start routines and jump tables, Sly's per-frame update and its
      rocket-run, treasure-map and disguise branches, Murray's ball mode, ball update and ball contact routine,
      Bentley's fishing-pole controller and the Grapple-Cam controller entry; constants read at the addresses given
    used_for: section 6 (the [X] statements); what a routine is for is marked [I] where it is an interpretation
  - kind: executable
    what: >-
      SCUS_974.64 (same hash) re-read on 2026-11-18: Sly's state table 0x322550 and state-19 path, the slot routine
      0x1ADBE0, the item-permission routine 0x1AF308, the costume map 0x173560, all readers of actor +4360, and the
      start of Murray's ball update 0x1E3488
    used_for: section 6.9
  - kind: disc
    what: Sly 3 USA world row 05 (v_apt), package jt, clips 290-292 and the root-motion clips of Sly, read with a scratch clip reader
    used_for: section 6.9 (the lean clips have no root motion; comparison speeds)
  - kind: disc
    what: Sly 3 USA hub worlds rows 03, 08, 15, 23, 31 (class-153 map objects and class-127 marker factories); executable option-table entries 1901 to 1906 and the Grapple-Cam controller 0x271F78 (same hash)
    used_for: section 6.9 (Grapple-Cam phases, treasure map object)
  - kind: executable
    what: >-
      SCUS_974.64 (same hash) re-read on 2026-11-19: Murray's ball routines 0x1E2A20, 0x1E33E0 and the Square branch
      of 0x1E3488 with the vector helpers 0x11FD70, 0x1205F0 and 0x11E940; Bentley's item-start branch 0x140280 to
      0x140360; every caller of the phase-change helper 0x2734E8; the map object's set-up routine 0x278A48; option
      1905's routine 0x110978 and the list routine 0x279190
    used_for: section 6.10
  - kind: executable
    what: >-
      SCUS_974.64 (same hash) re-read on 2026-11-19: every float load of map fields +616 and +76 near the map code;
      the trail check 0x279568 / 0x2795C0, the map view's per-frame routine 0x279AF8, the look-at helper 0x166E48,
      the heading snap at 0x279A7C and the easing record at 0x3292C0
    used_for: section 6.10.1
  - kind: disc
    what: >-
      Sly 3 USA hub worlds rows 03, 08, 15, 23, 31: the class-153 map objects' option 1905 pairs, the classes and
      matrices of the eight option-1904 objects and of objects 9912 and 4028 (row 03), and the row-03 string table
    used_for: section 6.10 (treasure-map trail)
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 gadgets and moves

Read [../sly2/gadgets.md](../sly2/gadgets.md) first; the item table itself is in [characters.md](characters.md) 3.1.
Tags as in [guards.md](guards.md).

## 1. The item record's usage text [D]

Field +36 of the 60-byte record (characters.md 3.1 called it "an icon id") is the string ID of the item's **usage
text**, as Sly 2's field 7 (offset 28). The strings (Venice hub string table):

| String | Text | Items |
|---:|---|---|
| 286 | Press %s to place. | 16 |
| 287 | Press %s to trigger. | 9, 12, 14, 18, 20 to 25, 29 to 31, 37 to 40 |
| 288 | Usage is automatic. | 19 |
| 290 | Press and hold %s to use. | 7, 32 to 35 |
| 291 | Press %s to aim, %s to throw, then %s to trigger. | 6, 10, 13 |
| 292 | Then finish them off in silence with &2S&.. | 28 |
| 293 | Press &2R&. while jumping to deploy your paraglider. | 27 |
| 294 | Press &2X&. while jumping to deploy your hover pack. | 11 |
| 298 | Press %s to dodge while in combat. … | 26 |
| 299 | Press %s to aim, then %s to throw. | 8, 15 |
| 300 | Press %s to trigger, and &2S&. to speed up. | 17 |
| 301 | Press %s to bring up treasure map. | 36 |

Strings 295 to 297 are untranslated placeholders (high jump, uppercut, carry trigger) that no record names; the
attack upgrades (41 to 49) have −1. `%s` is the button the item is on, as in Sly 2.

The last five words (+40 to +56) are set on 18 records: an ID from 1967 to 1985 and two (`s32`, `s32`) pairs such as
(312, 4, 312, 4) [D]; records 50 to 66 hold only these. *Interpretation:* the ID is the gadget's effect or object and
the pairs its animation or sound entries [I]. Open.

## 2. Buttons, slots and power

- **Slots**: three per character on L1, L2 and R2, button masks `0x4`, `0x1`, `0x2`: the executable's 3-entry table
  at `0x349040` [X] (Sly 2 `0x2C3258`). Assignment, ownership and the refusal rules: Sly 2 section 2 [R].
- **Assignment-screen grids** [X]: three tables of 32-byte cells in Sly 2's cell layout
  ([../sly2/safehouse-and-shop.md](../sly2/safehouse-and-shop.md) 6): `s32` item record, three `s32` screen-part
  IDs, `s32` −1, `f32` x, `f32` y, `s32` flags. They sit just before the slot table. The code at `0x0021FC48` picks
  the screen ID, the cell count and the table address together, one set per character:

  | Address | Cells | Screen | Character | Items, in cell order |
  |---|---:|---:|---|---|
  | `0x348BE0` | 15 | 1062 | Sly | 25, 26, 29, 30, 27, 31, 35, 33, 28, 36, 32, 34, 38, 39, 40 |
  | `0x348DC0` | 11 | 1063 | Bentley | 6, 7, 8, 9, 10, 12, 13, 14, 15, 16, 11 |
  | `0x348F20` | 8 | 1064 | Murray | 17, 18, 20, 21, 22, 23, 24, 19 |

  - The item owners (record +4: 24 Sly, 25 Bentley, 26 Murray) match each grid. Sly's item 37 is on no grid [X].
  - For a cell whose first part ID is n + 1, the three part IDs are (n + 1, n, n + 2). In Sly 2's order these are
    the open, owned and selected parts. Sly's first cell holds (1174, 1173, 1175); the IDs run from 1116 (Bentley)
    to 1217 (Sly).
  - The flags are 1 exactly when the item's button use (record +0) is not −1. The four flag-0 cells are items 27,
    28, 11 and 19, the passive ones.
  - Positions, as x values by row: Sly, three rows of five, x 165, 255, 345, 435, 525 at y 155, 235, 315. Bentley:
    x 175, 285, 395, 505 at y 155 and 235, then 228, 333, 443 at y 315. Murray: x 185, 325, 465 at y 155 and 235,
    then 265, 385 at y 315.
- **Power**: Sly 2 section 3 [R] (maximum 100; cost taken once from record field +12). The **held** items (button
  use 1) of Sly 3 are 32 to 36: Shadow Power 1, Thief Reflexes, Shadow Power 2, Rocket Boots, Treasure Map [D].
- **Drain of held items** [X]: field +16 is the drain in power per second, stored as an integer. The routine at
  `0x00154608` takes the item's record, converts +16 to a float and passes it to `0x001F4498`, which adds drain × a
  clock step (the float at `0x0036BB9C`, *interpretation:* the frame's step [I]) to a fractional accumulator (actor
  +380) and takes its whole part off the power (actor +372, clamped to 0 … the maximum at +376); when the power is
  then 0 or less, the item stops. Values: Shadow Power 1 = 3, Thief Reflexes = 2, Shadow Power 2 = 3, Rocket Boots
  = 5 per second; the Treasure Map (36) has 0, so it is held without cost. The entry cost +12 of items 32 to 35 is 1.
- **Fishing Pole (7)**: its record says button use **2 (press)**, power 0, drain 0 [D]; its usage text is string 290
  ("Press and hold %s to use."), shared with the held items 32 to 35. The slot code reads the record, so the pole
  is started by a press and costs nothing; *interpretation:* the text describes keeping the button down while the
  line is out, which the pole's own action would test [I]. Open until its action code is read.
- Items with button use −1 work whenever owned: Hover Pack (11), Juggernaut Throw (19), Paraglide (27), Silent
  Obliteration (28), the attack upgrades 41 to 49 [D].

## 3. The gang's gadget objects [D]

| Class | Group | Records | Used by (item) |
|---:|---|---|---|
| 86 | `bentley_webcam` | 34 (one per gang world) | Grapple-Cam (13) |
| 124 | `bentley_webcam` | 34 (nested) | its camera part |
| 119 | `bentley_webcam`, `rc_chopper`, … | 38 | remote-controlled bodies |
| 113 | `bentley_webcam_tossable`, `alarm_clock` | 68 | thrown items (Alarm Clock 8, the tossed camera) |
| 114 | `bentley_mine` | 34 | Bentley's bombs (Trigger Bomb 6 and the others) |
| 115 | `guard_trap` | 34 | Health Extractor (10) |

Thrown items (class 113):

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 32 | 68 | 34 | 1 ×68 | 32 |
| 1197 | 68 | 34 | 6611 ×34; 6841 ×34 | 1102 |
| 1199 | 34 | 34 | 13 ×34 | — |

Bombs (class 114):

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 32 | 34 | 34 | 1 ×34 | 32 |
| 53 | 34 | 34 | list (67108863, 2) ×34 | 53 |
| 1197 | 34 | 34 | 610 ×34 | 1102 |
| 1203 | 34 | 34 | 20 ×34 | — |

Health Extractor (class 115):

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 32 | 34 | 34 | 1 ×34 | 32 |
| 53 | 34 | 34 | list (67108863, 2) ×34 | 53 |
| 1197 | 34 | 34 | 6947 ×34 | 1102 |
| 1209 | 34 | 34 | 6930 ×34 | 1113 |

The Grapple-Cam (class 86) carries 17 parts of its own (1864 to 1880) and 18 animation entries (1877) [D]:

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 1877 | 612 | 34 | list (3392) ×34; list (3393) ×34; list (3394) ×34 | — |
| 1879 | 170 | 34 | list (2, 12, 0, 1.0, 0) ×34; list (4, 13, 0, 1.0, 0) ×34; list (2, 226, 1, 1.0, 0.5) ×34 | — |
| 32 | 34 | 34 | 1 ×34 | 32 |
| 1864 | 34 | 34 | 6676 ×34 | — |
| 1865 | 34 | 34 | 6627 ×34 | — |
| 1866 | 34 | 34 | 6701 ×34 | — |
| 1867 | 34 | 34 | 6655 ×34 | — |
| 1868 | 34 | 34 | 6653 ×34 | — |
| 1869 | 34 | 34 | 6642 ×34 | — |
| 1870 | 34 | 34 | 6635 ×34 | — |
| 1871 | 34 | 34 | 6651 ×34 | — |
| 1872 | 34 | 34 | 6674 ×34 | — |
| 1873 | 34 | 34 | 6658 ×34 | — |
| 1874 | 34 | 34 | e100 ×34 | — |
| 1875 | 34 | 34 | 6672 ×34 | — |
| 1880 | 34 | 34 | 6624 ×34 | — |

1197 (Sly 2's 1102) names each gadget's effect object, 1203 = 20 on the bomb (a radius or damage [I]); 1209 (Sly 2's
1113) is the extractor's capture part [R]. Detonation, blast and capture follow Sly 2 section 7 [R].

## 4. New items in Sly 3 [D] [O]

| Item | What it does [O] |
|---|---|
| Fishing Pole (7, Bentley, press) | takes a pocket item from a guard at range |
| Grapple-Cam (13, Bentley) | a remote camera that flies to and grabs things; class 86 |
| Be The Ball (17, Murray) | Murray rolls as a ball; Square speeds up (string 300) |
| Rocket Boots (35, Sly, held) | fast movement |
| Shadow Power 1 and 2 (32, 34, Sly, held) | invisibility, then attacking while invisible |
| Treasure Map (36, Sly) | shows a hub's treasure trail ([pickpocket-and-pickups.md](pickpocket-and-pickups.md) 7) |
| Disguises (38 to 40, Sly) | fool guards in Venice, China and the pirate hub |
| Spin, Jump and Push Attack levels (41 to 49, Sly) | combo finishers ([characters.md](characters.md) 5) |

## 5. Open questions

1. The +40 to +56 fields.
2. The Grapple-Cam's parts and controls; the disguises' effect on guard sight; Rocket Boots' speed factor; Be The
   Ball's speed and Square boost; the Fishing Pole's reach and whether it needs the button held; the Treasure Map's
   trail rules. None is in the item record, and the reference does not implement Sly 3's versions; each needs its
   action code in the executable or observation.
3. Bentley's chair melee (Insanity Strike 12, Size Destabilizer 14): both are press items on a gadget slot (button
   use 2, power 10 and 5) [D]; how the chair hit is made (animation, reach, effect on the guard) is open.
3. Every [R] rule needs checking in play.

## 6. Executable findings: item actions [X]

Read from the machine code of SCUS_974.64 (hash as in the front matter). Item numbers are those of the item table
([characters.md](characters.md) 3.1). Addresses are virtual. "State" is the character's own move state (Sly: the
word at actor +6624, its sub-state at +6632; Murray and Bentley: the word at +5408).

### 6.1 How an item starts

Each character has its own item-start routine, reached through the character's dispatch table (the entry at
+916; stopping an item uses the entry at +932) with the item number as argument [X]. Each routine switches on the
item number with a jump table; items outside its range fall to the generic start [X].

| Character | Start routine | Jump table | Items covered | Stop routine |
|---|---|---|---|---|
| Sly | `0x001AF6C0` | `0x00322A60` | 25 to 40 | `0x001AF990` |
| Murray | `0x001E25B0` | `0x003249A0` | 17 to 24 | generic `0x001546B0` |
| Bentley | `0x00140128` | `0x0031FF20` | 4 to 16 | `0x001403F8` |

What the items asked about do on start [X]:

| Item | What starting it does |
|---|---|
| Rocket Boots (35, Sly) | puts Sly in state 19, sub-state 6 |
| Treasure Map (36, Sly) | looks up the hub's map object (stored at +7968), then state 24, sub-state 8 |
| Disguises (38 to 40, Sly) | one shared branch: if a costume is on, takes it off; otherwise puts on costume part 1881 (38), 1882 (39) or 1883 (40) through Sly's attachment controller (+4360) |
| Fishing Pole (7, Bentley) | Bentley state 16; its per-frame work is the pole controller at `0x0017EBA0` (actor +6192) |
| Grapple-Cam (13, Bentley) | spawns the camera body from the item's template record (returned by `0x001400C0`), then Bentley state 14; per-frame work is the camera controller at `0x00271F78` (actor +4752), with 8 phases (jump table `0x00329030`) |
| Be The Ball (17, Murray) | Murray state 22; pressing the item again while in state 22 sets the ball mode to 2 (leaving the ball), see 6.4 |

### 6.2 Rocket Boots (35)

- **Start-up** [X]: sub-state 6 holds until at least **0.07 s** (`0x3D8F5C29`, an immediate) have passed and Sly's
  heading is within **0.1 rad** (`0x3DCCCCCD`) of the stick's direction; then sub-state 7, the rocket run.
- **Ending** [X]: the run ends when the power reaches 0 (drain 5 per second, section 2), and when Sly meets a wall.
  Each frame every contact whose surface normal has |z| < 0.7 is tested; with c the cosine between Sly's facing and
  the contact, the run stops when c is below q(u) = −0.6 − 0.3·u², clamped to −0.9 … −0.6, where u is a 0–1 factor
  returned by `0x00171C98`. The coefficients are the floats at `0x00322B30` (−0.6, 0, −0.3) and the clamp is at
  `0x00322B40` (−0.9, −0.6). *Interpretation:* a head-on hit stops the run, a glancing one does not [I].
- **In the run** [X]: the camera's look height is lowered by 75 (`0x42960000`); a lean value is eased each frame from
  the turn rate (scale 0.08, `0x3DA3D70A`; factor 0.8, `0x3F4CCCCD`; easing record at `0x003225E0`) and written to
  three blend weights (+7876, +7888, +7900). *Interpretation:* these drive a left/right lean of the run clip [I].
- **Speed factor: not found as a constant.** Sly's state-19 animation is the clip in his animation slot 19 (the slot
  list at +6224, read by `0x001ADBE0`) [X]. *Interpretation:* the run speed is that clip's root motion, as for Sly 2's
  runs (../sly2/characters.md 1.1) [I]. Open until that clip's travel is measured.

### 6.3 Treasure Map (36)

- **Holding** [X]: state 24 lasts while the item's button stays down (the button mask for item 36, from
  `0x00154798`, tested against the pad's held bits). When the button is up and at least **0.5 s** (`0x3F000000`) have
  passed since entering the state, Sly returns to state 0.
- **Steering** [X]: in sub-state 8, while the stick is pushed forward (pad +8 > 0), Sly's heading is set to the
  camera-relative stick heading **snapped to n directions**, n being the integer at +644 of the map object (the
  snapping routine `0x0011E438` divides the circle into n equal sectors). *Interpretation:* the map makes Sly face
  along the trail in fixed steps [I].
- The trail itself (the class-127 map and the trail objects) and how it is drawn remain open.

### 6.4 Be The Ball (17)

Ball mode is the word at Murray +6868: −1 off, 0 rolling, 1 after a hard hit, 2 leaving [X].

- **Entering** (from −1) [X]: the body's collision radius is set to **75** (`0x42960000`); the physics object switches
  to its second simulation kind (+440 = 2); a gravity-related value is saved and replaced by −1.0; Murray is lifted
  by **300** (`0x43960000`) when a flag at +536 is set, otherwise placed **600** (`0x44160000`) up [I for which case
  is which]; a speed cap at +6940 starts at the largest negative float (`0x00324A28`).
- **Square boost** [X]: in modes 0 and 1, a Square press (pad mask 128, this frame) records the time (+6932) and
  applies an impulse through the body. Its axis is Murray's heading + π/2 (`0x3FC90FDB`), its size the stick's
  forward value × **14** (`0x41600000`), scaled by **4** (`0x40800000`); sound 95 plays at volume 1.0 and pitch
  offset −0.7 (`0xBF333333`). *Interpretation:* a forward roll kick of 56 × stick-forward [I].
- **Jump** [X]: in mode 0, Cross (pad mask 64) within 0.25 s (`0x3E800000`), once at least 0.15 s (`0x3E19999A`)
  have passed since +6920, launches with vertical speed √(g × −630) (`0xC41D8000`, g the body's gravity z).
  *Interpretation:* a jump to height 315 [I].
- **Hard hits** [X]: a contact faster than **500** (`0x43FA0000`) units/s switches to mode 1 and plays one of two
  impact sounds (the IDs at +6896 and +6900), the heavier one above **1,350** (`0x44A8C000`).
- **Rolling speed: open.** The ball update (`0x001E3488`, 882 instructions) holds further constants as immediates
  (for example 1,000, 850, −2,000, 562, 1.5, 0.75, 400 and 5,000) whose roles are not yet traced [X for the values].

### 6.5 Fishing Pole (7)

The controller keeps a **phase** (+12, 0 to 6; jump table `0x003217C0`) and a **hook state** (+20) [X]:

- **Hold behaviour** [X]: in phases 1, 2, 4 and 5 the pole stays out while the item's button is held (the button
  mask for item 7); releasing it moves to phase 6, which ends the move (Bentley's movement machine goes to state 11
  when the end animation is done). The usage text "Press and hold" is therefore literal: holding keeps the line out.
  Phases 2 and 5 are the airborne variants of 1 and 4, chosen when the time since the actor's +540 time stamp is at
  least **0.1 s** (`0x002503C8`) [X], [I for "off the ground"].
- **Reach** [X]: in hook state 0 (phase 1), a target within **250** (`0x437A0000`) units of the pole tip (the tip part
  of Bentley's held object, +4532 → +1196) becomes hooked (hook state 1). After **0.35 s** (`0x3EB33333`) still in
  reach it is held (hook state 2) and the pole goes to phase 4 (reeling); losing the target in hook state 1 resets to
  0. While reeling (phase 4), if the target gets farther than **400** (`0x43C80000`) the line lets go (phase 3, back to
  phase 1 when its animation ends).
- What can be a target (the routine `0x0017E810` takes it from Bentley's +4532 object when +4540 = 3) and what is
  taken from it (a pocket item, as in Sly 2's pickpocketing [R]) need the rest of the controller.

### 6.6 Disguises (38 to 40) and guard sight

- Putting a disguise on and off is the costume swap of 6.1 [X]. The routine that tests "a costume part is on"
  (`0x00173428`: attachment +80 set and +100 clear) is used by all three characters' updates and by no guard code
  [X], so guards do not test the disguise through it. **The effect on guard sight is still open**: no guard routine
  reading Sly's costume was found. Candidates are the guards' sight test ([guards.md](guards.md)) checking an option
  of the costume part, or scripts in the disguise hubs [I].

### 6.7 Grapple-Cam (13)

- Starting it is described in 6.1 [X]. The camera controller (`0x00271F78`) eases the camera between two stored
  points over a timed move (+288, +304, +308, +312), turns with the stick scaled by 1.5 (`0x3FC00000`) and the frame
  step, and switches among 8 phases [X]. **The controls (which button grabs or releases, speeds) are open**; they are
  in that controller's phase handlers.

### 6.8 Still open after this pass

Rocket Boots' speed (6.2), the treasure trail's data rules (6.3), Be The Ball's rolling speed (6.4), the fishing
pole's target rule and what it takes (6.5), the disguises' effect on guard sight (6.6) and the Grapple-Cam's controls
(6.7). The routine addresses above are the starting points.

### 6.9 Second pass (2026-11-18): Rocket Boots, disguises, Be The Ball [X]

Read from the machine code of SCUS_974.64 (hash as in the front matter); "Sly's actor" offsets as in 6.1.

**Rocket Boots (35): where the run's motion comes from.**

- **The run sets no velocity of its own** [X]. In state 19 Sly's per-frame update (state table at `0x00322550`,
  entry 19 → `0x001AC504`) does only this each frame:
  - ends the run when the power counter (read through `0x001F4430`) is 0 or less;
  - when the flag at +536 is set, a Cross press (pad mask 64) within 0.1 s leaves through `0x001B7268` [X]
    (*interpretation:* a jump out of the run [I]);
  - a Circle press (pad mask 32) within 0.2 s leaves through `0x001B5170` [X] (*interpretation:* an attack [I]);
  - otherwise runs the wall test of 6.2 (`0x001B04A0`).

  No routine on the state-19 path writes a speed or a velocity.
- **The animation** [X]: state 19's animation slot (+6224 + 19 × 4 = +6300) is never filled. None of Sly's three
  lookup tables has a record for that field, and no code in Sly's routines stores to it. The slot routine
  (`0x001ADBE0`) therefore falls back to slot 0, the same animation as state 0 (lookup record 60, OID 252).
- **The lean blend** [D] [X]: the three blend weights of 6.2 (+7876, +7888, +7900) sit next to Sly's lookup records 92,
  93 and 94, which bind clips **290, 291 and 292**. In the `jt` package (row 05, v_apt) these are 1.0 s clips with no
  root-motion channel: their only vector channels are constant poses. They lean the body; they do not move it.
- **Result:** the rocket run moves Sly with state 0's animation and the ordinary movement path, plus the lean poses
  [I]. **The speed is not a stored constant** [X], so it must be measured in play or traced through state 0's
  movement. For comparison, Sly's root-motion clips in the same package include 1,000 units/s (OIDs 175–180, 0.5 s)
  and 1,500 units/s (OIDs 312, 313, 0.333 s) [D].

**Disguises (38–40): what a costume changes.**

- **Costume parts** [X]: the item-to-part routine `0x00173560` maps 38 → 1881, 39 → 1882, 40 → 1883, and anything else
  to −1.
- **Items are blocked while a costume is on** [X]. Sly's "may this item start" routine `0x001AF308` checks the costume
  first (`0x00173428` on the attachment controller at +4360). While a costume is on, every item is refused except
  item 3 and the disguises 38–40, so the disguise button can still take the costume off.
- **Guard sight: still not found.** 41 routines read Sly's attachment controller (+4360). Those outside the three
  characters' own code (for example `0x00226BE0`, `0x00229438`, `0x0022B238`, `0x0018BB20`) only attach, detach or
  re-place costume parts. None is part of a sight test [X], so the costume is not tested directly by the guards'
  sight [I]. What remains possible: an option on the costume part that the guards read, or the scripts of the jobs
  that use disguises.

**Be The Ball (17): correction to 6.4.**

- The value at +6940, described in 6.4 as a speed cap, is a **predicted highest point** [X].
  - Each frame it becomes the larger of its old value and z + v_z² / (−2 g), with g the body's gravity z (+136).
  - When the actor is on a support object (+6912), it is also raised to at least 562.5 (`0x440C8000`) above a height
    taken from that support, less the actor's own offset at +616.
  - On landing it restarts from the largest negative float (the value at `0x00324AA0`) [X].
- **Landing rumble:** 0.2 strength (`0x3E4CCCCD`) when the landing speed exceeds 850 or the vertical speed is below
  −2,000 [X]. A 1.0 strength applies (`0x002412C8`) in mode 0 on a support when the speed is between 500 and 1,000
  [X]. *Interpretation:* these are pad-vibration calls [I].
- **The rolling speed is still open.** It is not limited by a constant in `0x001E3488`. The rolling is pushed by the
  Square impulse of 6.4 and by the routines `0x001E2A20` and `0x001E33E0`, which this pass did not trace.

**Grapple-Cam (13): the controller's phases** [X] (controller `0x00271F78`; phase word at +4, phase table `0x00329030`;
phase changes go through `0x002734E8`; the pad is the owner's +308; pad masks as in Sly 2: Triangle 16, Circle 32,
Cross 64, Square 128).

| Phase | Entry | What it does |
|---:|---|---|
| 0 | `0x272164` | Measures the angle between the camera body's heading and the direction to its current target (+16). Beyond +20 (a threshold field) it goes to phase 2, below −(+20) to phase 1; in between it sets two blend weights, w = 0.5 + 0.5·(angle / threshold) at +76 and 1 − w at +64, and plays them |
| 1, 2 | `0x27224C` | Wait for the turn playback (owner +4412) to finish, then back to phase 0 |
| 3 | `0x27226C` | When the playback ends, or when field +200 is set and Cross was pressed within 0.25 s: if the target is the current player and that player is in state 10, go to phase 4. Otherwise, if Sly is in state 20, go to phase 7. Otherwise run the release routine `0x00273278` and return to phase 0 |
| 4, 5, 6 | `0x27230C` | If the target carries option 113 and its +312 is not −1, the owner's exit method is called. Otherwise: **Cross or Square** (mask 192) within 0.1 s → phase 7; **Triangle or Circle** (mask 48) within 0.1 s → release routine, then the owner's exit method when +88 is clear. In phase 4 the same angle test as phase 0 uses threshold +24, giving phase 6 (positive) or 5 (negative) |
| 7 | `0x2724A0` | When the playback ends: back to phase 0 if +88 is set, otherwise the owner's exit method |

- **Release routine** `0x00273278` [X]: acts only when the target is the current player or Sly. It works with a point
  400 units (`0x43C80000`) along an axis and the camera body's position. *Interpretation:* it hands control back to
  the character [I].
- **Stick turning** (6.7): the stick input is scaled by 1.5 (`0x3FC00000`) [X].
- **Still open:** which button starts a grab. Phases 0–2 aim, 3 is the grab, 4–6 hold and 7 is the end
  [I for the names].

**Treasure Map (36): the hub map object** [D] [X]. Each hub with a treasure map has one map object: class 153,
named `v_ext_treasure_map`, `o_ext_treasure_map`, `h_ext_treasure_map`, `c_ext_treasure_map` or `p_ext_treasure_map`
in rows 03, 08, 15, 23 and 31. Its body is an option list [D]. The field each option writes is from the executable's
option table (`0x0032CA80`, 40-byte entries) [X]:

| Option | Value type, field | Value on the disc |
|---:|---|---|
| 1901 | float, +76 | 5,000 (4,000 in row 31) |
| 1903 | float, +616 | 400 |
| 1902 | object, +80 | object 9,912 in every hub |
| 1906 | object, +648 | object 4,028 in every hub |
| 1904 | object appended to a list (count at +84, up to 32 entries of 8 bytes from +88; routine `0x00279160`) | **8 entries** in each hub: eight object IDs |
| 1905 | method (routine `0x00110978`) | argument 16 |

- **Snapping** [X]: the direction count n that 6.3's snapping uses is the map object's +644. The snapping routine
  `0x0011E438` divides the circle into n sectors of 2π/n, and n = 0 or 1 disables snapping.
- **Map markers** [D]: the class-127 dot and X marker factories, objects 46833 and 46829, are the same in all five
  hubs.
- **Still open** [I]: what the two distances (5,000 and 400) bound, presumably a trail range and a dot spacing; what
  the eight listed objects are, presumably the trail's way points or the treasure sites; and how +644 is filled
  (no option writes it).

**Still open after this pass:** the button that starts a Grapple-Cam grab; what the treasure map's distances and its
eight listed objects mean, and how +644 is filled; Be The Ball's rolling (`0x001E2A20`, `0x001E33E0`, not traced);
Rocket Boots' speed; and the costume's effect on guards.

### 6.10 Third pass (2026-11-19): Be The Ball rolling, Grapple-Cam buttons, treasure-map trail [X] [D]

**Be The Ball (17): how the ball rolls.**

- **The roll is a spin, not a speed** [X]. The player drives Murray's ball only through its angular velocity; no
  routine on the ball path sets a linear speed or caps it.
- **Square, the push** (in `0x001E3488`, pad mask 128) [X]:
  - target spin = a horizontal vector of length **14 × stick magnitude** (`0x41600000`) about the axis at the
    body's heading + π/2 (`0x3FC90FDB`);
  - the change from the current spin toward that target is limited to a length of **4** (`0x40800000`, through the
    length-clamp routine `0x001205F0`) and added to the current spin;
  - then sound 95 starts (pitch −0.7, as in 6.4).

  So each press moves the spin by at most 4 toward 14 × stick about the sideways axis [X]. This corrects 6.4's
  reading "14 × stick-forward × 4": the 4 is a limit on the change, not a factor. Units: the values feed the body's
  angular velocity, so radians per second [I].
- **Rolling speed** [I]: rolling without slipping on the 75-unit collision sphere of 6.4, a full-stick target spin
  of 14 rad/s gives about **1,050 units/s**; reaching it from rest takes four presses (4 + 4 + 4 + 2). Slopes,
  gravity and contacts add to or take from this through the ordinary physics.
- **Random spin** (`0x001E33E0`) [X]: adds to the current spin a horizontal vector with a random angle between −π
  and π and a random length between **1.5 and 2.5** (`0x00324A30`, `0x00324A34`). It runs on entering the ball and at
  one further point of the update. *Interpretation:* a tumble when the ball starts or is knocked [I].
- **Contact routine** (`0x001E2A20`) [X]: takes the ball and a second object, measures the offset between them,
  passes it to the second object's push method, then starts a hit effect on it (`0x001FE728`, argument 1). It is the
  ball hitting something, not part of the rolling.
- **Smoothed velocity** [X]: the ball keeps a filtered copy of its velocity at +6976 that follows the real velocity,
  changing by at most **20,000** units/s per second (`0x469C4000` × the frame step). That is the only write to the
  field; it does not drive the motion [I].

**Grapple-Cam (13): what each button does** [X].

- **Deploying is the item's own button.** Bentley's item-start branch (`0x00140280`): when no camera body exists,
  it creates one (`0x001FBE30`), stores it at +4768, puts Bentley in state 14 (`0x0013E510`) and sets his
  controller (at +4752, the controller of 6.9) to **phase 4** at once. Phase 4 is where the player controls the
  camera; phases 0 to 3 are not part of starting it.
- **Pressing the item button again** while the controller is in phase 4, 5 or 6 sets **phase 7** (`0x00140320`),
  the same end as the Cross or Square press below.
- **In control (phases 4 to 6)**, pad masks as in 6.9:
  - **Cross or Square** (mask 192) within 0.1 s: **phase 7**, which plays its animation and then, when +88 is
    clear, ends through the owner's exit method;
  - **Triangle or Circle** (mask 48) within 0.1 s: the release routine (`0x00273278`), then the exit method.

  *Interpretation* [I]: Cross or Square is the grab (phase 7 plays the grab animation before the camera
  returns) and Triangle or Circle cancels. There is no separate "start a grab" press before phase 7.
- **Phases 0 to 3 serve other users of the same controller** [X]: Sly's actor carries one at +6368 and sets
  phase 3 from his own code (`0x001B9444`). Phase 3's Cross check (mask 64, 0.25 s, field +200) is that user's, not
  the Grapple-Cam's.

**Treasure Map (36): the trail** [D] [X].

- **n = 8** [X]: the map object's set-up routine (`0x00278A48`) writes **8** to +644 (and 5,000 to +76, **500** to
  +616, 54 to +656, −1 to +64). No option writes +644, so every treasure map snaps to **eight directions** (45°).
  Option 1903 then overwrites +616 with 400 on the disc (6.9).
- **Option 1902** (object at +80) is object 9912, a class-158 screen object (`stock_swf`, the shared overlay
  screens) [D]. **Option 1906** (object at +648) is object 4028, a **class-135 command script** in the map's own
  package [D]. *Interpretation:* the screen that draws the map, and the script that runs its sequence [I].
- **Option 1904: the trail markers** [D]. The eight listed objects are class-3 placed markers (56-byte bodies with
  a rotation and a position). Row 03, in list order:

  | Marker | Position (x, y, z) | Facing | Leg to the next marker |
  |---:|---|---:|---|
  | 158661 | (−3306, 4350, 100) | −90° | 800 units at −90° |
  | 158659 | (−3306, 3550, 91) | −135° | 2,800 at −135° |
  | 158657 | (−5286, 1570, 91) | 135° | 3,200 at 135° |
  | 158655 | (−7548, 3833, 92) | 90° | 1,800 at 90° |
  | 158653 | (−7548, 5633, 91) | 0° | 1,200 at 0° |
  | 158651 | (−6348, 5633, 91) | 45° | 2,200 at 45° |
  | 158649 | (−4793, 7188, 112) | 135° | 1,200 at 135° |
  | 158647 | (−5641, 8037, 357) | — | end of the trail |

  Every leg runs along one of the eight snap directions, and each marker faces along the leg that leaves it [D].
- **Option 1905: the clues** [D] [X]. Its routine (`0x00110978`, then `0x00279190`) appends a pair to a second list
  on the map (count at +344, up to 32; first values from +348, second from +476). Each hub has **eight** pairs, and
  both values are **string IDs** in the hub's string table: a riddle and a plain hint (−1 = none) [D].
  - Row 03: (1650, −1), (1651, 1652), (1653, 1654), …, (1663, 1664). For example 1653 asks the player to look
    down an alley and take 14 paces; 1654 says the same plainly [D].
  - Rows 08, 15 and 23 have the same shape (2264–2278, 2764–2778, 3301–3315). Row 31 (4282–4295) has riddles only,
    every plain hint −1 [D].
- **One pace is 200 units** [D]. In row 03 the clues' pace counts for the seven legs are 4, 14, 16, 9, 6, 11 and
  6, and the leg lengths above are exactly 200 times those counts.
- **The trail** [D] [I]: start at the first marker; for each clue, walk its pace count in its snapped direction to
  reach the next marker; the eighth marker ends the trail. How the game checks the player against a leg is still
  open: +616 (400) and +76 (5,000) are the candidates, as a tolerance and a display range [I].

**Still open after this pass:** what +616 (400) and +76 (5,000) do in checking the trail; what the Grapple-Cam's
phase-7 animation does to the target; Rocket Boots' speed; the costume's effect on guards.

#### 6.10.1 What options 1903 (+616) and 1901 (+76) do [X] [I]

- **Option 1903 (+616, 400 on every map): the arrival radius of the current marker, not a pace length.** The
  routine `0x002795C0`, reached through `0x00279568`, is the only reader of +616 in the map code [X]. It runs only
  while the map is active (+664 set) and the current clue index (+632) is below the marker count (+84), and only for
  the marker whose entry matches the stored current marker (+636 against the entry's second word) [X]. It takes the
  position of the object at map +72 (the player [I]) and the current marker's position (the translation of its
  world transform, offset 48 of what the marker's position method returns), and computes their **full 3-D
  distance** (the squares of x, y and z are summed, then a square root; no axis is dropped) [X]. It returns true
  when that distance is **at most +616** [X]. So the player reaches a clue's marker by coming within 400 units of
  it, in any direction [X for the rule, I for "reaches"].
- **Relation to the pace** [D] [I]: 400 is two paces. The legs in row 03 are 800 to 3,200 units long (4 to 16
  paces), so the radius is at most half the shortest leg (800) and neighbouring markers' circles at most touch [D]. The default
  written by the set-up routine is 500 (2.5 paces) [X]; every map on the disc overrides it with 400 [D].
- **Option 1901 (+76, 5,000; 4,000 in row 31): the height of the overhead map view.** It is read only by the map
  view's per-frame routine `0x00279AF8`, an object whose +32 points at the map [X]:
  1. If the player's stick is pushed, the stick's direction (as an angle, wrapped) and **half its magnitude times
     +76** give a horizontal offset, so full stick pans the view by up to 2,500 units (2,000 in row 31) [X].
  2. That offset is eased each frame with the record at `0x003292C0` = (1,500, 700, 0.5) [X]; released, it eases
     back toward zero [I].
  3. The look-at point is the player's position plus the eased offset [X].
  4. The view's object is placed **+76 straight above** that point, (0, 0, +76) added, and turned to look straight
     down at it, by the look-at helper `0x00166E48` [X].
  So +76 is a camera height in units, not a trail range [X]. The heading snap to n = 8 sectors (6.3) is applied in
  the same family of routines (`0x00279A7C`, with +644) [X].
- **The pace stays 200** [D]: neither field is a pace length. 200 per pace is what the clue counts and marker
  spacing in row 03 show (above).

This closes the "+616 and +76" item of the lists above.

**Still open after 6.10.1:** what the Grapple-Cam's phase-7 animation does to the target; Rocket Boots' speed; the
costume's effect on guards.

