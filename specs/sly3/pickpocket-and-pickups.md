---
id: sly3-behaviour-pickpocket-and-pickups
title: Sly 3 pickpocketing, pickups, loot and treasure - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), all 39 worlds walked; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: pickup, pocket-item, treasure and treasure-map records with their options; the loot and pocket tables and the pickup constants at the addresses given; item names from the Venice hub string table
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/pickup.cpp, pocket.cpp, booty.cpp, vault.cpp and game/gen (class and option correspondence tables)
    used_for: >-
      the Sly 2 to Sly 3 correspondence of classes and options (rows the reference scores 80 or more), and the
      statement that Sly 3 runs Sly 2's behaviour for these classes; marked [R]
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 pickpocketing, pickups, loot and treasure

Read [../sly2/pickpocket-and-pickups.md](../sly2/pickpocket-and-pickups.md) first. Tags as in [guards.md](guards.md).

## 1. What changed from Sly 2

- **No clue bottles** and no clue class: no class on the disc plays that role [D]. Vaults are opened through the
  vault tumbler (class 121, section 6).
- Pickpocketing is Sly's, as in Sly 2, but Bentley also takes pocket items at range with the **Fishing Pole** (item
  7, [characters.md](characters.md) 3.1) [D for the item, R for the rule].
- **Treasure maps** (item 36) and their markers are new (section 7).

## 2. Classes [D]

| Sly 3 class | Sly 2 class | Group | Records | What it is |
|---:|---:|---|---|---|
| 103 | 91 | `coin` | 1 per world, 39 worlds | coin |
| 104 | 92 | `key`, `h_hanky`, `i_prize_trainer` | 4 | task item |
| 105 | 93 | `medicine` | 39 | health and power |
| 106 | 94 | `pickpocket_item` | 27 | the shown pocket prize |
| 112 | 99 | `pickpocket_item_dispenser` | 27 | the pocket object attached to givers |
| 116 | 103 | `booty` | 39 | treasure definitions (section 5) |
| 108 | — | `p_treasure_at_sea` | 1 | the treasure of the sea job |
| 121 | 78 | `vault_tumbler` | 4 (v_apt, h_hangar_b, c_ext, m_ext) | vault dial |
| 125 | — | `compass` | 36 | the hub compass |
| 127 | — | `treasure_map_dot_factory`, `treasure_map_x_factory` | 12 in 6 hubs | treasure-map trail markers |
| 153 | — | `…_treasure_map` (v, o, h, …) | 6 | a hub's treasure map |

## 3. Pickups

Coin (103) options [D]:

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 13 | 39 | 39 | 4000 ×39 | 13 |
| 32 | 39 | 39 | 1 ×39 | 32 |
| 49 | 39 | 39 | object/id ×39 | 49 |
| 954 | 39 | 39 | 8494 ×39 | — |
| 955 | 39 | 39 | 8496 ×39 | — |
| 956 | 39 | 39 | 9946 ×39 | — |
| 957 | 39 | 39 | 9948 ×39 | — |
| 958 | 39 | 39 | 9950 ×39 | — |
| 959 | 39 | 39 | list (6829, 6) ×39 | — |
| 964 | 39 | 39 | (4,5) ×39 | 902 |

