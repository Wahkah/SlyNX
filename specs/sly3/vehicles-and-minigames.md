---
id: sly3-behaviour-vehicles-and-minigames
title: Sly 3 vehicles, turrets, hacking, locks and minigames - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), all 39 worlds walked; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: records of classes 68 and 73 to 88 (every depth) with their worlds, groups and option values; option kinds and fields from the option table at 0x32CA80
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/hack.cpp, turret.cpp, vehicle.cpp and game/gen (class and option correspondence tables)
    used_for: >-
      the Sly 2 to Sly 3 correspondence of classes and options (rows the reference scores 80 or more), and the
      statement that Sly 3 runs Sly 2's behaviour for these classes; marked [R]
  - kind: executable
    what: >-
      SCUS_974.64 machine code read on 2026-11-18: the function tables of classes 81 to 88 (class table 0x3409E0)
      compared with the plain solid's; their set-up and per-frame routines, the controller reads and the defaults
    used_for: section 7
  - kind: executable
    what: >-
      SCUS_974.64 machine code, second pass 2026-11-18 (hash checked): per-frame control of classes 82, 83, 84, 87
      and 88 (biplane routines 0x141580, 0x142E18, 0x144A50/0x144A70; raft 0x20EE58, 0x20FC58, 0x20F3C0, 0x20FB58;
      jolly boat 0x2100E8; swimmer 0x26C088, 0x26D470, 0x26DAB8; ships 0x216020, 0x216350, 0x219000, 0x219058) and the
      constant records they read (0x317530, 0x317550, 0x328B48 to 0x328BD0, 0x341FF0)
    used_for: sections 7.0.1, 7.2.1, 7.3.1, 7.3.2, 7.4.1, 7.5.1 and the second-pass open list in 7.6
  - kind: executable
    what: >-
      SCUS_974.64 machine code, third pass 2026-11-18 (hash checked): biplane computer flight 0x143E50, 0x144580,
      target choice 0x1448B0, threat choice 0x144640, firing 0x144E40, 0x144A90, 0x142D20; swimmer states 0x26C088
      to 0x26C390, 0x26D1E8, 0x26D450; ships 0x219000, 0x219058, set-up 0x2139D0, record routines 0x2151A0,
      0x215160, 0x215198, 0x217F10, 0x219420, 0x2194C0; option-table entries 1686, 1701 to 1703, 1954 to 1961, 1989,
      2005 at 0x32CA80; constants 0x3200D0 to 0x320110
    used_for: sections 7.2.2, 7.4.2, 7.5.2 and the third-pass open list in 7.6
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 vehicles and minigames

Read [../sly2/vehicles-and-minigames.md](../sly2/vehicles-and-minigames.md) first. Sly 3 keeps Sly 2's turret and
hacking classes (renumbered) and adds new driven machines: biplanes, ships, a raft, a jolly boat, a truck, remote
cars and a swimmer. The reference project carries over Sly 2's turret and hacking code but **does not implement
Sly 3's new machines**; for those this document gives the disc data and observable roles only, and their behaviour
is an open question. Tags as in [guards.md](guards.md). "Kind" is the option's value type and the object field it
writes, read from the option table ([scripts.md](scripts.md)); a value type of `0x2000` plus a class number is shown
as "object of class N" (*interpretation:* a reference that must be of that class [I]); option meanings without a Sly 2 counterpart are open.

## 1. Which job uses what [D] [O]

| Class | Sly 2 class [R] | Groups (records) | Worlds | Role [O] |
|---:|---:|---|---|---|
| 68 | 69 | `c_turret` (1, nested) | `c_ext` | a mounted gun |
| 73 | 71 | `hack_color_lock`, `hack_gravity_well`, `hack_shield…`, `hack_gateway`, `hack_dock` (11 + 2 nested) | `h_ext`, `h_hack`, `c_hall_a`, `c_hall_b` | hacking: fixed pieces (locks, wells, shields) |
| 74 | 72 | `hack_ship` (3), `hack_ship_multi` (1) | the same four | hacking: the player's ship |
| 75 | 73 | `hack_enemy…`, `hack_turret`, `hack_gunship`, `hack_eye_turret`, `hack_mouth_turret`, `hack_prince`, `hack_mp_…` (22) | the same four | hacking: enemies and turrets |
| 76 | 74 | `hack_bullet…`, `hack_ship_bullet`, `hack_mine`, `hack_black_hole` (16) | the same four | hacking: shots and mines |
| 77 | 75 | `hack_lock` (3) | `h_ext`, `c_hall_a`, `c_hall_b` | hacking: the exit lock |
| 78 | 76 | `h_ext_hack`, `c_hall_a_webcam`, `c_hall_b_stealing_thiefnet` (10, nested) | `h_ext`, `c_hall_a`, `c_hall_b` | the terminal that starts a hacking game |
| 79 | — | `o_lock` (2), `v_cell_lock`, `h_truck_lock` | `o_cave_a`, `o_cave_b`, `v_interpol`, `h_ext` | a lock opened by a minigame |
| 80 | — | `h_wall_safe`, `m_vault_controls` (3, nested) | `h_ext`, `m_vault` | a safe's or vault's controls |
| 81 | — | `o_truck_player` (1) | `o_quarry` | the truck the player drives |
| 82 | — | `h_biplane_hero_multi` (2), `h_biplane_enemy`, `m_biplane_hero`, `m_biplane_enemy` | `h_ext`, `h_dogfight`, `m_ext` | biplanes: the player's and enemies |
| 83 | — | `h_raft` (2) | `h_ext`, `h_sewer` | the raft |
| 84 | 77 (low confidence) | `p_jolly_boat` (1) | `p_ext` | the jolly boat |
| 85 | — | `c_rccar` (5), `m_van_suv`, `m_robot_car` | `c_ext`, `c_intro`, `c_tilt_hall`, `p_ext`, `m_ext` | the remote-controlled car and vans |
| 87 | — | `dmitri_swimmer` (2) | `p_dive`, `m_underwater` | Dimitri swimming |
| 88 | — | `p_boat_hero` (4), `p_boat_galleon_medium` (2), `p_boat_deaths_head`, `p_boat_galleon_small_battle`, `p_boat_galleon_medium_standin` | the four pirate worlds | sailing ships: the gang's and enemies |

