---
id: sly2-behaviour-missions
title: Sly 2 jobs and tasks
side: reference
status: draft
sources:
  - kind: disc
    what: Sly 2 - Band of Thieves, USA, SCUS-97316; executable SCUS_973.16 SHA-256 a511a94b1dbe8cd9bc9b764c34ffda3f88869713a338e188b4f407732fec6a73; all 43 worlds
    used_for: >-
      every class-132 and class-133 record of all 43 worlds read field by field (388 + 4,080 records; the whole-disc
      walk of worlds.md 5 still ends every body exactly); option IDs and values used by jobs and tasks; job and task
      names (records and the worlds' string table); the prerequisite/successor graph; constants at 0x2BDC00
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly2-Release/task.h, task.cpp
    used_for: task states and transitions, job start/end/restart, world effects, progress recording, the difficulty help; marked [R]
  - kind: reference-project
    what: OpenThievius sly2-decomp @ 33e2bab9 (GPL-3.0; read for facts only)
    used_for: confirmation of the constant table at 0x2BDC00 and of the option table entries
  - kind: executable
    what: SCUS_973.16 (hash checked), option table at 0x002AC880 (entries 1236-1294 and 790-795), class option map
    used_for: 2026-11-18 re-check of the job, task and arrival-point options (stream options or script methods, handler addresses)
  - kind: disc
    what: 2026-11-18 walk of every job and task record (873 distinct) and of every script module of all 43 worlds; save-variable names
    used_for: option counts, static script call counts of the job and task methods, save-key names
  - kind: reference-project
    what: SafeHouse @ c70803ce, Sly2-Release/task.cpp (method registrations, task manager events, job begin and end), frontend.cpp (new game, continue)
    used_for: 2026-11-18 answers on options 1236-1294, script events 17 and 19-23, the current job, task and checkpoint task; marked [R]
  - kind: disc
    what: row 0 (jb_intro_macro, jb_intro_rig) and row 14 (p_prison_macro) object records, option lists and script modules (2026-11-20); record condition lists of p_prison_macro
    used_for: section 4.1 (what finishes task 1696; how the hack sequences 2410/2411 exist in row 14) [D]
  - kind: executable
    what: SCUS_973.16 (hash checked), option-table entries 634, 670, 671 (routines 0x106F90, 0x108890, 0x108948) and 1280 (0x10EE60) at 0x2AC880 + 40 × ID
    used_for: section 4.1, what the sequence commands 634, 670, 671 build [D for the entries]
  - kind: reference-project
    what: sly2-decomp src/new_100008 (the three command routines) and src/stepguard-jsg (record kinds)
    used_for: section 4.1, command meanings [R]
reviewed_by: pending
date: 2026-11-20
---

# Sly 2 jobs and tasks

Sly 2's heists are **jobs**; each job is a chain of **tasks**. Both are ordinary objects in the world files (classes
132 and 133, layouts in [classes/effects-logic-ui.md](classes/effects-logic-ui.md) 3); their options name the
task graph, the objects each task switches on and off, and the save entries that record progress. Sly 1 has no
equivalent (its progress is per level, `../sly1/pickups-and-progress.md`). Option numbers below are stream option
IDs ([object-stream.md](object-stream.md)).

## 1. Where the records are

- Every world of an episode carries the **whole episode's** job and task records, so any world can show and change
  any of the episode's progress [D]. Totals over the disc: 388 job and 4,080 task records = the per-episode sets
  below repeated in each of the episode's worlds; the menu world (row 1) has none [D].
- Records whose level condition does not include the current world are loaded but marked inactive: they keep their
  state and take part in the graph, but do nothing to the world [R] ([classes/effects-logic-ui.md](classes/effects-logic-ui.md) 3).

