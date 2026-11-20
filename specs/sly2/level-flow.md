---
id: sly2-behaviour-level-flow
title: Sly 2 level flow - start, episodes, exits, arrival points, world loads, wipes, ending
side: reference
status: draft
sources:
  - kind: disc
    what: Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds
    used_for: >-
      every class-26 exit body of all 43 worlds read with its options and destination records (370 bodies, 104
      distinct objects); class-41 and class-42 marker bodies with their name strings; job and task options naming
      exits and arrival points; objects 777, 778 and 393 located in every world; the executable's file-record table,
      option-table entries 778 to 792, the episode exit table at 0x2BE2A8 and the movie-count table at 0x2BE2D0;
      the string table (episode titles). The whole-disc body walk of worlds.md 5 still ends every body exactly
      (121,153 bodies, 0 failures)
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly2-Release/transition.h, transition.cpp, frontend.cpp, game.cpp, menu.cpp, task.cpp, disc.h
    used_for: the world-load sequence, the exit state machine, arrival-point lookup and application, the wipe, boot, new game, continue, the episode menu and its unlocking; marked [R]
  - kind: executable
    what: SCUS_973.16 (hash checked), option table at 0x002AC880, class option map
    used_for: 2026-11-18 re-check of options 790-795 and the job and task options (stream options or script methods)
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/task.cpp, frontend.cpp, transition.cpp
    used_for: 2026-11-18 answers on arrival-point lists, the new-game start, the current job and task, the episode save group; marked [R]
  - kind: disc
    what: >-
      SCUS-97316, 2026-11-19 read of every episode macro package's script modules attached to tasks 1752, 1978, 2168,
      2388, 2624, 2842, 3022 and 3339, their intro sequences' option lists and the episode-menu exits 1329-1336
    used_for: what starts each episode's opening sequence (section 4.1)
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/task.cpp (restoring tasks on load)
    used_for: the intro task becoming available on load; [R]
reviewed_by: pending
date: 2026-11-17
---

# Sly 2 level flow

How the game moves the player between worlds: the start of the game, the episode menu, exits (doors), arrival
points, the load itself, deaths and restarts, the screen wipe and the ending. It is a delta of
[../sly1/level-flow.md](../sly1/level-flow.md); most of the mechanism is new, so most sections are written in full.
World rows are rows of the level table ([disc.md](disc.md) 6, 7). Option numbers are stream option IDs
([object-stream.md](object-stream.md) 1). Jobs and tasks are in [missions.md](missions.md).

## 1. What changed from Sly 1

| Item | Sly 1 | Sly 2 |
|---|---|---|
| World list | executable table of 46 levels ([../sly1/disc.md](../sly1/disc.md) 4.2) | level table in every world, 44 rows, 43 worlds [D] |
| Structure | hub and levels per episode, keys open job doors | per episode: a hub and job worlds; progress by jobs and tasks (missions.md) [D] |
| Exits | class 125 with two level descriptors | class 26 volume with one destination world and up to two movies [D] |
| Arrival | warps (class 116) and aliases | arrival points: class-42 named markers, default object 393 [D] |
| Checkpoints | class 56 volumes | a task's checkpoint is an arrival point (task option 1274) [D] |
| Wipe | fade or iris drawn by the game | a screen object (777 or 778) plays an out and an in animation [D] [R] |
| Episode choice | hub selector and world map | the menu world (row 1) with one exit per episode [D] |

## 2. Game structure [D]

- 9 episodes: 0 is the intro, 1 to 8 the story episodes. Each has one hub (hub flag set): rows 0, 2, 8, 12, 14,
  17, 27, 32, 38. Row 1 is the menu world (`dvd_menu`, episode 0); row 18 is an empty slot. ([disc.md](disc.md) 6, 7)
- Episode titles are strings 109 to 117 of the world string table: 109 "A Shadow From The Past", 110 "The Black
  Chateau", 111 "A Starry Eyed Encounter", 112 "The Predator Awakes", 113 "Jailbreak", 114 "A Tangled Web",
  115 "He Who Tames the Iron Horse", 116 "Menace in the North, Eh?", 117 "Anatomy for Disaster". String 108 is the
  menu line "Start Episode", 118 "Watch a Movie", 66 "Exit to Episode Menu".
