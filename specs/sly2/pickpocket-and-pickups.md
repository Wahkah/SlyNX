---
id: sly2-behaviour-pickpocket-and-pickups
title: Sly 2 pickpocketing and pickups
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256
      a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds
    used_for: >-
      a walk of every package of every world collecting the bodies and own options of classes 91 to 96, 99 and 103
      and the placements of the pickup packages; the pocket tables, the drop table, the loot table and the launch and
      ring constants read from the executable at the virtual addresses given below
  - kind: reference-project
    what: >-
      SafeHouse @ c70803ce (2026-11-12), Sly2-Release, files pickup.cpp, pickup.h, pocket.cpp, vault.cpp, booty.cpp,
      sly.cpp, update.cpp
    used_for: what each pickup does when touched, how pockets fill and give, the drop rule, clue-bottle counting and
      the vault; marked [R]
  - kind: reference-project
    what: sly2-decomp @ 33e2bab9 (2026-11-12), GPL-3.0, read for facts only
    used_for: confirmation of the table addresses
  - kind: executable
    what: SCUS_973.16 (hash checked), floats at 0x002C0D60 to 0x002C0D7C, re-read 2026-11-17
    used_for: the gravity vector, the bounce caps and the touch distance of section 3.3
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/pickup.cpp (touch test and floor bounce)
    used_for: which constant each rule uses; marked [R]
  - kind: disc
    what: >-
      SCUS-97316, the vault package's scripts in the 8 vault worlds (rows 4, 10, 13, 16, 20, 28, 34, 42) and the
      executable tables at 0x002C0EC8 (vault parts) and 0x002C0FF0 (dial key directions), read 2026-11-19
    used_for: the vault combinations, rewards and lines per episode, the dial layout and the state sequences of 6.1
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/vault.cpp (states, dial input, options 1439 to 1443)
    used_for: what each vault option and state does in 6.1; marked [R]
reviewed_by: pending
date: 2026-11-14
---

# Sly 2 pickpocketing and pickups

The pickup part of [../sly1/pickups-and-progress.md](../sly1/pickups-and-progress.md), redone for Sly 2. Saving is
left to `saving.md`. Tags: [D] disc data, [O] observable, [R] reference behaviour (to be confirmed by play),
[I] inferred. Executable addresses are virtual; file offset = address − 0x100000 + 0x80 ([disc.md](disc.md) 3), read
after the hash check above.

## 1. What changed from Sly 1

- There are **no placed coins**. Coins come only from breakables, knocked-out guards and pockets [D: the coin package
  is placed once per world as a pool template, never as scattered pickups] [O].
- **Medicine** replaces Sly 1's charm and life token: it heals health and gadget power (section 4) [D] [R].
- **Pickpocketing** is new: guards and some other characters carry a pocket of coins and loot (section 2).
- **Loot** (pocket prizes and carried treasure) is sold for coins at the safe house
  ([safehouse-and-shop.md](safehouse-and-shop.md)).
- **Clue bottles**: 30 per episode, all in the hub; together they open the episode's vault (section 6).
- **Keys** are task items: picking one up completes a task ([missions.md](missions.md)).

## 2. Pickpocketing

### 2.1 The take [R]

- Only Sly (class 53) pickpockets. A guard (or other giver) of the "can be robbed" rank in reach and **Circle pressed
  within 0.15 s** starts the pickpocket move (Sly state 11, [characters.md](characters.md) 3).
- The item is taken at an event of the pickpocket animation; the next item of the pocket is then readied.
- Each take gives **one pocket entry** (2.3).

### 2.2 What a pocket holds [R], tables [D]

A pocket holds up to **8 entries**, filled when its owner resets, from the episode's row of one of two tables in the
executable ([guards.md](guards.md) 8 gives the row layout):

| Table | Address | Used for | Coins | Prize chance |
|---|---|---|---|---:|
| guards | `0x2C3968` | class 58 | 6 to 12 | 0.4 |
| other givers | `0x2C3748` | other prize givers | 4 to 8 | 0.2 |

Row 0 (no episode) is empty. Filling [R]:

