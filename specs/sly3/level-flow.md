---
id: sly3-behaviour-level-flow
title: Sly 3 level flow - episodes, exits, arrival points, world loads, wipes - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), all 39 worlds walked; executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: >-
      every class-24 exit body (503 bodies, 81 distinct exits) with its destination and movie records; every
      class-40 and class-39 point with its options and name; the job, task and command-script references to exits and
      points; the executable's world, episode and first-world tables, the option-table entries 834 to 848, the exit
      start-up code at 0x27FAC8, the episode start code at 0x289198, the callers of the load request at 0x289070, the
      reload code at 0x1855F0 and the wipe-pair table at 0x3297A0 [X]
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/transition.cpp, frontend.cpp, game/scptrun3.cpp, game/task3.cpp, game/sly3.h, game/gen
    used_for: >-
      the exit state machine and what firing does, the arrival-point options' effects, boot, the wipe sequence and
      the wipe-pair shuffle; the Sly 2 to Sly 3 class and option correspondence (rows scored 80 or more); marked [R]
reviewed_by: pending
date: 2026-11-18
---

# Sly 3 level flow

Read [../sly2/level-flow.md](../sly2/level-flow.md) first: the mechanism (exits with a state machine, arrival points,
a load requested and performed at the top of a frame, an optional wipe) carries over. This document gives Sly 3's
classes, options, tables and data, and the places where behaviour differs. World slots are those of
[disc.md](disc.md) 5.1. Tags as in [guards.md](guards.md).

## 1. What changed from Sly 2

| Item | Sly 2 | Sly 3 |
|---|---|---|
| World list | level table in every world file | executable tables, 40 slots ([disc.md](disc.md) 5.1) [X] |
| Episode start | menu world fires one exit per episode | the menu world has **no exits**; code picks the episode's first world (section 3) [X] |
| Exit class, options | 26; options 779 to 788 | **24**; options 834 to 841; only 834 (point) and 835 (wipe) occur on the disc [D] |
| Arrival points | class 42, default 393 | class **40**, default **424** (`0x1A8`), second id 425 [X] [D] |
| Warp points | class 41 | class **39** [D] |
| Wipe on exits | 777 or 778 on every exit | **874** or **875** (875 plays one of three animation pairs); 7 exits have none [D] |
| Wipe on restart | 778 | **875** (section 7) [X] |
| Exit firing | pad locked by the load | the game is also paused from firing until the load (state 5, section 4.3) [R] |
| Task checkpoint | task option 1274 | task option **1400** [R for the correspondence; D for the values] |
| Exit fired by a task / a job | options 1278 / 1259 | options **1406** / **1381** [R; D] |

## 2. Game structure [X] [D]

- Episode of each world slot: the table at `0x002EDF10` ([disc.md](disc.md) 5.1): 1 the menus, 2 the training
  world, 3 Venice, 4 the Outback, 5 Holland, 6 China, 7 the pirates, 8 Kaine Island.
- **First world of each episode**: 9 `s32` at `0x002EDFB0`, indexed by episode: −1, 0 (`dvd_menu`), 2
  (`i_trainer`), 3 (`v_ext`), 8 (`o_ext`), 15 (`h_ext`), 23 (`c_ext`), 31 (`p_ext`), 35 (`m_ext`). Episodes 3 to 8
  start in their hub.
- Each episode's exits are stored once, in the episode's macro group (`v_macro`, `o_macro`, `h_macro`, `c_macro`,
  `p_macro`, `m_macro`, `i_macro`), which every world of the episode loads; so every world of an episode holds the
  same exits (5, 7, 8, 8, 4 and 5 worlds for episodes 3 to 8) [D]. No exit has a world filter or a condition [D].
- `dvd_menu` (slot 0) has no exits and no arrival or warp points [D].

## 3. Starting the game and an episode

- **Boot** [R]: the logo movies (movie identities 338 and 339) play, then the world in slot 35 (`m_ext`) is loaded
  at point 424 with no wipe. Sly 3 opens on Kaine Island (the prologue job, task 1798) before the title [O].
