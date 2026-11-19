---
id: sly2-behaviour-gadgets
title: Sly 2 gadgets and special moves
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 USA (SCUS-97316) image; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds
    used_for: >-
      every item record of the item table at 0x002BCD00 (all 43 records, including the usage-text field), the item
      names, descriptions and usage texts from the worlds' string tables, every constant marked [D] read from the
      executable at the address given, and the counts and option values of the gadget object classes (93, 100, 101,
      102, 108) and of the three characters from a walk of every package of all 43 worlds
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce (2026-11-12), Sly2-Release/gadget.cpp, gadget.h, gang.cpp, carry.cpp, jump.cpp,
      interact.cpp, sly.cpp, murraycarry.cpp, jtthrow.cpp, pickup.cpp
    used_for: what each item does in play, the slot and power rules, the carried-gadget states and the blast; marked [R]
  - kind: disc
    what: >-
      re-check 2026-11-17: the bomb package's script modules in all 43 worlds (blast calls and their radii), the
      jump, glide, dodge and juggle constants at 0x002BF414 to 0x002BF880 and 0x002BC9A8 to 0x002BC9B8, root travel
      of the move clips in the nightclub hub
    used_for: the values marked [D] in sections 5.1, 5.2 and 7.1
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce, Sly2-Release/jump.cpp (special jump kinds, the glide), sly.cpp (dodge, juggle strike),
      gang.cpp (Berserker Charge, Adrenaline Burst), carry.cpp (the blast method)
    used_for: the rules of sections 5.1 and 5.2; marked [R]
  - kind: disc
    what: >-
      re-check 2026-11-18: the clips behind the Lightning Spin, juggle-strike and slam slots of Sly's and Murray's
      animation lookup tables (animation.md 9) measured in the nightclub hub, Murray's package checked in all nine
      episode hubs for object 507; the target-scoring polynomials at 0x002BD280 to 0x002BD2B0 and the glide
      constants at 0x002BF850 to 0x002BF870
    used_for: the clip lengths of section 5.3 and the [D] values of sections 5.1 and 5.4
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce, Sly2-Release/target.cpp (the actors' target choice and its score), sly.cpp (dodge side and
      motion, juggle-strike and charged-strike clips), gang.cpp (the slam's clip choice), jump.cpp (the glide's turn)
    used_for: the rules of sections 5.1, 5.3 and 5.4; marked [R]
reviewed_by: pending
date: 2026-11-14
---

# Sly 2 gadgets and special moves

Every entry of the item table: what it does, its numbers, and the objects it uses. The table itself (owner,
button use, price, power cost, opening episode, names) is in [characters.md](characters.md) section 6 and is not
repeated here. Item numbers below are the table's record numbers. Tags: **[D]** disc data, **[O]** observable,
**[R]** reference behaviour to be confirmed in play, **[I]** inferred.

## 1. The item record's last field [D]

characters.md 6 names fields 0 to 6 of the 32-byte record. Field 7 (offset 28, `s32`) is the string ID of the
item's **usage text**, shown with the description. All 36 sold or unlockable items have one, ids 204 to 216:

| String ID | Text (game string) | Used by items |
|---:|---|---|
| 204 | Press %s to place. | 9, 13 |
| 205 | Press %s to trigger. | 8, 10, 14, 15, 20, 21, 22, 23, 30, 31, 32, 33, 34, 35, 39 |
| 206 | Usage is automatic. | 17, 18, 36 |
| 208 | Press and hold %s to use. | 12, 25, 29, 40, 42 |
| 209 | Press %s to aim, %s to throw, then %s to trigger. | 7, 11, 41 |
| 210 | Juggle an unaware enemy with (Triangle), then finish them off quietly with (Square). | 28 |
| 211 | Press (R1) while jumping to deploy your paraglider. | 27 |
| 212 | Press and hold %s to crouch, then press (Cross) to jump. | 16 |
| 214 | Press %s to trigger while carrying an enemy. | 19 |
| 215 | Press %s to dodge while in combat. Use the left analog stick to choose a direction. | 24 |
| 216 | Press %s to aim, then %s to throw. | 26, 37, 38 |