| Episode | Hub row | Jobs | Tasks |
|---|---:|---:|---:|
| 0 intro | 0 | 1 | 16 |
| 1 nightclub | 2 | 11 | 99 |
| 2 palace | 8 | 12 | 93 |
| 3 temple | 12 | 10 | 82 |
| 4 prison | 14 | 9 | 110 |
| 5 castle | 17 | 8 | 102 |
| 6 train | 27 | 8 | 72 |
| 7 sawmill | 32 | 10 | 122 |
| 8 blimp | 38 | 9 | 99 |
| **total** | | **78** | **795** |

[D]

## 2. Job record (class 132)

The body is an option list and a name string (an internal name such as `m1_sat`) [D]. Options used [D] (count over
the 78 distinct jobs in brackets where not all):

| Option | Value | Meaning |
|---:|---|---|
| 1243 | object ID | first task of the job (all) |
| 1244 | object ID | last task of the job (all) |
| 1245, 1246 | `s16` string IDs | display name and description, from the world's string table (all but one) |
| 1258 | 3 arguments: object IDs | the "job complete" screen object and its two parts shown in turn when the job is done (68) [R for the use] |
| 1259 | object ID | a level exit fired after the job, when no next task is reachable (19) [R] |
| 1257 | 2 × `f32` | two times in seconds; values (240, 480), (240, 600), (480, 960), (600, 1200), (1800, 3600) on 5 jobs; default (480, 720) [D for values, R for the default]. *Interpretation:* time thresholds compared with the job timer (section 5) |
| 1242 | flag | the job does not close its last task and does not autosave on completion (6) [R] |
| 1255 | 4 typed arguments, repeatable | **job object switch**: object ID, one switch group, owner (−1), level-table row (−1 any). 5 jobs use it once each, always with group 4, so the object is present only while the job is current (added when the job begins, removed when it ends) [D for the values, R for the use]. Example: `06 00 e3 a1 01 00 50 10 04 00 00 00 06 00 ff ff ff ff 0f 00 ff ff` = object 106,979, group 4, owner −1, any row |
| 1260 to 1263 | progress key paths | where the job is saved, named by the last name of each path in the save-variable section [D]: **1260** `time_played` (the job timer), **1261** `suck_value` (difficulty help), **1262** `focus_count` (play count), **1263** `is_complete` (all jobs) |

## 3. Task record (class 133)

The body is an option list, a **prerequisite list** (`u8` count, count × `s32` object ID), a **successor list**
(same form) and an internal name (`t1_sat_intro` …) [D]. The two lists mirror each other: in every episode, each
successor link has the matching prerequisite link (974 links, no exceptions) [D]. Most tasks have one of each; up to
9 occur [D]. The last task of a job usually has the first task of the next job as successor, so the episode is one
graph [D].

Options used (counts over the 795 distinct tasks) [D]:

| Option | Value | Meaning |
|---:|---|---|
| 1264 | object ID | the task's job (most tasks) |
| 1265 | `s16` string ID | an on-screen objective shown while the task is current, e.g. "Locate The Job Start Point", "Reposition All 3 Dishes" (158) |
| 1274 | object ID | the checkpoint the task starts at (307) |
| 1275 | `s16` level-table row | the world the task starts in (307) |
| 1276 | object ID | the player character the task starts with: 7 Sly (211), 8 Bentley (51), 9 Murray (43), another object (2) |
| 1279 | object ID | an object present only while the task is available, e.g. the job's start marker (70) [R] |
| 1278 | object ID | a level exit fired when the task becomes current (22) [R] |
| 1271 | flag | while available, the task already counts as done for its successors' prerequisites (37) [R] |
| 1272 | flag | the task becomes current as soon as it becomes available (228) [R] |
| 1273 | flag | **resume as current after loading a save**: when progress is restored and the task comes back available, it is remembered as the current task and made current when the restore ends [R]. Set on 24 tasks, always 1: the briefings `t1_chalktalk1` … `t8_chalktalk2` and two purchases, `t5_paraglide_purchase` and `t7_alarmclock_purchase` [D] |
| 1287 | 4 arguments, repeatable | **object switch**: object ID, list of task states in which it is present, owner object ID (−1 none), level-table row (−1 any) |
| 1288 | 6 arguments, repeatable | **state-machine goal**: task state (−1 = by state), object ID, state-machine object ID, goal state ID, "instant only" flag, row |
| 1295 to 1299 | progress key paths | where the task is saved, by the last name of each path [D]: **1295** `focus_count` (play count), **1296** `complete_count` (done count), **1297** `is_complete`, **1298** `state`, **1299** `from_memcard` |