- **Starting an episode** [X: the routine at `0x00289198`, called with the episode]:
  1. The world is the episode's first world (section 2).
  2. Episode 6 (China): if the progress value `t4_recon_complete` / `is_complete` is 0, the world is 24
     (`c_intro`) instead.
  3. Episode 7 (pirates): if `t5_treasure_complete` / `is_complete` is set and `t5_heist_complete` / `is_complete`
     is not, the world is 32 (`p_at_sea`) instead.
  4. Episode 1 (the menus): unless a test on the save succeeds (routine `0x0025E4A0`; *interpretation:* "the opening
     has been played" [I]), world 35 (`m_ext`) is loaded and task 1798 is started there.
  5. Episode 8 reads the saved forced checkpoint (`oidCheckpointForce`, a task ID, and `widCheckpointForce`, its
     world; [saving.md](saving.md) 3). With no task saved (0 or −1) it starts task 1841; otherwise it starts the
     saved task in the saved world.
  6. If no task was chosen, the world is loaded at point **424**, with no wipe; otherwise the chosen task is started,
     which loads its world (Sly 2 [missions.md](../sly2/missions.md) 5).
- The episode menu itself (screen objects of `dvd_menu`) is described with the user interface.

## 4. Exits (class 24)

Layout: [classes/paths-and-volumes.md](classes/paths-and-volumes.md) 3 (a trigger shape, a destination file record,
`u8` n and n more file records) [D].

### 4.1 On the disc [D]

- 503 bodies, 81 distinct exits, all at top level in the macro groups. Their option lists (after the 54-byte
  header) hold only options 834 and 835 (section 4.2); every copy of an exit carries the same options.
- Every first record names a world (its identity is a world id of [disc.md](disc.md) 5.1). The extra records are
  movies (MPEG program streams, first bytes `00 00 01 BA`): 21 exits carry one, two exits (3440, 4245) carry two.
- 16 of the movies named by exits are not in the executable's movie table ([disc.md](disc.md) 5.2): identities
  `0x131` to `0x133`, `0x136` to `0x138`, `0x13B` to `0x13D`, `0x140` to `0x143`, `0x147` to `0x149` (the
  mid-episode story movies; section 4.5).

### 4.2 Options and defaults [X]

Option-table entries (`0x0032CA80 + 40 × ID`; value type, flags, field):

| Option | Sly 2 | Value | Field | Meaning [R] |
|---:|---:|---|---|---|
| 834 | 779 | object ID | +0x178 | the arrival point in the destination |
| 835 | 780 | object ID | +0x17C | the wipe object |
| 836 | 781 | float | +0x184 | seconds outside before the exit arms |
| 837 | 782 | bool | +0x120 | target of redirected exits |
| 838 | 783 | bool | +0x124 | redirect: firing fires the last exit with 837 set |
| 839 | 785 | float | +0x188 | delay between firing and loading |
| 840 | 786 | method | — | fire the exit |
| 841 | 788 | (task, state) pairs | — | task states applied on firing |

Sly 2's option 784 (load flag) has no Sly 3 counterpart [R]. The exit's start-up code (`0x0027FAC8`) sets
**+0x178 = 424** and **+0x17C = −1** (no wipe) before the options apply [X].

On the disc [D]:

- **835 (wipe)**: 875 on 229 bodies (37 distinct exits), 874 on 232 bodies (37 exits); 7 exits (42 bodies) have no
  wipe: the four exits to `dvd_menu` without a movie (2053, 2549, 2961, 3833), the two exits with two movies (3440,
  4245) and the prologue's end (4236). *Pattern:* doors between the hub and job worlds, and the
  training world's exits, use **875**; exits fired by job ends and by tasks (hub returns with or without a story
  movie, episode endings) use **874** [D].