`%s` is the button the item is on, and the bracketed button names are icons in
the string. Records 0 to 6 have no owner and no strings. Records 4 and 5 have button use 2 and record 6 has 1, but
they are otherwise empty. *Interpretation:* they are internal entries; the reference starts Bentley's bombs with
items 4 and 5 [R] [I].

String 145/146 ("Uppercut", "Surprise enemies with a quick punch and grab") exists in the string table, but no
record names it [D].

## 2. Buttons and slots

- Each character has **three gadget slots**, on the buttons **L1, L2 and R2** in that order. In the controller's
  button mask they are `0x4`, `0x1` and `0x2`; this order is the executable's 3-entry table at virtual address
  `0x002C3258` [D].
- A slot holds an item number or −1. Putting an item in a slot clears any other slot of the same character that
  held it, and ends that slot's held action first. The three slots are saved with the game, per character [R].
- Which items the player owns is a per-item flag in the save. An item can be put on a button only when it is owned
  and its button use is 2 or 1 [R]. Items with button use −1 work without a button whenever they are owned:
  Juggernaut Throw, Atlas Strength, Paraglide, Silent Obliteration, Tornado Strike and Long Toss (section 5) [D for
  the use values] [R].
- **Pressing the button** (a press is recognised for 0.15 s) [R]:
  - if the character cannot use the item in its current state, a refusal sound plays (sound ID 46);
  - else, if the character's power is below the item's cost, the refusal sound plays and the power meter flashes;
  - else the item starts. Use 2 items fire once. Use 1 items (Thief Reflexes, Time Rush) keep running while the
    button is held and the character can still use them, and end when it is released.
- When an item starts, the mission system receives event 60 with the character [R] (see missions.md).
- Bentley's Hover Pack is drawn on his back only while he is the current player and the Hover Pack is on one of
  his buttons [R].

## 3. Power

- Each character has a **power** counter and a **maximum**. The maximum is always 100 [R]; scripts can read and
  set both through options 728 (maximum) and 729 (current) [R].
- **Spending** [R]: an item's cost (record field 3) is taken once. Most items pay when they start; the ones that
  play an animation first (Smoke Bomb, Temporal Lock, Guttural Roar, Fists of Flame, Insanity Strike, Voltage
  Attack, Size Destabilizer) pay at the animation's activation event. The pocket items (Alarm Clock, Rage Bomb,
  Music Box, TOM) pay when they are thrown. Power never drops below 0.
- **Draining** [R]: some items cost power per second while they run. The drain is accumulated as a fraction and
  taken in whole units:

  | Item | Drain per second | Executable constant |
  |---|---:|---|
  | Thief Reflexes (29) | 2 | `0x002BD260` = 2.0 [D] |
  | Time Rush (42) | 1 | `0x002BD264` = 1.0 [D] |
  | Shadow Power (40) | 3 | `0x002BF5C0` = 3.0 [D] |
  | Hover Pack (12) | 1 | `0x002BF878` = 1.0 [D] |
  | Stealth Slide (25) | 1 | [R] |

  A draining item ends when power reaches 0.
- **Refilling** [R]: the health pickup (class 93, package `medicine`, one in every playable world [D]) restores
  health and power. Its default amounts are 10 health and 20 power, each limited to the character's maximum. A
  character's scripts can refuse the pickup when health and power are both full. Scripts can also set power
  directly (option 729). No other refill exists in the reference.
- Option 1305 = 30 is carried by the 126 class-105 objects nested inside the three characters' bodies, not by the character records themselves (options-by-class.md) [D]. *Unconfirmed:* that this is
  the starting power.

## 4. Items with a button (button use 2 or 1)

### 4.1 Sly (owner 7)