Object switches per task: 0 (367), 1 (232), 2 to 10 (196) [D]. The state lists seen: `(1)` 539 times, `(0, 1)` 74,
`(2, 3)` 69, `(1, 2)` 58, `(2)` 54 and others [D]. No task carries a time limit option (option 1285 occurs nowhere
on the disc) [D]; timed jobs are timed by their scripts [I].

### 3.1 Methods that scripts call

Several job and task options are never stored in the records: they are **methods** that scripts call (option-table
entries with the method flag and a handler address) [D]. Script IDs come from the class option map [D]; call counts
are static counts over all script modules of the disc where the receiver is a job or task [D].

| Class | Stream option (script ID) | What it does [R] | Calls [D] |
|---|---|---|---:|
| job | 1238 (1067) | counts a failure of the job: broadcasts task event 0x1D with the job, so the difficulty help rises by 0.1 (section 6). It does **not** end the job | 159 |
| job | 1239 (1068) | restarts the job at its checkpoint task, or its first task (section 5) | 368 |
| job | 1240 (1069) | **ends the job**: the player is sent back to the episode's hub at object 393, unless a level change is already under way | 222 |
| job | 1250 (1079) | resets the play counts of the job and of all its tasks to 0, and saves them | 32 |
| job | 1236 / 1237 (1065 / 1066) | add a task-event number to the job's "count a failure" / "end the job" list (up to 32 each); when that event is broadcast while the job is current, the job acts as for 1238 / 1240 | 0 |
| job | 1256 (526) | adds a state-machine goal to the job, like task option 1288, applied when the job begins and ends | 0 |
| task | 1280 (1104) | requests a state; first argument the state (−1 or 4 = make current, 5 = stop being current), second the requester | 1,434 (state 2 ×1,073, 3 ×175, 5 ×138, 4 ×48) |
| task | 1281 (1105) | the same request, forced (no checks) | 0 |
| task | 1284 (1106) | raises script event 23 on the task with one integer (only while the task is live) | 0 |
| task | 1290–1293 (1110–1113) | forces the task to state 0, 1, 2 or 3 respectively, then the game re-chooses the current job | 5 (all 1291) |
| task | 1294 (1114) | makes the task current | 0 |

Jobs and tasks also answer read-only options [R]: task 1286 (state), 1267 (play count), 1268 (done), 1269 (current),
1270 (the state as a string ID, 0x612 + state); job 1251 (last task done), 1252 (current).

### 3.2 Script events of jobs and tasks

The game tells a job or task about changes by raising script events on it ([scripts.md](scripts.md) 6) [R]:

| Event | Raised on | When | Values after the object |
|---:|---|---|---|
| 17 | task, job | a **task event** was broadcast (the numbers written 0x… in this spec: 0x10 defeat, 0x1A arrival, 0x1C–0x22 job events, 0x23–0x27 task events, and others) | the event number, the object it concerns |
| 19 | task | the task **became current** | — |
| 19 | job | the job **began** (became the current job) | — |
| 20 | task | the task **stopped being current** | — |
| 20 | job | the job **ended** | — |
| 21 | task | **after** the task's state changed (the new state is already set) | — |
| 22 | task | **before** the task's state changes | — |
| 23 | task | option 1284 was called | the integer passed |