The `…_multi` and `hack_mp_…` templates and the world `h_hack` belong to the two-player modes [I]. Sly 2's dance
(classes 68 and 121) and combat car have no Sly 3 counterpart on the disc [D].

## 2. Turret (class 68)

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 53 | method (2 args) | 2 | list (67108863, 0) ×1; list (256, 2) ×1 | 53 |
| 27 | method (0 args) | 1 | list () ×1 | 26 |
| 1094 | float pair at +0x1AC | 1 | (-0.78,0.78) ×1 | 1000 |
| 1095 | float pair at +0x1B4 | 1 | (-0.78,0.78) ×1 | 1001 |
| 1103 | bool at +0x1E4 | 1 | 0 ×1 | 1009 |
| 1104 | float pair at +0x1BC | 1 | (0.8,0.8) ×1 | 1010 |

As Sly 2 section 3 [R]: 1094 / 1095 are the tilt and pan limits (±0.78 rad here), 1104 the field-of-view range,
1103 the smoothing flag (0 here) [R for the meanings; D for the values].

## 3. Hacking (classes 73 to 78)

The rules are Sly 2 section 4 [R]. Option tables [D]:

Fixed pieces (73):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 8 | method (2 args) | 6 | list (2310, 76124) ×3; list (2312, -1) ×3 | 8 |
| 53 | method (2 args) | 5 | list (256, 2) ×5 | 53 |
| 13 | setter | 4 | 10000 ×4 | 13 |
| 937 | setter | 4 | 5 ×4 | — |
| 1552 | setter | 4 | 10 ×4 | — |
| 1553 | object of class 28 at +0x198 | 4 | 74150 ×4 | 1366 |
| 1557 | object at +0x1D0 | 4 | -1 ×2; 68931 ×2 | — |
| 1558 | setter | 4 | 3000 ×2; 2500 ×1; 2250 ×1 | — |
| 1559 | float pair at +0x1D8 | 4 | (200,1000) ×2; (0,500) ×1; (0,2000) ×1 | — |
| 39 | setter | 3 | 1 ×3 | 39 |

The ship (74):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 3 | method (2 args) | 12 | list (2319, 0) ×3; list (2317, 0) ×3; list (2318, 0) ×3 | 3 |
| 1563 | method (3 args) | 8 | list (2, 74176, 0) ×4; list (6, 9170, 0) ×4 | 1372 |
| 33 | setter | 4 | 5 ×4 | 33 |
| 34 | setter | 4 | 4 ×4 | 34 |
| 53 | method (2 args) | 4 | list (256, 2) ×4 | 53 |
| 937 | setter | 4 | 100 ×3; 20 ×1 | — |
| 1557 | object at +0x1D0 | 4 | 68931 ×4 | — |
| 1560 | float at +0x1E0 | 4 | 1 ×4 | — |
| 1562 | method (2 args) | 4 | list (74182, 74178) ×4 | 1371 |
| 1564 | type 0xE at +0x304 | 4 | 5702 ×4 | 1374 |
| 8 | method (2 args) | 3 | list (2463, -1) ×1; list (2464, -1) ×1; list (2477, -1) ×1 | 8 |
| 948 | setter | 3 | 74160 ×3 | — |
| 1566 | int at +0x284 | 1 | 16 ×1 | — |

Enemies (75):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 1574 | method (3 args) | 34 | list (3, 000000000000fa44, 0000000000401c45) ×12; list (1, 0000000000007a44, 0000000000000000) ×6; list (2, 0000000000007a44, 0000000000000000) ×6 | 1380 |
| 1569 | method (3 args) | 32 | list (4, 74176, 0) ×17; list (2, 15430, 0) ×10; list (5, 78415, 0) ×3 | 1375 |
| 53 | method (2 args) | 22 | list (256, 2) ×22 | 53 |
| 937 | setter | 22 | 30 ×9; 5 ×5; 100 ×2 | — |
| 1553 | object of class 28 at +0x198 | 22 | 74150 ×22 | 1366 |
| 33 | setter | 14 | 5 ×14 | 33 |
| 1570 | setter | 14 | 1 ×9; 0 ×3; 0.05 ×2 | 1376 |
| 1576 | float at +0x3A0 | 14 | 400 ×4; 750 ×3; 200 ×2 | 1382 |
| 1578 | type 0x9 at +0x3C0 | 14 | (1,-1,0) ×7; (1,-0.5,0) ×4; (1,0,0) ×3 | 1384 |
| 1571 | setter | 12 | 0.5 ×12 | 1377 |
| 5 | method (2 args) | 10 | list (2321, 1144750080) ×5; list (2321, 1147207680) ×5 | 5 |
| 1560 | float at +0x1E0 | 10 | 1 ×8; 0.2 ×1; 0 ×1 | — |
| 1575 | method (3 args) | 10 | list (2, 000000bf0000003f, 0000000000000000) ×9; list (1, c3f548c0c3f54840, 0000000000000000) ×1 | 1381 |
| 1552 | setter | 9 | 10 ×9 | — |
| 1555 | type 0x9 at +0x1B0 | 9 | (0,10,0) ×5; (0,20,0) ×4 | 1369 |
| 1556 | float pair at +0x1C0 | 9 | (0,1000) ×9 | 1370 |

Shots (76):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 13 | setter | 16 | 10000 ×16 | 13 |
| 33 | setter | 16 | 5 ×16 | 33 |
| 1553 | object of class 28 at +0x198 | 16 | 74150 ×16 | 1366 |
| 53 | method (2 args) | 4 | list (256, 2) ×4 | 53 |
| 1557 | object at +0x1D0 | 4 | 68931 ×4 | — |
| 1558 | setter | 3 | 4000 ×3 | — |
| 1559 | float pair at +0x1D8 | 3 | (20,1000) ×3 | — |
| 1587 | float at +0x1F4 | 3 | 0 ×3 | 1389 |