| # | Item | What it does [R] | Numbers |
|---:|---|---|---|
| 23 | Smoke Bomb | Sly plays an animation. At its activation event the cost is paid and a smoke effect is taken from Sly's own pool and placed where he is. Every alarm the player has set off is cleared, and guards cannot raise a new alarm for 2 s | resume delay `0x002BCC94` = 2.0 s [D] |
| 24 | Combat Dodge | Sidestep in the stick's direction. Allowed only during combat (Sly's attack state) | — |
| 25 | Stealth Slide | A silent roll. Held: power drains while it lasts. Cross jumps out of it. It ends when the button is released, when power is 0, or when the roll's forward speed falls below 150 | 1 power/s; 150 units/s minimum |
| 26 | Alarm Clock | A pocket item: aim and throw (section 6). On landing it goes off and distracts guards | — |
| 29 | Thief Reflexes | Held: game time slows to 0.25 (rate factor 1) and a loop sound plays (sound ID 103). Released, or out of power: factor back to 1, loop stopped | 0.25 at `0x002BF3EC` [D]; 2 power/s |
| 30 | Feral Pounce | Starts a long jump (jump kind 14) | — |
| 31 | Mega Jump | A burst effect plays and a high jump (jump kind 9) starts. A same-world reload that requests it removes Mega Jump from the slots and from the owned items | — |
| 33 | Knockout Dive | A dive at enemies (Sly state 9) that stuns them on the ground | — |
| 34 | Insanity Strike | Sly plays an animation. At its activation event a cane overlay animation is applied (cost paid); using the item again while it is applied removes it. Enemies hit attack each other (guards.md) | — |
| 35 | Voltage Attack | As Insanity Strike, with the electric cane overlay | — |
| 37 | Rage Bomb | A pocket item (section 6); it confuses enemies in the area into fighting each other | — |
| 38 | Music Box | A pocket item (section 6); it attracts guards, then puts them to sleep. Its attraction is a sphere that switches on when it is placed | — |
| 39 | Lightning Spin | A charged strike (Sly attack state, sub-state 9) that makes an electric field | — |
| 40 | Shadow Power | Held: Sly moves unseen at 0.75 of his run-gait speed while power drains. Ends when power is 0, the button is released, or a blocking target is reached | 3 power/s; `0x002BF5C4` = 0.75 [D] |
| 41 | TOM | A pocket item (section 6). When thrown it heads for the nearest guard (up to 32 guards searched, distance with the height difference counted twice; Sly when there is none) and draws its attention | — |
| 42 | Time Rush | Held: game time speeds up to 1.35 (rate factor 1) with loop sound ID 102. Released, or out of power: back to 1 | 1.35 at `0x002BF3F0` [D]; 1 power/s |

Thief Reflexes and Time Rush can start only while rate factor 1 is 1 or already their own value, and not while Sly is
in his state 4 (characters.md 3) [R].

### 4.2 Bentley (owner 8)