1. A prize kept from the previous fill stays, unless it is a task item whose task is done.
2. Otherwise, with the row's chance, one prize kind is drawn by the six weights (30, 30, 15, 15, 5, 5).
3. A number of coins is drawn in the row's range and split over **2 or 3 entries**, as evenly as possible, each at
   least 1 coin.
4. The prize, if any, is the last entry.

### 2.3 The prize kinds 1114 to 1143 [D]

The prize kinds are loot kinds. The pocket item object (class 94, package `pickpocket_item`, one per world) lists
30 display definitions (option 907): kind 1114 + *i* shows loot table record *i*, for *i* = 0 to 29, with its own
mesh group (option 909). The loot table at `0x2C3288` ([safehouse-and-shop.md](safehouse-and-shop.md) 5) carries
the same kind number in its third field for records 0 to 29. So:

| Kinds | Loot records | Names (game strings) | Values |
|---|---|---|---|
| 1114 to 1116 | 0 to 2 | Bronze, Silver, Gold Comb | 11, 19, 27 |
| 1117 to 1119 | 3 to 5 | Bronze, Silver, Gold Ring | 17, 40, 60 |
| 1120 to 1122 | 6 to 8 | Bronze, Silver, Gold Watch | 14, 31, 44 |
| 1123 to 1125 | 9 to 11 | Bronze, Silver, Gold Pen | 22, 49, 71 |
| 1126 to 1128 | 12 to 14 | Bronze, Silver, Gold Medal | 29, 54, 84 |
| 1129 to 1131 | 15 to 17 | Bronze, Silver, Gold Pocket Watch | 32, 62, 97 |
| 1132 to 1134 | 18 to 20 | Small Nugget, Medium Nugget, Large Gold Bar | 41, 79, 120 |
| 1135 to 1137 | 21 to 23 | Topaz, Sapphire, Ruby | 39, 81, 122 |
| 1138 to 1140 | 24 to 26 | Small, Medium, Large Diamond | 45, 91, 137 |
| 1141 to 1143 | 27 to 29 | Small, Medium, Large Necklace | 42, 84, 127 |

Prizes by episode [D]. Each row names three item families, two grades each, at weights (30, 30), (15, 15),
(5, 5). Guards carry the silver and gold grades (or the middle and top sizes); other givers the bronze and silver
grades (or the bottom and middle sizes):

| Episodes | Families (weights 30 / 15 / 5) |
|---|---|
| 1 | combs / watches / rings |
| 2, 3 | rings / pens / medals |
| 4, 5 | medals / pocket watches / gems (topaz to ruby) |
| 6, 7 | gems / nuggets and bars / necklaces |
| 8 | nuggets and bars / necklaces / diamonds |

### 2.4 Giving [R], constants [D]

The taken entry is thrown toward the current character [R]:

- to a point around him at a random angle of ±1.5 rad from the line to the giver, 100 to 150 units out, 75 below
  his centre (constants at `0x2C0CF0`: −1.5, 1.5, 100, 150) [D];
- in a flight time drawn from 0.75 to 1.2 s under gravity −2250 (`0x2C0CE0`; the gravity vector of 3.3) [D];
- several copies of an entry leave 0.05 s apart; coins are collected by themselves 0.15 s × their order after the
  throw, wherever they land (`0x2C0CE0`: 0.05, 0.15) [D for the values, R for the use];
- a coin entry gives its count of coins; a prize entry adds one to that loot kind's count (`loot` / `mplootkc`,
  [safehouse-and-shop.md](safehouse-and-shop.md)).

A robbable giver shows a glint while its next entry is a prize (and the current character is Sly), a plain marker
while it holds only coins, and none when alerted [R].

## 3. Pickups in general

### 3.1 Classes [D]