Lock (77):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 1591 | method (3 args) | 3 | list (1, 74225, 0) ×3 | 1392 |
| 1593 | object of class 3 at +0x1FC | 3 | 74224 ×3 | 1394 |

Terminal (78):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 1596 | method (4 args) | 20 | list (74352, 8, 3, 25.0) ×2; list (78455, 8, 3, 25.0) ×2; list (78457, 8, 3, 25.0) ×2 | 1398 |
| 39 | setter | 10 | 1 ×10 | 39 |
| 53 | method (2 args) | 10 | list (256, 2) ×10 | 53 |
| 69 | float at +0x90 | 10 | 500 ×10 | 67 |
| 86 | setter | 10 | 1 ×10 | — |
| 1552 | setter | 10 | 10 ×10 | — |
| 1553 | object of class 28 at +0x198 | 10 | 74148 ×10 | 1366 |
| 1597 | object of class 47 at +0x1F0 | 10 | 74378 ×10 | 1399 |

937 is the hit points option of Sly 3 (as for guards, [guards.md](guards.md)) [I]; 1553 names the hacking game's
controller object (74150 in all four worlds, 74148 on the terminals) [D]. New Sly 3 pieces: colour locks and keys,
gravity wells, black holes, shields, eye and mouth turrets, a prince [D]; their rules are open.

## 4. Locks and safes (classes 79, 80) [D]

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 13 | setter | 4 | 4000 ×4 | 13 |
| 1602 | object of class 45 at +0x1D8 | 4 | object/id ×4 | — |
| 1603 | object of class 45 at +0x1DC | 4 | object/id ×4 | — |
| 1604 | type 0xE at +0x1E0 | 4 | d601 ×4 | — |

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 1610 | setter | 9 | 38241 ×3; 38239 ×3; 38231 ×3 | — |

*Interpretation:* 79 is the lock-picking minigame (Venice cell, Outback caves, Holland truck) [I] [O]. Class 80 is
the safe controller with its tumblers: combination, input and states in
[pickpocket-and-pickups.md](pickpocket-and-pickups.md) 6.1 [X].

## 5. Driven machines

None of these has a Sly 2 counterpart [R]. Options 33, 34, 35, 61, 62, 82, 83 are the shared physics options of
Sly 2's solid objects (35 = gravity: (0, 0, −4500) on the truck, −4000 on the jolly boat, −900 on the remote cars)
[R for the meanings; D for the values].

Truck (81):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 1617 | method (3 args) | 4 | list (0, 1134559232, 0000c8c200007042) ×1; list (1, 1134559232, 0000c8c200007042) ×1; list (2, 1134559232, 0000c8c200007042) ×1 | — |
| 33 | setter | 1 | 0 ×1 | 33 |
| 34 | setter | 1 | 0 ×1 | 34 |
| 35 | setter | 1 | (0,0,-4500) ×1 | 35 |
| 82 | setter | 1 | 1000 ×1 | 80 |
| 83 | setter | 1 | 100 ×1 | 81 |
| 937 | setter | 1 | 200 ×1 | — |
| 948 | setter | 1 | object/id ×1 | — |
| 1618 | object of class 45 at +0x1E0 | 1 | object/id ×1 | — |
| 1619 | object of class 45 at +0x240 | 1 | object/id ×1 | — |
| 1620 | object of class 45 at +0x2A0 | 1 | object/id ×1 | — |
| 1621 | object of class 45 at +0x300 | 1 | object/id ×1 | — |
| 1622 | float pair at +0x35C | 1 | (250,2000) ×1 | — |
| 1623 | object of class 45 at +0x364 | 1 | object/id ×1 | — |
| 1624 | float at +0x334 | 1 | 0.9 ×1 | — |
| 1625 | float at +0x32C | 1 | 3500 ×1 | — |

Biplanes (82):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 53 | method (2 args) | 6 | list (256, 2) ×3; list (512, 2) ×3 | 53 |
| 1701 | method (2 args) | 6 | list (37418, -1) ×2; list (37416, 24805) ×2; list (35487, 35510) ×1 | — |
| 1728 | setter | 6 | 35545 ×3; 35542 ×3 | — |
| 33 | setter | 5 | 0 ×5 | 33 |
| 35 | setter | 5 | (0,0,0) ×5 | 35 |
| 36 | float at +0x114 | 5 | 2000 ×5 | 36 |
| 1684 | type 0x1072 at +0x190 | 5 | 0 ×3; 1 ×2 | — |
| 1699 | object of class 133 at +0x240 | 5 | 6701 ×5 | — |
| 1738 | object of class 45 at +0x384 | 4 | 35411 ×3; 37456 ×1 | — |
| 8 | method (2 args) | 3 | list (1823, 24) ×2; list (2011, -1) ×1 | 8 |
| 32 | setter | 3 | 1 ×3 | 32 |
| 61 | float at +0xD4 | 3 | 1 ×3 | 59 |
| 1694 | float at +0x1B8 | 3 | 0.45 ×2; 0.6 ×1 | — |
| 1698 | object of class 8 at +0x23C | 3 | 37426 ×2; 35491 ×1 | — |
| 1702 | float at +0x27C | 3 | 9 ×2; 20 ×1 | — |
| 1703 | float at +0x280 | 3 | 10000 ×3 | — |

Raft (83):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 33 | setter | 2 | 0 ×2 | 33 |
| 34 | setter | 2 | 0 ×2 | 34 |
| 53 | method (2 args) | 2 | list (67108863, 2) ×2 | 53 |
| 61 | float at +0xD4 | 2 | 2 ×2 | 59 |
| 62 | float at +0xD8 | 2 | 0.5 ×2 | 60 |
| 948 | setter | 2 | 35613 ×2 | — |
| 1792 | object of class 45 at +0x200 | 2 | 21014 ×2 | — |