- **834 (arrival point)**: 12 exits (76 bodies), all fired by job ends: 2051 → 140590 `entrance_ext_from_gauntlet`,
  2537 → 121480 `entrance_bar_exit`, 2541 → 121481 `entrance_ext_from_claw`, 2542 → 121482 `entrance_dumptruck`,
  2547 → 121054 `entrance_from_cave_b`, 2953 → 94489 `entrance_frame_a_complete`, 3364 → 70146
  `entrance_get_job_done`, 3365 → 70147 `entrance_webcam_done`, 3827 → 51411 `entrance_dive_after`, 4238 → 15964
  `entrance_track1`, 4241 → 16367 `entrance_vault_intro`, 4244 → 9836 `entrance_boss_intro`. The other exits use
  424 unless command `0x68` supplies a point (section 4.4).

### 4.3 Behaviour [R]

As Sly 2 section 5.2 (states 0 inside since the start, 1 outside, 3 armed, 4 fired; only the current player, if it
is a character of the gang's family, counts; firing applies the 841 pairs, raises event 5 to the exit's script and,
after the 839 delay, queues the movies and requests the load). Differences:

- Firing does not release camera views.
- Entering state 4 **pauses the game** (pad locked, AI and subtitles held) and keeps the exit updating while paused;
  after the load request the exit goes to a new state **5** (load requested). Leaving states 4 and 5 for a lower state
  resumes the game.
- A fire request is honoured only in state 3 (armed).

### 4.4 What fires an exit [D] [R]

- **The player walking in** (state machine above) [R].
- **A job ending** without a next task: job option **1381** names the exit (25 jobs; 155 bodies) [D].
- **A task becoming current**: task option **1406** names the exit (16 tasks: 1825 to 1840) [D].
- **Command `0x68`** of a command script ([scripts.md](scripts.md) 7.2): arguments (`s32` exit, `s32` point). When
  the point is not −1 it is written to the exit's +0x178 before the exit fires [R]. 472 uses, 63 distinct (exit,
  point) pairs; every point is a class-40 object of the destination [D]. This is how Sly 3 names arrival points:
  the same exit is fired with different points (exit 2952 to `h_ext` with 8 points).
- Option 840 from scripts [R].

### 4.5 The exits [D]

"Fired by" lists jobs (1381), tasks (1406) and the arrival points given by command `0x68` (by name).

