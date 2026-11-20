---
id: sly3-behaviour-missions
title: Sly 3 jobs and tasks - records, options, state rules, events, checkpoints, save keys - differences from Sly 2
side: reference
status: draft
sources:
  - kind: disc
    what: >-
      Sly 3 USA (SCUS-97464), every class-168 and class-169 record of all 39 worlds (440 + 5,519 bodies) with their
      option values (untruncated survey); executable SCUS_974.64 SHA-256
      ce817858306efd711f08bbc14f393455280119cb00b894bdc9fb1fa47bcbb338
    used_for: >-
      option counts and values; the option table at 0x32CA80 (value types, fields, setters); the "task becomes
      current" routine at 0x188658; the player-control routine at 0x1F4B00 [X]
  - kind: reference-project
    what: SafeHouse @ c70803ce (2026-11-12), Sly3-Release/game/task3.cpp, scptrun3.cpp, game/gen (class and option correspondence tables)
    used_for: the Sly 2 to Sly 3 correspondence (rows scored 80 or more) and the statement that Sly 3 runs Sly 2's task rules; marked [R]
reviewed_by: pending
date: 2026-11-19
---

# Sly 3 jobs and tasks

Read [../sly2/missions.md](../sly2/missions.md) first: the task graph, states (−1 unknown, 0 locked, 1 available,
2 done, 3 closed; switch groups 4 "became current" and 5 "stopped being current"), requests, propagation, job
begin / complete / end, failure and restart, the difficulty help and the script events carry over [R]. This
document gives Sly 3's classes, options and data, and what the executable confirms. Tags as in [guards.md](guards.md).

## 1. Records [D]

- Jobs are class **168** (options and a name, e.g. `m1_ball`); tasks are class **169** (options, a prerequisite list,
  a successor list, a name, e.g. `t0_ascend_complete`; [classes/simple-layouts.md](classes/simple-layouts.md) 5).
- Every world of an episode carries the whole episode's jobs and tasks, in its macro group. 440 job and 5,519 task
  bodies on the disc; distinct: **76 jobs, 965 tasks**, the same as the save tree ([saving.md](saving.md) 2).
- Per episode: see [gameplay-overview.md](gameplay-overview.md) 5.

## 2. Job options (class 168)

"Kind" is the value type and field from the option table [X]; "Sly 2" is the reference's correspondence [R] unless
marked otherwise.