Jolly boat (84):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 33 | setter | 1 | 0 ×1 | 33 |
| 34 | setter | 1 | 0 ×1 | 34 |
| 35 | setter | 1 | (0,0,-4000) ×1 | 35 |
| 53 | method (2 args) | 1 | list (67108863, 2) ×1 | 53 |
| 61 | float at +0xD4 | 1 | 1.5 ×1 | 59 |
| 62 | float at +0xD8 | 1 | 0.5 ×1 | 60 |
| 937 | setter | 1 | 50 ×1 | — |
| 948 | setter | 1 | 7465 ×1 | — |
| 1778 | vector at +0x1A0 | 1 | (0.0001,0.005,0.015) ×1 | — |
| 1779 | float at +0x1B4 | 1 | 0.5 ×1 | — |
| 1783 | float at +0x1C0 | 1 | 25000 ×1 | — |
| 1784 | float at +0x1C4 | 1 | 1.2 ×1 | — |
| 1792 | object of class 45 at +0x200 | 1 | 21014 ×1 | — |
| 1793 | object of class 47 at +0x218 | 1 | 62595 ×1 | — |
| 1794 | object of class 3 at +0x21C | 1 | 35353 ×1 | — |
| 1795 | object at +0x27C | 1 | 62578 ×1 | — |

Remote cars and vans (85):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 1809 | method (3 args) | 28 | list (0, 1101004800, 000070c100002041) ×5; list (1, 1101004800, 000070c100002041) ×5; list (2, 25.0, 000070c100002041) ×5 | — |
| 1831 | setter | 10 | 35487 ×5; 35489 ×5 | — |
| 1832 | setter | 10 | 38616 ×5; 38618 ×5 | — |
| 33 | setter | 7 | 0 ×7 | 33 |
| 34 | setter | 7 | 0 ×7 | 34 |
| 35 | setter | 7 | (0,0,-900) ×6; (0,0,-2000) ×1 | 35 |
| 61 | float at +0xD4 | 7 | 1 ×7 | 59 |
| 82 | setter | 7 | 80 ×6; 260 ×1 | 80 |
| 83 | setter | 7 | 1 ×7 | 81 |
| 1810 | object of class 45 at +0x1E0 | 7 | 34733 ×7 | — |
| 1811 | object of class 45 at +0x240 | 7 | 34731 ×7 | — |
| 1812 | object of class 45 at +0x2A0 | 7 | 34729 ×7 | — |
| 1813 | object of class 45 at +0x300 | 7 | 34727 ×7 | — |
| 1814 | float at +0x334 | 7 | 0.19 ×5; 0.3 ×1; 0.75 ×1 | — |
| 1815 | float at +0x32C | 7 | 900 ×5; 1800 ×1; 1600 ×1 | — |
| 1816 | float at +0x330 | 7 | 900 ×6; 1800 ×1 | — |

Swimmer (87):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 32 | setter | 2 | 1 ×2 | 32 |
| 1919 | object of class 8 at +0x234 | 2 | 27729 ×2 | — |
| 1922 | object of class 133 at +0x240 | 2 | 6701 ×2 | — |
| 1923 | object of class 158 at +0x2C0 | 2 | 27682 ×2 | — |

Ships (88):

| Option | Kind [X] | Uses | Commonest values [D] | Sly 2 option [R] |
|---:|---|---:|---|---:|
| 1989 | method (4 args) | 92 | list (1, 44948, 37194, 44934) ×5; list (1, 44946, 37194, 44934) ×5; list (1, 44944, 37194, 44934) ×5 | — |
| 2005 | method (1 args) | 72 | list (0000fac400803bc4000048c3) ×9; list (0000fac400803b44000048c3) ×9; list (008009c400007ac4000096c3) ×9 | — |
| 1954 | method (1 args) | 18 | list (4) ×9; list (2) ×9 | — |
| 1955 | float at +0x4 | 18 | 750 ×5; 3000 ×5; 1000 ×4 | — |
| 1956 | float at +0x8 | 18 | 10000 ×18 | — |
| 1957 | float at +0x28 | 18 | 0.1 ×9; 0.5 ×9 | — |
| 1958 | type 0xB at +0x1C | 18 | (0.1,0,0.25) ×9; (0.6,0,0.25) ×9 | — |
| 1959 | float at +0x18 | 18 | 0.9 ×10; 1.58 ×4; 0.8 ×4 | — |
| 1960 | type 0xB at +0xC | 18 | (1.4,0,0.1) ×18 | — |
| 1961 | method (0 args) | 18 | list () ×18 | — |
| 2013 | method (1 args) | 16 | list (48) ×8; list (88) ×8 | — |
| 8 | method (2 args) | 15 | list (2157, -1) ×4; list (2156, -1) ×4; list (2160, 44708) ×4 | 8 |
| 2014 | method (1 args) | 10 | list (50623) ×5; list (51594) ×5 | — |
| 53 | method (2 args) | 9 | list (256, 2) ×9 | 53 |
| 1968 | method (2 args) | 9 | list (250, 250) ×4; list (100, 100) ×3; list (200, 200) ×2 | — |
| 1979 | float at +0x220 | 9 | 4500 ×5; 4000 ×4 | — |

*Interpretation* [I]: the truck, biplanes, ships, raft and jolly boat are driven by the player in their jobs; the
remote car (`c_rccar`) is Penelope's; the swimmer is Dimitri's diving form. Which job drives which needs the
task records (missions).

## 6. Open questions

1. The behaviour of every class in section 5 (controls, speeds, damage, cameras): read their update code or observe
   in an emulator; no reference implements them.
2. The new hacking pieces' rules, and whether the two-player modes are reachable from the menus.
3. Classes 79 and 80: the minigames' inputs and rules.
4. The meanings of the options marked "—".

## 7. Executable findings: driven machines (classes 81 to 88)

From the machine code of SCUS_974.64 (hash checked), 2026-11-18. Each class's own routines were found by comparing
its function table (from the class table at `0x003409E0`) with that of the plain solid object; the defaults below are
the values a machine's set-up routine writes before its options are read, so a disc option (section 5) overrides
the default at the same field. Field offsets are given only to tie defaults to the options of section 5 [X].
Controller buttons use the 16-bit mask of the dialogue spec: L2 1, R2 2, L1 4, R1 8, Triangle 16, Circle 32,
Cross 64, Square 128, L3 512 [X for the masks used; I for the button names].