| Episode | Exit | Destination | Movies | Fired by |
|---:|---:|---|---|---|
| 2 | 4569 | `i_trainer` | — | — |
| 2 | 4570 | `dvd_menu` | — | `0x68` with point 424 |
| 3 | 2045 | `v_ext` | — | job 2168; `entrance_recon_chase`, `entrance_heist`, `entrance_from_apt1`, `entrance_from_apt2`, `entrance_outside_apt3` |
| 3 | 2046 | `v_interpol` | — | `entrance_recon_vent` |
| 3 | 2047 | `v_canal` | — | 424 |
| 3 | 2048 | `v_apt` | — | `entrance_apt1` to `entrance_apt3` |
| 3 | 2049 | `v_gauntlet` | — | `entrance_gauntlet_interior_intro`, `entrance_heist_disguise_door1` |
| 3 | 2050, 2051 | `v_ext` | — | 2051: job 2283 |
| 3 | 2052 | `dvd_menu` | `0x135` | job 2419 (episode end) |
| 3 | 2053 | `dvd_menu` | — | — |
| 3 | 2135, 2226, 2417 | `v_ext` | `0x131`, `0x132`, `0x133` | tasks 1825, 1826, 1827 |
| 4 | 2536 | `o_ext` | — | four points (`entrance_from_cave_a`, …) |
| 4 | 2537, 2540, 2541, 2542, 2547 | `o_ext` | — | jobs 2730, 2663, 2623, 2695, 2805 |
| 4 | 2538, 2539, 2543 to 2546 | `o_bar`, `o_quarry`, `o_arena`, `o_cave_a`, `o_cave_b`, `o_cave_murray` | — | one or two points each |
| 4 | 2548 | `dvd_menu` | `0x13A` | job 2843 (episode end) |
| 4 | 2549 | `dvd_menu` | — | — |
| 4 | 2594, 2728, 2841 | `o_ext` | `0x136`, `0x137`, `0x138` | tasks 1828, 1829, 1830 |
| 5 | 2952 | `h_ext` | — | 8 points |
| 5 | 2953 | `h_ext` | — | job 3061 |
| 5 | 2954 to 2959 | `h_hangar_a`, `h_hangar_b`, `h_hangar_c`, `h_hotel`, `h_sewer`, `h_dogfight` | — | points |
| 5 | 2960 | `dvd_menu` | `0x13F` | job 3259 (episode end) |
| 5 | 2961 | `dvd_menu` | — | — |
| 5 | 3023, 3156, 3251 | `h_ext` | `0x13B`, `0x13C`, `0x13D` | tasks 1831, 1832, 1833 |
| 6 | 3359 | `c_ext` | — | job 3584; 6 points |
| 6 | 3363, 3364, 3365 | `c_ext` | — | jobs 3629, 3449, 3540 |
| 6 | 3360, 3361, 3362, 3366, 3367, 3368 | `c_hall_a`, `c_forest`, `c_hall_b`, `c_tilt_hall`, `c_flashback`, `c_apt` | — | points |
| 6 | 3369 | `dvd_menu` | `0x146` | job 3712 (episode end) |
| 6 | 3440 | `c_ext` | `0x145`, then `0x140` | task 1834 |
| 6 | 3583, 3627, 3710 | `c_ext` | `0x141`, `0x142`, `0x143` | tasks 1835, 1836, 1837 |
| 7 | 3825, 3830 | `p_ext` | — | 3825: `entrance_heist_ship_battle` |
| 7 | 3826, 3827, 3831 | `p_at_sea` | — | 3826: `entrance_treasure_from_ext`; jobs 4101 (3827), 3991 (3831) |
| 7 | 3828, 3829 | `p_island_map`, `p_dive` | — | `entrance_treasure_map`, `entrance_dive` |
| 7 | 3832 | `dvd_menu` | `0x14B` | job 4145 (episode end) |
| 7 | 3833 | `dvd_menu` | — | — |
| 7 | 3905, 4069, 4143 | `p_ext`, `p_at_sea`, `p_at_sea` | `0x147`, `0x148`, `0x149` | tasks 1838, 1839, 1840 |
| 8 | 4236 | `dvd_menu` | `0x14E` | job 1797 (end of the prologue) |
| 8 | 4237, 4238 | `m_ext` | — | 4238: job 4396 |
| 8 | 4239 | `m_underwater` | — | `entrance_dive_tunnel` |
| 8 | 4240, 4241 | `m_vault` | — | 4241: jobs 4436, 4479 |
| 8 | 4242 | `m_gauntlet` | — | `entrance_gauntlet` |
| 8 | 4243, 4244 | `m_boss` | — | 4244: job 4505 |
| 8 | 4245 | `dvd_menu` | `0x14D`, then `0x12F` | job 4544 (the finale and credits [I]) |

Episodes 3, 4, 5 and 7 each have one exit to `dvd_menu` without a movie that nothing on the disc fires (2053, 2549,
2961, 3833) [D]; *interpretation:* walked into, or fired by script modules (option 840) [I].

## 5. Arrival points (class 40) and warp points (class 39)

Layout: [classes/simple-layouts.md](classes/simple-layouts.md) 3 (header, options, a `u16`-length name) [D].
Lookup, application and where characters are placed: [characters.md](characters.md) 2 and Sly 2 section 6 [R].

### 5.1 On the disc [D]

- **312** arrival-point bodies and **208** warp-point bodies, counted at every depth: 7 points and 5 warps sit
  nested in placements or solids. The section-8 table counts at every depth. Names are game strings as
  in Sly 2: `ENTRANCE_default`, `ENTRANCE_outside_safehouse` (5 hubs), `entrance_heist`, `entrance_from_apt1`, …;
  `WARP_1`, `WARP_SafeHouse`, `warp_turf_war_battle1`, ….