| Option | Kind [X] | Sly 2 | Meaning | On the disc [D] |
|---:|---|---:|---|---|
| 1364 | task (object of class 169) at +0x54 | 1243 | **first task** | all 440 |
| 1365 | task at +0x58 | 1244 | **last task** | all 440 |
| 1370, 1371 | 16-bit string IDs at +0x60, +0x64 | 1245, 1246 | display name and description | 329, 357 |
| 1361 | bool at +0x6C | 1242 | **keep open**: the job does not close its last task and does not autosave on completion | 5 jobs, all 1 |
| 1381 | exit (object of class 24) at +0xC0 | 1259 | exit fired when the job ends with no next task ([level-flow.md](level-flow.md) 4.4) | 155 |
| 1379 | method, 6 arguments | 1256 | a state-machine goal applied when the job begins and ends | 34, e.g. (5, 69222, 69135, 13758, 65535, 0) |
| 1387 to 1390 | save paths at +0xE0, +0xE8, +0xD0, +0xD8 | 1260 to 1263 | save keys `time_played`, `suck_value`, `focus_count`, `is_complete` (by the path's last name ID: 88, 89, 86, 87) | all 440 |
| 1366 | task at +0x5C | — | the job's **resume task**: when the previous job completes, the job that follows it supplies this task as the forced checkpoint (section 4.1) [X]. Only on Kaine Island's jobs (tasks 1841 to 1843) | 40 |
| 1372 | screen object (class 158) at +0x78 | — | a screen shown for the job, e.g. 121565 `m2_shaman_info` [D]. *Interpretation:* the job's info card, the counterpart of Sly 2's "job complete" screen (1258) [I] | 22 |
| 1385 | item-table record at +0xF8 | — | an **item granted when the job completes** (38, 39, 40: the three disguises, and others) [X: section 4.1] | 21 |
| 1386 | task at +0xFC | — | a task paired with 1385 (3451, 2353, 3885, …); its use is open | 21 |
| 1362 | bool at +0x70 | — | **the job ignores the player's defeat**. The job's task-event handler (`0x00186730`) reacts to event **12** (the player's defeat, [level-flow.md](level-flow.md) 7) only when 1362 is 0. With 0 it calls `0x00186EC0`, or `0x00186FE8` when the job is not the current one; the latter requests the checkpoint reload (`0x00189F38`) and runs the job's failure path (`0x001878E0`) [X]. With 1 the job does nothing, so a scripted sequence handles the defeat [I] | 30, all 1 |
| 1363 | bool at +0x74 | — | open; the job's own methods also set and clear this field at run time (`0x0018B700`, `0x0018B780`) | 4 |
| 1367 | value type `0x108A` at +0x40 | — | **completion mode**: 2 = on completion the first task goes back to state 0, so the job can be played again; otherwise (1) the last task is closed unless 1361 is set [X: section 4.1] | 62: 1 ×37, 2 ×25 |
| 1368 | method, 1 argument (`0x001860D8`) | — | adds a number to a list of at most 8 kept with the job (count +0x44, list +0x48) [X]. The values (21, 26, 11, …) are task-event numbers. *Interpretation:* Sly 2's "events that count a failure or end the job" (1236 / 1237) [I] | 37 |
| 1369 | int at +0x4C | — | open (14, 20, 19) | 17 |
| 1380 | value type `0x1065` at +0xB0 | — | **the job's picture on the job screen**: a single bit (1, 2, 4, … 128) that `0x00186930` turns into a **part ID of the job screen object** (game state +0x433C), played by `0x00262450` and tested by `0x00260758` [X]. In the normal layout, bits 1 to 128 give parts 1086, 1088, … 1100 (1102 for any other value). A second layout gives 1068 … 1082 (default 1084), and a third, mode 2, gives 1104 to 1108. The layout comes from field +80 of the object at `0x004BC244`; outside it the job's own value is used (`0x00186900`). *Interpretation:* the eight pictures are the episode's job icons; the layouts belong to the single- and two-player screens [I] | 418: 1 ×152, 4 ×63, 2 ×55, … |
| 7 | method, 2 arguments | 7 | (1852, two floats), on 5 jobs | 5 |

Not found in Sly 3: Sly 2's job object switch (1255) and the two times of 1257 (the job timer thresholds); the
job's ending and closing events are task events as in Sly 2 [R].

## 3. Task options (class 169)

| Option | Kind [X] | Sly 2 | Meaning | On the disc [D] |
|---:|---|---:|---|---|
| 1391 | job (object of class 168) at +0x6C | 1264 | the task's job | 5,137 |
| 1392 | 16-bit string ID at +0x4C | 1265 | on-screen objective | 1,103 |
| 1398 | bool at +0x5C | **1271** | while available, the task already **counts as done** for its successors' prerequisites | 45, all 1 |
| 1399 | bool at +0x60 | **1272** | the task **becomes current as soon as it becomes available** | 3,010 (1 ×2,871, 0 ×139) |
| 1400 | arrival point (class 40) at +0xA8 | 1274 | **checkpoint** | 2,136 ([level-flow.md](level-flow.md) 5.3) |
| 1401 | 16-bit world slot at +0xAC | 1275 | the world the task starts in (15 ×296, 23 ×272, 3 ×220, …) | 2,136 |
| 1402 | method, 2 arguments | 1276 | the **character** the task starts with, and a second value (always −1): (24 Sly ×952, 25 Bentley ×385, 26 Murray ×306, …) | 2,136 |
| 1406 | exit at +0xC8 | 1278 | exit fired when the task becomes current | 104 |
| 1407 | object of class **120** at +0x70 | 1279 | an object present only while the task is available: the **mission waypoint** (`mission_waypoint`, `mission_interior_waypoint`, `mission_start_jt`) | 321 |
| 1408 | command script (class 135) at +0xCC | — | a script **started when the task becomes current** [X, section 4] | 3,563 |
| 1405 | bool at +0xC4 | — | the task is **done as soon as it becomes current** [X, section 4] | 514, all 1 |
| 1416 | method, 5 arguments | 1287 | **object switch**: Sly 2's four arguments (object, states list, owner, world) plus a fifth value (0) | 5,391 |
| 1417 | method, 7 arguments | 1288 | **state-machine goal**: Sly 2's six arguments plus a seventh (0) | 2,513 |
| 1423 to 1427 | save paths at +0xD0 … +0xF0 | 1295 to 1299 | save keys `focus_count`, `complete_count`, `is_complete`, `state`, `from_memcard` (name IDs 86, 166, 87, 167, 168) | all 5,519 |
| 1403 | float pair at +0xB8 | — | (900, 1,800) on 8 tasks. *Interpretation:* two times in seconds, like Sly 2's job times [I] | 8 |
| 1404 | int at +0xC0 | — | a world slot (15 ×32, 14 ×9, 23 ×5) [I] | 46 |

No task carries Sly 2's "resume as current after loading" (1273): no Sly 3 option corresponds [R], and none of the
unexplained options has its pattern [I].

### 3.1 Corrections to earlier working readings

| Earlier reading | Correct |
|---|---|
| 1398 = becomes current when available, 1399 = resume after loading | **1398 = counts as done** (Sly 2 1271), **1399 = becomes current when available** (Sly 2 1272). The values agree: 1399 is set on most tasks, as 1272 was in Sly 2 |
| "counts as done" not found | it is 1398 |
| job keep-open flag not found | it is 1361 |
| 1407 names a placement (class 8) | 1407 names a **class-120** object (the mission waypoint) |
| 1402 = (24, −1) a character | correct; it is a 2-argument method, not a plain object option |

### 3.2 Script methods of tasks and jobs [X]

Scripts reach these through the class option map (`0x002ED518`; [scripts.md](scripts.md) 5). **A script names the
script option ID** (left column), not the stream option. Each entry has the script-method flag (grf bit 0), a
handler and an argument count. Every handler checks that the receiver is of the right kind, calls the routine
named below, and returns nothing.

**Tasks** (class 169):

| Script ID | Stream option | Arguments | Handler → routine | What it does | Sly 2 (stream / script ID) |
|---:|---:|---:|---|---|---|
| **1309** | 1409 | 2: state, requester | `0x0010E090` → `0x00189078` | **requests a state** through the request queue (`0x00189240`, kind 0: checked). As in Sly 2, −1 or 4 makes the task current and 5 stops it being current [R] | 1280 / 1104 |
| **1310** | 1410 | 2: state, requester | `0x0010E138` → `0x001890B0` | the same request, **forced** (`0x00189240`, kind 1: no checks) | 1281 / 1105 |
| **1311** | 1413 | 1: integer | `0x0010E1E0` → `0x00189DA0` | raises script event 23 on the task with the integer | 1284 / 1106 |
| **1314** | 1418 | 0 | `0x0010E460` → `0x00189710` | **forces state 0**, then re-chooses the current job (`0x00189640`) | 1290 / 1110 |
| **1315** | 1419 | 0 | `0x0010E4D8` → `0x001897C8` | forces state 1, then re-chooses the current job | 1291 / 1111 |
| **1316** | 1420 | 0 | `0x0010E550` → `0x00189880` | forces state 2, then re-chooses the current job | 1292 / 1112 |
| **1317** | 1421 | 0 | `0x0010E5C8` → `0x00189938` | forces state 3, then re-chooses the current job | 1293 / 1113 |
| **1318** | 1422 | 0 | `0x0010E640` → `0x00188540` | **makes the task current** | 1294 / 1114 |

- 1315 and 1316 carry grf `0x4001` (one extra flag); the others carry `0x1`.
- The four force routines are the same code: each runs the forced request (`0x001890B0`) with its constant (0, 1,
  2, 3), then `0x00189640`.
- Read-only task properties (grf `0x80`, value at an offset): script IDs 1292 (stream 1423, +0xD0), 1319 (1424,
  +0xD8), 1293 (1425, +0xE0), 1320 (1426, +0xE8) and 1321 (1427, +0xF0). They are Sly 2's state, play count, done,
  current and state-string reads [I by the layout].
- Other script-visible task entries: 1302 (stream 1402, a 2-argument stream method; grf `0x401`), 1313 (1416, the
  object switch), 578 (1417, the state-machine goal). The stored fields are reachable through their script IDs, for
  example 1307 → 1407 (+0x70, the waypoint) and 1295 → 1391 (+0x6C, the job).

**Jobs** (class 168), the script methods (meanings by Sly 2's order [I]; handlers [X]):

| Script ID | Stream option | Arguments | Handler → routine | Sly 2 counterpart [I] |
|---:|---:|---:|---|---|
| 1259 | 1355 | 1 | `0x0010DB10` → `0x00186E80` | 1238 count a failure |
| 1260 | 1356 | 2 | `0x0010DB98` → `0x00186EA0` | 1239 restart at the checkpoint |
| 1261 | 1357 | 2 | `0x0010DC40` → `0x00186EC0` | 1240 end the job; also the defeat path of 1362, section 2 |
| 1262 | 1358 | 3 | `0x0010DCE8` → `0x00186EE0` | (no direct Sly 2 match) |
| 1263 | 1359 | 0 | `0x0010DDA0` → `0x00187040` | 1250 reset the play counts |

Sly 3's job methods take arguments where Sly 2's took none. Their meanings are open (section 7).

**For the Venice job start:** a script that starts `t1_recon_intro` must call script ID **1309** (request, usually
state 4 or −1 to make it current, or 2) or **1318** (make current) on task 2086, or the forced forms. A call that
names Sly 2's IDs (1104 to 1114, or the stream IDs 1280 to 1294) reaches nothing in Sly 3. Script IDs 1280 and
1281 exist on tasks, but they are the stored fields +0xFC (state) and +0x100 (stream 1411, 1412). So an engine that
maps 1280 / 1281 by Sly 2's meaning writes the state field directly instead of making a request.

## 4. A task becoming current [X]

The routine at `0x00188658` (Sly 2's "become current") does, in this order:

1. Applies switch group 4 ("became current") through `0x00185DB0`.
2. Shows the task's objective prompt when it has one (field +0x4C).
3. Raises script event 22 on the task (`0x188780` onward).
4. If 1408 names a command script, starts it (`0x00226010`, with player 0's controlled object as its context [I]).
5. If **1405** is set, requests state **2** (done) at once (`0x00189078(task, 2, 0)`).
6. If 1406 names an exit, fires it.

The rest (previous task stops being current, play count, checkpoint remembered, job begins) is Sly 2's [R].

### 4.1 A job completing [X]

The routine at `0x00187270`:

1. If the job's 1367 is 2, the **first task goes back to state 0** (`0x00189B08(first task, 0)`). Otherwise, unless
   1361 is set, the last task is closed (state 3).
2. If 1385 names an item, the item is **granted** (`0x0021F5D0(item, 1)`: `0x0021F510` with the value 1, then a refresh).
   *Interpretation:* that sets the item's owned flag [I].
3. Among the last task's successors, it finds one whose job has a resume task (1366). If there is one, that task's
   ID and world are written to `oidCheckpointForce` and `widCheckpointForce` ([saving.md](saving.md) 3). Starting
   episode 8 then resumes there ([level-flow.md](level-flow.md) 3).

### 4.2 Mission waypoints (class 120) [X]

The object that task option 1407 names (for example object 121540) is a **mission waypoint**, class 120 (Sly 2's
job marker). Walking into it is what matters: the waypoint reads no button and shows no prompt of its own.

**Options** (field offsets in the object) [X]; counts over the disc [D]:

| Option | Field | Default (set at load, `0x00281AD0`) | Meaning |
|---:|---|---|---|
| 1486 | +0x130 | 0 | **kind**: 0 a job's start waypoint (counts only while no job runs), 1 a waypoint used during a job, 2 a waypoint that also counts only while no job runs (Sly 2's vault marker kind [R]), −1 a fall-back for command `0x69` (below). Disc: 1 ×50, 0 ×16, −1 ×9 |
| 1483 | +0x134 | 49 | the class the player's controlled object must derive from (51 on 15 waypoints) |
| 1484 | +0x138, +0x13C | 300, 500 | the **enter radius** and the **leave radius** (on the disc: (150, 400) twice) |
| 1488 | +0x148 | | the anchor object (class 3) |
| 1489, 1490, 1491 | +0x14C to +0x154 | | the three drawn parts: icon, line, base (class 47) |
| 1487 | +0x198 | | a flag (2 waypoints) |
| 1492, 1493 | +0x19C, +0x1A0 | 255, 255 | two masks; +0x1A0 is the mask that command `0x69` tests (below) |
| 1494 | | | a method, (1, 4500) twice |
| 1495 | +0x1AC | 5000 | a distance (1000 once) |

**Each frame** (the update at `0x00281C50`) [X]:

1. Take player 0's controlled object (`0x001F4AD0`). Unless it derives from the class in +0x134, the player is not
   in.
2. The player is in when its distance to the waypoint is below the enter radius, or below the leave radius once it
   is already in (the flag at +0x144). This gives hysteresis.
3. Kinds 0 and 2 never count as in while a job is running (game state +0x4044).
4. A class-51 player is checked further: one field (+0x33C) must be −1 and one of its methods must not answer 2.
   *Interpretation:* the player must not be busy, for example carrying or in a move [I].
5. When the in/out state changes, the waypoint stores it and raises **script event 5** (entering) or **6**
   (leaving) on itself.

**What the event does** [X]: the shared event handler (vtable slot 3, `0x001C86B8`) first passes the event to the
object's own script, then **broadcasts a task event**: code **29** on entering and **30** on leaving, with the
waypoint as the object (`0x001854C8` on the game state at `0x0036C700`). These are Sly 2's arrival events 0x1A and
0x1B.

**Who receives the broadcast** [X: `0x001854C8`], in this order:

1. `0x001855F0` (code 12 only; the defeat path).
2. The level values (`0x00171AA0`, which reacts only to codes 12, 16, 32 and 41).
3. Every task in the task list (`0x00479498`, linked through +0x78) whose job (+0x6C, option 1391) **equals the
   current job** (`0x004794A4`; both may be none). The task must also pass `0x001880F0`: its +0x40 is 0 and its
   state is not 3. Such a task gets **script event 20** with the values (code, object) (`0x00189DF8`). This is
   Sly 2's event 17.
4. Every object in the **task-event listener list** (`0x00479B98`) gets the same script event 20 with (code, object)
   (`0x001772D8`). Only **class 147**, the macro-package objects of the `*_macro` groups, joins this list: its
   enter-world and leave-world methods (`0x00177258`, `0x00177220`, vtable `0x357200` slots 12 and 11) add and remove
   it.
5. The current job, if there is one (`0x00186730`).
6. The sequence at `0x004BC230` (`0x002849A8`).

So, with no job current, a task that belongs to a job (such as `t1_recon_intro`) does **not** hear code 29. The job
is started by a **macro-package module** that listens to every task event [X for the delivery; the module's body is
data to decompile].

**Test vector: `t1_recon_intro` in v_ext** [D] (world 3, `03_v_ext`):

- Waypoint object 35785 (`mission_start_jt`, class 120) is placed by placement 121540 in group `v_ext_recon`.
- Task **2086** (class 169, group `v_macro`, world filter 3, no module of its own) holds 121540 (option 1407).
- The listeners in v_ext are the two class-147 objects of `v_macro`: **2058** (module index **4**, no filter) and
  **2228** (module index **57**, world filter 3).
- Their handlers for event 20 must test code 29 and the waypoint, then make task 2086 current (Sly 2's macro
  modules do it with a task-state request). Making the job's first task current begins the job (Sly 2's rule
  [R]).
- To answer exactly what they call, decompile v_macro modules 4 and 57 of v_ext. With a script interpreter, deliver
  event 20 (29, the waypoint) to objects 2058 and 2228.

The waypoint's own script is a second possible handler. It receives events 5 and 6 before the broadcast. Object
35785 names **module 0 of its group `mission_start_jt`**, and placement 121540 names none [D: the records' module
indices]. So decompile that module too.

**Game-object checks used by waypoint handlers** [X]. The game object is class 1 in the class option map. Script
option IDs 984 and 1000 are script methods with one integer argument, the player index n:

| Script ID | Stream option | Handler → routine | Returns |
|---:|---:|---|---|
| **984** | 986 | `0x0010AA80` → `0x001F4AD0` | **player n's controlled object** (field +0x110C of player record n; records of 4,544 bytes from `0x0036E740`); value type `0x2031`, an object |
| **1000** | 1001 | `0x0010ADB8` → `0x001A9478` | **whether player n is held**: true when player n's hold count (record +0x1180) is above 0. n = 0 reads player 0, n = 1 reads player 1, any other value adds both |

- The hold count goes up by one for each hold the game takes (`0x001A9630`, the pause and hold used by script
  command `0x1F` and by menus) and down by one when the hold is released (`0x001A96C8`).
- **When the player can start a job:** 1000(0) is **false** (count 0: no script, cutscene or menu holds player 0),
  and 984(0) is the character player 0 controls (in the Venice hub, Sly), not none.
- A handler that requires 1000(0) false refuses while any hold is still open. In an engine that is usually a pause
  or hold taken by a script and never released [I].

## 5. Command-script commands used by missions [R] [X where noted]

Argument letters as in [classes/animation.md](classes/animation.md) 3.

| Command | Arguments | What it does |
|---|---|---|
| `0x1E` | `wb`: task, state | sets a task's state as a checkpoint does (the checkpoint-task path, Sly 2 4) [R] |
| `0x13` | `wb`: object, player index | **gives player n's control to the object** [X: `0x001F4B00`]: player slot n (4,544-byte records from `0x0036E740`) has its controlled object at +0x110C. The old object is told it lost control, the new one that it gained it, and if the new one is a playable character (class-51 family), it becomes that player's current character |
| `0x68` | `ww`: exit, point | fires an exit at an arrival point ([level-flow.md](level-flow.md) 4.4) |
| `0x11`, `0x17` | `w`: script | stop, start another command script |
| `0x0E`, `0x0F`, `0x10` | | wait for, set, clear an alarm (script-to-script signals) |
| `0x12` | `w`: object | **activates one object**; what that means depends on the object's class ([scripts.md](scripts.md) 7.3) [X] |
| `0x6F`, `0x70` | `wb` | take, release an instance of a placement |
| `0x38`, `0x39` | `wf`: object, fade time | add an object to the world, remove it |
| `0x19` | `wwwb`: object, state machine, goal, snap | drive a state machine to a goal |

The full command list and the runner's rules are in [scripts.md](scripts.md) 7.

## 6. Saving [D]

Each job and task is saved under its own name in the save tree, with the keys of sections 2 and 3
([saving.md](saving.md) 2). Restoring follows Sly 2 [R]; the episode-8 forced checkpoint is in
[saving.md](saving.md) 3.

## 7. Open questions

1. The job options still open (1363, 1369, 1386, 7), the readings of 1372, 1403 and 1404, and the arguments of the job methods 1355 to 1359 (3.2).
2. The fifth argument of 1416 and the seventh of 1417 (always 0 on the commonest values).
3. Whether Sly 3 has a resume-after-loading rule at all.
4. Every [R] rule needs checking in play.