#### 7.0.1 How the controller is read (second pass)

All five machines below read the same controller record as the truck: a pointer at object +0x134. In it, +0x0 and
+0x4 are the left stick's x and y, +0x8 the left stick's magnitude, +0xC and +0x10 the right stick, and +0x18 the
held-button mask (masks as in the introduction to section 7) [X]. A shared routine (`0x001A7A80`) answers "was a
button of this mask pressed within the last t seconds", and is how most presses are taken (t = 0.15 s or 0.1 s
below) [X]. A machine may read the controller of the object that drives it rather than its own (noted per
machine) [X].

### 7.1 Truck (81) and remote cars (85)

The two classes share one field layout and one control scheme (set-up `0x00279DD8` and `0x00203C18`; per-frame
control `0x0027A0B0` and `0x00204048`) [X].

| Field | Option (section 5) | Default | Disc values | Meaning [X unless marked] |
|---|---:|---:|---|---|
| +0x32C | 1625 (truck), 1815 (cars) | 1,100 | truck 3,500; cars 900, 1,800, 1,600 | forward drive force while Cross is held |
| +0x330 | 1816 (cars) | 700 | cars 900, 1,800 | reverse drive force while Circle or Square is held |
| +0x334 | 1624 (truck), 1814 (cars) | 0.8 | truck 0.9; cars 0.19, 0.3, 0.75 | steering limit in radians |
| +0x338 | — | 1.5 | — | not traced |
| +0x35C, +0x360 | 1622 (truck) | 250, 2,000 | 250, 2,000 | a distance range (not traced) |
| +0x3C0 | — | 100 | — | look-ahead along the followed curve (computer driving) |

Control, by the machine's mode word (+0x3B0) [X]:

- **Mode 1, driven by the player.** The steering angle eases toward −(stick x) × steering limit, through the
  smoothing record at `0x003292E8`. Cross gives the forward force; otherwise Circle or Square gives the reverse
  force; with neither the force is 0. On the remote cars, a press of L3 (counted for 0.15 s) plays sound 42 (a horn
  [I]); the truck's routine has no L3 check.
- **Modes 2 and 3, computer driving.** The machine finds the nearest point of its curve (+0x3B8), looks ahead by
  +0x3C0 along it, and steers toward that point by the angle atan2 of its local offset (mirrored in mode 2), wrapped
  to ±π and limited to ± the steering limit.
- **Mode 0** sets steering and force to 0.
- The resulting steering angle is written to the wheel parts; the four wheel objects are options 1618 to 1621
  (truck) and 1810 to 1813 (cars), class 45 [D].
- The cars' engine sound falls off from 150 to 1,000 units (`0x00204E00`) [X]. Option 35 gives the gravity (truck
  −4,500, cars −900 or −2,000) [D].

### 7.2 Biplanes (82)

Set-up `0x00141058` writes these defaults (angles in radians: exactly 15°, 30°, 65° and 70°) [X]; the flight model
itself is spread over seven per-frame routines called from `0x00141500` and is not traced.

| Field | Default | Option and disc values [D] |
|---|---:|---|
| +0x19C | 1,800 | — |
| +0x1A0 | 1.6 | — |
| +0x1A4 | 0.7 | — |
| +0x1A8 | 10,000 | — |
| +0x1AC | 1.22173 (70°) | — |
| +0x1B0 | 1.13446 (65°) | — |
| +0x1B4 | 0.261799 (15°) | — |
| +0x1B8 | 0.523599 (30°) | 1694: 0.45, 0.6 |
| +0x1BC | 2 | — |
| +0x1C8, +0x1D4 | 0.8, 0.6 | — |
| +0x1F0, +0x1F4 | 0.25, 4,000 | — |
| +0x27C | 10 | 1702: 9, 20 |
| +0x280 | 15,000 | 1703: 10,000 |

#### 7.2.1 Per-frame control (second pass)

The biplane's per-frame routine dispatches on a control mode at +0x198 [X]:

| Mode | Routine | What it is |
|---:|---|---|
| 0 | `0x00142E18` | flown by the player |
| 1 | `0x00143E50` | flown by the computer: picks a target, aims with the projectile-aim predictor and fires [X for the calls; I for the role] |
| 2 | `0x00144580` | flies toward a point (formation or path following) [X for the calls; I for the role] |

Player flight (mode 0) [X unless marked]:

- **Throttle.** The forward speed command (+0x400) is the cruise speed at +0x19C (default **1,800**). Holding
  **Cross** multiplies it by +0x1A0 (default **1.6**, so 2,880) and holding **Square** by +0x1A4 (default **0.7**,
  so 1,260), each only when its enable flag (+0x3CC for Cross, +0x3D0 for Square) is set.
- **Stick.** The left stick is read; when the plane has a gunner object (+0x4BC) with its own controller, that
  controller's right stick is read instead. Each axis is multiplied by a control-authority factor (0 to 1, ramped
  in over the take-off time at +0x2D4 through the limit record `0x00341FF0`), and an axis below **0.2** in
  magnitude counts as 0.
- **Bank and heading.** Stick x sets the bank command to x × +0x1B0 (default **65°**) and moves the heading target
  to the current heading − x × +0x1B8 (default **30°**; option 1694 sets 0.45 or 0.6 rad on the disc).
- **Pitch.** Stick y sets the pitch target to the current pitch + y × +0x1B4 (default **15°**), limited to
  ±+0x1AC (default **70°**).
- **Speed ramp.** With a take-off time at +0x2D4 set, the speed is eased in over that time from the start time at
  +0x2C8 with the gain at +0x2D0; the plane's velocity and angular velocity are then set directly (no forces).
  *Interpretation:* the plane flies on rails of speed and angle rather than an aerodynamic model [I].