| Class | Package | Per world | What it is |
|---|---|---|---|
| 91 | `coin` | 1 template in 42 worlds | coin |
| 92 | `key` (9), `key_card` (6), `key_schedule` (1), `c_radio_transmitter` (1) | 17 templates in 16 worlds | task item |
| 93 | `medicine` | 1 in 42 worlds | health and power |
| 94 | `pickpocket_item` | 1 in 38 worlds | the shown pocket prize (2.3) |
| 95 | `clue` | 1 in each of the 8 hubs | clue bottle |
| 96 | `f_mini_pigeon` (2), `f_mini_rat` (1) | 3 | minigame creatures (not pickups) |
| 99 | `pickpocket_item_dispenser` | 1 in 38 worlds | the pocket object attached to givers |
| 103 | `booty` | 1 in 41 worlds | carried treasure (section 7) |

Worlds without these (the intro hub, row 0, and the menu world, row 1) have no pickups [D].

### 3.2 Template options [D]

Values are the same on every template of a class:

| Class | Option | Value | Meaning |
|---|---|---|---|
| 91, 92, 93, 95, 103 | 13 | 4,000 | activation radius ([clock-and-loop.md](clock-and-loop.md)) |
| 91 | 902 | (4.0, 5.0) | time on the ground before it starts to expire, s (a random value in the range) |
| 91, 92, 93 | 894 | objects 12919, 12921, 12923 | the glint effect's parts |
| 91, 93, 94 | 891, 892, 893 | object references | collection effect, sound and expiry parts (92: 891, 892) |
| 95 | 892 | object 27160 | the bottle's collection effect |
| 94 | 907, 908, 909 | 30 definitions | loot kind, loot record, mesh groups (2.3) |

### 3.3 Touch and motion [R]

| Class | Pick-up radius | Collect radius | Rest time (s) |
|---|---:|---:|---|
| 92 task item | 35 | 35 | — |
| 93 medicine | 45 | 75 | 4 to 5 |
| 94 loot item | 35 | 40 | 8 to 10 |
| 95 clue bottle | — | 50 | — |
| 91 coin | from its options (3.2) | | 4 to 5 [D] |

- A pickup is collected when the current character's body reaches it (first the 300-unit check below, then the radii).
- Launched pickups fall under the gravity vector (0, 0, −2250) [D]. The executable stores it as four floats at
  `0x2C0D60` (0, 0, −2250, 0): the scalar −2250 is the third float, at `0x2C0D68` [D].
- They bounce on floors (contact normal z > 0.7): the velocity along the floor is multiplied by the pickup's own
  along-floor factor and capped at **300** (`0x2C0D74`), the upward velocity is the reflected fall speed times its
  own upward factor, capped at **600** (`0x2C0D78`) [D for the caps, R for the rule]. On the first landing, if the
  bounce would leave too slowly, the pickup settles with a fixed small upward speed instead [R].
- The touch test first rejects a pickup farther than **300** from the character's centre (`0x2C0D7C`) [D for the
  value, R for the use], then applies the radii above.
- The float at `0x2C0D70` is 0.1 [D]; the reference uses 0.1 s as the delay between successive pickups launched
  together from one source [R], which is probably this constant [I].
- On the ground a coin, medicine or loot item waits its rest time, then expires and disappears.
- A glint flashes as the pickup turns past the camera, at most once a second (`0x2C0D90`: 1.0) [D for the value].
- A collected coin flies to the coin counter (screen point 0.83, 0.75 at depth 100, `0x2C0DA0`) and counts when it
  arrives; medicine flies to the health meter (−0.7, 0.72, `0x2C0E10`) [D for the values].

## 4. Coins and medicine

- **Coin** [R]: +1 coin, sound 41, script event 5 on the coin, task event 18 (missions.md); the coin counter shows
  for 5 s. There is no stored cap (the 9,999 limit is reference-only; [safehouse-and-shop.md](safehouse-and-shop.md)).
- **Medicine** [R]: taken only if the character's scripts accept it (they refuse when health and power are both
  full); restores **10 health and 20 power**, each up to its maximum (health 40, power 100,
  [characters.md](characters.md), [gadgets.md](gadgets.md)).

### 4.1 Drops from breakables and guards [R], table [D]

Executable table at `0x2BDC00` (shared with the difficulty help, [missions.md](missions.md)):