Who receives a broadcast task event as event 17 [R]: every available task whose job is the current job (when no job
is current: every available task with no job), then the current job. Tasks and jobs receive nothing while progress is
being restored or a level change is under way. A job that is not current receives nothing. The disc's handlers agree
with this split: 639 event-17 handlers on jobs and tasks, 3,358 for event 19, 263 for 20, 907 for 21, 68 for 22 and
none for 23 ([scripts.md](scripts.md) 6) [D].

## 4. Task states

| State | Name (ours) | Meaning |
|---:|---|---|
| −1 | unknown | not yet restored from the save [R] |
| 0 | locked | a prerequisite is not done |
| 1 | available | can be started; listed as active |
| 2 | done | completed |
| 3 | closed | done, and its job finished |

Two more values select object switches that are not states: **4** "the task became current" and **5** "the task
stopped being current" [R].

Rules [R]:

- **Requests.** A task can be asked to become current (from scripts, triggers and the job board). If it is locked and
  its prerequisites are done (or available with option 1271), it becomes available and current; otherwise the
  request is refused and the refusal is announced (event 0x27, a message to the job). A request for a state is
  allowed only one step up (0→1 with prerequisites met, 1→2, 2→3) or from 1 straight to 3; a closed task never
  changes. A forced request skips the checks.
- **Propagation.** When a task becomes done or closed, its prerequisites that are below that state are raised with it
  (done tasks of another job become closed). Then each successor is re-evaluated: locked if its prerequisites are not
  all done, available if they are.
- **Effects of a state change.** The world's object switches are applied: every object listed for the task is added to
  the world if the new state is in its list and removed otherwise, limited to its row; state-machine goals for that
  state are set (instantly while restoring, otherwise run to). Events 0x26 (before) and 0x25 (after) are broadcast,
  and the state, play and done counts are written to the save.
- **Becoming current.** Only one task is current. The previous one stops being current (switch group 5, event 0x24,
  its objective prompt removed). The new one: switch group 4, event 0x23, its play count increases, its objective
  prompt (option 1265) is shown, its checkpoint is remembered (option 1274), its exit (option 1278) fires, and its job
  begins if it is not the current job.
- **Episode percentage.** After every change: the share of the episode's tasks that have successors and are closed,
  as a whole percentage, 0 only if none, 100 only if all, otherwise clamped to 1–99; saved per episode as
  "percent complete".
- **Autosave.** When a task outside a job is done and all its successors belong to jobs, the game autosaves.

- **Episode intro tasks** (added 2026-11-19) [D, R for the load rule]: each episode's `tN_intro` task (1752, 1978,
  2168, 2388, 2624, 2842, 3022, 3339) has no prerequisites, so it is available on load; a macro-package module then
  starts its intro sequence, which sets the task to 2 or 3 and so opens the chain to the first briefing. Details in
  [level-flow.md](level-flow.md) 4.1.

### 4.1 Worked routes: what finishes task 1696, and where the hack sequences come from (added 2026-11-20)

**Task 1696 (`t0_chase_hallway_complete`, row 0, job 1671 `m0_intro`).** No option names 1696 because a script
requests its state through a value passed up from a sequence. The chain [D unless marked]:

1. Task 1689 (`t0_chase_hallway_start`; prerequisite 1684, successor 1696) owns module 14 of `jb_intro_macro`.
   Its event-19 handler (task becomes current, 3.2) starts sequence 1690 with no starter (script ID 235).