- **Object 424** (`ENTRANCE_default`, in each hub's rig group) exists in 10 worlds: 2, 3, 8, 9, 15, 21, 23, 31, 32,
  35. No world has an object 425.

### 5.2 Options [X] [D]

| Option | Sly 2 | Value | Meaning [R] | On the disc |
|---:|---:|---|---|---|
| 843 | 790 | method, no arguments | the point also answers to 424 | 17 points in 17 worlds |
| 844 | 791 | method, no arguments | the point also answers to 425 | never |
| 846 | 793 | method, 4 arguments | set a state-machine goal on arrival (a flag, the object, the state machine, the state) | never |
| 847 | 794 | method, 2 arguments | a (task, state) pair applied on arrival | 1 point: (2985, 4) |
| 848 | 795 | method, 2 arguments | place the first object at the second on arrival | 17 points, e.g. (24, 113265), (25, 113263), (26, 113261): Sly, Bentley, Murray placed at three markers |
| 845 | — | object ID, field +0x170 | a command script (class 135) of the episode's macro group [D]; *interpretation:* run when the point is applied [I] | 16 points (Venice, Outback and Holland worlds and `p_ext`) |

Each point answers to at most 4 ids, and holds at most 4 pairs of each kind [R].

### 5.3 Checkpoints of tasks [D]

Task option **1400** names a class-40 point: 365 of the 965 distinct tasks set it. Of its 2,136 occurrences, 374
name a point in the same world, 1,743 a point in another world of the episode (the task records are copied into every
world of the episode), and 19 a point not found on the disc (16 of them 94305). One task names 424.

### 5.4 Warp points [R]

As Sly 2: a script's call moves the current player to the warp point (its position and orientation), raises event 5
to the point's script and broadcasts task event 26. No warp point carries options on the disc [D].

## 6. Loading a world [R]

As Sly 2 section 7, with these differences:

- The destination record is matched against the executable's world records (section 2); the slot index becomes the
  world filter of conditional records ([world-file.md](world-file.md) 6).
- The default point is 424; with no matching point the characters stay at their placements
  ([characters.md](characters.md) 2).
- The current player, when none is set, is the saved one, else the first present of Sly (class 52), Bentley (54),
  Murray (53) ([characters.md](characters.md) 2).
- Callers of the load request (`0x00289070`) pass point 424 and no wipe everywhere except the exit (its +0x178 and
  +0x17C, so wipes 874 and 875 from option 835) and the restart (section 7) [X].

## 7. Restart and the wipe

- **Restart** [X: `0x001855F0`]: on game event 12, unless a game-state flag is set, the **current world reloads at
  the stored checkpoint** (a game-state field) **with wipe 875**. *Interpretation:* event 12 is the player's defeat
  [I]. Which checkpoint is stored follows the task rules of Sly 2 ([missions.md](../sly2/missions.md) 5) [R].
- **Wipe 875** is a screen object (class 158, group `stock_swf`, present in every world) [D]. It has three
  out/in animation pairs: the table at `0x003297A0`, 3 × (`s32` out, `s32` in) = (880, 881), (882, 883), (884, 885)
  [X]. Each use takes the next pair of a shuffled order of the three; after all three, the order is reshuffled, and
  the pair just used is not allowed first [R] (the same rule as the dialogue line lists,
  [dialogue-and-voice.md](dialogue-and-voice.md) 1.1).
- **Wipe 874** (class 158, `stock_swf`, 427 bytes, one element) and any other wipe object play their parts 886
  (out) and 888 (in) [R]. The wipe sequence and coverage are Sly 2
  section 9 [R].

## 8. Test vectors [D]

Per world: exits held, arrival points, warp points, whether object 424 exists, points with option 843.

| Slot | World | Exits | Points | Warps | 424 | 843 |
|---:|---|---:|---:|---:|---|---:|
| 2 | i_trainer | 2 | 7 | 0 | yes | 1 |
| 3 | v_ext | 12 | 35 | 16 | yes | 1 |
| 4 | v_canal | 12 | 2 | 0 | | 1 |
| 5 | v_apt | 12 | 6 | 6 | | 1 |
| 6 | v_gauntlet | 12 | 7 | 15 | | 0 |
| 7 | v_interpol | 12 | 3 | 2 | | 0 |
| 8 | o_ext | 17 | 30 | 13 | yes | 1 |
| 9 | o_quarry | 17 | 6 | 3 | yes | 1 |
| 10 | o_arena | 17 | 3 | 0 | | 0 |
| 11 | o_cave_a | 17 | 3 | 5 | | 1 |
| 12 | o_cave_b | 17 | 3 | 3 | | 1 |
| 13 | o_bar | 17 | 2 | 0 | | 0 |
| 14 | o_cave_murray | 17 | 4 | 2 | | 0 |
| 15 | h_ext | 13 | 32 | 15 | yes | 1 |
| 16 | h_hotel | 13 | 6 | 2 | | 0 |
| 17 | h_hangar_a | 13 | 1 | 0 | | 0 |
| 18 | h_hangar_b | 13 | 2 | 0 | | 0 |
| 19 | h_hangar_c | 13 | 2 | 0 | | 0 |
| 20 | h_sewer | 13 | 4 | 6 | | 1 |
| 21 | h_dogfight | 13 | 4 | 1 | yes | 1 |
| 22 | h_hack | 13 | 1 | 0 | | 0 |
| 23 | c_ext | 15 | 33 | 34 | yes | 1 |
| 24 | c_intro | 15 | 13 | 4 | | 0 |
| 25 | c_flashback | 15 | 1 | 1 | | 0 |
| 26 | c_forest | 15 | 2 | 3 | | 1 |
| 27 | c_apt | 15 | 1 | 2 | | 0 |
| 28 | c_hall_a | 15 | 2 | 2 | | 0 |
| 29 | c_hall_b | 15 | 4 | 0 | | 0 |
| 30 | c_tilt_hall | 15 | 5 | 3 | | 0 |
| 31 | p_ext | 12 | 28 | 23 | yes | 1 |
| 32 | p_at_sea | 12 | 16 | 4 | yes | 1 |
| 33 | p_dive | 12 | 4 | 3 | | 0 |
| 34 | p_island_map | 12 | 2 | 1 | | 0 |
| 35 | m_ext | 10 | 20 | 15 | yes | 1 |
| 36 | m_underwater | 10 | 2 | 0 | | 0 |
| 37 | m_vault | 10 | 1 | 10 | | 0 |
| 38 | m_gauntlet | 10 | 11 | 10 | | 0 |
| 39 | m_boss | 10 | 2 | 4 | | 1 |

Other vectors:

- Exit 2417 in `v_ext`: destination record = world id `0xEF` (slot 3), sector 797,472, 13,425,318 bytes; movie
  record identity `0x133`, sector 430,112, size field 29,982,724; the movie's first bytes `00 00 01 BA` [D].
- Exit 2045 in `v_ext`: option list = 835 → 875 only; exit 2051: 835 → 874, 834 → 140590 [D].
- Point 424 in `v_ext`: class 40, group `v_ext_rig`, name `ENTRANCE_default`, one option (843) [D].
- Option-table entries 834 to 839: value types 6, 6, 1, 0, 0, 1; fields `0x178`, `0x17C`, `0x184`, `0x120`,
  `0x124`, `0x188` [X].
- First-world table `0x002EDFB0`: `ffffffff 00000000 02000000 03000000 08000000 0f000000 17000000 1f000000
  23000000` [X].

## 9. Open questions

1. What fires the four unreferenced exits to `dvd_menu` and exits 2050, 3830, 4237, 4240, 4243, 4569.
2. Option 845's script: when it runs, and whether it replaces the point's own script event.
3. The episode-1 save test (`0x0025E4A0`).
4. The game-state flag that suppresses the restart, and which code raises event 12.
5. The wipe animations' lengths (screen-object data, user-interface spec).
6. The load-request flags (3, 2, 18 and others passed by the callers of `0x00289070`).
7. All [R] statements need checking in play.