964 = (4, 5) is Sly 2's 902, the time on the ground before expiry; 954 to 958 name the effect, sound, glint and expiry
parts (Sly 2's 891 to 894); 959 a list of six [D] [R]. Medicine (105) carries the same options without 964 and with
954 = 9944 [D]:

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 13 | 39 | 39 | 4000 ×39 | 13 |
| 32 | 39 | 39 | 1 ×39 | 32 |
| 49 | 39 | 39 | object/id ×39 | 49 |
| 954 | 39 | 39 | 9944 ×39 | — |
| 955 | 39 | 39 | 8496 ×39 | — |
| 956 | 39 | 39 | 9946 ×39 | — |
| 957 | 39 | 39 | 9948 ×39 | — |
| 958 | 39 | 39 | 9950 ×39 | — |
| 959 | 39 | 39 | list (6829, 6) ×39 | — |

Constants [X] (block at `0x346FD0`, read by the pickup code):

| Address | Value | Use (Sly 2's) [R] |
|---|---|---|
| `0x346FD0` | (0, 0, −2250, 0) | launched pickups' gravity (Sly 2 `0x2C0D60`) |
| `0x346FF0` | (0.83, 0.75, 100) | screen point a collected coin flies to (Sly 2 `0x2C0DA0`) |
| `0x347060` | (−0.7, 0.72, 100) | screen point medicine flies to (Sly 2 `0x2C0E10`) |

Touch, bounce, expiry, drops from breakables and guards: Sly 2 sections 3.3 and 4 [R]. The bounce caps and the
300-unit touch distance were not found as stored data in Sly 3 [D for the search].

## 4. Pocket items and the loot table

The pocket item (106) carries **30 definitions** per world (810 occurrences of options 970, 971, 972 in 27 worlds),
Sly 2's 907 to 909: prize ID (1255 to 1284), loot record, mesh group [D]:

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 970 | 810 | 27 | list (1255, 0) ×27; list (1256, 1) ×27; list (1257, 2) ×27 | 907 |
| 971 | 810 | 27 | list () ×810 | 908 |
| 972 | 810 | 27 | list (14045) ×27; list (14046) ×27; list (14047) ×27 | 909 |
| 32 | 27 | 27 | 1 ×27 | 32 |
| 954 | 27 | 27 | 8494 ×27 | — |
| 955 | 27 | 27 | 8496 ×27 | — |

**Loot table** [X]: virtual address `0x349070`, **64 records of 12 bytes**: `s32` value in coins, `s32` name string
ID, `s32` prize ID. Records 0 to 29 are the pocket prizes 1255 to 1284 (combs, rings, watches, pens, medals, pocket
watches, nuggets and bars, topaz, sapphire, ruby, diamonds, necklaces, values 21 to 147); records 30 to 35 are
value-0 copies (prize IDs 1288 to 1293); records 36 to 63 are treasures (paintings, chalices, vases, …, values 163 to
854; prize IDs 1285 to 1287 and 1294 to 1318). Names are the strings of the world's string table [D]. The pocket
tables are in [guards.md](guards.md) 3. Pickpocketing and filling: Sly 2 section 2 [R].

## 5. Treasure definitions (class 116)

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 1259 | 390 | 39 | list (10003) ×78; list (10087) ×39; list (9993) ×39 | 1154 |
| 1257 | 351 | 39 | list (16685, -1, 16686) ×39; list (16687, -1, -1) ×39; list (16675, -1, 16675) ×39 | 1152 |
| 1258 | 351 | 39 | list () ×351 | 1153 |
| 1260 | 351 | 39 | 10103 ×312; 10080 ×39 | 1155 |
| 1267 | 312 | 39 | 1 ×195; 0 ×117 | 1162 |
| 1268 | 234 | 39 | 1 ×234 | 1163 |
| 13 | 39 | 39 | 4000 ×39 | 13 |
| 32 | 39 | 39 | 1 ×39 | 32 |
| 53 | 39 | 39 | list (33684990, 2) ×39 | 53 |
| 61 | 39 | 39 | 1 ×39 | 59 |
| 62 | 39 | 39 | 1 ×39 | 60 |
| 1262 | 39 | 39 | (0.25,0,0) ×39 | 1157 |
| 1266 | 39 | 39 | 1 ×39 | 1161 |

1257 is Sly 2's 1152 (one occurrence per treasure: identifier, and two object references), 1259 Sly 2's 1154, 1260
1155 [R]; 9 definitions per world (351 / 39) [D]. Carrying a treasure to the safe house sells it (its loot record's
value) [R].

## 6. Vaults

The vault tumbler (class 121, 4 records, no options on the disc) is the dial of an episode's vault [D]. Its
combination comes from the safe controller that owns it (6.1) [X].

**Tumbler behaviour** [X: class 121's methods `0x00170DF0` to `0x001710E0`]:

- A tumbler is a **dial with ten positions**, 0 to 9. Its current position is field +0x144, the wanted position
  +0x140, its state +0x130 (−1 at start, then 3), with the state's start time at +0x134.
- Each position has a state of the tumbler's state machine: the table at `0x003210E0` holds 10 × (`s32` position,
  `s32` state ID) = (0, 1853) … (9, 1862). The state machine is built at start (`0x0023AAE8`, from state 1853) and
  kept at +0x13C.
- Entering state **1** turns one position up ((position + 1) mod 10). State **0** turns one down ((position + 9)
  mod 10). State **2** jumps to a random position, 0 to 9 inclusive (`0x0011E8F0(0, 9)`). Each then drives the state machine
  toward that position's state.
- While in states 0 to 2, the update checks whether the state machine has reached the position's state, and then
  sets state **3** (resting).
- Three more methods: reset (position 0, snap, state 3), snap to the wanted position (state 3), and "is solved"
  (state 3 and position = wanted).

### 6.1 The safe controller (class 80) [X] [D]

Class 80 (`h_wall_safe`, `m_vault_controls`; 3 records, used through placements) drives the tumblers. Its methods
are at `0x00171108` to `0x00171868`; its options:

| Option | Kind [X] | Meaning | On the disc [D] |
|---:|---|---|---|
| 1605 | method (one argument, the user) | start using the safe: state 2 | — |
| 1606 | int at +0x1C0 | the **combination**, a decimal number | in placement blocks: v_apt 113, 968, 479; h_hangar_b 510, 223; m_ext 123 |
| 1607 | bool at +0x1C8 | after leaving, scramble again (state 1) instead of state 0 | never |
| 1608 | bool at +0x1CC | **open as soon as the dials match**, without a confirm press | 1 on all six placements |
| 1610 | adds a tumbler (an object of class 121; at most 6) | the dials, in order | three tumblers on each (38231, 38239, 38241) |
| 1611, 1612 | methods | reset every tumbler to 0; snap every tumbler to its wanted position | — |

- **Combination to dials** (`0x00171740`, on entering state 0): the combination's decimal digits go to the
  tumblers' wanted positions from the **last tumbler backwards**: the last gets the units, the one before it the
  tens, and so on while digits remain. So 113 on three tumblers wants 1, 1, 3. With fewer digits than tumblers, the
  first tumblers keep their previous wanted position.
- **Controller states** (+0x190; time at +0x194):
  - **0** idle.
  - **1** scrambling: every tumbler is sent to state 2 (random), and the controller returns to 0 once none is
    still in state 2.
  - **2** in use: the user is kept at +0x1A0, the selected dial (+0x1C4) starts at 0, and input is read every frame.
  - **3** leaving: sets +0x1D0, then goes to state 1 if 1607 is set, else to 0.

  Leaving state 2 hands control back to the user (`0x001F4B00`).
- **Input in state 2** (pad of the user, a 0.15 s repeat, `0x001A7A80`):
  - **Triangle** (`0x10`): leave (state 3).
  - **Circle** (`0x20`): confirm. If every tumbler rests on its wanted position, the safe is solved and goes to
    state 0. Otherwise sound 62 plays (only when 1608 is clear).
  - With **1608** set, the safe is solved as soon as all tumblers rest on their wanted positions; no confirm
    is needed.
  - **Stick** (sub-state at +0x198, read through `0x001A99E0` (horizontal) and `0x001A9C38` (vertical) from the
    input block at +0x1D4):
    - Left / right moves the selected dial; past either end, sound 54 plays.
    - A positive vertical step turns the selected tumbler down (its state 0), a negative one up (state 1).
    - The controller waits until that tumbler rests (state 3) before reading the stick again.

  Moving the selection or starting points the camera at the selected tumbler (`0x00154B48` with the tumbler's
  field +0x148) [I for the camera].
- Being solved is seen by scripts through the controller's state [I].

### 6.2 Compass (class 125)

**Compass (class 125)**, 36 records, one per world [D]: option 32 = 1, and 1894 = 5818, a reference to a class-47
object (value type `0x202F`) at field +0x13C [X]. Object 5818 is nested in the compass body [D: no top-level object
has that ID]. *Interpretation:* 1894 is the needle that turns toward the current objective [I]. Behaviour is open.

## 7. Treasure maps (new)

A hub's treasure-map object (class 153, 6 hubs) [D]:

| Option | Uses | Worlds | Commonest values | Sly 2 option [R] |
|---:|---:|---:|---|---:|
| 1904 | 47 | 6 | object/id ×1; object/id ×1; object/id ×1 | — |
| 1905 | 47 | 6 | list (1650, 65535) ×1; list (1651, 1652) ×1; list (1653, 1654) ×1 | — |
| 1901 | 6 | 6 | 5000 ×5; 4000 ×1 | — |
| 1902 | 6 | 6 | 9912 ×6 | — |
| 1903 | 6 | 6 | 400 ×6 | — |
| 1906 | 6 | 6 | 4028 ×6 | — |

Option kinds [X]. 1904 adds a class-3 marker (value type `0x2003`; setter `0x00279160`): **one trail point** per
occurrence. 1905 is a method with two arguments, the point's **clue string** and its **short hint string**, where
65535 means none. Examples [D]: point 1 has clue 1650 "Centered on a trap to a watery grave, below the large clock's
face starts the path for the brave." and no hint. Point 2 has clue 1651 "…Make 4 paces away from them onto solid
ground." and hint 1652 "Take 4 paces away from the hanging flags." Further settings:

- 1901: a float at +0x4C, 5,000 (4,000 in one hub). *Interpretation:* a radius [I].
- 1902: a screen object (class 158) at +0x50. The value 9912 is the shared steps screen, "X take 1 pace" / "O next
  clue/dig" ([user-interface.md](user-interface.md) 3).
- 1903: a float at +0x268, 400. *Interpretation:* **the length of one pace** [I].
- 1906: a command script (class 135) at +0x288. The value 4028 is in each map's group.

Trail rules [O] [I]: with the map held, the clue tells the player how many paces to take, X takes one pace and O
moves on to the next clue or digs. Reaching the last point unburies the treasure. The exact rules are open. The marker factories (class 127, option 1911: 256 or 16) make the dots
and the X along the trail [D]. Behaviour [O]: with the Treasure Map item held, the trail of dots leads to a buried
treasure; class 108 (`p_treasure_at_sea`, options 978 = (75, 100), 979 = (5, 10)) is the sea version.

## 8. Test vectors [D]

- Loot record 0 = (21, 421, 1255) "Bronze Comb"; record 63 = (736, 484, 1318) "Golden Vase".
- Coin option 964 = (4.0, 5.0) on all 39 coin templates.
- `0x346FD8` = −2250.0.

## 9. Open questions

1. Options 959, 1258, 1262, 1266 to 1268, 1901 to 1906 in detail.
2. How the treasure map chooses its trail and what it awards.
3. The vault combinations and rewards.
4. Every [R] rule needs checking in play.