2. Sequence 1690 (the chase) ends by adding three objects to the world with command 670 (option 670, "add an
   object": object, time, flag; routine 0x108890) — 150498, 150633 and **150620** — and then command 642 requests
   task 1689 state 2. 1689's successor 1696 becomes available (4).
3. Object 150620 is a trigger volume (class 24) in package `jb_intro_rig`. It carries option 26 (removed at level
   start), option 432 = 7 (only Sly counts) and option 441 = 1697, so it is absent until step 2 adds it, and Sly
   entering it starts sequence 1697 with Sly as the starter (dialogue-and-voice.md 2.1).
4. Sequence 1697 (44 commands) first removes the volume again (command 671, "remove an object", routine
   0x108948), plays its scene, and ends with command **634 = 106777** (option 634, routine 0x106F90: append a
   "call the sequence's script with this value" record [R]).
5. Sequence 1697 owns module 20 of `jb_intro_macro`. That module's only handler, `s4`, takes (object, value) and
   switches on the value; its three cases are exactly the three 634 values in 1697's list (152651, 152652,
   106777) [D], so a 634 command raises event 4 on the sequence's own module with its value [D for the match, I for
   the event number]. For 106777 it tests whether op104 returns 1; op104 always returns 0 (scripts.md 4.2), so the
   else branch runs: task 1696's script ID 1104 = stream option **1280** with (2, the object) — request state 2
   (done) (3.1). The other branch (task 1711, `t0_van_escape`, forced with 1281) never runs on this disc.
6. With 1696 done, its successor 1699 (`t0_rooftop_start`) becomes available.

So 1696 is finished by the script of its completion sequence 1697 (module 20), driven by a value command, not by
job 1671's module, sequence 1690 or a state machine. An engine needs: commands 670/671 (add/remove object),
634 (raise event 4 on the sequence's module with the value), and trigger volumes that start a sequence on entry.

**Hack sequences 2410/2411 in row 14.** They are in the world file of row 14 (`4_prison/00_0x082_hub.s2w`), in
package `p_prison_macro` (keep flag set: no clone, 1 copy; not placed; package 110 of 284) [D]. The records carry
**level conditions**: 2408, 2410 and 2411 (class 114) each list row 14, and screen object 2409 lists rows 14 and
15 [D]. In this package 171 records are conditioned on row 14, 60 on row 15, 4 on (14, 15) and 1 on (15, 16) [D].
A record whose condition list does not contain the current level-table row is not loaded (object-stream.md;
textures.md 2.5) [R]; with the current row correctly 14 they load with the package at world load — no switch,
placement or task brings them in. The hack task handlers then use them: each of tasks 2414–2424
(`t4_hack_station1`–`6`) has an event-19 handler (modules 28–33) that sets the job's focus pair and starts
sequence 2410 with the current character (vehicles-and-minigames.md 10.3); `p_prison_ext_hack`'s event-6 handler
calls 2411.

If 2410/2411 are missing at run time, the likely cause is the condition test using the wrong number for the
current level: row 14 is the level-table row, not the file's index (00) or the world identity (0x82 = 130) [I].
The 12 "too deep" errors are then probably the scripts' method calls on the missing objects (lookups that return
no object) cascading through handlers, not a data recursion [I]; nothing in row 14's scripts calls itself through
2410/2411 [D].

## 5. Jobs

Rules [R]:

- **Begin.** A job begins when its first task is reached; the current player's carried loot is dropped (event 0x1C).
  Only one job is current; beginning a job ends the previous one. Starting it: play count +1, switch group 4, event
  0x21.
- **Job timer.** While a job is current, its timer adds the game step each frame, but only while the controller has
  been touched in the last 10 s; the value is saved (option 1260's key, `time_played`).
- **Complete.** When the last task becomes done: if the job has a "job complete" screen object (option 1258), the game
  pauses and shows its first part, then the second, then a wipe; then the job closes. Closing: the last task becomes
  closed (unless option 1242), event 0x20, the job ends, and the game autosaves (unless option 1242). Then the next
  task is requested: the first successor of the last task whose other prerequisites are done; if none, the job's exit
  (option 1259) fires.
- **End.** Event 0x22, switch group 5; in the episode's hub the respawn point is reset to object 393 (`0x189`).
  A task event listed by the job's option 1237 also ends it; one listed by option 1236 counts a failure (section 3.1).
- **Failure and restart.** When the player is defeated during a job (event 0x10) the job restarts at its checkpoint:
  the remembered checkpoint task (or the first task) becomes available and current, the task's player character is
  restored, and its world (option 1275) is loaded at its checkpoint (option 1274; object 393 if none). Restarting a
  whole job resets the play counts of its tasks, makes the first task available again and raises event 0x1F. Outside
  a job, defeat reloads the current world at the last checkpoint.
- **Ending a job by leaving.** A job that is ended (not completed) returns the player to the episode's hub at object
  393.

## 6. Difficulty help

A value between **0.0 and 1.0** (bounds and steps read at executable address `0x2BDC00`: 0.0, 1.0, +0.1, −0.2) [D]:

- +0.1 on each defeat of the player (event 0x10) and each job restart (event 0x1D) [R];
- −0.2 the first time a task of the current job is done [R];
- during a job the value is the job's own (saved under option 1261's key, `suck_value`); outside a job it is the mean of the last
  three jobs' values, shifted in when a job ends [R].