- Every world of an episode carries the episode's job and task records ([missions.md](missions.md) 1) and the
  episode's story exits (section 5.4).
- The save keeps the progress symbols `widCurrent` (the loaded world's row), `episodeCurrent`, `episodeBest`, and
  per episode `dt` (time played) and `percent_complete`; all are strings in every world's save-variable section [D].

## 3. Start of the game

1. **Boot** [R]: the logo movie plays (executable file record 23 at virtual address `0x002A3868`: identity `0x808`,
   2,998,276 bytes [D]), the memory-card slots are read, then the **boot world** loads: record 24 at `0x002A38A8`,
   identity `0x5D` = row 0, the intro hub [D], at arrival point 393 (section 6), with no wipe.
2. **Title** [R]: the boot world shows the title screen ("Press START button for New Game" / "... to Resume Game",
   strings 5 and 6 [D]).
3. **New game** [R]: the intro hub (episode 0's hub, row 0) loads again at point 393 with task 1583 (`0x62F`,
   `t0_security_shutdown`) queued, so the intro's first job starts (missions.md 5). On arrival every task is restored
   from the (empty) progress, then task 1583 is set available and current directly, without its prerequisites being
   done; the current character becomes the task's option-1276 character (7, Sly) and the task's checkpoint (option
   1274, object 150,524) becomes the arrival point. The current job and task are cleared first.
   - **The attract-mode chain is not played by a new game.** Tasks 1664 (`t0_rooftop_prerunup`) → 1668
     (`t0_rooftop_runup`) → 1582 (`t0_attract_mode`) → 1583 form the head of the intro's graph [D], but a new game
     queues 1583 itself [R]; the three earlier tasks have no job and no checkpoint [D] and belong to the title
     screen's backdrop in the boot world [I]. A port that starts a new game by queuing 1583 behaves as the game.
4. **Continue** [R]: the saved `episodeCurrent` episode's hub loads at point 393, unless that episode's
   `percent_complete` is 100 or more; then the menu world loads (record 22 at `0x002A3828`, identity `0x39` [D]).
5. **End of the intro** [D]: the intro hub has one exit (object 150652): destination row 1 (the menu world), movie
   `0x806`, wipe 778, arrival point 393, and one option-788 entry (task 1711, value 3; meaning per section 5.2).

## 4. The episode menu (menu world, row 1)

- The menu world holds 8 exits, objects 1329 to 1336, one per episode 1 to 8; each leads to that episode's hub with
  the episode's opening movie [D]:

  | Exit | Episode | Hub row | Opening movie |
  |---:|---:|---:|---|
  | 1329 | 1 | 2 | `0x7D9` |
  | 1330 | 2 | 8 | `0x7DE` |
  | 1331 | 3 | 12 | `0x7E3` |
  | 1332 | 4 | 14 | `0x7E7` |
  | 1333 | 5 | 17 | `0x7EC` |
  | 1334 | 6 | 27 | `0x7F1` |
  | 1335 | 7 | 32 | `0x7F6` |
  | 1336 | 8 | 38 | `0x7FA` |

  The executable lists the same objects per episode at `0x002BE2A8` (9 words: −1, 1329, ..., 1336) [D].
- **Starting an episode** [R]: on its first visit (the episode's `dt` is 0) the menu fires that episode's exit, so the
  opening movie plays and the hub loads; afterwards the hub loads directly at point 393 with wipe 778.
- **Which episodes can be chosen** [R]: the highest selectable episode is the largest one, counting down from 8,
  that has been played (`dt` > 0) or whose previous episode is at 100 %; episode 1 is always selectable. A flag
  word at `0x002BE26C` (0 on the disc [D]) would open everything.
- **Movies in "Watch a Movie"** [R]: the table at `0x002BE2D0` gives 2, 4, 6, ..., 16 movies for episodes 1 to 8 [D]
  (an opening and an ending per episode); the list is shortened by 2 if the newest open episode was never played, by
  1 if it is below 100 %.
- **Exit to Episode Menu** (pause menu, string 66) [R]: ends the current job and loads the menu world from file record
  26 (`0x002A3928`, identity `0x39` [D]).

### 4.1 What starts each episode's opening sequence (added 2026-11-19)

- **Not the menu exit or the movie** [D]: exits 1329 to 1336 carry only option 780 = 778 (the wipe) and no task or
  sequence options; the opening movie has no script hook. The opening is started from inside the hub.
- **The intro task becomes available on load** [R]: each episode's root task (`tN_intro`) has no prerequisite tasks,
  and a task with no saved state and no prerequisites is set to available (state 1) when the world's tasks are
  restored. Tasks that follow it stay locked until it is done (missions.md 4).
- **A script module reacts to that state** [D]: a module in the episode's macro package, attached to the intro task,
  handles the task's "after state change" event (21; the prison uses "became current", 19). When the task's state
  (script ID 342 = option 1286) is 1, it starts the intro sequence (class 114) with the current character (script ID
  638 = option 614); the nightclub uses script ID 235 (option 613) with no starter.
- **The sequence closes the intro task** [D]: each intro sequence holds option 660 (command 0x23, arguments 0, −1,
  −1) and option 642 (command 0x11, set task state) setting the intro task to 3 (episodes 1, 3, 4, 8) or 2
  (episodes 2, 5, 6, 7). Completing it opens the successor chain toward the episode's first briefing task
  `tN_chalktalk1` (its option 1273 = 1).

  | Ep. | Intro task | Intro sequence | Package (module) | Event | Start option | State set | First briefing |
  |---:|---:|---:|---|---:|---:|---:|---:|
  | 1 | 1752 | 1753 | `f_nightclub_macro` (14) | 21 | 613 | 3 | 1799 |
  | 2 | 1978 | 1979 | `i_palace_macro` (13) | 21 | 614 | 2 | 1999 |
  | 3 | 2168 | 2169 | `i_temple_macro` (7) | 21 | 614 | 3 | 2186 |
  | 4 | 2388 | 2389 | `p_prison_macro` (11) | 19 | 614 | 3 | (see below) |
  | 5 | 2624 | 2625 | `p_castle_macro` (3) | 21 | 614 | 2 | 2642 |
  | 6 | 2842 | 2843 | `c_train_macro` (5) | 21 | 614 | 2 | 2865 |
  | 7 | 3022 | 3023 | `c_sawmill_macro` (6) | 21 | 614 | 2 | 3043 |
  | 8 | 3339 | 3340 | `a_blimp_macro` (13) | 21 | 614 | 3 | 3378 |

  Episode 1's sequence also holds options 635, 670 and 671; episode 4's holds dialogue options 695, 697 and 698. No
  `t4_chalktalk1` with option 1273 = 1 was found for the prison.
- **Related hooks** [D]: in the prison a rig module (sequence 27254) requests `t4_intro` state 4 on event 6; in the
  nightclub a rig module requests `t1_intro` state 3 (script ID 1104 = option 1280). Arrival point 393's event-5
  handlers enable the safe house (option 884 via script ID 806); in the nightclub only once `t1_intro` is done
  (script ID 1080 = option 1268). The nightclub, temple and sawmill handlers also call method 1491 on object 33735
  while tasks 1931, 2342 and 3291 respectively are not done.

## 5. Exits (class 26)

Layout: [classes/paths-and-volumes.md](classes/paths-and-volumes.md) 6 (a volume, then one destination file record,
then `u8` n and n more file records) [D]. On the disc: 370 exit bodies, 104 distinct exit objects; 369 at top level,
1 nested; every exit names a destination world (its first record decodes to a row of the level table); 164 bodies
(39 distinct exits) carry one or two extra records, which are movies [D].

### 5.1 Options [D]

The meaning of each field comes from the executable's option table (`0x002AC880`, 40-byte entries: value type, then
the field it sets) [D]; the behaviour is [R].

| Option | Value | Field | Meaning | On the disc |
|---:|---|---|---|---|
| 779 | object ID | +0x150 | the **arrival point** in the destination (default 393) [R] | 51 bodies (36 distinct exits), e.g. 33697 (7), 106189 (5), 106844 (4), 393 (3) |
| 780 | object ID | +0x154 | the **wipe** screen object (default none) [R] | every exit: 777 (186), 778 (184) |
| 781 | float | +0x15C | seconds the player must be outside before the exit arms | never |
| 782 | bool | +0x130 | this exit is the target of redirected exits | never |
| 783 | bool | +0x134 | redirect: firing fires the last exit that has 782 set | never |
| 784 | bool | +0x138 | load with transition flag `0x20` | never |
| 785 | float | +0x160 | delay between firing and loading | never |
| 786 | method | — | fire the exit (a script command) | — |
| 788 | list of (task, state) pairs | — | task states applied when the exit fires | 1 exit (object 150652, row 0) |

### 5.2 Behaviour [R]

- **States**: 0 the player is inside since the world started; 1 outside; 3 armed; 4 fired.
  0 → 1 when the player leaves; 1 → 0 when it re-enters, 1 → 3 after option 781 seconds outside (0 by default, so
  at once); 3 → fire when the player's centre enters the volume. Only the current player character counts.
- **Firing**: apply the option-788 task states; release the camera's special views; state 4; after the option-785
  delay (0 by default): queue the extra records as movies (at most 4 queued), then request the load of the
  destination world at the option-779 point (393 by default), with the option-780 wipe. Event 5 goes to the exit's
  script.
- Scripts and tasks also fire exits: task option 1278 fires an exit when the task becomes current (114 references
  [D]); job option 1259 fires one when the job ends with no next task (91 references [D]); option 786 from scripts.
- An arrival point applies again on every reload, and all exits' states return to 0 when an arrival point is
  applied (section 6).

### 5.3 Job doors [D]

In each episode, an episode-wide package (present in every world of the episode, so the same objects appear in each)
holds one exit per job world, leading from the hub to that world with wipe 778, and one exit back to the hub with
wipe 777. Doors placed in the levels themselves mostly carry an explicit arrival point (option 779): 36 of the 104 distinct exits do.

### 5.4 Story exits and movies [D]

Every episode has exits that play a story movie and reload the hub, and one that plays the episode's ending and
loads the menu world; all use wipe 777:

| Episode | Hub row | Movies back to the hub | Ending → row 1 |
|---:|---:|---|---|
| 1 | 2 | `0x7D6`, `0x7D7`, `0x7D8` | `0x7DA` |
| 2 | 8 | `0x7DB`, `0x7DC`, `0x7DD` | `0x7DF` |
| 3 | 12 | `0x7E0`, `0x7E1`, `0x7E2` | `0x7E4` |
| 4 | 14 | `0x7E5`, `0x7E6` | `0x7E8` |
| 5 | 17 | `0x7E9`, `0x7EA`, `0x7EB` | `0x7ED` |
| 6 | 27 | `0x7EE`, `0x7EF`, `0x7F0` | `0x7F2` |
| 7 | 32 | `0x7F3`, `0x7F4`, `0x7F5` | `0x7F7` |
| 8 | 38 | `0x7F8`, `0x7F9` | `0x7FB` then `0x7D5` |

The 16 opening and ending movies are the movie records 0 to 16 of the executable's file table, together with
`0x7D5` ([disc.md](disc.md) 5) [D]. Which screen plays them, and the formats, are in [movies.md](movies.md).

## 6. Arrival points (class 42) and warp points (class 41)

- **Arrival points** are class-42 named markers (transform header, options, name) [D]: 282 bodies. Names are game
  strings such as `ENTRANCE_default`, `entrance_default`, `entrance_from_ext`, `ENTRANCE_return_1` to `_7`,
  `ENTRANCE_outside_safehouse`, `entrance_recon_retry` [D].
- **Object 393** (`ENTRANCE_default`) is the default arrival point; 19 worlds have one: rows 0, 2, 8, 12, 14, 15, 17,
  27, 29 to 34, 37, 38, 40 to 42 [D]. The other worlds are entered only at named points or by tasks.
- An arrival point can answer to extra IDs: option 790 adds 393 (28 points), option 791 adds 394, option 792 adds its
  argument (402 and 403 on the disc); up to 4 IDs [D for the options and values, R for the rule].
- **Lookup** [R]: the point used is the last class-42 object (in class order) whose own ID or one of its extra IDs
  equals the requested point, among the children of a "key" object when one is given.
- **Applying a point** [R]: up to 4 (object, target) pairs place objects at other objects; up to 3 state-machine goals
  are set; up to 4 (task, state) pairs are applied (missions.md 4); every exit returns to state 0; the camera cuts;
  event 5 goes to the point's script and task event 26 is broadcast. Without a point, the player characters stay
  where their placements put them.
- **The three point lists** are filled by options that are script methods with a load-time list flag (option-table
  entries with handlers at `0x10BA70`, `0x10BB38`, `0x10BBE0`) [D]:

  | Option (script ID) | Arguments | Adds [R] |
  |---|---|---|
  | 793 (526) | 4: on flag, root object, state machine (−1 any), goal state | a state-machine goal set on arrival (ignored when the flag is 0; an entry for the same object and machine is replaced) |
  | 794 (527) | 2: task, state | a task state requested on arrival, with the point as requester |
  | 795 (773) | 2: object, target object | the object moved to the target's position and orientation on arrival |

  **None of the three occurs on the disc**, neither in the records nor in any script [D], so every arrival applies
  empty lists. (Order on arrival: pairs, goals, task states, then the rest as above.)
- Task option 1274 names a class-42 point as the task's checkpoint (427 references resolve to class 42 in the same
  world; the rest name points in other worlds of the episode) [D].