| # | Item | What it does [R] | Numbers |
|---:|---|---|---|
| 7 | Trigger Bomb | Taken from Bentley's pool and aimed and thrown (section 6). It stays where it lands; the next press of its button detonates it (section 7) | — |
| 8 | Size Destabilizer | A crossbow animation; at its activation event the cost is paid. A guard hit is shrunk. A shrunk guard's hits on the player do half damage (characters.md 2) | — |
| 9 | Snooze Bomb | Summoned and held (Bentley's holding state), then placed or thrown; it puts enemies in the area to sleep | — |
| 10 | Adrenaline Burst | Bentley runs fast (his sprint state) | — |
| 11 | Health Extractor | Aimed and thrown like the Trigger Bomb; the next press starts it (section 7.2) | — |
| 12 | Hover Pack | A hovering jump (jump kind 17), section 4.4 | 1 power/s |
| 13 | Reduction Bomb | Summoned and held like the Snooze Bomb; it shrinks enemies in the area | — |
| 14 | Temporal Lock | An animation; at its activation event the cost is paid and every guard in the world is frozen. The freeze ends 6 s later | `0x002BF938` = 6.0 s [D] |

### 4.3 Murray (owner 9)

| # | Item | What it does [R] | Numbers |
|---:|---|---|---|
| 15 | Fists of Flame | An animation; at its activation event a fiery-punch overlay animation is applied (cost paid). Using it again removes it | — |
| 16 | Turnbuckle Launch | Murray crouches (his launch state); Cross jumps very high | cost 0 |
| 19 | Diablo Fire Slam | Only while carrying an enemy: a slam (Murray state 8) with a firestorm | — |
| 20 | Berserker Charge | A powerful run (Murray state 12) that scatters enemies | — |
| 21 | Guttural Roar | An animation; at its activation event the cost is paid and every guard in the world switches to a frightened mode | — |
| 22 | Raging Inferno Flop | His falling body slam (the Thunder Flop, characters.md 4.5) with a wall of flame on impact | — |

### 4.4 Hover Pack in detail (item 12) [D for the constants] [R]

While hovering:

- the horizontal velocity is 300 × the stick's deflection, in the stick's direction (`0x002BF87C` = 300);
- the vertical velocity is −100 (`0x002BF880`);
- the character turns to face its motion when its horizontal speed is above 200 (`0x002BF848`);
- power drains at 1 per second.

The hover lasts at least 0.5 s (`0x002BF82C`) and at most 3 s (`0x002BF830`). After 0.5 s it ends on landing or
when the button is released, and at any time when power reaches 0.

## 5. Items without a button (button use −1)

| # | Item | Effect while owned [R] |
|---:|---|---|
| 17 | Juggernaut Throw | Murray's throw uses its alternative throw animation; thrown objects explode on impact |
| 18 | Atlas Strength | While carrying, Murray can jump (Cross; jump kind 15) and run (R1 held) |
| 27 | Paraglide | R1 in the air deploys the paraglider (jump state 19). Allowed only for the current player and not within 0.3 s of landing. Cost 0 |
| 28 | Silent Obliteration | Changes Sly's finishing strike on a juggled enemy (his state 16): another clip and a homing motion (section 5.2) |
| 32 | Tornado Strike | A spinning attack; not traced in the reference |
| 36 | Long Toss | Sly and Bentley throw at 1.5 × the maximum aim range, with 1.35 × the predicted flight time (characters.md 7) |

### 5.1 Move items in detail

Jump kinds, launch heights and the shared air steering of **515 × stick** are in [characters.md](characters.md) 1.2;
this section gives what each move item adds. Clip numbers are object IDs in the nightclub hub (row 2) found through
the character lookup tables ([animation.md](animation.md) 9) [D]; root travel is measured on transform 1 [D]. The
air-steering factor multiplies the 515 [R].

| Item | Move | Numbers |
|---|---|---|
| 30 Feral Pounce | Jump kind 14: launched to **250** above the floor [R]; air-steering factor **1.75**, so the air speed target is 901.25 × stick [R]. If the stick is nearly centred (under 0.2) at the launch, a horizontal push of 515 × 1.75 is added along the stick's direction [R] | factor 1.75 at `0x002BF760` [D] |
| 31 Mega Jump | Jump kind 9: launched to **1,200** above the floor [R]; air-steering factor **1.5** and a second jump factor 1.2 whose use is not established. Sly's physical size is set to **0.75** for the jump and restored when it ends; the camera switches to the jump's own zone [R] | 1.5 at `0x002BF758`, 1.2 at `0x002BF75C`, 0.75 at `0x002BF750` [D] |
| 18 Atlas Strength | Jump kind 15 while carrying: the ordinary launch to **215** above the floor, air-steering factor 1, no double jump [R]. The carrying run uses Murray's carry gait (characters.md 1.1) | |
| 27 Paraglide | Opening (jump state 19), then gliding (state 20) [R]. In the glider's frame each velocity component eases toward a target: forward between **300** (stick back) and **800** (stick forward); vertical between **−200** and **−400** (sinking faster when pushing forward); sideways 0 [R]. With f = (forward part of the stick + 1) / 2, each target is (1 − f)·back value + f·forward value. Turning rate 5 to 12 rad/s by the same blend, times the stick's signed sideways part relative to Sly's body heading (section 5.4). Easing parameters (800, 0, 0.1) for forward and sideways and (5000, 0, 0.1) for vertical. The acceleration is capped at **8,000** and its vertical part is never downward [R]. The camera's path offset eases between −250 and −500 with (300, 0, 0.3) | 300/800 at `0x002BF850`; −200/−400 at `0x002BF858`; 5/12 at `0x002BF870`; easings at `0x002BF7A8`, `0x002BF7B8`, `0x002BF7C8`; 8,000 at `0x002BF7D4`; −250/−500 at `0x002BF798`; (300, 0, 0.3) at `0x002BF860` [D] |
| 24 Combat Dodge | Sly's state 6, only in combat. The side (left or right clip) comes from the stick against the direction to the target, else the last side [R]. Start velocity = the clip's root velocity: clips 281 and 282 each move **500** in **0.333 s**, that is **1,500**/s [D]. While dodging Sly circles the target, which is chosen as in section 5.4: the horizontal distance eases toward **250** with (500, 0, 0.5), the radial correction is limited to 1,500 per second, and the acceleration to 1500²/250 + 1500 = 10,500 [R]. It ends with the clip; the velocity is then halved [R] | (500, 0, 0.5) at `0x002BC9A8`; 0.5 at `0x002BC9B8` [D] |
| 10 Adrenaline Burst | Bentley's state 14 for **2.0 s** [R]: his velocity is the root velocity of clip 588, **1,500**/s (250 in 0.167 s, looped) [D for the clip]; his heading follows the stick directly [R]. Cost paid at the start | |
| 33 Knockout Dive | Sly's state 9: a rush at a chosen target with clip 189, **0.917 s**, root travel **660** (about 720/s) [D for the clip]; it ends with the clip [R] | |
| 20 Berserker Charge | Murray's state 12 in phases [R]: a wind-up clip; the charge, moving with clip 441's root velocity (**350** in **0.5 s**, 700/s) [D for the clip]; after **0.66 s** of charging (a code value) the stop clip 443 (**250** over **1.0 s**) [D for the clip]; hitting a wall makes him bounce back along a scripted path and the move ends. A flag on Murray set by an upgrade keeps the charge going instead of stopping [R] | |
| 19 Diablo Fire Slam | Murray's state 8 with a carried enemy: the slam clip, standing still [R]. On the disc it plays clip 492, **0.8333 s** (section 5.3) [D] | |
| 39 Lightning Spin | Sly's charged strike (attack state, sub-state 9) with its own clip, 348, **1.0 s** (section 5.3) [D]; cost paid at the start [R] | |

### 5.2 Silent Obliteration and the juggle strike (item 28)

Sly's state 16 is the finishing strike on an enemy he has knocked into the air [R]. Without the item he plays clip 196
and moves by its root motion; with it he plays clip 198 instead [D for the clips] and homes in on the target each
frame: he turns to face it and moves straight at it with a speed eased from the current distance toward **200**
units, easing parameters (1000, 0, 0.5), at half the frame step [R]. Constants: 200 at `0x002BF414`, (1000, 0, 0.5)
at `0x002BF418` [D].

### 5.3 Clip lengths of the strike moves [D]

Each move plays the clip its character's animation lookup table gives for a fixed slot (animation.md 9) [R for which
slot each move uses]. Measured in the nightclub hub (level-table row 2):