- **Guns.** Holding **R1** plays gun animation 9 through the plane's gun objects (+0x22C/+0x230) while a shot is
  available; releasing it stops the animation [X for the button and calls; I for "fire"].

#### 7.2.2 Computer flight and shots (third pass)

The control mode at +0x198 is option 1686 (an enumeration) [X]. In modes 1 and 2 the speed command and the bank,
heading and pitch targets are set by the computer; the same flight step as mode 0 then applies them [X].

**Mode 1, computer pilot.** It keeps a sub-mode at +0x394 and two picks, refreshed on timers [X]:

- **Chase target** (+0x3B8), re-chosen every **0.5 s**: among all biplanes of another side (side at +0x194), the
  one with the lowest score wins. The score is the distance, plus a penalty when the plane lies outside a cone of
  ±+0x1B8 (default 30°) around the nose: (1 − u)·**500** + u·**3,000**, with u = (bearing − cone)/(π − cone); the
  current target gets **−1,000** and a player-flown plane (mode 0) **−750** (constants at `0x320108`, `0x32010C`)
  [X]. *Interpretation:* the computer prefers what is ahead of it, keeps its target and goes for the player [I].
- **Threat** (+0x3C4), re-chosen every **0.25 s**: the nearest plane of another side that is player-flown or has
  this plane as its target (optionally only inside the zone object at +0x3C0); the current threat gets **−500**.
  The threat is dropped after **2.5 s** without one [X].
- **Sub-modes** [X]: 0, cruise (speed = cruise speed +0x19C); 1, chase, entered when there is a chase target;
  2, evade, entered when a threat has been on the plane for more than **1 s**, left when it is gone (back to 1 or 0).
- **Chase** [X]: at random intervals the plane re-aims at the target's predicted position, with the lead time and
  the next re-aim delay drawn from **0.25–1.5 s** (target flown by the player) or **0.25–0.5 s** (otherwise), and an
  aim offset of **−1,500** (player target) or **+4,500** along the target's axis (records at `0x3200D0` to
  `0x3200F0`). Heading and pitch targets point at the aim point, pitch limited to ±+0x1AC (70°). The speed blends
  from the cruise speed toward the fast speed (cruise × +0x1A0) as the distance grows, by distance × 0.0001 limited
  to 0..1 [X].
- **Evade** [X]: every **2 s** (1 s with probability 1/3) the plane picks one of 8 random manoeuvres: the heading
  target turns by ±0.6 × the turn step (+0x1B8) or stays, the pitch target is set to +70°, −70° or 0, and the speed
  is drawn between the slow (× +0x1A4) and fast (× +0x1A0) speeds, each only when that speed is enabled.
- **Avoidance** [X]: every 0.25 s, if another plane lies inside this plane's avoidance shape (+0x388), the pitch
  target is set to ±70° away from it. Below the world's minimum flying height or above its maximum (world record
  at `0x475480` + 19,992 to 20,004) the pitch target is limited so the plane climbs or dives back.
- The bank target is the heading error, limited to ±+0x1B0 (65°) [X].

**Mode 2, fly to a point** (`0x00144580`) [X]: heading and pitch toward the point at +0x3A0 (pitch limited to
70°), bank = heading error limited to 65°; the speed is left as it is; the plane fires as below.

**Shots (all modes)** [X]:

- **Guns.** Option 1701 adds gun entries (up to the count at +0x274, 12 bytes each from +0x244): a projectile
  template object and an optional effect object (disc: (37418, −1), (37416, 24805), (35487, 35510)) [D for the
  values]. Successive shots use the entries in turn.
- **A shot** takes a copy of the entry's template, places it at the nose and launches it with velocity = the
  plane's forward axis × +0x280 (default **15,000**; option 1703 sets **10,000** on the disc) plus the plane's own
  velocity; the shooter is recorded as the projectile's launcher. Damage is the projectile's own and is not traced
  here.
- **Rate.** The next shot may come 1 / +0x27C seconds later (default **10** shots per second; option 1702 sets
  **9** or **20**).
- **Player** (mode 0): holding R1 fires while ammunition remains (an object counter, decremented per shot unless
  +0x290 is set); with none, an empty sound plays at most every **0.5 s** and alarm 1922 is raised on +0x20C.
- **Computer**: fires in bursts. When a burst window ends, the next starts **0.5–1.5 s** later and lasts a further
  **1.5–2 s**; inside a window it fires only while its target is inside its firing zone (+0x3C0, +0x294) and on
  screen, checked every 0.25 s [X; I for "on screen"].

### 7.3 Raft (83) and jolly boat (84)

The jolly boat's function table reuses three of the raft's routines (`0x0020EDE0`, `0x0020F798`, `0x0020F7D8`) and
has its own set-up, placement and per-frame routines [X].

- Raft set-up `0x0020ED28` defaults: +0x1B0 1, +0x1B4 3, +0x1B8 20, +0x1BC 1, +0x1C0 6,000, +0x1C4 2.5 [X].
- Jolly boat set-up `0x0020FEF0` defaults: +0x220 0.4, +0x22C 0.5, +0x240 600, +0x250 600, +0x254 2,600, +0x274 1,000,
  +0x2A0 −270, +0x2B0 1.25, +0x2B4 2 [X]. Its disc options 1778 to 1784 (section 5) sit at +0x1A0 to +0x1C4.
- Raft only: unless its state word (+0x1CC) is 0 or 1, a press of L3 (0.15 s) plays sound 42 (per-frame routine
  `0x0020EE58`) [X].

#### 7.3.1 Raft per-frame control (second pass)

- **Paddle direction** [X]: the raft's own heading is the angle (atan2) of its forward axis. When the rider's
  left-stick magnitude (+0x8) is above **0.5**, the wanted heading is computed from the stick and that heading by
  the shared routine `0x00168108` (*interpretation:* the stick direction relative to the camera, as for running
  [I]); otherwise it is the current heading. It is stored at +0x204. The difference (wanted − current, wrapped to
  ±π) beyond **0.15 rad** to one side or the other selects the left or right stroke animation, otherwise straight.