It scales damage taken (characters.md) and pickup drops [R].

## 7. Progress in the save

Each job and task names its save entries by key paths (options 1260–1263, 1295–1299): played count, done count,
closed flag, state and a "restarted" flag for tasks; played count, completion, timer and difficulty help for jobs
[D for the options, R for the meaning]. On loading a world, every task's state is restored from the save in
graph order; a task whose `from_memcard` flag is set and that was mid-way goes back to available (or locked if its
prerequisites are not done); a task with no saved state is available if it has no prerequisites, otherwise locked
[R].

**The current job, current task and checkpoint task are not saved** [D]: the save-variable section has no entry for
them ([saving.md](saving.md) 2). The game keeps them in memory, across the world loads of one session, and after each
world load the restore makes them current again [R]. **Loading a save from the card starts with none of them** [R]
(no current job, no current task, no checkpoint task); the only task that becomes current after such a load is one
restored as available that carries option 1273 (section 3), in practice the episode's pending briefing or purchase.
A port should keep the three in session state, not in the save block [I]. The save format itself is for
`saving.md`.

## 8. Jobs by episode (names from the game's string table) [D]

| Episode | Jobs (internal name: display name) |
|---|---|
| 0 | `m0_intro`: Cairo Museum Break-in |
| 1 | `m1_sat`: Satellite Sabotage; `m1_recon`: Breaking and Entering; `m1_brawl`: Silence the Alarms; `m1_bug`: Bug Dimitri's Office; `m1_follow_dimitri`: Follow Dimitri; `m1_moulin_power`: Theater Pickpocketing; `m1_neela`: Moonlight Rendezvous; `m1_disco`: Disco Demolitions; `m1_waterpump`: Waterpump Destruction; `m1_heist`: Operation: Thunder Beak; `m1_boss`: Printing Press Duel |
| 2 | `m2_recon`: Recon the Ballroom; `m2_lower_drawbridge`: Lower the Drawbridge; `m2_turret`: Battle the Chopper; `m2_boardroom_brawl`: Boardroom Brawl; `m2_dance_audition_fake`: Ballroom Dance Party; `m2_steal_tuxedo`: Steal a Tuxedo; `m2_dance_audition`: Dominate the Dance Floor; `m2_rc_chopper`: RC Bombing Run; `m2_elephant_rampage`: Elephant Rampage; `m2_heist_bomb_bridge`: Bomb the Bridge; `m2_heist_dance`: Tango with Carmelita; `m2_heist_cover_murray`: Clear the way for Murray |
| 3 | `m3_recon`: Spice Room Recon; `m3_waterbug`: Water Bug Run; `m3_free_elephant`: Freeing the Elephant; `m3_heartneela`: Neyla's Secret; `m3_lead_rajan`: Leading Rajan; `m3_tntbarrel`: Spice Grinder Destruction; `m3_turret`: Blow the Dam; `m3_steal_ruby`: Rip-off the Ruby; `m3_heist`: Operation: Wet Tiger; `m3_boss`: Showdown with Rajan |
| 4 | `m4_recon`: Eavesdrop on Contessa; `m4_hack`: Train Hack; `m4_rc_chopper`: Wall Bombing; `m4_brawl`: Big House Brawl; `m4_lightning`: Lightning Action; `m4_bomb_bridge`: Disguise Bridge; `m4_spycam`: Code Capture; `m4_pickpocket`: Close to Contessa; `m4_heist`: Operation: Trojan Tank |
| 5 | `m5_recon`: Know Your Enemy; `m5_ghost`: Ghost Capture; `m5_ewok_trap`: Mojo Trap Action; `m5_kidnap`: Kidnap the General; `m5_pickpocket`: Stealing Voices; `m5_battlezone`: Tank Showdown; `m5_hack`: Crypt Hack; `m5_heist`: Operation: High Road |
| 6 | `m6_recon`: Cabin Crimes; `m6_paraglide`: Spice in the Sky; `m6_train_red`: Ride the Iron Horse; `m6_carmelita`: A Friend in Need; `m6_rc_chopper`: Aerial Assault; `m6_train_blue`: Theft on the Rails; `m6_kidnap_bearcubs`: Bear Cub Kidnapping; `m6_heist`: Operation: "Choo-Choo" |
| 7 | `m7_recon`: Recon the Sawmill; `m7_bearcave`: Bearcave Bugging; `m7_robot_war`: RC Combat Club; `m7_laser`: Laser Redirection; `m7_lighthouse`: Lighthouse Break In; `m7_bear`: Old Grizzle Face; `m7_boat_hack`: Boat Hack; `m7_thermals`: Thermal Ride; `m7_heist`: Operation: Canada Games; `m7_boss`: Brains over Brawn |
| 8 | `m8_recon`: Blimp HQ Recon; `m8_post_recon`: (no display name); `m8_tntbarrel`: Charged TNT Run; `m8_engine_room_jt`: Murray/Sly Tag Team; `m8_engine_room_bentley`: Sly/Bentley Conspire; `m8_engine_room_murray`: Bentley/Murray Team Up; `m8_megajump`: Mega-Jump Job; `m8_turret`: Carmelita's Gunner; `m8_boss`: Showdown with Clock-La |