| Move | Character | Clip (object ID) | Length |
|---|---|---:|---:|
| Lightning Spin (charged strike, sub-state 9) | Sly | 348 | 1.0 s |
| Ordinary charged strike (sub-state 8), for comparison | Sly | 297 | 1.0 s |
| Juggle strike without Silent Obliteration | Sly | 196 | 1.1667 s |
| Juggle strike with Silent Obliteration | Sly | 198 | 1.3333 s |
| Diablo Fire Slam, and the plain slam of a carried enemy | Murray | 492 | 0.8333 s |

The Diablo Fire Slam asks first for a slam clip of its own (lookup key 507) and falls back to the plain slam clip
when that is missing [R]. Object 507 is not a clip in Murray's package in any of the nine episode hubs [D], so on
this disc the two slams play the same clip, 492.

### 5.4 Glide turning and the dodge's target

**Glide turning (Paraglide, jump state 20)** [R for the rule, D for the constants]. The turn uses the stick's
direction as a world heading (the stick read relative to the camera, as for running) and Sly's current body heading:

- s = stick magnitude × sin(stick heading − body heading), the stick's signed sideways part **relative to the body**,
  not the raw left/right axis of the pad;
- the body heading changes by rate × s × frame step, with rate = (1 − f)·5 + f·12 rad/s and f as in 5.1;
- headings are angles about +Z (Z up), counter-clockwise seen from above. A stick pointing to the left of Sly's
  facing gives s > 0 and turns him left (his heading angle increases); to the right gives s < 0 and turns him right.