- **Strokes** [X]: **Cross** pressed within **0.15 s** requests a forward stroke (flag +0x208); **Square** within
  0.15 s requests a backward stroke (flag +0x20C). With neither, the raft's sideways speed beyond ±**150** picks
  the drift animation. The animation for each (stroke, side) pair comes from the 3 × 3 table at `0x00348040`.
- **Forces** (the raft's physics hook, `0x0020F3C0`, applied after gravity; only in raft state 2 at +0x1CC) [X]:
  a forward stroke accelerates the raft along its local +X by +0x1C0 (default **6,000**) for one physics step; a
  backward stroke by −6,000. Each step the raft is also turned about world +Z toward the wanted heading (+0x204)
  with the rate at +0x1C4 (default **2.5**) and a smoothing time of **0.25 s**.
- **Tether** [X]: if two tether objects are set (+0x1D8, +0x1DC) and are farther apart than +0x1E0, the raft is
  pulled along the line between them by +0x1E4 × (1 − +0x1E0 / d) + +0x1E8 / d, d being their distance (a spring)
  [X; I for "tether"].
- **Path controllers** [X]: setting the raft's position or rotation also moves the controllers at +0x1D0 and
  +0x1D4, so scripted paths can carry it.

#### 7.3.2 Jolly boat per-frame control (second pass)

The jolly boat runs the raft's per-frame routine first (so its paddling, steering and horn are the raft's) and
then its own cannon routine `0x002100E8` [X]:

- **Cannon cycle** on **R1** (button bit 0x8), state at +0x210 [X]:
  - 0, idle: pressing R1 starts charging (state 1).
  - 1, charging: the charge grows from +0x240 as t·(+0x244 + t·+0x248) with the held time t, limited to
    +0x250 … +0x254 (defaults 600 … **2,600**); releasing R1, or reaching the upper limit, fires (state 2).
  - 2, fire: a projectile is taken from the ammunition template (+0x224), placed at the launch point (+0x21C,
    option 1794), given a ballistic velocity toward the aimed point with the parameter +0x220 (default 0.4), and
    launched; the time is noted (+0x234) and the boat enters state 3.
  - 3, cooldown: back to idle after +0x22C (default **0.5 s**).
- *Interpretation:* the charge is the shot's range in units [I].

### 7.4 Swimmer (87)

Set-up `0x0026BE28` defaults: turn limits ±1.5708 (+0x1E4, +0x1E8), −1.17 and −1.04 (+0x1EC, +0x1F0) and 1.17, 1.04
(+0x1FC, +0x204), 100 (+0x21C, +0x224), 3 (+0x22C), 1.3 (+0x208), 2 (+0x1B4, +0x200), 1 (+0x20C, +0x210, +0x230) and
0.5 (+0x214, +0x218) [X]. The per-frame routine `0x0026C088` reads three buttons: L3, Triangle and Cross; Triangle and
Cross each start a stroke routine with the value at +0x1E0 [X] (*interpretation:* surfacing or diving and the swim
stroke [I]).

#### 7.4.1 Per-frame control (second pass)

The swimmer's per-frame routine dispatches on a state at +0x190 (1 to 5, table `0x00328B00`) [X]. In every state
except 0 and 5, L3 plays sound 42.

- **Aim (state 1, routine `0x0026D470`)** [X]: with the stick magnitude above **0.2**, the aim pan and tilt
  (+0x260, +0x264) move by stick × (−2 × frame step), limited by the record at `0x00328B48` (±**0.8** rad, then
  200 and 60), at a fixed range of **4,000**. **Circle** and **Square** start the moves of records `0x00328BB0`
  (0, −1, 0) and `0x00328BD0` (0, 0, −1); the records at `0x00328BA0` and `0x00328BC0` hold the opposite directions.
- **Swim (state 2)** [X]: **Triangle** starts the move of record `0x00328B80` (+1, 0, 0) when the tilt (+0x1E0) is
  negative, and **Cross** the move of record `0x00328B90` (−1, 0, 0) when the tilt is positive, each with its own
  animation (+0x280, +0x284). *Interpretation:* a stroke that pushes the swimmer back toward level [I].
- **Steering (routine `0x0026DAB8`)** [X]: the stick is scaled by the pan and tilt rates (+0x1EC, default −1.17;
  +0x1F0, default −1.04, adjusted between the limits set up at +0x1FC and +0x204) and normalised. The actual turn
  rates (+0x1F4, +0x1F8) chase those targets with a turn acceleration of at most **12 rad/s²**; the pan (+0x1DC)
  and tilt (+0x1E0) integrate the rates, the pan wraps, and the tilt is limited to ±+0x1E4 (default **±π/2**).
  With no controller in charge it instead turns to look at its target object.

#### 7.4.2 Direction moves and states 3 to 5 (third pass)

The records at `0x328B80` to `0x328BD0` are unit vectors: +X, −X, +Y, −Y, +Z, −Z [X].

- **A stroke** (state 2): Triangle (when the tilt is below 0) or Cross (when above 0) stores the record +X or −X as
  the push direction (+0x1A0) and adds 1 to a push amount u (+0x19C) if u is below 0.5; it plays the stroke clip
  (+0x280 or +0x284) and switches to state 3 [X].
- **The push** [X]: every physics step u falls by 2 per second (kept within 0..1), and while u > 0 the swimmer is
  pushed along the push direction (turned into world space) with strength
  2,250 × (1 − 0.5·cos(π·u)) × +0x230 (× +0x22C when +0x228 is set); so 3,375 at u = 1, falling to 1,125 as u
  reaches 0. A positive push direction counts as "moving forward" for the animation.
- **State 3** [X]: after **0.5 s** the swimmer is moved to the point stored at +0x1C0 and state 4 starts.
  *Interpretation:* the point is a clear spot found by the routine at `0x0026D340`, which tests 400 units above the
  swimmer and then 10 below [I].
- **State 4** [X]: as state 1 (aim and steer), while animation 1009 plays on +0x2C0; when it ends, back to state 1.
- **State 5** [X]: no controller input at all (no aim, no steering, no L3 sound); only the passive updates run.

