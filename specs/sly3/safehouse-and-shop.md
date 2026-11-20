---
id: sly3-behaviour-safehouse-and-shop
title: Sly 3 safe house and shop - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), the six worlds holding a safe house (v_ext, o_ext, h_ext, c_ext, p_ext, p_at_sea) and
      all other worlds for the shared groups; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: >-
      the safe-house object, its options and the option blocks of its placements; the command scripts those name;
      the shop screen object; the item table's prices and episodes (0x343098) [X]
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/shop.cpp, game/gen (class and option correspondence tables)
    used_for: >-
      the Sly 2 to Sly 3 correspondence of the safe-house class and options (rows scored 80 or more), and the
      statement that Sly 3 runs Sly 2's safe-house and shop behaviour; marked [R]
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 safe house and shop

Read [../sly2/safehouse-and-shop.md](../sly2/safehouse-and-shop.md) first; the behaviour carries over [R]. This
document gives Sly 3's objects, options and data. Characters and the item table: [characters.md](characters.md) 3.1
and 6; gadget slots: [gadgets.md](gadgets.md) 2; loot and coins: [pickpocket-and-pickups.md](pickpocket-and-pickups.md).
Tags as in [guards.md](guards.md).

## 1. The safe-house object [D]

- Class **69** (Sly 2's 70), group `safe_house`, placed by a placement with object ID **15065** in each hub's rig
  group (`v_ext_rig`, `o_ext_rig`, …). Six worlds: `v_ext`, `o_ext`, `h_ext`, `c_ext`, `p_ext`, `p_at_sea`
  (Kaine Island has none). The group's body is 83,444 bytes in `v_ext`, 68,390 in `o_ext`, `h_ext` and `c_ext`,
  53,336 in the two pirate worlds.
- Options, the same in all six worlds:

  | Option | Sly 2 | Value | Meaning |
  |---:|---:|---|---|
  | 13 | 13 | 3000.0 | activation radius |
  | 1516 | 1337 | object 44674 | the state machine that seats the characters and the computer [R] |
  | 1518 | 1339 | list (25, 27, 24, 26) | the **selectable states** at the start [R for the meaning] |
  | 1517 | 1338 | (27, buttons `0x40`, object 1065) | an **action**: in state 27, Cross opens screen object 1065, the shop [R for the meaning] |
  | 8 | 8 | three entries (the first values 2153, 2155, 2154; then −1 each) | not established |

- **States and characters**: the states 24, 25 and 26 have the same numbers as the object IDs of Sly (24, class 52),
  Bentley (25, class 54) and Murray (26, class 53) in every hub, and as the owner codes of the item table [D];
  *interpretation:* state 24 seats Sly, 25 Bentley, 26 Murray, and 27 is the computer [I]. The list on the disc
  offers all four: Bentley, the computer, Sly and Murray, in that order [D]. (An earlier reading listed only three
  because the survey tool cut lists at three entries.) Scripts may still narrow it per story stage, as in Sly 2 [R].
- **Placement actions** (option 1517 in the placement's option blocks, five hubs, the same in each):

  | State | Button | Target | What the target does [D] |
  |---:|---|---:|---|
  | 24 | Cross (`0x40`) | command script 44600 | starts command script 2061 with Sly as its object |
  | 25 | Cross | 44602 | starts 2061 with Bentley |
  | 26 | Cross | 44598 | starts 2061 with Murray |

  The three targets are one-command scripts in the hub's rig group; 2061 is a command script of the episode's macro
  group (one per episode, same object ID). In `v_ext` it sets the states of four tasks, removes the door's use point
  (object 15152, class 33), opens the door (a state-machine goal on object 139038), walks the character out to two
  markers under two camera keys, closes the door and adds the use point back [D for the commands; R for their
  meanings, [scripts.md](scripts.md) 7.2]. *Interpretation:* this is "take this character out into the hub", and the
  door's use point is how the player enters [I].
- `p_at_sea` has the safe house but **no** placement actions and no scripts 44598 to 44602 [D]: the ship's safe house
  offers only the computer [I]. There the placement 15065 is **nested in the gang's ship** (`p_boat_hero`, class 88),
  as a filtered record: world filter (32), condition (17) [D]. It exists only while the world's condition is 17
  ([world-file.md](world-file.md) 6).

## 2. Being in the safe house [R]

As Sly 2 section 2 (no damage while the safe house is the current player; on entering, a running job not marked
"keep" ends, characters are restored, gadget objects are removed; left and right step through the selectable
states, wrapping; the action of the shown state fires; scripts may set the list, at most 6 states). Characters
switch only here in hubs ([characters.md](characters.md) 6). Arrival at a hub's default point hands control to the
safe house once the episode's first jobs are done ([characters.md](characters.md) 2) [R].

## 3. The shop [R] [X]

- The shop is screen object **1065** (class 158, group `stock_swf`, present in every world) [D]. Its behaviour is
  Sly 2 section 3 (pauses the game; lists open, unowned items in table order; a confirmation before buying; a
  message when coins are short) [R].
- **What is sold** [X, item table `0x343098`]: the 34 records with an episode (field +20) other than −1, each from
  its episode on. Prices total **23,700** coins: Sly 18 items, 12,100; Bentley 9 items, 5,900; Murray 7 items,
  5,700. Per episode:

  | Episode | Items (record numbers) | Coins |
  |---:|---|---:|
  | 3 Venice | 8, 9, 25, 26, 31, 47 | 1,700 |
  | 4 Outback | 10, 11, 18, 19, 27, 35, 42, 44 | 3,200 |
  | 5 Holland | 12, 13, 20, 21, 28, 32, 43, 48 | 5,200 |
  | 6 China | 14, 15, 22, 23, 29, 33, 45, 49 | 8,200 |
  | 7 pirates | 16, 24, 34, 46 | 5,400 |

  Nothing opens in episode 8. Field +24 is 14 on 15 of these and 15 on the other 19 [X]; *interpretation:* the shop
  page or row group [I].
- Items never sold (episode −1): the starting gadgets (Trigger Bomb 6, Fishing Pole 7, Be The Ball 17) and the
  others granted by the story [D] [O].
- Sly 2's Sell page sold loot. Sly 3's shop screen has **no sell page**: its streamed file holds only buying texts
  ("Connecting to ThiefNet…", "Buy Gadgets", "Available Gadgets") [D: [user-interface.md](user-interface.md) 2];
  loot becomes coins as described in [pickpocket-and-pickups.md](pickpocket-and-pickups.md) [R].

## 4. Gadget assignment

Slots and buttons: [gadgets.md](gadgets.md) 2 (L1, L2, R2; table `0x349040`) [X]. The assignment screen's grids are
three 32-byte-cell tables at `0x348BE0` (Sly, screen 1062), `0x348DC0` (Bentley, 1063) and `0x348F20` (Murray, 1064):
[gadgets.md](gadgets.md) 2 [X].

## 5. Test vectors [D]

- Safe house in `v_ext`: class 69, placement 15065, group body 83,444 bytes.
- Option 1517 on the safe house: (27, `0x40`, 1065); in the placement blocks: (24, `0x40`, 44600), (25, `0x40`,
  44602), (26, `0x40`, 44598).
- Option 1518: (25, 27, 24, 26).
- Script 44600 (`v_ext_rig`): commands `0x00` (argument 0), `0x17` (object 24, wait flag, argument 2061), `0x01`.

## 6. Open questions

1. The state machine 44674's states (it lies inside the safe-house body), to confirm 24 to 27.
2. Option 8's three entries.
3. Whether scripts narrow the selectable list per story stage.
4. Field +24.
5. All [R] statements need checking in play.