**Which target the dodge circles** [R for the rule, D for the constants]. The dodge circles the player's current
"nearest" target, which is re-chosen every frame while the player is not locked onto a target (the juggle strike
locks it; the dodge does not):

- candidates: every guard that is currently active; there is **no distance limit**;
- each candidate gets a score s = d − c·(500 + 0.5·d), where d is the horizontal distance from the player to the
  guard and c = clamp(0.5 − 0.5·k, 0, 1), with k the stick magnitude times the cosine of the angle between the
  stick heading and the direction from the guard to the player. So c is 1 when the stick points straight at the
  guard (score 0.5·d − 500), 0.5 with the stick released (0.75·d − 250), 0 when it points straight away (d);
- the candidate with the **lowest score** is the target. Polynomials (−500, −0.5) at `0x002BD2B0` and (0.5, −0.5) at
  `0x002BD2A0` [D];
- a stay-with-the-current-target bonus (−200 − 0.2·d, at `0x002BD280` [D]) exists in the scoring, but in the game
  it never applies, because the test that should enable it compares the player with the player's own target [R];
- with no active guard there is no target, and the dodge plays its clip without circling [R].

This differs from "the nearest guard within 400" in two ways: the range is unlimited, and the stick biases the
choice. Example: with the stick released every guard scores 0.75·d − 250, so the nearest wins; with the stick pushed
fully toward a guard 1,000 away (score 0), it beats a guard 300 away behind the player (score 300).

## 6. Pocket items (Sly 26, 37, 38, 41) and Bentley's thrown items (7, 11) [R]

These items are **carryable objects**. Each comes from a pool, a proxy that holds ready copies. Using one is
refused when the pool has fewer than 2 free copies. A copy that is still out and kept, such as a planted Trigger
Bomb, is reused instead of taking a new one.

1. **Take**: a copy is taken from the pool and given the item's number as its kind. The character enters the aim
   mode of characters.md 7.
2. **Throw / cancel**: as in characters.md 7. Throwing pays the item's cost.
3. **States of the object**:
   - **0** held by its carrier (it follows the hand);
   - **1** flying, free of constraints. TOM (41) also keeps its rotation fixed and picks its target guard here;
   - **2** landed and fixed in place. Item 11 turns to face away from the surface it landed on;
   - **3** used. Alarm Clock and TOM go straight to 3 when they land;
   - **4** fading out over 0.25 s, then returned to the pool;
   - **5** the Health Extractor at work (7.2).

   Each state change also sends a goal (109 + state) to the object's own state machine. Its animations and effects
   are data in the object's package.