- **Warp points** (class 41, 228 bodies, names such as `WARP_start`, `WARP_end`, `WARP_safehouse`) [D] move the
  current player within the world when a script calls their option-778 method: the player takes the point's
  position and orientation, the point's script gets event 5 and task event 26 is broadcast [R].

## 7. Loading a world [R]

At the top of the next frame after a request:

1. The old world ends (its tasks get their end event), the clock resets, the controller is locked until the HUD
   unlocks it, pause state clears.
2. The destination is matched to its level-table row; the row number becomes the **level filter** for conditional
   records ([object-stream.md](object-stream.md) 2.3) and is stored as `widCurrent`; `episodeCurrent` is set and
   `episodeBest` raised if lower.
3. The world file loads ([world-file.md](world-file.md)), then its sound bank; the camera initialises.
4. The world starts (every object's start), then the queued task, if any, may change the requested point.
5. A request for the same world as before with flag `0x40` takes the Mega Jump away again.
6. The arrival point is found and applied (section 6).
7. If no player character is current: the one in the save's `oidPoCurrent`, else the first of Sly (class 53),
   Bentley (55), Murray (54) present is added, enabled and placed at the arrival point (unless a point pair places
   it); disabled characters are removed.
8. The camera updates and cuts; task event 5 is broadcast; the wipe uncovers the world.

A running job timer carries over a reload of the same job (missions.md 5).

## 8. Death and restart [R]

- Defeated during a job: the job restarts at its checkpoint task; that task's world loads at its checkpoint point
  (option 1274, 393 if none) ([missions.md](missions.md) 5).
- Defeated outside a job: the current world reloads at the current arrival point, with wipe 778.
- Abandoning a job (pause menu) ends it; the player stays in the world.

## 9. The wipe

- The wipe is a screen object (class 126) present in every world: object **778** (184 exits, the menu's episode
  starts and deaths) or **777** (186 exits: returns to the hub and story movies) [D].
- Sequence [R]: request → the wipe plays its "out" animation (ID 785, `0x311`) → when it has finished, the load is
  performed (section 7) with the screen covered → the "in" animation (ID 783, `0x30F`) plays over the new world →
  done. A request without a wipe, or when the wipe object is missing, loads at once. A new request while a wipe is
  covering is ignored.
- Coverage for drawing = elapsed fraction of the out animation, 1 while loading, remaining fraction of the in
  animation. The animations and their lengths are part of the screen objects ([user-interface.md](user-interface.md)).

## 10. Ending [D]

Episode 8's last story exit (object 3494, in every episode-8 world) loads the menu world after two movies, `0x7FB`
(85,835,780 bytes) and `0x7D5` (56,918,020 bytes) ([disc.md](disc.md) 5). *Interpretation:* the finale and the
credits [I].

## 11. Exit graph (test vector) [D]

Per world: number of exit bodies, destinations (row: count), wipes, explicit arrival points, extra (movie) records.
Exits shared by an episode appear in every world of that episode.

| Row | Exits | Destinations | Wipe 778 / 777 | Arrival points named | Movies |
|---:|---:|---|---|---|---:|
| 0 | 1 | 1 | 1 / 0 | 393 | 1 |
| 1 | 8 | 2, 8, 12, 14, 17, 27, 32, 38 | 8 / 0 | — | 8 |
| 2, 4 to 7 | 11 each | 1, 2×5, 3, 4, 5, 6, 7 | 6 / 5 | — | 4 |
| 3 | 12 | 1, 2×5, 3, 4, 5×2, 6, 7 | 7 / 5 | 402 | 4 |
| 8 | 11 | 1, 8×5, 9, 10, 11×3 | 6 / 5 | 106189, 106844, 106893 | 4 |
| 9, 11 | 9 | 1, 8×5, 9, 11×2 | 4 / 5 | 106189, 106844 | 4 |
| 10 | 10 | 1, 8×6, 9, 11×2 | 5 / 5 | 106189, 106844, 109314 | 4 |
| 12, 13 | 6 | 1, 12×4, 13 | 2 / 4 | — | 4 |
| 14 to 16 | 8 | 1, 14×5, 15, 16 | 4 / 4 | 77593 | 3 |
| 17 | 15 | 1, 17×4, 19, 20×2, 21, 22×2, 23, 24, 25, 26 | 10 / 5 | 33697, 60397, 60408, 60443 | 4 |
| 19, 21, 23 to 26 | 6 | 1, 17×5 | 1 / 5 | one point each | 4 |
| 20 | 7 | 1, 17×6 | 2 / 5 | 60399, 60415 | 4 |
| 22 | 8 | 1, 17×7 | 3 / 5 | 60177, 60544, 66373 | 4 |
| 27 | 11 | 1, 27×3, 28×3, 29×2, 30, 31 | 7 / 4 | 393, 49691, 49704, 49711, 49771 | 4 |
| 28 | 8 | 1, 27×6, 30 | 4 / 4 | 49698, 49705, 49712 | 4 |
| 29 | 7 | 1, 27×5, 30 | 1 / 6 | 51978, 51979 | 4 |
| 30 | 5 | 1, 27×3, 30 | 1 / 4 | — | 4 |
| 31 | 6 | 1, 27×4, 30 | 1 / 5 | 49501 | 4 |
| 32 | 12 | 1, 32×5, 33, 34, 35×2, 36, 37 | 7 / 5 | 33494 | 4 |
| 33 to 37 | 11 | 1, 32×5, 33, 34, 35, 36, 37 | 6 / 5 | — | 4 |
| 38 | 9 | 1, 38×3, 39, 40, 41, 42, 43 | 6 / 3 | 17477 | 4 (one exit nested) |
| 39 to 43 | 8 | 1, 38×3, 40, 41, 42, 43 | 5 / 3 | — | 4 |

Single-point worlds: 19 → 66926, 21 → 63271, 23 → 60371, 24 → 60379, 25 → 60375, 26 → 60052.

Other vectors:

- Arrival point 393 in row 0: body at decompressed offset 7,618,028, 76 bytes, name `ENTRANCE_default`; first bytes
  `ff ff a1 1d 1d bf 8c 1d 4a bf ...` (parent −1, then the matrix) [D].
- Executable option-table entries 779 to 785 at `0x002AC880 + 40 × ID`: value types 6, 6, 1, 0, 0, 0, 1 and fields
  `0x150`, `0x154`, `0x15C`, `0x130`, `0x134`, `0x138`, `0x160` [D].
- Totals: 370 exits, 282 arrival points, 228 warp points, 19 worlds with point 393 [D].

## 12. Open questions

- The wipe animations' lengths (screen-object data) and whether 777 and 778 look different.
- What option 788's value 3 on the intro's exit sets (task 1711's state, per missions.md 4, is assumed).
- How the hub's job doors are closed or opened by task state (scripts and task world effects), and whether walking
  into a job door is the only way to start a job.
- The exact flags of each kind of load request (same world, new game, task start) beyond those listed.
- Which task follows `t4_intro` toward the prison's first briefing (no `t4_chalktalk1` with option 1273 = 1 found).
- All [R] statements need checking in play.