### 7.5 Ships (88)

Set-up `0x002139D0` defaults include 0.2618 (15°) at +0x3FC, 0.03491 (2°) and 0.13963 (8°) at +0x3F0 and +0x3F4,
20,000 and 7,500 at +0x428 and +0x42C, 30,000 at +0x3D0, 4,000 at +0x220 (option 1979: 4,500 and 4,000 on the disc),
a ladder of factors 0.3 to 1.55 at +0x1E0 to +0x210, and 100, 200, 200, 400 at +0x508 to +0x514 [X]. None of the
ship's own routines reads the controller directly [X]; *interpretation:* the ships are steered through scripts and
the per-ship gun records (options 1954 to 1961, 1989, 2005) rather than a stick mapping [I].

#### 7.5.1 Per-frame control (second pass)

Correction to 7.5: the ships do read a controller, through the shared press routine of 7.0.1 [X]. A ship is
steered with its own controller in state 2 and with the controller of the object at +0x1D0 in state 4 [X]. Its
main state is at +0x1B0 and a sub-state at +0x1B8 [X].

- **Rudder** [X]: **L2** gives +1 and **R2** −1 (both held: 0). The rudder angle (+0x22C) eases toward that input
  times the ship record's turn value (record pointer at +0x1DC, field +0x18) through a smoothing record; when the
  input opposes the current swing, the easing is **2.5×** faster.
- **Turning and speed** [X]: in the steering states the turn rate (+0x228) is speed × sin(rudder) / +0x220 (option
  1979: 4,500 or 4,000 on the disc; default 4,000). The forward speed (+0x224) is the ship record's speed (field
  +4) times a throttle factor from `0x00219058`, or eased toward a target when +0x550 is set. In state 5 the speed
  eases to 0; state 7 fades the ship out (sunk). *Interpretation:* a bicycle-style model with +0x220 as the ship's
  length [I].
- **Boarding** [X]: the ships form a list (head at `0x00479BEC`, next at +0x554). Another ship that is not sunk,
  lies within +0x428 (default **20,000**; compared squared) and is closer than +0x42C (**7,500**) enables a
  **Circle** press (within 0.1 s) that plays animation 891 on the ship's helper object (+0x430) [X; I for
  "boarding"]. A press of any of Triangle, Cross, Square, L3 or R3 within 0.1 s (mask 0x6D0) while +0x448 is set
  leaves the helm (state 3) [X; I for the meaning].
- **Heading** [X]: `0x00219000` computes the ship's heading as atan2 of its forward axis, normalised to ±π.

#### 7.5.2 Throttle, sailing records and the "gun" options (third pass)

Correction to 7.5: options 1954 to 1961 are not guns. They build **per-state sailing records** [X]:

- 1954 (one argument, the state number) starts a record (44 bytes, up to 4 per ship from +0x1D4, count +0x1D8); 1961
  closes it. On the disc each ship has two records, for states **4** and **2** [D].
- 1955 to 1960 fill the open record: +0x4 the **speed** (1955: 750, 3,000, 1,000), +0x8 a distance (1956: 10,000),
  +0x18 the **rudder turn value** read by the steering of 7.5.1 (1959: 0.9, 1.58, 0.8), +0x1C and +0xC two easing
  records (1958: (0.1, 0, 0.25) or (0.6, 0, 0.25); 1960: (1.4, 0, 0.1)), +0x28 a factor (1957: 0.1 or 0.5) [D for
  the values, X for the fields].
- When the ship changes state, the record whose state number matches becomes current (+0x1DC) [X].

**Throttle** [X]: the forward speed is the current record's speed × a factor. The factor is read from a ladder of
13 values set up at +0x1E0 (**0.3, 0.5, 0.743, 0.9, 1.1, 1.2, 1.55, 1.5, 1.45, 1.4, 1.35, 1.3, 1.3**), indexed by
|a| / π × 12 with linear interpolation, where a is the ship's heading minus (a world angle at `0x47A2C8` + π),
wrapped to ±π. With the mode field at +0x1C0 equal to 4 with |a| ≥ π/2 the factor is fixed at 0.5 instead. *Interpretation:*
the world angle is the wind, and the ladder is a sailing polar: slowest heading into the wind (0.3), fastest across
it (1.55 at 90°), 1.3 running before it [I].

**Options 1989 and 2005** [X]:

- 1989 (four arguments) appends an entry (two objects and a value, 28 bytes) to list n (first argument; up to 6
  entries per list, lists of 168 bytes from +0x270). On the disc n = 1 with objects such as 44948 and 37194 [D].
  What the lists drive is not traced.
- 2005 (one vector) appends a point (up to 8, from +0x470); the points are passed with the nearest object of a
  given class to a routine at `0x0014B590` [X]. Disc points include (−2,000, −750, −200) [D]. *Interpretation:*
  attachment or spawn points on the hull [I].

### 7.6 Still open after this pass

- The biplane flight model, the raft and jolly-boat water model, the swimmer and the ships: their per-frame physics
  are spread over many routines not traced here.
- The meaning of most defaulted fields in 7.2 to 7.5.
- Which job drives which machine and with which camera.

Still open after the second pass (7.0.1 to 7.5.1):

- the biplane's computer-flight rules (modes 1 and 2), its speed-ramp formula in detail, and what R1's shots do;
- what the swimmer's direction records move (stroke, roll or dodge) and the meaning of states 3 to 5;
- the ship's throttle-factor inputs, what leaving the helm and boarding lead to, and the ships' guns (options
  1954 to 1961, 1989, 2005);
- whether the raft's one-physics-step stroke acceleration is meant as an impulse of 6,000 × step [I];
- which job drives which machine.

Still open after the third pass (7.2.2, 7.4.2, 7.5.2):

- the damage of the biplane shots (a property of the projectile templates);
- what the ship's list option 1989 and point option 2005 drive, the distance (+0x8) and factor (+0x28) of the
  sailing records, and where the world wind angle is set;
- the clear-spot rule of the swimmer's state 3 in detail.