| Address | Values | Use |
|---|---|---|
| `0x2BDC10` | 4, 6 | a "rich" drop: 4 to 6 coins |
| `0x2BDC18` | 2, 3 | an ordinary drop: 2 to 3 coins |
| `0x2BDC28` | 4, 6 / 4, 6 | breaks between rich drops, at difficulty help 0 / 1 |
| `0x2BDC38` | 600, 800 / 300, 600 | medicine countdown range, at difficulty help 0 / 1 |

Rule [R]: a counter of "breaks until a rich drop" is drawn when it runs out (interpolated between the two ranges by
the difficulty help); each break counts 1, a guard's knock-out 2; a drop is rich when the counter has run out. A
second counter falls by the damage dealt; when it runs out, one medicine drops. Drops land on walkable floor around
the source: up to 50 tries on a ring around the nearest navigation point (within 2,000), each try turning 1.8 rad
and moving 23 units out from 100 (back by 275 past 300) (`0x2C0E80` to `0x2C0E94`) [D for the values].

## 5. Keys and other task items (class 92)

- Packages: `key`, `key_card`, `key_schedule`, `c_radio_transmitter` [D].
- Placed per world [D]: nightclub hub 1 key; disco 1 key card; Moulin Rouge 6 keys; waterpump 1 key card; palace
  exterior 5 keys; temple interior 2; prison exterior 6 keys and 1 schedule; castle exterior 6; train exterior 3;
  sawmill: mulch, burn, lighthouse and boss 1 key card each, bear cave 6 radio transmitters; blimp exterior 5 and
  interior 4.
- Collecting one plays sound 0, asks its task for state "done" and raises script event 5 [R]. A task item can also be
  a pocket prize; it is dropped from the pocket once its task is done [R].

## 6. Clue bottles and the vault

- **30 bottles per episode**, all placed in the hub, through 30 placements of the `clue` package [D] (8 hubs, 240
  bottles).
- Each placement in a world is numbered in load order; collecting bottle *n* sets bit *n* of the episode's bottle
  record in the progress (`afClue`, a name stored in the save variables) and removes it for good [R] [D for the name].
  Collecting plays the bottle's effect, makes a noise and shows the "*n*/30" counter [R].
- **The vault** (class 78, object 18351, one per episode in one job world: rows 4, 10, 13, 16, 20, 28, 34, 42) [D].
  With all 30 bottles, Sly opens it with a three-digit dial (the stick picks one of nine keys, Cross enters, Triangle
  gives up); the right combination gives the episode's reward, one of the twelve moves that are not sold
  ([characters.md](characters.md) 6), and sets the episode's vault flag [R]. The combination and reward are not
  options of the vault body on the disc; the vault package's script sets them at load (6.1) [D].

### 6.1 The vault in detail (added 2026-11-19)

**Options** (class 78; its script map sends script IDs 1225, 1226, 1227, 1228, 1229 and 235 to stream options 1439,
1440, 1441, 1442, 1443 and 1444) [D for the map; R for the meanings]:

| Stream option | Meaning | Source on the disc |
|---|---|---|
| 1439 | the combination, a three-digit integer (class default 123) | set by script per episode |
| 1440 | line shown when Sly steps up to the vault (it reads out the combination) | set by script per episode |
| 1441 | line shown when Sly is missing bottles | vault body option, 3161 on all 8 vaults [D] |
| 1442 | the reward item record | set by script per episode |
| 1443 | line shown when the vault opens (the move gained) | set by script per episode |
| 1444 | "use the vault" (a method: starts the vault from a script) | [R] |

**Where the combination comes from** [D]. The vault package's own script, on load, reads the current episode and
sets 1439, 1440, 1442 and 1443 on the vault; it also sets a script-local option 1327 to the reward's name string.
Row 13's logged 1439 = 386 is episode 3's value from this script, not a body option.

| Episode | Combination | Reward item | Start line | Open line | Reward name string |
|---|---|---|---|---|---|
| 1 | 231 | 33 Knockout Dive | 3179 | 3180 | 183 |
| 2 | 857 | 34 Insanity Strike | 3193 | 3194 | 185 |
| 3 | 386 | 35 Voltage Attack | 3205 | 3206 | 187 |
| 4 | 248 | 36 Long Toss | 3208 | 3209 | 189 |
| 5 | 969 | 37 Rage Bomb | 3210 | 3211 | 191 |
| 6 | 129 | 38 Music Box | 3212 | 3213 | 193 |
| 7 | 583 | 39 Lightning Spin | 3217 | 1447 | 195 |
| 8 | 725 | 40 Shadow Power | 3223 | 3224 | 197 |