4. Contacts during the first 0.25 s after a state change to 0 are ignored (`0x002C42E0` = 0.25) [D].

**Packages on the disc** [D] (counts are package bodies across the 43 worlds):

| Class | Package name | Count | Options seen (ID = value) |
|---:|---|---:|---|
| 100 | `tommy` | 42 | 1102 (an object reference) |
| 100 | `tossable_distract` | 42 | 1102 |
| 100 | `music_box` | 42 | 1102 |
| 100 | `c_eagle_egg` | 1 | 1102 |
| 101 | `bentley_mine` | 42 | 1108 = −20, 1102 |
| 102 | `guard_trap` | 42 | 1113, 1102 |
| 93 | `medicine` | 42 | 891 to 894, 13 = 4000, 49 |
| 108 | `robot_shell_ammo`, `robot_dual_ammo` | 1 each | 30, 33, 34, 1435 to 1437 |

Class 100 is the carryable object. 101 is the mine (the Trigger Bomb). 102 is the trap, which the reference
identifies as the Health Extractor. *Interpretation from the package names:* `tossable_distract` is the Alarm
Clock, `tommy` is TOM and `music_box` is the Music Box [I]. No separate package for the Rage Bomb was found.

## 7. Detonation and capture [R]

### 7.1 Blast (Trigger Bomb and other bombs)

A blast at the object's centre with radius **r**. The radius is passed by the bomb package's own script, which
calls the blast method (stream option 1110, script ID 964 in classes 100 to 102) when the bomb goes off [D]. One
package, `bentley_mine` (class 101), serves every bomb; its kind option (stream 1104, script ID 960) selects the
values [D]:

| Bomb kind (option 1104) | Item | Radius r | Filter (option 1107) | State code (option 1108) | Effect object | Sound |
|---:|---|---:|---:|---:|---:|---:|
| 5 or 7 | Trigger Bomb (7) | **300** | 256 | −20 | 3995 | 149 |
| 9 | Snooze Bomb (9) | **300** | 524,288 | −60 | 3978 | — |
| 13 | Reduction Bomb (13) | **600** | 16,777,216 | −5 | 3977 | — |
| 37 | Rage Bomb (37) | **600** | 33,554,432 | −5 | 3970 | — |

These are the only calls of the blast method on the disc: 84 with 300.0 and 84 with 600.0, two of each per world in
the 42 worlds that hold the package [D]. The filter and state code tell each hit object what kind of bomb hit it;
what the Snooze, Reduction and Rage codes do to guards belongs to guards.md [R].

- Every solid whose bounding sphere comes within r is a candidate (up to 256), except the bomb's own parts.
- Rays are cast in **11 rings** at elevations −1.5, −1.2, −0.9, −0.6, −0.3, 0, 0.3, 0.6, 0.9, 1.2, 1.5 rad, with
  3, 7, 9, 10, 11, 12, 11, 10, 9, 7, 3 rays in each ring, evenly spaced in heading. One more ray goes toward each
  candidate's centre, shortened to r.
- For each ray, the first candidate it hits is recorded once (at most 32 in all). The hit object receives a
  contact message carrying:
  - the bomb's filter (option 1107);
  - the bomb's state code (option 1108; −20 on `bentley_mine` [D], default −10);
  - an impulse of 1000 along the ray (`0x002C4334` = 1000 [D]).
- The body that was hit is pushed along the ray:
  - With u = (speed of the hit point along the ray) / 1000, the response is 1 − u², limited to 0..1. The
    push is response × 100 (`0x002C4330` = 100 [D]).
  - The resulting change of velocity is applied only as far as it brings the point's speed along the ray up to
    1000. Bodies that cannot move are not pushed.
- Finally, a fixed noise is made at the blast that guards hear (guards.md).

### 7.2 Health Extractor (item 11, class 102)