## 9. Test vectors

Nightclub hub (row 2, file SHA-256 in [disc.md](disc.md) 7) [D]:

- Job object 1760 (`m1_sat`, "Satellite Sabotage"): body at decompressed offset `0x15C46C`, 82 bytes; first task
  1761, last 1769; name/description string IDs 323/324; option 1258 = (1750, 17425, 17426).
- Task object 1761 (`t1_sat_intro`): body at `0x15C4CD`, 101 bytes, beginning
  `f0 04 e0 06 00 00 f7 04 01 ff 04 3f e4 01 00 fa 04 40 e4 01 00 fb 04 02 00 fc 04 07 00 00 00`, that is option 1264
  = job 1760, 1271 = 1, 1279 = object 123,967, 1274 = checkpoint 123,968, 1275 = row 2, 1276 = character 7 (Sly),
  then progress keys and option 1265 = string 325 ("Locate The Job Start Point"); prerequisite list (1754),
  successor list (1762).
- The job's task chain: 1761 → 1762 (`t1_sat_dish1`, 10 object switches) → 1764 → 1766 → 1769
  (`t1_sat_complete`, objective string 326 "Reposition All 3 Dishes") → 1772 (first task of `m1_recon`).

## 10. Open questions

- The job option 3 (one job) is not understood; the use of option 1257's two times (ranking, hints?) is an
  inference. (Resolved: option 1273 and the methods, sections 3 and 3.1.)
- (Resolved: the save entries of options 1260–1263 and 1295–1299 are named by the last names of their paths, sections 2 and 3.)
- The world-row option 1275 is set on 307 of 795 tasks; the rule for the others (current world assumed) is [I].
- Event numbers (0x10, 0x1C…0x27) are the script event IDs of [scripts.md](scripts.md); their names there should match.
- All [R] behaviour needs checking in play (job board, restarts, autosaves).