The start lines spell the code out (for example episode 1's ends "... Dimitri's vault is ... 2-3-1."); the open
lines read "You've gotten the *move* move." [D].

**Dial** [D for the table; R for the rules]. Nine keys laid out as a phone keypad, 3 by 3, with stick directions at
`0x2C0FF0` (nine 16-byte vectors): 1 up-left (−0.7, 0.7), 2 up (0, 0.8), 3 up-right (0.7, 0.7), 4 left (−0.8, 0),
5 centre (0, 0), 6 right (0.8, 0), 7 down-left (−0.7, −0.7), 8 down (0, −0.8), 9 down-right (0.7, −0.7). The left
stick selects the nearest key, the current key's distance counted at 0.7 times so the selection does not flicker;
Cross or Square enters the selected digit; Triangle or Circle gives up. After each entry the last three digits are
compared with option 1439; equal opens the vault [R].
The vault package's scripts also check each digit as it is entered (custom procedures 1328, reset, and 1329, check,
called from each key's event 4 with the digits 1 to 9): a correct digit plays effect 298 at a pitch of 0.6 rising
0.2 per correct digit, a wrong one plays effect 54 and starts over [D for the calls and numbers; I for reading them
as sounds].

**States and their sequences** [D for the object IDs and classes; R for the binding]. The vault parts table at
`0x2C0EC8` (24 entries of kind, object ID, field offset) binds the vault's poses 1596 to 1604, the dial pose 1606,
the nine key animations 1607 to 1615, and five sequences (class 114) inside the vault package, one per state:

| State | Meaning | Sequence started |
|---|---|---|
| 0 | idle | none |
| 1 | stepping up to the dial | 1616 |
| 2 | dialing | 1617 |
| 3 | right combination | 1618 |
| 4 | wrong combination or gave up | 1619 |
| 5 | open (reward given, vault flag set) | 1620 |

## 7. Treasure (class 103)

The `booty` object (one per world, 41 worlds) lists 32 treasure definitions (option 1152: identifier, loot record,
an object) [D]. Loot records 36 to 63 (paintings, chalice and other treasures) are carried by a character from the
job to the safe house; delivering one adds it to the loot count [R]. A hit that drops it puts it back on its stand
([characters.md](characters.md) 2) [R]. Loot records 30 to 35 (value 0) are copies of records 3 to 8 and are not
listed by any pocket or treasure definition [D].

## 8. Test vectors [D]

- Guard pocket row, episode 1 (`0x2C3968 + 60`): `6, 12, 0.4, (30, 1116), (30, 1115), (15, 1122), (15, 1121),
  (5, 1119), (5, 1118)`. Other givers (`0x2C3748 + 60`): `4, 8, 0.2, (30, 1114), (30, 1115), (15, 1120), (15, 1121),
  (5, 1117), (5, 1118)`.
- Pocket item object (row 0, OID 12799): first definition bytes `06 00 5a 04 00 00 45 10 00 00 00 00` = kind 1114,
  record 0; first mesh-group option `06 00 70 32 00 00` = object 12912.
- Coin template (OID 12929): option 902 bytes `00 00 80 40 00 00 a0 40` = (4.0, 5.0).
- Treasure definitions in the nightclub hub (row 2, OID 6435): first entries (700, record 36), (701, 37), (702, 38).
- Launch constants at `0x2C0CE0`: 0.75, 1.2, 0.05, 0.15.

## 9. Open questions

- Which characters besides guards carry pockets (the "other givers" table), and the robbable rank's source.
- The coin's own pick-up and collect radii (not options on the disc; the reference keeps class defaults).
- (Resolved 2026-11-19: the vault combinations and rewards, section 6.1.)
- What the three object references of option 1152 mean beyond identifier and loot record.
- All [R] behaviour needs checking in play.