When started, the extractor looks for the nearest guard within 400 of its source point that can be captured, and
pulls it in:

- the guard's velocity becomes half its velocity with an upward 400 (`0x002C43F0` = 0.5, `0x002C43F4` = 400 [D]);
- while captured, the guard's gravity is cancelled and it is pulled toward the source with spring constants
  (0, 15, 0) for position and (0, 2.5, 0) for velocity (`0x002C43D0`, `0x002C43E0` [D]);
- it turns to the source's heading and, within 100 of the source, enters its held pose;
- its scale is 1.2 − u², limited to 0..1, with u the time since capture divided by 3 s (`0x002C43B0` = 3.0 [D]);
- after 3 s a health pickup (class 93) appears at the source, the guard is removed from the world, and the mission
  system receives event 24 with the guard.

If no guard is found the extractor is used up (state 3). If the guard's capture ends early, the extractor
returns to state 2.

## 8. Test vectors [D]

- Executable check: SHA-256 of SCUS_973.16 as in `sources:`.
- Item records (8 × `s32`, field 7 = usage text), selected:
  - 9 = (2, 8, 550, 5, 3, 133, 134, 204) "Snooze Bomb";
  - 10 = (2, 8, 1000, 15, 4, 135, 136, 205) "Adrenaline Burst";
  - 12 = (2, 8, 1200, 1, 6, 139, 140, 208);
  - 27 = (−1, 7, 600, 0, 5, 171, 172, 211);
  - 29 = (1, 7, 1600, 2, 7, 175, 176, 208);
  - 32 = (−1, 7, 0, 0, −1, 181, 182, 205);
  - 36 = (−1, 7, 0, 0, −1, 189, 190, 206);
  - 41 = (2, 7, 0, 5, −1, 199, 200, 209).
- Records 0 to 3 are (−1, 0, …, 0); 4 and 5 are (2, 0, …, 0); 6 is (1, 0, …, 0).
- Slot buttons at `0x002C3258`: (4, 1, 2).
- Float constants: `0x002BF3EC` 0.25, `0x002BF3F0` 1.35, `0x002BD260` 2.0, `0x002BD264` 1.0, `0x002BF938` 6.0,
  `0x002BCC94` 2.0, `0x002BF5C0` 3.0, `0x002BF5C4` 0.75, `0x002BF878` 1.0, `0x002BF87C` 300, `0x002BF880` −100,
  `0x002BF82C` 0.5, `0x002BF830` 3.0, `0x002BF848` 200, `0x002C43B0` 3.0, `0x002C43F0` 0.5, `0x002C43F4` 400,
  `0x002C4330` 100, `0x002C4334` 1000, `0x002C42E0` 0.25, `0x002C43D0` (0, 15, 0), `0x002C43E0` (0, 2.5, 0).
- Gadget-object counts over all package bodies of the 43 worlds: class 100: 127, class 101: 42, class 102: 42,
  class 93: 42, class 108: 2. The three characters (classes 53, 54, 55): 42 each.
- Options of the class-105 objects nested in the characters (not of the character records): option 1305 = 30 (Sly, Murray, Bentley); option 1304 = 0.25 (84 Sly records) and
  0.2 (42); option 726 = 60 on Murray.

## 9. Open questions

- What the Snooze, Reduction and Rage bombs' state codes (−60, −5) do to guards (guards.md). The radii are resolved
  (section 7.1).
- Tornado Strike (32): not traced.
- What the internal records 4, 5 and 6 are for.
- Whether option 1305 = 30 is the starting power, and what options 1304 and 1307 mean.
- (Resolved: the Rage Bomb uses the `bentley_mine` package with bomb kind 37; section 7.1.)
- The power and health amounts carried by the health pickup's own options (891 to 894) versus the reference's
  defaults of 10 and 20.
- Mega Jump's second factor (1.2) and Feral Pounce's centred-stick push need checking in play.
- All [R] behaviour needs checking in play.
